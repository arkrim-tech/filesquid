//! Cucumber harness for the F-054 Gherkin under `tests/features/`.
//!
//! Each run clears `tests/allure-results/` and writes
//! `tests/allure-results/cucumber-junit.xml` with one `<testcase>` per
//! scenario. Scenarios whose steps belong to later tasks (reading, minting,
//! editing, the UI panel) have no step definitions yet and are reported as
//! skipped.

#[path = "support/deps.rs"]
#[allow(dead_code)]
mod deps;
#[path = "support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
#[path = "support/oracle.rs"]
#[allow(dead_code)]
mod oracle;
#[path = "support/source_scan.rs"]
#[allow(dead_code)]
mod source_scan;
#[path = "support/wasm.rs"]
mod wasm;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use cucumber::{given, then, when, writer, StatsWriter as _, World, WriterExt as _};
use filesquid_c2pa::crypto::{CryptoBackend, CryptoError, RustCryptoBackend, SigningAlgorithm};
use filesquid_c2pa::fetch::{
    fetch_bounded, FetchError, FetchPurpose, FetchRequest, FetchResponse, Fetcher, OfflineFetcher,
};
use filesquid_c2pa::io::{read_exact_at, ByteIoError, ByteSource, MemorySource};
use filesquid_c2pa::jumbf::JumbfError;
use filesquid_c2pa::manifest::{
    AssertionContent, ClaimVersion, IgnoreReason, ManifestStore, ParseLimits, StoreError,
};
use filesquid_c2pa::read::{fetch_remote_manifest_store, read_manifest_store, ReadError};

/// RFC 8032 section 7.1 TEST 1 public key as an Ed25519 SubjectPublicKeyInfo.
const RFC8032_T1_SPKI: &str =
    "302a300506032b6570032100d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

/// RFC 8032 section 7.1 TEST 1 signature over the empty message.
const RFC8032_T1_SIGNATURE: &str = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

/// The caller-supplied fetcher a scenario configured.
#[derive(Debug, Default)]
enum CallerFetcher {
    /// No fetcher: the engine uses [`OfflineFetcher`].
    #[default]
    Offline,
    /// A fetcher returning a body of this many bytes.
    Fixed(usize),
    /// A fetcher serving these manifest store bytes.
    Store(Vec<u8>),
}

/// A fetcher returning a fixed-size body.
struct FixedFetcher(usize);

impl Fetcher for FixedFetcher {
    async fn fetch(&self, _request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        Ok(FetchResponse {
            content_type: None,
            body: vec![0u8; self.0],
        })
    }
}

/// A fetcher serving fixed bytes and recording each request's purpose.
struct StoreFetcher {
    /// The served body.
    body: Vec<u8>,
    /// Purposes of the requests seen.
    purposes: std::sync::Mutex<Vec<FetchPurpose>>,
}

impl Fetcher for StoreFetcher {
    async fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        self.purposes
            .lock()
            .expect("purposes")
            .push(request.purpose);
        Ok(FetchResponse {
            content_type: Some("application/c2pa".to_string()),
            body: self.body.clone(),
        })
    }
}

/// A sidecar source that counts reads, for the size-limit scenario.
#[derive(Debug, Default)]
struct SidecarSource {
    /// The sidecar bytes.
    inner: MemorySource,
    /// Number of `read_at` calls.
    reads: usize,
}

impl ByteSource for SidecarSource {
    async fn size(&mut self) -> Result<u64, ByteIoError> {
        self.inner.size().await
    }

    async fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, ByteIoError> {
        self.reads += 1;
        self.inner.read_at(offset, buf).await
    }
}

/// World state for F-054 scenarios.
#[derive(Debug, Default, World)]
struct C2paWorld {
    /// Package names or text captured by the last inspection step.
    inspected: Vec<String>,
    /// Outcome of the last wasm build (`Ok(None)` = built, `Ok(Some)` = skipped).
    build: Option<Result<Option<String>, String>>,
    /// (SPKI DER, signature) for the signature scenarios.
    signature_input: Option<(Vec<u8>, Vec<u8>)>,
    /// Outcome of the last verification.
    verification: Option<Result<(), CryptoError>>,
    /// The caller-supplied byte source.
    source: Option<MemorySource>,
    /// Outcome of the last byte-source read.
    read: Option<Result<Vec<u8>, ByteIoError>>,
    /// The caller-supplied fetcher.
    fetcher: CallerFetcher,
    /// Outcome of the last fetch.
    fetched: Option<Result<FetchResponse, FetchError>>,
    /// The sidecar a scenario reads.
    sidecar: SidecarSource,
    /// Limits for the read; `None` means the defaults.
    limits: Option<ParseLimits>,
    /// Outcome of the last manifest store read.
    store: Option<Result<ManifestStore, ReadError>>,
    /// Request purposes seen by a store-serving fetcher.
    purposes: Vec<FetchPurpose>,
}

