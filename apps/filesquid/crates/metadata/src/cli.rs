//! Metadata commands of the `fsquid` CLI: extract, get, set, export, import,
//! validate and namespace.
//!
//! The `filesquid-cli` crate builds the only CLI binary, `fsquid`, and routes
//! metadata commands here through [`run`]. This module ships no binary of its own.
//! Every command reads and writes `metadata.md` in an OCFL object directory
//! through [`LocalObject`]: each edit is a new OCFL version committed through
//! the `filesquid-ocfl` change model (MD-S-008).

use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::csv::{apply_csv_with, to_csv};
use crate::extract::{extract_from_bytes, extract_from_reader, Extracted};
use crate::file_ref::FileRef;
use crate::json::{apply_json_with, to_json, JsonTarget};
use crate::namespace::{NamespaceRegistry, Registrant, Registration};
use crate::record::{parse_curie_with, Record};
use crate::store::ocfl::LocalObject;
use crate::store::StoreError;
use crate::xmp::{from_xmp_with, to_xmp_with};
use filesquid_ocfl::VersionNum;

/// Usage text for the metadata commands of `fsquid`.
pub const USAGE: &str = "\
fsquid metadata — metadata CRUD (via filesquid-ocfl)

Usage:
  fsquid <command> [args] [--object <dir>] [--id <object-id>]
  fsquid metadata <command> [args] [--object <dir>] [--id <object-id>]

Commands:
  extract <file>          Extract EXIF/ID3/XMP from a media file into metadata.md
  extract --file <logical-path>
                          Extract from a logical file of the object
  get [prefix:term]       Print populated statements (or one CURIE)
  set <prefix:term> <val> Set a statement and commit metadata.md
  export csv|xmp|json <path>  Write CSV, XMP or JSON sidecar interchange
  import csv|xmp|json <path>  Read CSV, XMP or JSON sidecar into metadata.md
  validate [prefix:term]  Check a term against the /terms/ register and the
                          namespace registry, or, without a term, every
                          statement of metadata.md; names each unregistered
                          prefix (always run as `fsquid metadata validate`)
  namespace register <prefix> <iri> [--terms a,b,...] [--custom]
                          Register an extension namespace (optional term
                          list) or, with --custom, declare a custom namespace
                          of free keys
  namespace list          Print the registered namespaces
  help                    Show this help

--object is an OCFL object directory (default `.`). --id names the object:
required to create one in a directory without an object, and checked
against an existing object's identifier.

--registry is the OCFL object directory that keeps the namespace registry
(namespaces.md); without it, the registry is the one in --object.
dcterms:, dc:, schema: and exif: are built in and cannot be registered.

JSON sidecars also take --file <logical-path>: the logical file of the
object's head whose SHA-256 digest binds the sidecar with the object
identifier. Import rejects a sidecar whose identifier or digest differs.

Canonical store: metadata.md (YAML front matter + Markdown body), a logical
file of the OCFL object. Each edit commits a new OCFL version; earlier
versions are never modified.
";

/// Metadata command names, as routed by `fsquid`.
pub const COMMANDS: &[&str] = &[
    "extract",
    "get",
    "set",
    "export",
    "import",
    "validate",
    "namespace",
    "help",
];

/// Flags that take a value for every command.
const OBJECT_FLAGS: &[&str] = &["--object", "--id", "--registry"];

/// Flags that take a value for `extract`, `export` and `import`, which also
/// name a logical file of the object.
const INTERCHANGE_FLAGS: &[&str] = &["--object", "--id", "--registry", "--file"];

/// Flags that take a value for `namespace`.
const NAMESPACE_FLAGS: &[&str] = &["--object", "--id", "--registry", "--terms"];

/// Every flag that takes a value; none is ever taken as another's value.
const VALUE_FLAGS: &[&str] = &["--object", "--id", "--registry", "--file", "--terms"];

/// The flag that declares a custom namespace.
const CUSTOM_FLAG: &str = "--custom";

/// Most links of an error's source chain the CLI prints.
const MAX_ERROR_SOURCES: usize = 8;

