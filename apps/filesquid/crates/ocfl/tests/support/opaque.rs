//! A test-only caller transform for the opaque-bytes tests (IO-0.8;
//! OCFL-S-020; ADR 010 "Hosted / optional encrypted package", ADR 023
//! "Dual-profile encryption").
//!
//! The engine stores the bytes its caller supplies. The hosted profile's
//! encryption is the caller's job (`filesquid-api` with the ADR 009 suite),
//! so these tests play the caller with [`ToyCipher`]: a keyed SHA-256
//! counter-mode keystream XORed over content, names and version metadata.
//! It is NOT cryptography (no authentication, a keystream reused per name)
//! and exists only so the tests can store bytes and names the engine cannot
//! read, then check that the caller recovers the plaintext. The owner-
//! readable profile is the identity transform: the same engine calls with
//! the plaintext.

#![allow(dead_code)]

use std::collections::BTreeMap;

use filesquid_ocfl::inventory::User;
use filesquid_ocfl::storage::{read_verified, StorageBackend, StorageKey, MAX_CHUNK_BYTES};
use sha2::{Digest, Sha256};

use crate::content::{ready, ContentFixture, SourceVersion, VersionMeta};
use crate::source_scan;

/// Most keys read back from one written object.
const MAX_OBJECT_KEYS: usize = 1_000;

/// Most keystream blocks for one transform: 64 MiB of keystream.
const MAX_BLOCKS: usize = 2 * 1024 * 1024;

/// A keyed, deterministic XOR keystream transform standing in for a
/// caller's encryption. Applying it twice with the same nonce gives back
/// the input.
#[derive(Debug, Clone)]
pub struct ToyCipher {
    key: [u8; 32],
}

impl ToyCipher {
    /// A transform keyed by the SHA-256 of `secret`.
    pub fn new(secret: &str) -> Self {
        Self {
            key: Sha256::digest(secret.as_bytes()).into(),
        }
    }

    /// `len` keystream bytes for `nonce`: SHA-256 of key, nonce length,
    /// nonce and a block counter, block after block.
    fn keystream(&self, nonce: &[u8], len: usize) -> Vec<u8> {
        let blocks = len.div_ceil(32);
        assert!(blocks <= MAX_BLOCKS, "transform input above the test bound");
        let mut stream = Vec::with_capacity(blocks * 32);
        for counter in 0..blocks as u64 {
            let mut hash = Sha256::new();
            hash.update(self.key);
            hash.update((nonce.len() as u64).to_be_bytes());
            hash.update(nonce);
            hash.update(counter.to_be_bytes());
            stream.extend_from_slice(&hash.finalize());
        }
        stream.truncate(len);
        stream
    }

    /// XOR `bytes` with the keystream for `nonce` (seals and opens).
    pub fn apply(&self, nonce: &[u8], bytes: &[u8]) -> Vec<u8> {
        let stream = self.keystream(nonce, bytes.len());
        bytes.iter().zip(stream).map(|(b, k)| b ^ k).collect()
    }

    /// The opaque logical path for `path`: one lowercase hex segment of the
    /// sealed name, so no plaintext name or directory survives.
    pub fn seal_name(&self, path: &str) -> String {
        hex(&self.apply(b"name", path.as_bytes()))
    }

    /// The plaintext path an opaque name came from.
    pub fn open_name(&self, name: &str) -> String {
        let sealed = unhex(name);
        String::from_utf8(self.apply(b"name", &sealed)).expect("a sealed name opens to UTF-8")
    }

    /// The sealed content of the plaintext logical file `path`. The nonce
    /// is the plaintext path, so an unchanged file seals to the same bytes
    /// in every version (the engine then stores it once).
    pub fn seal_content(&self, path: &str, plain: &[u8]) -> Vec<u8> {
        self.apply(path.as_bytes(), plain)
    }

    /// The plaintext of sealed content stored under the opaque name `name`.
    pub fn open_content(&self, name: &str, sealed: &[u8]) -> Vec<u8> {
        self.apply(self.open_name(name).as_bytes(), sealed)
    }

