//! Integration tests for `scripts/linked-inventory.sh` (Task DE-F3.1; ADR 011, ADR 035
//! Decision 5, ADR 006): the script that lists what a desktop build links, for the licence and
//! patent review. They run the script against this test binary and against stand-in library
//! files in a temporary directory, on the platform the tests run on. Unix only: the script is
//! a shell script that reads ELF and Mach-O files.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

/// Distinguishes the scratch directories of one test process.
static SCRATCH: AtomicU32 = AtomicU32::new(0);

/// The inventory script in the crate.
fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/linked-inventory.sh")
}

/// Run the script with `arguments` and wait for it. Starts a process.
fn run_inventory(arguments: &[&str]) -> Output {
    Command::new("bash")
        .arg(script())
        .args(arguments)
        .output()
        .expect("the inventory script starts")
}

/// The path of this test binary, a real executable of the platform.
fn this_binary() -> String {
    std::env::current_exe()
        .expect("test binary path")
        .to_string_lossy()
        .into_owned()
}

/// The standard output of a run, as text.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A scratch directory removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// Make an empty scratch directory named for `label`. Performs filesystem I/O.
    fn new(label: &str) -> Self {
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let name = format!("de-f3.1-inventory-{label}-{}-{n}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).expect("scratch directory");
        Self(dir)
    }

    /// Write a stand-in file (not a real library) at `relative` and return its path. Performs
    /// filesystem I/O.
    fn stand_in(&self, relative: &str) -> String {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("stand-in directory");
        std::fs::write(&path, b"stand-in").expect("stand-in file");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir_all(&self.0) {
            eprintln!("scratch directory not removed: {err}");
        }
    }
}

/// The rows of `text` whose first tab-separated field is `kind`.
fn rows<'a>(text: &'a str, kind: &str) -> Vec<&'a str> {
    text.lines()
        .filter(|line| line.split('\t').next() == Some(kind))
        .collect()
}

#[test]
fn it_lists_the_dynamic_libraries_of_a_real_binary() {
    let output = run_inventory(&["--without-vips", "--no-crates", &this_binary()]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    assert_eq!(rows(&text, "file").len(), 1, "{text}");
    let libraries = rows(&text, "lib");
    let libc = if cfg!(target_os = "macos") {
        "libSystem"
    } else {
        "libc.so"
    };
    assert!(libraries.iter().any(|row| row.contains(libc)), "{text}");
    assert!(
        libraries.iter().all(|row| row.starts_with("lib\tlink\t")),
        "{text}"
    );
    assert_eq!(rows(&text, "finding"), vec!["finding\tnone\t-"], "{text}");
}

#[test]
fn it_refuses_a_subject_that_does_not_exist() {
    let output = run_inventory(&["--without-vips", "/nonexistent/FileSquid.app"]);
    assert_eq!(output.status.code(), Some(1));
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("subject not found"), "{message}");
}

#[test]
fn it_refuses_a_folder_with_no_executable_in_it() {
    let scratch = Scratch::new("empty");
    scratch.stand_in("Contents/Resources/readme.txt");
    let folder = scratch.0.to_string_lossy().into_owned();
    let output = run_inventory(&["--without-vips", "--no-crates", &folder]);
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn it_lists_the_run_time_loaded_libvips_and_its_loader_modules() {
    let scratch = Scratch::new("vips");
    let library = scratch.stand_in("lib/libvips.so.42");
    scratch.stand_in("lib/vips-modules-8.15/vips-heif.so");
    scratch.stand_in("lib/vips-modules-8.15/vips-jxl.so");
    let output = run_inventory(&["--no-crates", "--runtime-lib", &library, &this_binary()]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    let modules = rows(&text, "module");
    assert_eq!(modules.len(), 2, "{text}");
    assert!(modules[0].ends_with("vips-heif.so"), "{text}");
    let runtime: Vec<_> = rows(&text, "lib")
        .into_iter()
        .filter(|row| row.starts_with("lib\truntime\t"))
        .collect();
    assert_eq!(runtime.len(), 3, "{text}");
    assert!(
        rows(&text, "vips")[0].starts_with("vips\tlibrary\t"),
        "{text}"
    );
}

#[test]
fn it_fails_when_imagemagick_is_in_the_inventory() {
    let scratch = Scratch::new("magick");
    let library = scratch.stand_in("lib/libvips.so.42");
    scratch.stand_in("lib/vips-modules-8.15/vips-magick.so");
    let output = run_inventory(&["--no-crates", "--runtime-lib", &library, &this_binary()]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(2), "{text}");
    let findings = rows(&text, "finding");
    assert_eq!(findings.len(), 1, "{text}");
    assert!(findings[0].starts_with("finding\timagemagick\t"), "{text}");
    assert!(findings[0].ends_with("vips-magick.so"), "{text}");
}

#[test]
fn it_ends_on_a_symbolic_link_loop() {
    let scratch = Scratch::new("loop");
    let first = scratch.0.join("libvips.so.42");
    let second = scratch.0.join("libvips.so.42.17");
    std::os::unix::fs::symlink(&second, &first).expect("first link");
    std::os::unix::fs::symlink(&first, &second).expect("second link");
    let library = first.to_string_lossy().into_owned();
    let output = run_inventory(&["--no-crates", "--runtime-lib", &library, &this_binary()]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    let runtime: Vec<_> = rows(&text, "lib")
        .into_iter()
        .filter(|row| row.starts_with("lib\truntime\t"))
        .collect();
    assert_eq!(runtime.len(), 1, "{text}");
}

#[test]
fn it_lists_the_rust_crates_with_their_licences() {
    let output = run_inventory(&["--without-vips", &this_binary()]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    let crates = rows(&text, "crate");
    let named = |name: &str| {
        crates
            .iter()
            .find(|row| row.split('\t').nth(1) == Some(name))
            .unwrap_or_else(|| panic!("no crate row for {name}"))
            .split('\t')
            .collect::<Vec<_>>()
    };
    assert_eq!(named("filesquid-desktop")[3], "MIT");
    assert!(named("tauri")[3].contains("MIT"), "{:?}", named("tauri"));
    assert!(named("filesquid-native").len() == 4);
    assert!(
        crates.iter().all(|row| row.split('\t').count() == 4),
        "{text}"
    );
}

/// Whether a libvips 8 is installed where the script looks for it. Reads the filesystem or
/// runs `ldconfig`.
fn libvips_installed() -> bool {
    if cfg!(target_os = "macos") {
        return ["/opt/homebrew/lib", "/usr/local/lib"]
            .iter()
            .any(|dir| Path::new(dir).join("libvips.42.dylib").exists());
    }
    Command::new("ldconfig")
        .arg("-p")
        .output()
        .map(|output| stdout(&output).contains("libvips.so.42 "))
        .unwrap_or(false)
}

#[test]
fn it_says_when_libvips_is_missing_and_lists_it_when_it_is_installed() {
    let output = run_inventory(&["--no-crates", &this_binary()]);
    let text = stdout(&output);
    if libvips_installed() {
        assert!(rows(&text, "vips")[0].contains("libvips"), "{text}");
        assert!(rows(&text, "finding")
            .iter()
            .all(|row| !row.contains("libvips-missing")));
        return;
    }
    assert_eq!(output.status.code(), Some(3), "{text}");
    assert!(
        rows(&text, "finding")[0].starts_with("finding\tlibvips-missing\t"),
        "{text}"
    );
}