/// Run the CLI. Returns a process exit code.
pub fn run(args: &[String]) -> Result<i32, String> {
    if args.is_empty() || is_help(&args[0]) {
        print!("{USAGE}");
        return Ok(0);
    }
    let cmd = args[0].as_str();
    if !COMMANDS.contains(&cmd) {
        return Err(format!("unknown command: {cmd}\n{USAGE}"));
    }
    if cmd == "help" {
        print!("{USAGE}");
        return Ok(0);
    }
    let rest = &args[1..];
    match cmd {
        "validate" => validate_args(rest),
        "extract" => cmd_extract(rest),
        "get" => cmd_get(rest),
        "set" => cmd_set(rest),
        "export" => cmd_export(rest),
        "import" => cmd_import(rest),
        "namespace" => cmd_namespace(rest),
        _ => unreachable!(),
    }
}

fn is_help(arg: &str) -> bool {
    arg == "-h" || arg == "--help" || arg == "help"
}

/// Return the value following `flag`, if present.
///
/// A recognised flag is never taken as a value, so `--id --file x` leaves
/// `--id` without a value instead of binding the identifier `--file`.
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].as_str())
        .filter(|value| !VALUE_FLAGS.contains(value))
}

/// Return the value of `flag`, refusing a flag given without one.
fn checked_flag<'a>(args: &'a [String], flag: &str) -> Result<Option<&'a str>, String> {
    if !args.iter().any(|arg| arg == flag) {
        return Ok(None);
    }
    flag_value(args, flag)
        .map(Some)
        .ok_or_else(|| format!("{flag} requires a value"))
}

/// Return the object directory from `--object`, defaulting to `.`.
fn object_dir(args: &[String]) -> Result<PathBuf, String> {
    Ok(checked_flag(args, "--object")?
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".")))
}

/// Open the OCFL object `--object` names, with the identifier `--id` names
/// (file I/O).
fn open_object(args: &[String]) -> Result<LocalObject, String> {
    let dir = object_dir(args)?;
    let id = checked_flag(args, "--id")?;
    LocalObject::open(&dir, id).map_err(|err| match err {
        StoreError::MissingObjectId => format!(
            "{} holds no OCFL object yet; pass --id <object-id> to create it",
            dir.display()
        ),
        other => error_text(&other),
    })
}

/// An error and its sources, joined by `: `.
fn error_text(err: &(dyn Error + 'static)) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    for _ in 0..MAX_ERROR_SOURCES {
        let Some(next) = source else {
            break;
        };
        text.push_str(": ");
        text.push_str(&next.to_string());
        source = next.source();
    }
    text
}

/// Commit `record`, edited from version `base`, to `object` and report the
/// version (or no change). An edit from a head another writer has since
/// moved past is refused rather than overwriting the newer record.
fn commit(
    object: &LocalObject,
    base: Option<&VersionNum>,
    record: &Record,
) -> Result<String, String> {
    let version = object.save_over(base, record).map_err(|e| error_text(&e))?;
    Ok(match version {
        Some(version) => format!("committed {version}"),
        None => "unchanged; no version committed".to_string(),
    })
}

/// Load the head of `object` and its record.
fn load(object: &LocalObject) -> Result<(Option<VersionNum>, Record), String> {
    object.load_head().map_err(|e| error_text(&e))
}

/// Return the arguments that are neither one of `flags` nor their values.
///
/// Only the flags a command recognises are removed, so `set dcterms:title
/// --file` keeps `--file` as the literal value.
fn positional<'a>(args: &'a [String], flags: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut skip_next = false;
    for arg in args {
        let is_flag = flags.contains(&arg.as_str());
        if skip_next && !is_flag {
            skip_next = false;
            continue;
        }
        skip_next = is_flag;
        if !is_flag {
            out.push(arg.as_str());
        }
    }
    out
}

/// The namespace registry the command uses: the one in `--registry`, else
/// the one `object` keeps (file I/O).
fn load_registry(args: &[String], object: &LocalObject) -> Result<NamespaceRegistry, String> {
    match checked_flag(args, "--registry")? {
        Some(dir) => load_named_registry(dir),
        None => object.load_registry().map_err(|e| error_text(&e)),
    }
}

