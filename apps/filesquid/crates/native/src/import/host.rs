//! The importing host (ADR 038 Decision 3): operating system name, version and build, CPU
//! architecture, host name and the account that ran the import, from `uname`, `gethostname`
//! and the account database, plus this FileSquid build.

use filesquid_metadata::fact::{Fact, Reason};
use filesquid_metadata::import::Host;

/// Which FileSquid build imports: version, build and distribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    /// The FileSquid version.
    pub version: String,
    /// The build identifier.
    pub build: String,
    /// The distribution (`filesquid`, `imagesquid`).
    pub distribution: String,
}

/// The facts of this host for `build`. Reads `uname`, the host name and the account
/// database.
pub fn gather_host(build: &Build) -> Host {
    let system = sys::uname();
    let field = |value: Option<String>| match value {
        Some(text) if !text.is_empty() => Fact::value(text),
        _ => Fact::unavailable_because(Reason::NotSupportedBySource, "not read on this platform"),
    };
    Host {
        os_name: field(system.as_ref().map(|s| s.0.clone())),
        os_version: field(system.as_ref().map(|s| s.1.clone())),
        os_build: field(system.as_ref().map(|s| s.2.clone())),
        architecture: Fact::value(std::env::consts::ARCH.to_string()),
        host_name: field(sys::host_name()),
        account: field(sys::account()),
        filesquid_version: build.version.clone(),
        filesquid_build: build.build.clone(),
        distribution: build.distribution.clone(),
    }
}

#[cfg(unix)]
mod sys {
    //! `uname`, `gethostname` and the effective user's account name through libc.

    use std::ffi::CStr;

    /// A NUL-terminated `c_char` field as text.
    fn text(field: &[libc::c_char]) -> String {
        // SAFETY: `uname` fills each field with a NUL-terminated string within the array.
        unsafe { CStr::from_ptr(field.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    }

    /// The system name, release and version from `uname`.
    pub fn uname() -> Option<(String, String, String)> {
        // SAFETY: zeroed utsname storage that `uname` fills.
        let mut name: libc::utsname = unsafe { std::mem::zeroed() };
        // SAFETY: a valid pointer to that storage.
        if unsafe { libc::uname(&mut name) } != 0 {
            return None;
        }
        Some((
            text(&name.sysname),
            text(&name.release),
            text(&name.version),
        ))
    }

    /// The host name from `gethostname`.
    pub fn host_name() -> Option<String> {
        let mut buffer = vec![0u8; 256];
        // SAFETY: a buffer of the length passed.
        if unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) } != 0 {
            return None;
        }
        let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
        Some(String::from_utf8_lossy(&buffer[..end]).into_owned())
    }

    /// The account name of the effective user.
    pub fn account() -> Option<String> {
        // SAFETY: geteuid has no preconditions.
        let uid = unsafe { libc::geteuid() };
        super::super::facts::user_name(uid).map(|name| String::from_utf8_lossy(&name).into_owned())
    }
}

#[cfg(not(unix))]
mod sys {
    //! Platforms without these calls here: the facts are unavailable.

    /// Not read here.
    pub fn uname() -> Option<(String, String, String)> {
        None
    }

    /// Not read here.
    pub fn host_name() -> Option<String> {
        None
    }

    /// Not read here.
    pub fn account() -> Option<String> {
        None
    }
}
