//! The worker side of the media workers (Task DE-0.5; ADR 006): what one worker process
//! ([`super::worker_main`]) runs for one request.
//!
//! The worker loads libvips at run time (no build needs libvips headers, and a machine
//! without libvips answers `libvips_unavailable` rather than failing to start), initialises
//! it, and blocks every ImageMagick loader and saver before it touches a file: ImageMagick is
//! never a fallback, even where libvips was built with its Magick module (ADR 006). A libvips
//! too old to block operations is refused. It then answers one request:
//!
//! - `{"op":"probe"}` → `{"ok":true,"libvips":"<version>","magick_blocked":true}`;
//! - `{"op":"thumbnail","source":…,"dest":…,"max_edge":N}` → the still at `source` scaled to
//!   fit `N` pixels, written to `dest` in the format its extension names, answered as
//!   `{"ok":true,"width":…,"height":…}`;
//! - anything else → `{"ok":false,"error_code":"<code>"}`. Answers never carry a path.
//!
//! The foreign calls are confined to one private type, `Vips`; each one names the C signature it
//! relies on.

use super::MAX_EDGE;
use libloading::{Library, Symbol};
use serde_json::{json, Value};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

/// Largest request the worker reads, in bytes.
pub const MAX_REQUEST_BYTES: u64 = 64 * 1024;

/// The libvips 8 shared library under the names each platform gives it.
const LIBRARY_NAMES: [&str; 4] = [
    "libvips.so.42",
    "libvips.42.dylib",
    "/opt/homebrew/lib/libvips.42.dylib",
    "libvips-42.dll",
];

/// The ImageMagick operations libvips may carry, blocked with all their subclasses.
const MAGICK_OPERATIONS: [&str; 2] = ["VipsForeignLoadMagick", "VipsForeignSaveMagick"];

/// The output formats a thumbnail may be written in, by file extension.
const OUTPUT_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

/// Whether `dest` names an output format a worker writes. Pure.
pub fn writable(dest: &Path) -> bool {
    let extension = dest
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    OUTPUT_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
}

/// The JSON answer line to the JSON `request`. Loads libvips and reads and writes files.
pub fn serve(request: &str) -> String {
    let answer = match serde_json::from_str::<Value>(request) {
        Ok(request) => answer(&request),
        Err(_) => refusal("invalid_request"),
    };
    answer.to_string()
}

/// The answer to one parsed request.
fn answer(request: &Value) -> Value {
    match request["op"].as_str() {
        Some("probe") => with_vips(|vips| {
            Ok(json!({ "ok": true, "libvips": vips.version(), "magick_blocked": true }))
        }),
        Some("thumbnail") => thumbnail(request),
        _ => refusal("invalid_request"),
    }
}

/// The answer to a thumbnail request: its fields checked, then libvips run.
fn thumbnail(request: &Value) -> Value {
    let source = request["source"].as_str().unwrap_or_default();
    let dest = request["dest"].as_str().unwrap_or_default();
    let edge = request["max_edge"].as_u64().unwrap_or(0);
    let valid = !source.is_empty() && (1..=u64::from(MAX_EDGE)).contains(&edge);
    if !valid || !writable(Path::new(dest)) {
        return refusal("invalid_request");
    }
    if !Path::new(source).is_file() {
        return refusal("source_unreadable");
    }
    let edge = c_int::try_from(edge).unwrap_or(1);
    with_vips(|vips| vips.thumbnail(source, dest, edge))
}

/// Run `work` with libvips loaded and ImageMagick blocked, answering its refusal code on
/// failure.
fn with_vips(work: impl FnOnce(&Vips) -> Result<Value, &'static str>) -> Value {
    match Vips::load().and_then(|vips| work(&vips)) {
        Ok(answer) => answer,
        Err(code) => refusal(code),
    }
}

/// A refusal answer. Pure.
fn refusal(code: &str) -> Value {
    json!({ "ok": false, "error_code": code })
}

/// libvips, loaded, initialised and with ImageMagick blocked.
struct Vips {
    lib: Library,
}

