//! FileSquid desktop entry point. Started with
//! [`WORKER_ARG`](filesquid_native::workers::media::WORKER_ARG), the executable runs one
//! out-of-process media worker and exits before the app starts (Task DE-0.5).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use filesquid_native::workers::media;

fn main() {
    if media::is_worker_invocation(std::env::args()) {
        std::process::exit(media::worker_main());
    }
    filesquid_desktop_lib::run();
}