impl C2paWorld {
    /// The parsed store of the last read, which must have succeeded.
    fn parsed(&self) -> &ManifestStore {
        match self.store.as_ref().expect("a read ran") {
            Ok(store) => store,
            Err(err) => panic!("read failed: {err}"),
        }
    }

    /// The error of the last read, which must have failed.
    fn read_error(&self) -> &ReadError {
        match self.store.as_ref().expect("a read ran") {
            Ok(_) => panic!("read succeeded"),
            Err(err) => err,
        }
    }
}

/// Decode lowercase hex from a published test vector.
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex digit"))
        .collect()
}

#[when("I inspect filesquid-c2pa library dependencies")]
fn inspect_dependencies(world: &mut C2paWorld) {
    world.inspected = deps::resolved_runtime_packages(&[]).expect("cargo tree");
}

#[then(
    "filesquid-ocfl, axum, tauri, sqlite, libvips, ffmpeg, and filesquid-core should be absent from default dependencies"
)]
fn forbidden_absent(world: &mut C2paWorld) {
    let found = deps::violations(world.inspected.iter().map(String::as_str));
    assert!(found.is_empty(), "forbidden dependencies: {found:?}");
    assert!(world.inspected.iter().any(|n| n == deps::SELF_NAME));
}

#[then("no runtime dependency on the official c2pa crate should exist")]
fn official_crate_absent(world: &mut C2paWorld) {
    for name in &world.inspected {
        assert!(
            name != "c2pa" && !name.starts_with("c2pa-") && name != "c2patool",
            "{name} is a runtime dependency"
        );
    }
}

#[then("the c2patool oracle harness should report verdicts or a skip reason")]
fn oracle_reports(_world: &mut C2paWorld) {
    let config = match oracle::oracle_setup_from_env() {
        oracle::OracleSetup::Ready(config) => config,
        oracle::OracleSetup::Skipped(reason) => {
            assert!(!reason.is_empty());
            eprintln!("SKIPPED c2patool oracle: {reason}");
            return;
        }
    };
    let files = oracle::corpus_files(&config.corpus, oracle::MAX_CORPUS_FILES).expect("corpus");
    let first = files.first().expect("one corpus asset");
    let report = oracle::run_c2patool(&config.tool, first).expect("c2patool verdict");
    println!(
        "c2patool verdict for {}: {:?}, failure codes {:?}",
        first.display(),
        report.verdict,
        report.failure_codes
    );
}

#[when(expr = "I build filesquid-c2pa for wasm32-unknown-unknown with default features {word}")]
fn build_wasm(world: &mut C2paWorld, state: String) {
    let default_features = state == "on";
    let outcome = wasm::build_for_wasm(default_features).map(|build| match build {
        wasm::WasmBuild::Built => None,
        wasm::WasmBuild::Skipped(reason) => Some(reason),
    });
    world.build = Some(outcome);
}

#[then("the build should succeed")]
fn build_succeeded(world: &mut C2paWorld) {
    match world.build.take().expect("build ran") {
        Ok(None) => {}
        // `main` filters @wasm32 scenarios out when the check is opted out,
        // so a skip here means the filter and the opt-out disagree.
        Ok(Some(reason)) => panic!("wasm32 scenario ran although skipped: {reason}"),
        Err(output) => panic!("wasm32 build check failed:\n{output}"),
    }
}

#[when("I inspect the filesquid-c2pa sources")]
fn inspect_sources(world: &mut C2paWorld) {
    world.inspected = source_scan::read_sources()
        .expect("read src")
        .into_iter()
        .map(|(file, _)| file)
        .collect();
}

#[then("only the crypto backend should name a cryptography crate")]
fn crypto_behind_backend(world: &mut C2paWorld) {
    assert!(world
        .inspected
        .iter()
        .any(|file| file == source_scan::CRYPTO_BACKEND_MODULE));
    let found = source_scan::crypto_findings_outside_backend().expect("scan");
    assert!(found.is_empty(), "{found:?}");
}

#[then("no direct filesystem, network or process call should be present")]
fn no_direct_io(world: &mut C2paWorld) {
    assert!(!world.inspected.is_empty());
    let found = source_scan::io_findings().expect("scan");
    assert!(found.is_empty(), "{found:?}");
}

