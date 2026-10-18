//! `filesquid-c2pa`: a stand-alone FOSS C2PA engine, implemented from
//! scratch in Rust (FileSquid ADR 026).
//!
//! This release provides the engine's boundaries (byte I/O, remote fetching
//! and the crypto backend) and manifest store parsing. Locating stores in
//! asset formats, validation, minting and editing are not implemented yet;
//! they arrive in later releases built on these modules. FileSquid is one
//! caller; any Rust program can use the crate on its own. The core is
//! bytes-only and runs anywhere Rust runs, including
//! `wasm32-unknown-unknown`:
//!
//! - [`io`]: the caller supplies every byte through async [`io::ByteSource`]
//!   and [`io::ByteSink`] values. The core never opens files.
//! - [`fetch`]: remote manifests, revocation data and timestamps go through a
//!   caller-supplied [`fetch::Fetcher`]. The default [`fetch::OfflineFetcher`]
//!   refuses every request. The core never opens a network connection.
//! - [`crypto`]: every cryptographic primitive goes through the
//!   [`crypto::CryptoBackend`] trait. The default `rust-crypto` feature
//!   provides `crypto::RustCryptoBackend` on established RustCrypto crates;
//!   nothing is hand-rolled.
//! - [`jumbf`] and [`cbor`]: bounded parsers for the JUMBF boxes and CBOR
//!   that carry C2PA data.
//! - [`manifest`]: manifest stores, manifests, claims (v1 and v2), claim
//!   signatures and assertions, with the specification's failure codes.
//! - [`read`]: reading a store from a byte source (a sidecar `.c2pa` file)
//!   or by remote reference through the caller's fetcher.
//!
//! Trust lists and signers are always supplied by the caller; the crate ships
//! no implicit trust store and operates no certificate authority.

pub mod cbor;
pub mod crypto;
pub mod fetch;
pub mod io;
pub mod jumbf;
pub mod manifest;
pub mod read;

/// The C2PA Technical Specification version this crate implements.
///
/// The same pin is recorded in the crate README. Specification changes are
/// tracked by raising this pin in a reviewed change.
pub const C2PA_SPEC_VERSION: &str = "2.4";
