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

    /// Copy this test binary, a real object file of the platform, to `relative` as a stand-in
    /// library and return its path. Performs filesystem I/O.
    fn object(&self, relative: &str) -> String {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("stand-in directory");
        std::fs::copy(this_binary(), &path).expect("copy a real object file");
        path.to_string_lossy().into_owned()
    }

    /// Write a stand-in file that is not an object file at `relative` and return its path.
    /// Performs filesystem I/O.
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
    let library = scratch.object("lib/libvips.so.42");
    scratch.object("lib/vips-modules-8.15/vips-heif.so");
    scratch.object("lib/vips-modules-8.15/vips-jxl.so");
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
    let library = scratch.object("lib/libvips.so.42");
    scratch.object("lib/vips-modules-8.15/vips-magick.so");
    let output = run_inventory(&["--no-crates", "--runtime-lib", &library, &this_binary()]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(2), "{text}");
    let findings = rows(&text, "finding");
    assert_eq!(findings.len(), 1, "{text}");
    assert!(findings[0].starts_with("finding\timagemagick\t"), "{text}");
    assert!(findings[0].ends_with("vips-magick.so"), "{text}");
}

#[test]
fn it_reports_a_run_time_library_reached_through_a_symbolic_link_loop_as_missing() {
    let scratch = Scratch::new("loop");
    let first = scratch.0.join("libvips.so.42");
    let second = scratch.0.join("libvips.so.42.17");
    std::os::unix::fs::symlink(&second, &first).expect("first link");
    std::os::unix::fs::symlink(&first, &second).expect("second link");
    let library = first.to_string_lossy().into_owned();
    let output = run_inventory(&["--no-crates", "--runtime-lib", &library, &this_binary()]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    let findings = rows(&text, "finding");
    assert_eq!(findings.len(), 1, "{text}");
    assert!(
        findings[0].starts_with("finding\truntime-missing\t"),
        "{text}"
    );
    assert!(rows(&text, "vips").is_empty(), "{text}");
}

#[test]
fn it_reports_a_run_time_library_that_does_not_exist() {
    let output = run_inventory(&[
        "--no-crates",
        "--runtime-lib",
        "/nonexistent/libvips.so.42",
        &this_binary(),
    ]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    let findings = rows(&text, "finding");
    assert_eq!(
        findings,
        vec!["finding\truntime-missing\t/nonexistent/libvips.so.42"],
        "{text}"
    );
}

#[test]
fn it_reports_a_library_it_cannot_inspect() {
    let scratch = Scratch::new("not-an-object");
    let library = scratch.stand_in("lib/libvips.so.42");
    let output = run_inventory(&["--no-crates", "--runtime-lib", &library, &this_binary()]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    let findings = rows(&text, "finding");
    assert_eq!(findings.len(), 1, "{text}");
    assert!(findings[0].starts_with("finding\tuninspected\t"), "{text}");
    assert!(findings[0].ends_with("libvips.so.42"), "{text}");
}

#[test]
fn it_finds_the_app_executable_inside_a_mounted_volume() {
    let scratch = Scratch::new("volume");
    scratch.object("FileSquid.app/Contents/Frameworks/aaa-first.so");
    scratch.object("FileSquid.app/Contents/MacOS/filesquid-desktop");
    let volume = scratch.0.to_string_lossy().into_owned();
    let output = run_inventory(&["--without-vips", "--no-crates", &volume]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    let main = text
        .lines()
        .find(|line| line.starts_with("# main executable: "))
        .unwrap_or_else(|| panic!("no main executable line: {text}"));
    assert_eq!(
        main,
        "# main executable: FileSquid.app/Contents/MacOS/filesquid-desktop"
    );
}

#[test]
fn it_fails_when_imagemagick_is_bundled_inside_the_subject() {
    let scratch = Scratch::new("bundled-magick");
    let executable = scratch.0.join("Contents/MacOS/filesquid-desktop");
    let bundled = scratch.0.join("Contents/Frameworks/libMagickCore-7.so");
    for target in [&executable, &bundled] {
        std::fs::create_dir_all(target.parent().expect("parent")).expect("bundle folder");
        std::fs::copy(this_binary(), target).expect("copy a real object file");
    }
    let folder = scratch.0.to_string_lossy().into_owned();
    let output = run_inventory(&["--without-vips", "--no-crates", &folder]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(2), "{text}");
    let findings = rows(&text, "finding");
    assert_eq!(findings.len(), 1, "{text}");
    assert!(findings[0].starts_with("finding\timagemagick\t"), "{text}");
    assert!(findings[0].ends_with("libMagickCore-7.so"), "{text}");
}

/// Build, with the workspace's `rustc`, the shared library `libfilesquid_absent.so` in
/// `library_dir` and an executable in `dir` that links it, with `library_dir` as its run
/// path. Returns the executable's and the library's paths. Starts processes and performs
/// filesystem I/O.
#[cfg(target_os = "linux")]
fn build_executable_linking_a_library(dir: &Path, library_dir: &Path) -> (String, PathBuf) {
    let library_source = dir.join("absent.rs");
    let program_source = dir.join("program.rs");
    std::fs::write(
        &library_source,
        "#[no_mangle]\npub extern \"C\" fn absent() {}\n",
    )
    .expect("library source");
    std::fs::write(
        &program_source,
        "extern \"C\" { fn absent(); }\nfn main() { unsafe { absent() } }\n",
    )
    .expect("program source");
    std::fs::create_dir_all(library_dir).expect("library folder");
    let library = library_dir.join("libfilesquid_absent.so");
    let rpath = format!("link-arg=-Wl,-rpath,{}", library_dir.display());
    let program = dir.join("program");
    let rustc = |arguments: Vec<&std::ffi::OsStr>| {
        let status = Command::new("rustc")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(arguments)
            .status()
            .expect("rustc starts");
        assert!(status.success(), "rustc failed");
    };
    rustc(vec![
        "--crate-type".as_ref(),
        "cdylib".as_ref(),
        "--crate-name".as_ref(),
        "filesquid_absent".as_ref(),
        "-o".as_ref(),
        library.as_os_str(),
        library_source.as_os_str(),
    ]);
    rustc(vec![
        "-C".as_ref(),
        rpath.as_ref(),
        "-L".as_ref(),
        library_dir.as_os_str(),
        "-l".as_ref(),
        "dylib=filesquid_absent".as_ref(),
        "-o".as_ref(),
        program.as_os_str(),
        program_source.as_os_str(),
    ]);
    (program.to_string_lossy().into_owned(), library)
}

#[cfg(target_os = "linux")]
#[test]
fn it_fails_when_a_linked_library_cannot_be_found() {
    let scratch = Scratch::new("unresolved");
    let (program, library) = build_executable_linking_a_library(&scratch.0, &scratch.0);
    std::fs::remove_file(&library).expect("remove the library");
    let output = run_inventory(&["--without-vips", "--no-crates", &program]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    let findings = rows(&text, "finding");
    assert_eq!(findings.len(), 1, "{text}");
    assert!(
        findings[0].starts_with("finding\tunresolved\tlibfilesquid_absent.so"),
        "{text}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn it_keeps_the_whole_path_of_a_library_in_a_folder_with_spaces() {
    let scratch = Scratch::new("spaces");
    let folder = scratch.0.join("My Libraries");
    let (program, library) = build_executable_linking_a_library(&scratch.0, &folder);
    let output = run_inventory(&["--without-vips", "--no-crates", &program]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    let expected = format!("\t{}", library.display());
    assert!(
        rows(&text, "lib")
            .iter()
            .any(|row| row.ends_with(&expected)),
        "{text}"
    );
}

#[test]
fn it_fails_when_the_rust_crates_cannot_be_listed() {
    let scratch = Scratch::new("no-workspace");
    let copy = scratch.0.join("crates/desktop/scripts/linked-inventory.sh");
    std::fs::create_dir_all(copy.parent().expect("parent")).expect("script folder");
    std::fs::copy(script(), &copy).expect("copy the script");
    let output = Command::new("bash")
        .arg(&copy)
        .args(["--without-vips", &this_binary()])
        .output()
        .expect("the copied script starts");
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    let findings = rows(&text, "finding");
    assert_eq!(findings.len(), 1, "{text}");
    assert!(
        findings[0].starts_with("finding\tcrates-unlisted\t"),
        "{text}"
    );
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