#[given("the RFC 8032 Ed25519 test key and signature")]
fn ed25519_vector(world: &mut C2paWorld) {
    world.signature_input = Some((hex(RFC8032_T1_SPKI), hex(RFC8032_T1_SIGNATURE)));
}

/// Verify the scenario's Ed25519 signature over `message`.
async fn verify_over(world: &mut C2paWorld, message: &[u8]) {
    let (spki, signature) = world.signature_input.clone().expect("vector");
    let backend = RustCryptoBackend;
    let result = backend
        .verify_signature(SigningAlgorithm::Ed25519, &spki, message, &signature)
        .await;
    world.verification = Some(result);
}

#[when("I verify the signature through the default crypto backend")]
async fn verify_valid(world: &mut C2paWorld) {
    verify_over(world, b"").await;
}

#[when("I verify the signature over a different message through the default crypto backend")]
async fn verify_tampered(world: &mut C2paWorld) {
    verify_over(world, b"tampered").await;
}

#[then("the signature should be accepted")]
fn signature_accepted(world: &mut C2paWorld) {
    assert_eq!(world.verification.take(), Some(Ok(())));
}

#[then("verification should fail")]
fn verification_failed(world: &mut C2paWorld) {
    assert_eq!(
        world.verification.take(),
        Some(Err(CryptoError::VerificationFailed))
    );
}

#[given(expr = "a caller-supplied byte source holding {string}")]
fn byte_source(world: &mut C2paWorld, text: String) {
    world.source = Some(MemorySource::new(text.into_bytes()));
}

#[when(expr = "the engine reads {int} bytes at offset {int}")]
async fn engine_reads(world: &mut C2paWorld, len: usize, offset: u64) {
    let mut source = world.source.take().expect("byte source");
    let mut buf = vec![0u8; len];
    let result = read_exact_at(&mut source, offset, &mut buf).await;
    world.read = Some(result.map(|()| buf));
}

#[then(expr = "it should receive {string}")]
fn received(world: &mut C2paWorld, text: String) {
    assert_eq!(world.read.take(), Some(Ok(text.into_bytes())));
}

#[then("an unexpected end of data should be reported")]
fn unexpected_eof(world: &mut C2paWorld) {
    let result = world.read.take().expect("read ran");
    assert!(
        matches!(result, Err(ByteIoError::UnexpectedEof { .. })),
        "{result:?}"
    );
}

#[given("no network fetcher supplied by the caller")]
fn no_fetcher(world: &mut C2paWorld) {
    world.fetcher = CallerFetcher::Offline;
}

#[given(expr = "a caller fetcher that returns {int} bytes")]
fn fixed_fetcher(world: &mut C2paWorld, len: usize) {
    world.fetcher = CallerFetcher::Fixed(len);
}

#[when(expr = "the engine requests a remote manifest of at most {int} bytes")]
async fn request_manifest(world: &mut C2paWorld, max_bytes: u64) {
    let request = FetchRequest::get(
        "https://example.org/manifest.c2pa",
        FetchPurpose::RemoteManifest,
        max_bytes,
    );
    let result = match &world.fetcher {
        CallerFetcher::Offline => fetch_bounded(&OfflineFetcher, &request).await,
        CallerFetcher::Fixed(len) => fetch_bounded(&FixedFetcher(*len), &request).await,
        CallerFetcher::Store(body) => fetch_bounded(&FixedFetcher(body.len()), &request).await,
    };
    world.fetched = Some(result);
}

#[given("a sidecar manifest store with an ingredient manifest and an active manifest")]
fn sidecar_sample(world: &mut C2paWorld) {
    world.sidecar.inner = MemorySource::new(fixtures::sample_store());
}

#[given("a sidecar manifest store whose claim uses the legacy c2pa.claim label")]
fn sidecar_legacy(world: &mut C2paWorld) {
    let bytes = fixtures::store(&[fixtures::legacy_manifest(fixtures::ACTIVE_LABEL)]);
    world.sidecar.inner = MemorySource::new(bytes);
}

#[given("a sidecar manifest store cut short by one byte")]
fn sidecar_truncated(world: &mut C2paWorld) {
    let mut bytes = fixtures::sample_store();
    bytes.pop();
    world.sidecar.inner = MemorySource::new(bytes);
}

#[given("a sidecar manifest store one byte larger than the store size limit")]
fn sidecar_over_limit(world: &mut C2paWorld) {
    let bytes = fixtures::sample_store();
    world.limits = Some(ParseLimits {
        max_store_bytes: bytes.len() as u64 - 1,
        ..ParseLimits::default()
    });
    world.sidecar.inner = MemorySource::new(bytes);
}