impl Vips {
    /// Load the first libvips the platform has, initialise it and block ImageMagick.
    ///
    /// # Errors
    /// `libvips_unavailable` when none loads, it cannot be initialised, or it cannot block
    /// operations (libvips older than 8.13).
    fn load() -> Result<Self, &'static str> {
        let lib = LIBRARY_NAMES
            .iter()
            // SAFETY: loading libvips runs its library constructors, which only register
            // GLib types; nothing else in this process uses GLib.
            .find_map(|name| unsafe { Library::new(name) }.ok())
            .ok_or("libvips_unavailable")?;
        let vips = Self { lib };
        vips.init()?;
        vips.block_magick()?;
        Ok(vips)
    }

    /// The symbol `name` (NUL-terminated) as type `T`.
    ///
    /// # Errors
    /// `libvips_unavailable` when the library has no such symbol.
    fn symbol<T>(&self, name: &[u8]) -> Result<Symbol<'_, T>, &'static str> {
        // SAFETY: every caller names the C signature `T` the libvips 8 API documents for
        // `name`.
        unsafe { self.lib.get(name) }.map_err(|_| "libvips_unavailable")
    }

    /// `int vips_init(const char *argv0)`, once per process.
    fn init(&self) -> Result<(), &'static str> {
        let init = self.symbol::<unsafe extern "C" fn(*const c_char) -> c_int>(b"vips_init\0")?;
        // SAFETY: a NUL-terminated name; the first libvips call of this process.
        match unsafe { init(c"filesquid-media".as_ptr()) } {
            0 => Ok(()),
            _ => Err("libvips_unavailable"),
        }
    }

    /// `void vips_operation_block_set(const char *name, gboolean state)` for every
    /// ImageMagick operation.
    fn block_magick(&self) -> Result<(), &'static str> {
        let block = self
            .symbol::<unsafe extern "C" fn(*const c_char, c_int)>(b"vips_operation_block_set\0")?;
        for name in MAGICK_OPERATIONS {
            let name = CString::new(name).map_err(|_| "libvips_unavailable")?;
            // SAFETY: a NUL-terminated class name; blocking an unknown class is a no-op.
            unsafe { block(name.as_ptr(), 1) };
        }
        Ok(())
    }

    /// `const char *vips_version_string(void)`.
    fn version(&self) -> String {
        let Ok(version) =
            self.symbol::<unsafe extern "C" fn() -> *const c_char>(b"vips_version_string\0")
        else {
            return "unknown".into();
        };
        // SAFETY: libvips answers a static NUL-terminated string.
        let text = unsafe { version() };
        match text.is_null() {
            true => "unknown".into(),
            // SAFETY: checked non-null above; the string is static.
            false => unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned(),
        }
    }

    /// `int vips_thumbnail(const char *filename, VipsImage **out, int width, ..., NULL)` with
    /// `"height"`, then `int vips_image_write_to_file(VipsImage *, const char *, ..., NULL)`,
    /// the size getters and `g_object_unref`.
    ///
    /// # Errors
    /// `invalid_request` for a path with a NUL byte; `unsupported_format` when libvips (with
    /// ImageMagick blocked) cannot decode the source; `write_failed`.
    fn thumbnail(&self, source: &str, dest: &str, edge: c_int) -> Result<Value, &'static str> {
        type Thumbnail = unsafe extern "C" fn(*const c_char, *mut *mut c_void, c_int, ...) -> c_int;
        type Write = unsafe extern "C" fn(*mut c_void, *const c_char, ...) -> c_int;
        type Size = unsafe extern "C" fn(*const c_void) -> c_int;
        let thumbnail = self.symbol::<Thumbnail>(b"vips_thumbnail\0")?;
        let write = self.symbol::<Write>(b"vips_image_write_to_file\0")?;
        let width = self.symbol::<Size>(b"vips_image_get_width\0")?;
        let height = self.symbol::<Size>(b"vips_image_get_height\0")?;
        let unref = self.symbol::<unsafe extern "C" fn(*mut c_void)>(b"g_object_unref\0")?;
        let source = CString::new(source).map_err(|_| "invalid_request")?;
        let dest = CString::new(dest).map_err(|_| "invalid_request")?;
        let mut image: *mut c_void = std::ptr::null_mut();
        let none = std::ptr::null::<c_char>();
        // SAFETY: NUL-terminated strings, a valid out pointer, and the option list ends with
        // NULL as the variadic API requires.
        let made = unsafe {
            thumbnail(
                source.as_ptr(),
                &mut image,
                edge,
                c"height".as_ptr(),
                edge,
                none,
            )
        };
        if made != 0 || image.is_null() {
            return Err("unsupported_format");
        }
        // SAFETY: `image` is the live image libvips just made; it is released once, below.
        let (w, h) = unsafe { (width(image), height(image)) };
        // SAFETY: as above; the option list ends with NULL.
        let written = unsafe { write(image, dest.as_ptr(), none) };
        // SAFETY: releases the one reference `vips_thumbnail` gave this call.
        unsafe { unref(image) };
        match written {
            0 => Ok(json!({ "ok": true, "width": w, "height": h })),
            _ => Err("write_failed"),
        }
    }
}
