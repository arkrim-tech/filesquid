//! `filesquid-c2pa`: the foundation of a stand-alone FOSS C2PA engine, to be
//! implemented from scratch in Rust (FileSquid ADR 026).
//!
//! This release provides only the engine's boundaries: byte I/O, remote
//! fetching and the crypto backend. Reading manifest stores, validation,
//! minting and editing are not implemented yet; they arrive in later
//! releases built on these traits. FileSquid is one caller; any Rust program
//! can use the crate on its own. The core is bytes-only and runs anywhere
//! Rust runs, including `wasm32-unknown-unknown`:
//!
//! - [`io`]: the caller supplies every byte through async [`io::ByteSource`]
//!   and [`io::ByteSink`] values. The core never opens files.
//! - [`fetch`]: remote manifests, revocation data and timestamps go through a
//!   caller-supplied [`fetch::Fetcher`]. The default [`fetch::OfflineFetcher`]
//!   refuses every request. The core never opens a network connection.
//! - [`crypto`]: every cryptographic primitive goes through the
//!   [`crypto::CryptoBackend`] trait. The default `rust-crypto` feature
//!   provides [`crypto::RustCryptoBackend`] on established RustCrypto crates;
//!   nothing is hand-rolled.
//!
//! Trust lists and signers are always supplied by the caller; the crate ships
//! no implicit trust store and operates no certificate authority.

pub mod crypto;
pub mod fetch;
pub mod io;

/// The C2PA Technical Specification version this crate implements.
///
/// The same pin is recorded in the crate README. Specification changes are
/// tracked by raising this pin in a reviewed change.
pub const C2PA_SPEC_VERSION: &str = "2.4";