/// The registry kept by the object in `dir`, which must exist (file I/O).
fn load_named_registry(dir: &str) -> Result<NamespaceRegistry, String> {
    let registry = LocalObject::open(Path::new(dir), None)
        .map_err(|e| format!("--registry {dir}: {}", error_text(&e)))?;
    registry.load_registry().map_err(|e| error_text(&e))
}

/// The registry `validate <term>` checks against: `--registry`, else the
/// one in `--object`, else none, so a built-in term validates without an
/// object (file I/O).
fn validate_registry(rest: &[String]) -> Result<NamespaceRegistry, String> {
    if let Some(dir) = checked_flag(rest, "--registry")? {
        return load_named_registry(dir);
    }
    if checked_flag(rest, "--object")?.is_none() {
        return Ok(NamespaceRegistry::default());
    }
    open_object(rest)?
        .load_registry()
        .map_err(|e| error_text(&e))
}

fn validate_args(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, OBJECT_FLAGS);
    let Some(curie) = pos.first() else {
        return validate_record(rest);
    };
    if !curie.contains(':') {
        return Err("validate expects prefix:term".into());
    }
    let registry = validate_registry(rest)?;
    match parse_curie_with(curie, &registry) {
        Ok(_) => {
            println!("{curie} is a valid term");
            Ok(0)
        }
        Err(e) => {
            eprintln!("{curie}: {e}");
            Ok(1)
        }
    }
}

/// Check every statement of the object's record against the registry and
/// report each unregistered prefix or refused term; exit code 1 if any.
fn validate_record(rest: &[String]) -> Result<i32, String> {
    let object = open_object(rest)?;
    let registry = load_registry(rest, &object)?;
    let (_, record) = load(&object)?;
    let mut refused = 0;
    for statement in &record.statements {
        let mut check = Record::default();
        let outcome = check.set_registered(
            &registry,
            statement.namespace.clone(),
            &statement.term,
            statement.value.as_str(),
        );
        if let Err(e) = outcome {
            eprintln!("{}: {e}", statement.curie());
            refused += 1;
        }
    }
    if refused > 0 {
        return Ok(1);
    }
    println!("every statement is in a built-in or registered namespace");
    Ok(0)
}

/// Open `path` and extract its embedded tags into `record` (file I/O).
///
/// The CLI is the native path helper: it opens the file and hands the reader
/// to [`extract_from_reader`], which never sees a path. The [`FileRef`] is the
/// file's name, since a plain file has no object.
fn extract_file(path: &Path, record: &mut Record) -> Result<Extracted, String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} has no file name", path.display()))?;
    // The reference only labels the report; a name that is not a valid logical
    // path (for example one holding a backslash) must not stop extraction.
    let file_ref = FileRef::new("", &name)
        .or_else(|_| FileRef::new("", "file"))
        .map_err(|e| e.to_string())?;
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    extract_from_reader(&mut BufReader::new(file), &file_ref, record).map_err(|e| e.to_string())
}

/// Extract the embedded tags of the logical file `logical_path` of version
/// `base` (the version `record` was loaded from), read through
/// `filesquid-ocfl`, into `record`.
fn extract_logical(
    object: &LocalObject,
    base: Option<&VersionNum>,
    logical_path: &str,
    record: &mut Record,
) -> Result<Extracted, String> {
    let bytes = object
        .read_file(logical_path, base)
        .map_err(|e| format!("{logical_path}: {}", error_text(&e)))?;
    let file_ref = FileRef::new(object.object_id(), logical_path).map_err(|e| e.to_string())?;
    extract_from_bytes(&bytes, &file_ref, record).map_err(|e| e.to_string())
}

fn cmd_extract(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, INTERCHANGE_FLAGS);
    let logical = checked_flag(rest, "--file")?;
    if pos.is_empty() == logical.is_none() {
        return Err(
            "extract requires one media file path or --file <logical-path>, not both".into(),
        );
    }
    let object = open_object(rest)?;
    let (base, mut record) = load(&object)?;
    let found = match logical {
        Some(path) => extract_logical(&object, base.as_ref(), path, &mut record)?,
        None => extract_file(Path::new(pos[0]), &mut record)?,
    };
    if found.rejected > 0 {
        eprintln!("{} statements were refused by the register", found.rejected);
    }
    let outcome = commit(&object, base.as_ref(), &record)?;
    println!("extracted into metadata.md: {outcome}");
    Ok(0)
}