#[given("a sidecar manifest store nested one level deeper than the box depth limit")]
fn sidecar_too_deep(world: &mut C2paWorld) {
    // The sample store nests four superboxes: store, manifest, assertion
    // store, assertion.
    world.limits = Some(ParseLimits {
        max_box_depth: 3,
        ..ParseLimits::default()
    });
    world.sidecar.inner = MemorySource::new(fixtures::sample_store());
}

#[given("a sidecar manifest store whose ingredient claim is not valid CBOR")]
fn sidecar_broken_ingredient(world: &mut C2paWorld) {
    let broken = fixtures::manifest(
        b"c2ma",
        fixtures::INGREDIENT_LABEL,
        &[
            fixtures::assertion_store(&fixtures::standard_assertions()),
            fixtures::claim_box("c2pa.claim.v2", &[0xFF]),
            fixtures::signature_box(),
        ],
    );
    let bytes = fixtures::store(&[broken, fixtures::standard_manifest(fixtures::ACTIVE_LABEL)]);
    world.sidecar.inner = MemorySource::new(bytes);
}

#[given("a sidecar manifest store with a vendor box of an unknown type")]
fn sidecar_vendor_box(world: &mut C2paWorld) {
    let vendor = fixtures::superbox(fixtures::uuid(b"zzzz"), "vendor.box", &[]);
    let bytes = fixtures::store(&[vendor, fixtures::standard_manifest(fixtures::ACTIVE_LABEL)]);
    world.sidecar.inner = MemorySource::new(bytes);
}

#[given("a caller fetcher that serves a manifest store")]
fn fetcher_serving_store(world: &mut C2paWorld) {
    world.fetcher = CallerFetcher::Store(fixtures::sample_store());
}

#[when("I read the sidecar with filesquid-c2pa")]
async fn read_sidecar(world: &mut C2paWorld) {
    let limits = world.limits.clone().unwrap_or_default();
    let result = read_manifest_store(&mut world.sidecar, &limits).await;
    world.store = Some(result);
}

#[when(expr = "I read the remote manifest store {string}")]
async fn read_remote(world: &mut C2paWorld, url: String) {
    let limits = world.limits.clone().unwrap_or_default();
    let result = match &world.fetcher {
        CallerFetcher::Store(body) => {
            let fetcher = StoreFetcher {
                body: body.clone(),
                purposes: std::sync::Mutex::new(Vec::new()),
            };
            let result = fetch_remote_manifest_store(&fetcher, &url, &limits).await;
            world.purposes = fetcher.purposes.into_inner().expect("purposes");
            result
        }
        CallerFetcher::Offline => fetch_remote_manifest_store(&OfflineFetcher, &url, &limits).await,
        CallerFetcher::Fixed(len) => {
            fetch_remote_manifest_store(&FixedFetcher(*len), &url, &limits).await
        }
    };
    world.store = Some(result);
}

#[then("the active manifest should be the last manifest in the store")]
fn active_is_last(world: &mut C2paWorld) {
    let store = world.parsed();
    let last = store.manifests().last().expect("a manifest");
    assert_eq!(store.active_manifest().label, last.label);
    assert_eq!(last.label, fixtures::ACTIVE_LABEL);
}

