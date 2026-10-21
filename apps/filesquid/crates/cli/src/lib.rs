//! `fsquid`, the one FileSquid command-line tool.
//!
//! Every FileSquid CLI command is invoked as `fsquid` ([ADR 036]). A command
//! whose name only one module defines runs at the top level
//! (`fsquid get dcterms:title`). Any command can also be run inside its
//! module group (`fsquid metadata get dcterms:title`). A command listed in
//! [`QUALIFIED_ONLY`] always needs its module group (`fsquid metadata validate`).
//!
//! Each module keeps its command logic in its own library; this crate only
//! routes arguments. [`resolve`] is pure; [`run`] dispatches and performs the
//! module's I/O. [`journal`] journals every command that changes state or fails
//! (Task CLI-0.2, ADR 036 Decision 8); the `fsquid` binary runs every command
//! through it.
//!
//! [ADR 036]: ../../../../resources/architecture/036-single-fsquid-cli.md

use std::path::PathBuf;

pub mod journal;

/// One module's commands as seen by `fsquid`.
#[derive(Debug)]
pub struct Module {
    /// Group name used in `fsquid <module> <command>`.
    pub name: &'static str,
    /// Command names the module defines.
    pub commands: &'static [&'static str],
    /// Help text for `fsquid <module> help`.
    pub usage: &'static str,
    /// Runs a command; arguments start with the command name. Returns an exit code.
    pub run: fn(&[String]) -> Result<i32, String>,
    /// The OCFL object directories a command's arguments name (command first): the
    /// objects whose committed heads tell whether it changed state. Pure.
    pub objects: fn(&[String]) -> Vec<PathBuf>,
    /// Commands that write a file outside the module's objects whenever they succeed.
    pub file_writers: &'static [&'static str],
}

/// Every module `fsquid` routes to. OCFL and C2PA register here when their commands ship.
pub const MODULES: &[Module] = &[Module {
    name: "metadata",
    commands: filesquid_metadata::cli::COMMANDS,
    usage: filesquid_metadata::cli::USAGE,
    run: filesquid_metadata::cli::run,
    objects: metadata_objects,
    file_writers: &["export"],
}];

/// The metadata flags that name an OCFL object directory.
const METADATA_OBJECT_FLAGS: [&str; 2] = ["--object", "--registry"];

/// The object directories a metadata command names: `--object` (default `.`) and
/// `--registry` when given and not the same path. Pure.
fn metadata_objects(args: &[String]) -> Vec<PathBuf> {
    let named = |flag: &str| {
        args.windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| PathBuf::from(&pair[1]))
    };
    let object = named(METADATA_OBJECT_FLAGS[0]).unwrap_or_else(|| PathBuf::from("."));
    let mut dirs = vec![object];
    dirs.extend(named(METADATA_OBJECT_FLAGS[1]).filter(|registry| !dirs.contains(registry)));
    dirs
}

/// Commands that always need their module group, even when only one module defines them.
pub const QUALIFIED_ONLY: &[&str] = &["validate"];

/// Where a `fsquid` invocation goes.
#[derive(Debug)]
pub enum Route<'a> {
    /// Print the top-level help.
    Help,
    /// Run `args` (command first; empty means the module's help) in `module`.
    Run {
        /// The module that owns the command.
        module: &'static Module,
        /// Arguments passed to the module, starting with the command name.
        args: &'a [String],
    },
    /// The command needs its module group; `modules` lists the groups that define it.
    NeedsModule {
        /// The command that was given without a module.
        command: String,
        /// Modules that define the command.
        modules: Vec<&'static str>,
    },
    /// No module defines this command or group.
    Unknown(String),
}

/// Decide where `args` (without the program name) go. Pure: no I/O.
///
/// # Arguments
/// * `args` - the command line after `fsquid`.
///
/// # Returns
/// The [`Route`] for the invocation.
pub fn resolve(args: &[String]) -> Route<'_> {
    let Some(first) = args.first().map(String::as_str) else {
        return Route::Help;
    };
    if is_help(first) {
        return Route::Help;
    }
    if let Some(module) = MODULES.iter().find(|m| m.name == first) {
        return Route::Run {
            module,
            args: &args[1..],
        };
    }
    resolve_command(first, args)
}

/// Resolve a top-level command name to the one module that defines it.
fn resolve_command<'a>(command: &str, args: &'a [String]) -> Route<'a> {
    let owners = modules_defining(command);
    if owners.is_empty() {
        return Route::Unknown(command.to_string());
    }
    if owners.len() > 1 || QUALIFIED_ONLY.contains(&command) {
        let modules = owners.iter().map(|m| m.name).collect();
        return Route::NeedsModule {
            command: command.to_string(),
            modules,
        };
    }
    Route::Run {
        module: owners[0],
        args,
    }
}

/// Return the modules whose command list contains `command`.
fn modules_defining(command: &str) -> Vec<&'static Module> {
    MODULES
        .iter()
        .filter(|m| m.commands.contains(&command))
        .collect()
}

/// Return true for the top-level help forms.
fn is_help(arg: &str) -> bool {
    matches!(arg, "help" | "-h" | "--help")
}

/// Return every command name defined by more than one module.
///
/// Such names MUST be listed in [`QUALIFIED_ONLY`]; a test enforces it.
pub fn ambiguous_commands() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = MODULES
        .iter()
        .flat_map(|m| m.commands.iter().copied())
        .collect();
    names.sort_unstable();
    let mut shared: Vec<&'static str> = names
        .windows(2)
        .filter(|w| w[0] == w[1])
        .map(|w| w[0])
        .collect();
    shared.dedup();
    shared
}

/// Build the top-level help text.
pub fn usage() -> String {
    let groups: Vec<&str> = MODULES.iter().map(|m| m.name).collect();
    let mut text = format!(
        "fsquid — the FileSquid command line\n\n\
         Usage:\n  fsquid <command> [args]\n  fsquid <module> <command> [args]\n\n\
         Modules: {}\n\n\
         A command that only one module defines runs without the module name.\n\
         `validate` always needs it: fsquid metadata validate <prefix:term>.\n\
         Run `fsquid <module> help` for a module's commands.\n",
        groups.join(", ")
    );
    for module in MODULES {
        text.push('\n');
        text.push_str(module.usage);
    }
    text
}

/// Explain that `command` needs one of `modules` as its group.
pub(crate) fn needs_module_message(command: &str, modules: &[&str]) -> String {
    let forms: Vec<String> = modules
        .iter()
        .map(|m| format!("fsquid {m} {command}"))
        .collect();
    format!(
        "`{command}` needs its module name: run {}",
        forms.join(" or ")
    )
}

/// Run a `fsquid` invocation: print help, or dispatch to the owning module.
///
/// # Arguments
/// * `args` - the command line after `fsquid`.
///
/// # Returns
/// The process exit code, or an error message for unknown or ambiguous commands
/// and for errors the module reports.
pub fn run(args: &[String]) -> Result<i32, String> {
    match resolve(args) {
        Route::Help => {
            print!("{}", usage());
            Ok(0)
        }
        Route::Run { module, args } => (module.run)(args),
        Route::NeedsModule { command, modules } => Err(needs_module_message(&command, &modules)),
        Route::Unknown(command) => Err(format!("unknown command: {command}\n\n{}", usage())),
    }
}