    /// One version's plaintext files sealed as the hosted profile stores
    /// them: opaque names to sealed content.
    pub fn seal_files(&self, files: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
        files
            .iter()
            .map(|(path, bytes)| (self.seal_name(path), self.seal_content(path, bytes)))
            .collect()
    }

    /// The plaintext files a sealed logical state opens to.
    pub fn open_files(&self, files: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
        files
            .iter()
            .map(|(name, bytes)| (self.open_name(name), self.open_content(name, bytes)))
            .collect()
    }

    /// `fixture` as a hosted-profile caller would hand it to the engine:
    /// sealed names and content, an opaque object id, and sealed `message`
    /// and `user` text. `created` stays a plain RFC 3339 timestamp, which
    /// OCFL requires.
    pub fn seal_fixture(&self, fixture: &ContentFixture) -> ContentFixture {
        let opaque = self.seal_name(&fixture.name);
        let object_id = format!("urn:opaque:{}", self.seal_name(&fixture.object_id()));
        let versions = fixture
            .versions
            .iter()
            .map(|source| self.seal_version(source, &object_id))
            .collect();
        ContentFixture {
            name: opaque,
            versions,
        }
    }

    /// One version sealed, with metadata naming `object_id`.
    fn seal_version(&self, source: &SourceVersion, object_id: &str) -> SourceVersion {
        let meta = source.meta.as_ref();
        SourceVersion {
            name: source.name.clone(),
            files: self.seal_files(&source.files),
            meta: Some(VersionMeta {
                object_id: object_id.to_string(),
                created: meta
                    .map_or_else(|| "2026-10-18T00:00:00Z".to_string(), |m| m.created.clone()),
                message: meta
                    .and_then(|m| m.message.as_deref())
                    .map(|m| self.seal_name(m)),
                user: meta.and_then(|m| m.user.as_ref()).map(|u| User {
                    name: self.seal_name(&u.name),
                    address: None,
                }),
            }),
        }
    }
}

/// Lowercase hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The bytes of lowercase hex `text`.
fn unhex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "hex has an even length");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex digit pair"))
        .collect()
}

/// Words that would make the dual profile an engine type, matched in the
/// engine's code with comments and string contents removed, ignoring case.
pub const PROFILE_WORDS: [&str; 6] = [
    "encrypt",
    "decrypt",
    "cipher",
    "plaintext",
    "profile",
    "aead",
];

/// Encryption crates the engine must not depend on.
pub const CRYPTO_CRATES: [&str; 14] = [
    "aes",
    "aes-gcm",
    "aes-siv",
    "chacha20",
    "chacha20poly1305",
    "ring",
    "openssl",
    "age",
    "sodiumoxide",
    "orion",
    "rsa",
    "x25519-dalek",
    "hpke",
    "aead",
];

/// Each [`PROFILE_WORDS`] entry found in the code of `text` (the file
/// `file`), as `"<file> names <word>"`.
pub fn profile_words_in(file: &str, text: &str) -> Vec<String> {
    let code: String = source_scan::code_chars(text).iter().map(|c| c.c).collect();
    let code = code.to_ascii_lowercase();
    PROFILE_WORDS
        .iter()
        .filter(|word| code.contains(*word))
        .map(|word| format!("src/{file} names {word:?}"))
        .collect()
}

/// The crate names declared in every dependency table of `manifest`.
fn dependency_names(manifest: &str) -> Vec<String> {
    let mut in_dependencies = false;
    let mut names = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line.ends_with("dependencies]");
            continue;
        }
        let name = line.split('=').next().unwrap_or("").trim();
        if in_dependencies && !name.is_empty() && !name.starts_with('#') {
            names.push(name.trim_matches('"').to_string());
        }
    }
    names
}