fn cmd_get(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, OBJECT_FLAGS);
    let (_, record) = load(&open_object(rest)?)?;
    if let Some(curie) = pos.first() {
        match record.get(curie) {
            Some(s) => {
                println!("{} {}", s.curie(), s.value);
                Ok(0)
            }
            None => {
                eprintln!("{curie} is not populated");
                Ok(1)
            }
        }
    } else {
        for s in &record.statements {
            println!("{} {}", s.curie(), s.value);
        }
        Ok(0)
    }
}

fn cmd_set(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, OBJECT_FLAGS);
    let Some(curie) = pos.first() else {
        return Err("set requires prefix:term and a value".into());
    };
    let value = pos.get(1).copied().unwrap_or("");
    let object = open_object(rest)?;
    let registry = load_registry(rest, &object)?;
    let (ns, term) = parse_curie_with(curie, &registry).map_err(|e| e.to_string())?;
    let (base, mut record) = load(&object)?;
    record
        .set_registered(&registry, ns, term, value)
        .map_err(|e| e.to_string())?;
    println!("set {curie}: {}", commit(&object, base.as_ref(), &record)?);
    Ok(0)
}

/// The OCFL object identifier and logical-file digest a JSON sidecar binds.
struct JsonBinding {
    /// The object's identifier.
    object_id: String,
    /// Hex SHA-256 of the logical file named by `--file`.
    digest: String,
}

/// Hash the logical file `--file` of version `base` (the version whose
/// record is exported or imported) through `filesquid-ocfl`.
fn read_json_binding(
    rest: &[String],
    object: &LocalObject,
    base: Option<&VersionNum>,
) -> Result<JsonBinding, String> {
    let Some(logical_path) = checked_flag(rest, "--file")? else {
        return Err("json requires --file <logical-path>".into());
    };
    let digest = object
        .file_sha256(logical_path, base)
        .map_err(|e| format!("{logical_path}: {}", error_text(&e)))?;
    Ok(JsonBinding {
        object_id: object.object_id().to_string(),
        digest,
    })
}

/// Render `record` in the requested interchange format.
fn render_export(
    fmt: &str,
    rest: &[String],
    object: &LocalObject,
    base: Option<&VersionNum>,
    record: &Record,
) -> Result<String, String> {
    match fmt {
        "csv" => Ok(to_csv(object.object_id(), record)),
        "xmp" => to_xmp_with(record, &load_registry(rest, object)?).map_err(|e| e.to_string()),
        "json" => {
            let binding = read_json_binding(rest, object, base)?;
            Ok(to_json(&binding.object_id, &binding.digest, record))
        }
        _ => Err("export format must be csv, xmp or json".into()),
    }
}

fn cmd_export(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, INTERCHANGE_FLAGS);
    let fmt = pos.first().copied().unwrap_or("");
    let path = pos.get(1).copied().unwrap_or("");
    if path.is_empty() {
        return Err("export requires csv|xmp|json and a path".into());
    }
    let object = open_object(rest)?;
    let (base, record) = load(&object)?;
    let body = render_export(fmt, rest, &object, base.as_ref(), &record)?;
    std::fs::write(path, body).map_err(|e| e.to_string())?;
    println!("exported {fmt} to {path}");
    Ok(0)
}

