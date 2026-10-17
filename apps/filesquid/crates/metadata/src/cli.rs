//! Metadata commands of the `fsquid` CLI: extract, get, set, export, import, validate.
//!
//! The `filesquid-cli` crate builds the only CLI binary, `fsquid`, and routes
//! metadata commands here through [`run`]. This module ships no binary of its own.

use std::path::{Path, PathBuf};

use filesquid_ocfl::read_logical_file;

use crate::csv::{apply_csv, to_csv};
use crate::extract::extract_into;
use crate::json::{apply_json, sha256_hex, to_json, JsonTarget};
use crate::record::{parse_curie, Record};
use crate::reject_file_dcterms_term;
use crate::store::{load, save};
use crate::xmp::{from_xmp, to_xmp};

/// Usage text for the metadata commands of `fsquid`.
pub const USAGE: &str = "\
fsquid metadata — metadata CRUD (via filesquid-ocfl)

Usage:
  fsquid <command> [args] [--object <dir>]
  fsquid metadata <command> [args] [--object <dir>]

Commands:
  extract <file>          Extract EXIF/ID3 from a media file into metadata.md
  get [prefix:term]       Print populated statements (or one CURIE)
  set <prefix:term> <val> Set a statement and write metadata.md
  export csv|xmp|json <path>  Write CSV, XMP or JSON sidecar interchange
  import csv|xmp|json <path>  Read CSV, XMP or JSON sidecar into metadata.md
  validate <prefix:term>  Check a dcterms: term against the /terms/ register
                          (always run as `fsquid metadata validate`)
  help                    Show this help

JSON sidecars also take --id <object-id> --file <logical-path>: the OCFL
object identifier and the logical file whose SHA-256 digest binds the
sidecar. Import rejects a sidecar whose identifier or digest differs.

Canonical store: <object>/metadata.md (YAML front matter + Markdown body).
";

/// Metadata command names, as routed by `fsquid`.
pub const COMMANDS: &[&str] = &[
    "extract", "get", "set", "export", "import", "validate", "help",
];

/// Flags that take a value for every command.
const OBJECT_FLAGS: &[&str] = &["--object"];

/// Flags that take a value for `export` and `import`, which bind JSON sidecars.
const INTERCHANGE_FLAGS: &[&str] = &["--object", "--id", "--file"];

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
        .filter(|value| !INTERCHANGE_FLAGS.contains(value))
}

/// Return the object directory from `--object`, defaulting to `.`.
fn object_dir(args: &[String]) -> PathBuf {
    flag_value(args, "--object")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
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

fn validate_args(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, OBJECT_FLAGS);
    let Some(curie) = pos.first() else {
        return Err("validate requires a term such as dcterms:title".into());
    };
    let Some((prefix, term)) = curie.split_once(':') else {
        return Err("validate expects prefix:term".into());
    };
    if prefix != "dcterms" {
        return Err("validate currently checks dcterms: terms".into());
    }
    match reject_file_dcterms_term(term) {
        Ok(()) => {
            println!("{curie} is a valid file dcterms term");
            Ok(0)
        }
        Err(e) => {
            eprintln!("{curie}: {e}");
            Ok(1)
        }
    }
}

fn cmd_extract(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, OBJECT_FLAGS);
    let Some(file) = pos.first() else {
        return Err("extract requires a media file path".into());
    };
    let object = object_dir(rest);
    let mut record = load(&object).map_err(|e| e.to_string())?;
    extract_into(Path::new(file), &mut record);
    save(&object, &record).map_err(|e| e.to_string())?;
    println!("extracted into {}/metadata.md", object.display());
    Ok(0)
}

fn cmd_get(rest: &[String]) -> Result<i32, String> {
    let pos = positional(rest, OBJECT_FLAGS);
    let object = object_dir(rest);
    let record = load(&object).map_err(|e| e.to_string())?;
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
    let object = object_dir(rest);
    let (ns, term) = parse_curie(curie).map_err(|e| e.to_string())?;
    let mut record = load(&object).map_err(|e| e.to_string())?;
    record.set(ns, term, value).map_err(|e| e.to_string())?;
    save(&object, &record).map_err(|e| e.to_string())?;
    println!("set {curie}");
    Ok(0)
}

/// The OCFL object identifier and logical-file digest a JSON sidecar binds.
struct JsonBinding {
    /// OCFL object identifier from `--id`.
    object_id: String,
    /// Hex SHA-256 of the logical file named by `--file`.
    digest: String,
}

/// Check that `logical_path` resolves inside `object` (file I/O).
///
/// `filesquid-ocfl` rejects lexical escapes such as `..`, but a symlink inside
/// the object could still point elsewhere, so both paths are resolved first.
fn check_resolves_inside_object(object: &Path, logical_path: &str) -> Result<(), String> {
    let root = object.canonicalize().map_err(|e| e.to_string())?;
    let target = object
        .join(logical_path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !target.starts_with(&root) {
        return Err(format!("{logical_path} resolves outside the object"));
    }
    Ok(())
}

/// Read `--id` and hash the `--file` logical file of `object` (file I/O).
///
/// The file is read through `filesquid-ocfl`, which applies the Git guard and
/// rejects lexical path escapes; symlink escapes are rejected beforehand.
fn read_json_binding(rest: &[String], object: &Path) -> Result<JsonBinding, String> {
    let (Some(object_id), Some(logical_path)) =
        (flag_value(rest, "--id"), flag_value(rest, "--file"))
    else {
        return Err("json requires --id <object-id> and --file <logical-path>".into());
    };
    check_resolves_inside_object(object, logical_path)?;
    let bytes = read_logical_file(object, logical_path).map_err(|e| e.to_string())?;
    Ok(JsonBinding {
        object_id: object_id.to_string(),
        digest: sha256_hex(&bytes),
    })
}

/// Render `record` in the requested interchange format.
fn render_export(
    fmt: &str,
    rest: &[String],
    object: &Path,
    record: &Record,
) -> Result<String, String> {
    match fmt {
        "csv" => Ok(to_csv(&object.display().to_string(), record)),
        "xmp" => Ok(to_xmp(record)),
        "json" => {
            let binding = read_json_binding(rest, object)?;
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
    let object = object_dir(rest);
    let record = load(&object).map_err(|e| e.to_string())?;
    let body = render_export(fmt, rest, &object, &record)?;
    std::fs::write(path, body).map_err(|e| e.to_string())?;
    println!("exported {fmt} to {path}");
    Ok(0)
}

/// Merge XMP statements into `record`.
fn apply_xmp(text: &str, record: &mut Record) -> Result<(), String> {
    for s in from_xmp(text).statements {
        record
            .set(s.namespace, &s.term, s.value)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Merge interchange `text` in the requested format into `record`.
fn apply_import(
    fmt: &str,
    text: &str,
    rest: &[String],
    object: &Path,
    record: &mut Record,
) -> Result<(), String> {
    match fmt {
        "csv" => apply_csv(text, record).map_err(|e| format!("{e:?}")),
        "xmp" => apply_xmp(text, record),
        "json" => {
            let binding = read_json_binding(rest, object)?;
            let target = JsonTarget {
                object_id: &binding.object_id,
                digest: &binding.digest,
            };
            apply_json(text, &target, record).map_err(|e| e.to_string())
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
    let object = object_dir(rest);
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut record = load(&object).map_err(|e| e.to_string())?;
    apply_import(fmt, &text, rest, &object, &mut record)?;
    save(&object, &record).map_err(|e| e.to_string())?;
    println!("imported {fmt} from {path}");
    Ok(0)
}
