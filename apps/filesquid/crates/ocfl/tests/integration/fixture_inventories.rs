//! Root inventories, sidecars and object declarations of the pinned OCFL 1.1
//! community fixtures (OCFL-S-002, OCFL-S-011; `OCFL/fixtures` commit
//! `d323fc207126becec066998b43408d998bf0fb1b`).
//!
//! Set `FILESQUID_OCFL_FIXTURES` to the fixtures' `1.1` directory to run it.
//! Without it the test reports that it skipped and passes: the fixtures are
//! committed under `tests/` by IO-0.5, which then points this test at them.
//!
//! Checked here, from one root inventory and its sidecar only:
//! - every `good-objects` and `warn-objects` fixture parses, its sidecar
//!   verifies and its object declaration is exact;
//! - every `bad-objects` fixture in [`INVENTORY_LEVEL`] is refused, and the
//!   refusal's code is printed next to the codes its name lists.

use std::fs;
use std::path::{Path, PathBuf};

use filesquid_ocfl::inventory::{sidecar_name, verify_sidecar, FormatError, Inventory};
use filesquid_ocfl::namaste::{Declaration, SpecVersion};

/// Bad fixtures whose fault lies in the root inventory or its sidecar, so
/// this task's parser must refuse them. Object-level faults (files on disk,
/// version inventories, extensions) are IO-0.5's.
const INVENTORY_LEVEL: &[&str] = &[
    "E001_invalid_version_format",
    "E008_E036_no_versions_no_head",
    "E010_skipped_versions",
    "E011_E013_invalid_padded_head_version",
    "E017_invalid_content_dir",
    "E025_wrong_digest_algorithm",
    "E036_no_head",
    "E036_no_id",
    "E040_head_not_most_recent",
    "E040_wrong_head_doesnt_exist",
    "E040_wrong_head_format",
    "E041_no_manifest",
    "E049_E050_E054_bad_version_block_values",
    "E049_created_no_timezone",
    "E049_created_not_to_seconds",
    "E050_manifest_digest_wrong_case",
    "E050_state_digest_not_in_manifest",
    "E053_E052_invalid_logical_paths",
    "E058_no_sidecar",
    "E060_E064_root_inventory_digest_mismatch",
    "E061_invalid_sidecar",
    "E095_conflicting_logical_paths",
    "E095_non_unique_logical_paths",
    "E096_manifest_duplicate_digests",
    "E097_fixity_duplicate_digests",
    "E100_E099_fixity_invalid_content_paths",
    "E100_E099_manifest_invalid_content_paths",
    "E101_non_unique_content_paths",
    "E107_file_in_manifest_not_used",
];

/// The fixtures' `1.1` directory, when the environment names it.
fn fixtures() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("FILESQUID_OCFL_FIXTURES")?);
    assert!(
        dir.join("good-objects").is_dir(),
        "{} is not the 1.1 tree",
        dir.display()
    );
    Some(dir)
}

/// The object directories in `dir`, sorted by name.
fn objects(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .expect("list fixtures")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.is_dir())
        .collect();
    found.sort();
    found
}

/// Parse an object's root inventory, check its sidecar, and return the
/// inventory, or the first OCFL code refused. A missing sidecar is E058.
fn check_root(object: &Path) -> Result<Inventory, String> {
    let bytes = fs::read(object.join("inventory.json")).map_err(|e| format!("read: {e}"))?;
    let inventory = Inventory::parse(&bytes).map_err(code_text)?;
    let sidecar = object.join(sidecar_name(inventory.digest_algorithm));
    let Ok(sidecar) = fs::read(sidecar) else {
        return Err("E058".to_string());
    };
    verify_sidecar(inventory.digest_algorithm, &bytes, &sidecar).map_err(code_text)?;
    Ok(inventory)
}

/// The code of a format error, or its text when it has none.
fn code_text(err: FormatError) -> String {
    err.code().map_or_else(|| err.to_string(), str::to_string)
}

/// The object declaration must be the exact 1.1 declaration.
fn check_declaration(object: &Path) {
    let declaration = Declaration::object(SpecVersion::V1_1);
    let contents = fs::read(object.join(declaration.file_name())).expect("declaration");
    assert_eq!(
        declaration.check_contents(&contents),
        Ok(()),
        "{}",
        object.display()
    );
}

#[test]
fn valid_and_warning_fixtures_parse_and_verify() {
    let Some(root) = fixtures() else {
        eprintln!("FILESQUID_OCFL_FIXTURES is not set; skipped");
        return;
    };
    let mut checked = 0;
    for group in ["good-objects", "warn-objects"] {
        for object in objects(&root.join(group)) {
            if let Err(code) = check_root(&object) {
                panic!("{} was refused: {code}", object.display());
            }
            check_declaration(&object);
            checked += 1;
        }
    }
    println!("{checked} valid and warning fixtures parsed and verified");
    assert!(
        checked >= 25,
        "expected the 12 good and 13 warning fixtures"
    );
}

#[test]
fn inventory_level_bad_fixtures_are_refused() {
    let Some(root) = fixtures() else {
        eprintln!("FILESQUID_OCFL_FIXTURES is not set; skipped");
        return;
    };
    for name in INVENTORY_LEVEL {
        let object = root.join("bad-objects").join(name);
        let code = check_root(&object).expect_err(name);
        let listed = name.split('_').any(|part| part == code);
        println!("{name}: refused with {code} (named in the prefix: {listed})");
    }
}

#[test]
fn other_bad_fixtures_report_how_their_root_inventory_reads() {
    let Some(root) = fixtures() else {
        eprintln!("FILESQUID_OCFL_FIXTURES is not set; skipped");
        return;
    };
    for object in objects(&root.join("bad-objects")) {
        let name = object.file_name().and_then(|n| n.to_str()).expect("name");
        if INVENTORY_LEVEL.contains(&name) {
            continue;
        }
        let outcome = match check_root(&object) {
            Ok(_) => "root inventory and sidecar read".to_string(),
            Err(code) => format!("refused with {code}"),
        };
        println!("{name}: {outcome} (object-level fault, IO-0.5)");
    }
}