/// Merge XMP statements into `record`.
fn apply_xmp(text: &str, registry: &NamespaceRegistry, record: &mut Record) -> Result<(), String> {
    for s in from_xmp_with(text, registry).statements {
        record
            .set_registered(registry, s.namespace, &s.term, s.value)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Merge interchange `text` in the requested format into `record`.
fn apply_import(
    fmt: &str,
    text: &str,
    rest: &[String],
    object: &LocalObject,
    base: Option<&VersionNum>,
    record: &mut Record,
) -> Result<(), String> {
    let registry = load_registry(rest, object)?;
    match fmt {
        "csv" => apply_csv_with(text, &registry, record).map_err(|e| format!("{e:?}")),
        "xmp" => apply_xmp(text, &registry, record),
        "json" => {
            let binding = read_json_binding(rest, object, base)?;
            let target = JsonTarget {
                object_id: &binding.object_id,
                digest: &binding.digest,
            };
            apply_json_with(text, &target, &registry, record).map_err(|e| e.to_string())
        }
        _ => Err("import format must be csv, xmp or json".into()),
    }
}

fn cmd_import(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, INTERCHANGE_FLAGS);
    let fmt = pos.first().copied().unwrap_or("");
    let path = pos.get(1).copied().unwrap_or("");
    if path.is_empty() {
        return Err("import requires csv|xmp|json and a path".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let object = open_object(rest)?;
    let (base, mut record) = load(&object)?;
    apply_import(fmt, &text, rest, &object, base.as_ref(), &mut record)?;
    let outcome = commit(&object, base.as_ref(), &record)?;
    println!("imported {fmt} from {path}: {outcome}");
    Ok(0)
}

/// `namespace register|list`: change or print the namespace registry.
fn cmd_namespace(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, NAMESPACE_FLAGS);
    let pos: Vec<&str> = pos.into_iter().filter(|arg| *arg != CUSTOM_FLAG).collect();
    match (pos.first().copied(), pos.len()) {
        (Some("register"), 3) => namespace_register(rest, pos[1], pos[2]),
        (Some("list"), 1) => namespace_list(rest),
        _ => Err("namespace takes exactly register <prefix> <iri>, or list".into()),
    }
}

/// Open the object that keeps the registry: `--registry`, else `--object`
/// (file I/O).
fn open_registry_object(rest: &[String]) -> Result<LocalObject, String> {
    let Some(dir) = checked_flag(rest, "--registry")? else {
        return open_object(rest);
    };
    let id = checked_flag(rest, "--id")?;
    LocalObject::open(Path::new(dir), id)
        .map_err(|e| format!("--registry {dir}: {}", error_text(&e)))
}

/// The registration `namespace register` describes.
fn registration_from(rest: &[String], prefix: &str, iri: &str) -> Result<Registration, String> {
    let terms = checked_flag(rest, "--terms")?.map(|list| {
        list.split(',')
            .map(|term| term.trim().to_string())
            .collect()
    });
    if !rest.iter().any(|arg| arg == CUSTOM_FLAG) {
        return Ok(Registration::extension(
            prefix,
            iri,
            terms,
            Registrant::Owner,
        ));
    }
    if terms.is_some() {
        return Err(
            "a custom namespace has free keys; --terms cannot be used with --custom".into(),
        );
    }
    Ok(Registration::custom(prefix, iri))
}

/// Register one namespace and commit the registry.
fn namespace_register(rest: &[String], prefix: &str, iri: &str) -> Result<i32, String> {
    let registration = registration_from(rest, prefix, iri)?;
    let object = open_registry_object(rest)?;
    let mut registry = object.load_registry().map_err(|e| error_text(&e))?;
    registry
        .register(registration)
        .map_err(|e| format!("{prefix}: {e}"))?;
    let version = object
        .save_registry(&registry)
        .map_err(|e| error_text(&e))?;
    match version {
        Some(version) => println!("registered {prefix}: committed {version}"),
        None => println!("{prefix} is already registered; no version committed"),
    }
    Ok(0)
}

/// Print each registration: prefix, IRI, kind and term count.
fn namespace_list(rest: &[String]) -> Result<i32, String> {
    let object = open_registry_object(rest)?;
    let registry = object.load_registry().map_err(|e| error_text(&e))?;
    for registration in registry.registrations() {
        let terms = registration
            .terms()
            .map_or("any terms".to_string(), |terms| {
                format!("{} terms", terms.len())
            });
        println!(
            "{} {} {:?} {terms}",
            registration.prefix(),
            registration.iri(),
            registration.kind()
        );
    }
    Ok(0)
}