#[then("its v2 claim and its CBOR, JSON and embedded-file assertions should be returned")]
fn v2_claim_and_assertions(world: &mut C2paWorld) {
    let manifest = world
        .parsed()
        .active_manifest()
        .content
        .as_ref()
        .expect("manifest");
    assert_eq!(manifest.claim.version, ClaimVersion::V2);
    assert_eq!(manifest.claim.created_assertions.len(), 4);
    let kinds: Vec<_> = manifest
        .assertions
        .iter()
        .map(|a| match a.content.as_ref().expect("assertion") {
            AssertionContent::Cbor(_) => "cbor",
            AssertionContent::Json(_) => "json",
            AssertionContent::EmbeddedFile { .. } => "embedded",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["cbor", "cbor", "json", "embedded"]);
}

#[then("the claim should be reported as version 1 with its assertions")]
fn v1_claim(world: &mut C2paWorld) {
    let manifest = world
        .parsed()
        .active_manifest()
        .content
        .as_ref()
        .expect("manifest");
    assert_eq!(manifest.claim.version, ClaimVersion::V1);
    assert_eq!(manifest.claim.created_assertions.len(), 1);
    assert_eq!(manifest.assertions.len(), 1);
}

#[then("the fetcher should have been asked for a remote manifest")]
fn fetcher_asked(world: &mut C2paWorld) {
    assert_eq!(world.purposes, [FetchPurpose::RemoteManifest]);
}

#[then(expr = "the read should fail with status code {string}")]
fn read_status_code(world: &mut C2paWorld, code: String) {
    assert_eq!(world.read_error().status_code(), Some(code.as_str()));
}

#[then("the read should fail with a truncated JUMBF box error")]
fn read_truncated(world: &mut C2paWorld) {
    let err = world.read_error();
    assert!(
        matches!(
            err,
            ReadError::Store(StoreError::Jumbf(JumbfError::Truncated { .. }))
        ),
        "{err:?}"
    );
}

#[then("the read should fail as too large without reading the source")]
fn read_too_large(world: &mut C2paWorld) {
    let err = world.read_error();
    assert!(matches!(err, ReadError::TooLarge { .. }), "{err:?}");
    assert_eq!(world.sidecar.reads, 0);
}

#[then("the read should fail with a box depth limit error")]
fn read_too_deep(world: &mut C2paWorld) {
    let err = world.read_error();
    assert!(
        matches!(
            err,
            ReadError::Store(StoreError::Jumbf(JumbfError::TooDeep { max: 3, .. }))
        ),
        "{err:?}"
    );
}

#[then(expr = "the ingredient manifest should report {string}")]
fn ingredient_reports(world: &mut C2paWorld, code: String) {
    let ingredient = &world.parsed().manifests()[0];
    let err = ingredient.content.as_ref().expect_err("broken ingredient");
    assert_eq!(err.status_code(), Some(code.as_str()));
}

#[then("the active manifest should still be readable")]
fn active_readable(world: &mut C2paWorld) {
    assert!(world.parsed().active_manifest().content.is_ok());
}

#[then("the vendor box should be listed as ignored with an unknown type")]
fn vendor_box_ignored(world: &mut C2paWorld) {
    let ignored = world.parsed().ignored();
    assert_eq!(ignored.len(), 1);
    assert_eq!(ignored[0].path, "c2pa/vendor.box");
    assert_eq!(ignored[0].reason, IgnoreReason::UnknownType);
}

#[then("the request should be refused as offline")]
fn refused_offline(world: &mut C2paWorld) {
    let result = world.fetched.take().expect("fetch ran");
    assert!(
        matches!(result, Err(FetchError::Offline { .. })),
        "{result:?}"
    );
}

#[then("the response should be rejected as too large")]
fn rejected_too_large(world: &mut C2paWorld) {
    let result = world.fetched.take().expect("fetch ran");
    assert!(
        matches!(
            result,
            Err(FetchError::TooLarge {
                actual: 32,
                max_bytes: 16,
                ..
            })
        ),
        "{result:?}"
    );
}

#[when("I inspect the filesquid-c2pa specification pin")]
fn inspect_pin(world: &mut C2paWorld) {
    let readme = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("README.md");
    world.inspected = vec![fs::read_to_string(readme).expect("README.md")];
}

#[then("the library and the README should both name C2PA 2.4")]
fn pin_is_2_4(world: &mut C2paWorld) {
    assert_eq!(filesquid_c2pa::C2PA_SPEC_VERSION, "2.4");
    let readme = world.inspected.first().expect("README text");
    assert!(
        readme.contains("Pinned specification: C2PA Technical Specification 2.4"),
        "README must pin C2PA 2.4"
    );
}

/// Remove a directory, treating "already absent" as success.
fn remove_dir_if_present(dir: &Path) {
    match fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => panic!("remove {}: {err}", dir.display()),
    }
}

#[tokio::main]
async fn main() {
    let results_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/allure-results");
    remove_dir_if_present(&results_dir);
    fs::create_dir_all(&results_dir).expect("allure-results");
    let features = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/features");
    let junit = fs::File::create(results_dir.join("cucumber-junit.xml")).expect("junit file");
    // cucumber-rs has no runtime skip, so an explicit opt-out removes the
    // @wasm32 scenarios from the run and says so; it never reports a pass.
    let wasm_skip = wasm::opted_out();
    if let Some(reason) = &wasm_skip {
        println!("SKIPPED @wasm32 scenarios: {reason}");
    }
    let writer = C2paWorld::cucumber()
        .with_writer(
            writer::Basic::stdout()
                .summarized()
                .tee::<C2paWorld, _>(writer::JUnit::for_tee(junit, 0))
                .normalized(),
        )
        .filter_run(features, move |_, _, scenario| {
            wasm_skip.is_none() || !scenario.tags.iter().any(|tag| tag == "wasm32")
        })
        .await;
    if writer.execution_has_failed() {
        std::process::exit(1);
    }
}
