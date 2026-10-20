//! The desktop's folder choice for the library routes (Task DE-1.1; API-S-025, DE-S-017;
//! owner decision 2026-10-20): the host's [`LocationChooser`] behind
//! `POST /api/v1/libraries/location-choices`.
//!
//! The owner picks a folder in a native dialog that the host shows from Rust ([`RfdDialog`],
//! the `rfd` crate, not a Tauri plugin, so the webview keeps no dialog permission, DE-S-002).
//! The host registers the picked folder in its [`HostLocations`] and answers only the opaque
//! location ID; the path never leaves the host (DE-S-006). One dialog is open at a time.

use filesquid_api::local_library::choice::{ChoicePurpose, ChooseError, LocationChooser};
use filesquid_native::HostLocations;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A native folder dialog.
pub trait FolderDialog: Send + Sync {
    /// Show the dialog for `purpose` and answer the folder the owner picked, or `None` when
    /// they cancelled. Blocks until the dialog closes.
    ///
    /// # Errors
    /// [`ChooseError::Unavailable`] when no dialog can be shown.
    fn pick(&self, purpose: ChoicePurpose) -> Result<Option<PathBuf>, ChooseError>;
}

/// The platform's native folder dialog through `rfd` (GTK 3 on Linux, `NSOpenPanel` on
/// macOS, the common item dialog on Windows), awaited on the calling (blocking) thread.
#[derive(Clone, Copy, Debug, Default)]
pub struct RfdDialog;

impl FolderDialog for RfdDialog {
    fn pick(&self, purpose: ChoicePurpose) -> Result<Option<PathBuf>, ChooseError> {
        let dialog = rfd::AsyncFileDialog::new().set_title(dialog_title(purpose));
        let picked = tauri::async_runtime::block_on(dialog.pick_folder());
        Ok(picked.map(|handle| handle.path().to_path_buf()))
    }
}

/// The dialog title for `purpose`. Pure.
pub fn dialog_title(purpose: ChoicePurpose) -> &'static str {
    match purpose {
        ChoicePurpose::Create => "Choose an empty folder for the new library",
        ChoicePurpose::Open => "Choose the folder that holds your library",
        ChoicePurpose::CopyDestination => "Choose an empty folder for the copy",
    }
}

/// The desktop's folder choice: a dialog, then registration in the host's folder registry.
pub struct DesktopChooser {
    locations: Arc<HostLocations>,
    dialog: Box<dyn FolderDialog>,
    open: AtomicBool,
}

impl std::fmt::Debug for DesktopChooser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopChooser").finish_non_exhaustive()
    }
}

impl DesktopChooser {
    /// A chooser that shows `dialog` and registers the picked folder in `locations`. Pure.
    pub fn new(locations: Arc<HostLocations>, dialog: Box<dyn FolderDialog>) -> Self {
        Self {
            locations,
            dialog,
            open: AtomicBool::new(false),
        }
    }

    /// Register `folder`, answering its location ID; a folder that cannot be registered is
    /// logged with its code only (never the path) and refused. Creates directories in the
    /// data directory.
    fn register(&self, folder: &std::path::Path) -> Result<String, ChooseError> {
        self.locations.register(folder).map_err(|_| {
            tracing::warn!(
                operation = filesquid_api::local_library::choice::CHOOSE_OPERATION,
                error_code = "location_refused",
                "the picked folder could not be registered"
            );
            ChooseError::Refused
        })
    }
}

impl LocationChooser for DesktopChooser {
    fn choose(&self, purpose: ChoicePurpose) -> Result<Option<String>, ChooseError> {
        if self.open.swap(true, Ordering::SeqCst) {
            return Err(ChooseError::Busy);
        }
        let picked = self.dialog.pick(purpose);
        self.open.store(false, Ordering::SeqCst);
        match picked? {
            Some(folder) => self.register(&folder).map(Some),
            None => Ok(None),
        }
    }
}
