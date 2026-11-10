// With harness=false Rust omits #[test] functions but retains their private
// helpers/imports from the shared sources. The normal lib harness checks those.
#![allow(dead_code, unused_imports)]

// Compile the application owner into this custom harness so private lifecycle
// boundaries can use controlled workers, without a test-only production API.
include!("../src/lib.rs");

// GTK must run on the OS main thread on macOS, not Rust's test worker threads.
fn main() {
    if std::env::var_os("JUMPBOX_LIFECYCLE_SCENARIO").is_some() {
        app::tests::run();
    } else {
        // AppKit/GTK own one application delegate per process. Each scenario
        // gets a fresh process (also checking that editor text starts empty).
        for scenario in [
            "close-then-dialog-quit",
            "windowless-quit",
            "quit-worker",
            "stop-worker",
        ] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env("JUMPBOX_LIFECYCLE_SCENARIO", scenario)
                .status()
                .unwrap();
            assert!(status.success(), "lifecycle scenario failed: {scenario}");
        }
    }
}