/// The [`CRYPTO_CRATES`] that `manifest` declares as dependencies.
pub fn crypto_dependencies(manifest: &str) -> Vec<String> {
    dependency_names(manifest)
        .into_iter()
        .filter(|name| CRYPTO_CRATES.contains(&name.as_str()))
        .collect()
}

/// Every reason the engine would carry the dual profile itself: a profile
/// or encryption word in the code under `src/`, or an encryption crate in
/// `Cargo.toml` (OCFL-S-020). Empty when the profile stays a caller.
pub fn engine_profile_findings() -> Vec<String> {
    let sources = source_scan::read_sources().expect("scan src");
    assert!(sources.len() > 5, "engine sources not found");
    let mut findings: Vec<String> = sources
        .iter()
        .flat_map(|(file, text)| profile_words_in(file, text))
        .collect();
    let manifest_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(manifest_path).expect("Cargo.toml");
    assert!(
        dependency_names(&manifest).iter().any(|d| d == "sha2"),
        "manifest parsed"
    );
    let crates = crypto_dependencies(&manifest);
    findings.extend(crates.iter().map(|c| format!("Cargo.toml depends on {c}")));
    findings
}

/// The root inventory of the object at `root`, as stored bytes.
pub fn inventory_bytes<B: StorageBackend>(storage: &B, root: &str) -> Vec<u8> {
    let key = StorageKey::new(&format!("{root}/inventory.json")).expect("key");
    ready(read_verified(storage, &key, MAX_CHUNK_BYTES)).expect("inventory")
}

/// Every key stored under the object root `root`.
pub fn object_keys<B: StorageBackend>(storage: &B, root: &str) -> Vec<String> {
    let page = ready(storage.list(&format!("{root}/"), None, MAX_OBJECT_KEYS)).expect("list");
    assert!(page.next.is_none(), "object fits one page");
    page.keys.iter().map(|k| k.as_str().to_string()).collect()
}

/// Plaintext logical paths and metadata text of `fixture` that cannot be
/// mistaken for lowercase hex.
pub fn plaintext_markers(fixture: &ContentFixture) -> Vec<String> {
    let is_hex = |text: &str| text.bytes().all(|b| b.is_ascii_hexdigit() || b == b'/');
    let mut markers: Vec<String> = Vec::new();
    for source in &fixture.versions {
        markers.extend(source.files.keys().cloned());
        let meta = source.meta.as_ref();
        markers.extend(meta.and_then(|m| m.message.clone()));
        markers.extend(meta.and_then(|m| m.user.as_ref()).map(|u| u.name.clone()));
    }
    markers.retain(|m| !is_hex(m));
    markers.sort();
    markers.dedup();
    markers
}

/// Where the object at `root` shows plaintext of `fixture`: a plaintext
/// name or metadata text in its inventory or a stored key, or a stored
/// content file equal to a plaintext file of at least 4 bytes.
pub fn plaintext_leaks<B: StorageBackend>(
    storage: &B,
    root: &str,
    fixture: &ContentFixture,
) -> Vec<String> {
    let keys = object_keys(storage, root);
    let inventory = String::from_utf8(inventory_bytes(storage, root)).expect("UTF-8 JSON");
    let mut leaks = Vec::new();
    for marker in plaintext_markers(fixture) {
        if inventory.contains(&marker) {
            leaks.push(format!("inventory shows {marker:?}"));
        }
        let shown = keys.iter().filter(|k| k.contains(&marker));
        leaks.extend(shown.map(|k| format!("key {k} shows {marker:?}")));
    }
    let plain: Vec<&Vec<u8>> = fixture
        .versions
        .iter()
        .flat_map(|s| s.files.values())
        .filter(|bytes| bytes.len() >= 4)
        .collect();
    for raw in keys.iter().filter(|k| k.contains("/content/")) {
        let key = StorageKey::new(raw).expect("key");
        let stored = ready(read_verified(storage, &key, MAX_CHUNK_BYTES)).expect("content");
        if plain.contains(&&stored) {
            leaks.push(format!("{raw} holds plaintext"));
        }
    }
    leaks
}
