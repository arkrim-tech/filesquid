//! Step definitions for `inventory_types_OCFL-S-002.feature`: SHA-256
//! digests, inventory parsing, sidecars and NAMASTE declarations.

use cucumber::{given, then, when};
use filesquid_ocfl::digest::DigestAlgorithm;
use filesquid_ocfl::inventory::{
    sidecar_contents, verify_sidecar, FormatError, Inventory, MAX_INVENTORY_BYTES,
};
use filesquid_ocfl::namaste::{Declaration, SpecVersion};
use serde_json::{json, Value};

use crate::example::{base, A, C, EMPTY};
use crate::OcflWorld;

/// What the inventory scenarios carry between steps.
#[derive(Debug, Default)]
pub struct InventoryState {
    json: Option<Value>,
    bytes: Option<Vec<u8>>,
    parsed: Option<Result<Inventory, FormatError>>,
    digest: Option<(DigestAlgorithm, String)>,
    sidecar: Option<String>,
    declaration: Option<Declaration>,
}

#[when(expr = "I digest {string} with the engine's digest algorithm")]
fn digest_text(world: &mut OcflWorld, text: String) {
    let algorithm = DigestAlgorithm::ENGINE;
    world.inventory.digest = Some((algorithm, algorithm.digest_hex(text.as_bytes())));
}

#[then(expr = "the algorithm should be named {string}")]
fn algorithm_named(world: &mut OcflWorld, name: String) {
    let (algorithm, _) = world.inventory.digest.as_ref().expect("a digest");
    assert_eq!(algorithm.name(), name);
}

#[then(expr = "the digest should be {string}")]
fn digest_is(world: &mut OcflWorld, expected: String) {
    let (_, digest) = world.inventory.digest.as_ref().expect("a digest");
    assert_eq!(*digest, expected);
}

#[given("the example OCFL 1.1 inventory")]
fn example_inventory(world: &mut OcflWorld) {
    world.inventory.json = Some(base());
}

/// Apply the named change to the example inventory.
fn change(value: &mut Value, change: &str) {
    match change {
        "it has an unknown key" => value["extra"] = json!(true),
        "it has no id" => drop(value.as_object_mut().expect("root").remove("id")),
        "its digest algorithm is md5" => value["digestAlgorithm"] = json!("md5"),
        "its head is not the highest version" => value["head"] = json!("v1"),
        "its versions skip a number" => rename_version(value, "v2", "v3"),
        "a version was created without a time zone" => {
            value["versions"]["v1"]["created"] = json!("2018-01-01T01:01:01")
        }
        "a content path ends with a slash" => value["manifest"][C] = json!(["v2/content/"]),
        "a state digest differs in case from the manifest" => {
            let state = value["versions"]["v1"]["state"]
                .as_object_mut()
                .expect("state");
            let paths = state.remove(A).expect("A");
            state.insert(A.to_uppercase(), paths);
        }
        "two logical paths in a version conflict" => {
            value["versions"]["v2"]["state"][C] = json!(["foo"])
        }
        "a digest appears twice in the manifest" => {
            value["manifest"][EMPTY.to_uppercase()] = json!(["v2/content/again.txt"])
        }
        other => panic!("unknown change: {other}"),
    }
}

/// Rename a version block and point head at the new name.
fn rename_version(value: &mut Value, from: &str, to: &str) {
    let versions = value["versions"].as_object_mut().expect("versions");
    let block = versions.remove(from).expect("version");
    versions.insert(to.to_string(), block);
    value["head"] = json!(to);
}

#[given(regex = r"^the inventory is changed so that (.+)$")]
fn changed_inventory(world: &mut OcflWorld, what: String) {
    change(world.inventory.json.as_mut().expect("an inventory"), &what);
}

#[given("an inventory one byte larger than the parser's bound")]
fn oversized_inventory(world: &mut OcflWorld) {
    world.inventory.bytes = Some(vec![b' '; MAX_INVENTORY_BYTES + 1]);
}

/// The bytes the When step parses: given bytes, or the JSON serialised.
fn inventory_bytes(world: &OcflWorld) -> Vec<u8> {
    if let Some(bytes) = &world.inventory.bytes {
        return bytes.clone();
    }
    let value = world.inventory.json.as_ref().expect("an inventory");
    serde_json::to_vec(value).expect("serialise")
}

#[when("I parse the inventory")]
fn parse_inventory(world: &mut OcflWorld) {
    world.inventory.parsed = Some(Inventory::parse(&inventory_bytes(world)));
}

/// The parsed inventory; fails the scenario when it was refused.
fn parsed(world: &OcflWorld) -> &Inventory {
    match world.inventory.parsed.as_ref().expect("a parse ran") {
        Ok(inventory) => inventory,
        Err(err) => panic!("the inventory was refused: {err}"),
    }
}

#[then(expr = "the head should be {string}")]
fn head_is(world: &mut OcflWorld, head: String) {
    assert_eq!(parsed(world).head.to_string(), head);
}

#[then(expr = "the content directory should be {string}")]
fn content_directory_is(world: &mut OcflWorld, name: String) {
    assert_eq!(parsed(world).content_directory(), name);
}

#[then(expr = "the inventory should have {int} versions")]
fn version_count(world: &mut OcflWorld, count: usize) {
    assert_eq!(parsed(world).versions.len(), count);
}

/// The refusal from the When step; fails the scenario when it parsed.
fn refusal(world: &mut OcflWorld) -> FormatError {
    match world.inventory.parsed.take().expect("a parse ran") {
        Ok(_) => panic!("the inventory was accepted"),
        Err(err) => err,
    }
}

#[then(expr = "the inventory should be refused with {string}")]
fn refused_with(world: &mut OcflWorld, code: String) {
    assert_eq!(refusal(world).code(), Some(code.as_str()));
}

#[then("the inventory should be refused as too large")]
fn refused_as_too_large(world: &mut OcflWorld) {
    let err = refusal(world);
    assert!(matches!(err, FormatError::TooLarge { .. }), "{err:?}");
}

#[when("I write its sha256 sidecar")]
fn write_sidecar(world: &mut OcflWorld) {
    let bytes = inventory_bytes(world);
    world.inventory.sidecar = Some(sidecar_contents(DigestAlgorithm::Sha256, &bytes));
}

#[then("the sidecar should verify against the inventory")]
fn sidecar_verifies(world: &mut OcflWorld) {
    let sidecar = world.inventory.sidecar.as_ref().expect("a sidecar");
    let bytes = inventory_bytes(world);
    assert_eq!(
        verify_sidecar(DigestAlgorithm::Sha256, &bytes, sidecar.as_bytes()),
        Ok(())
    );
}

#[then(expr = "a sidecar for different bytes should be refused with {string}")]
fn other_sidecar_refused(world: &mut OcflWorld, code: String) {
    let other = sidecar_contents(DigestAlgorithm::Sha256, b"different bytes");
    let bytes = inventory_bytes(world);
    let err = verify_sidecar(DigestAlgorithm::Sha256, &bytes, other.as_bytes());
    assert_eq!(err.expect_err("refused").code(), Some(code.as_str()));
}

#[when("I make the OCFL 1.1 object declaration")]
fn make_declaration(world: &mut OcflWorld) {
    world.inventory.declaration = Some(Declaration::object(SpecVersion::V1_1));
}

#[then(expr = "its file name should be {string}")]
fn declaration_file_name(world: &mut OcflWorld, name: String) {
    let declaration = world.inventory.declaration.as_ref().expect("a declaration");
    assert_eq!(declaration.file_name(), name);
}

#[then(expr = "its contents should be {string} and a newline")]
fn declaration_contents(world: &mut OcflWorld, text: String) {
    let declaration = world.inventory.declaration.as_ref().expect("a declaration");
    assert_eq!(declaration.contents(), format!("{text}\n"));
}
