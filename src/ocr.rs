use crate::command::resolve_command;
use crate::system_check::command_stderr;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
fn tesseract_fallback_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/bin/tesseract"),
        PathBuf::from("/usr/local/bin/tesseract"),
    ]
}

#[cfg(not(target_os = "macos"))]
fn tesseract_fallback_paths() -> Vec<PathBuf> {
    Vec::new()
}

pub fn resolve_tesseract() -> Result<PathBuf, String> {
    let search_path = env::var_os("PATH");
    resolve_tesseract_with(search_path.as_deref(), &tesseract_fallback_paths())
}

pub fn tesseract_available() -> bool {
    resolve_tesseract().is_ok_and(|path| tesseract_executable_is_ready(&path))
}

fn tesseract_executable_is_ready(path: &Path) -> bool {
    Command::new(path)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn resolve_tesseract_with(
    search_path: Option<&OsStr>,
    fallback_paths: &[PathBuf],
) -> Result<PathBuf, String> {
    resolve_command("tesseract", search_path, fallback_paths)
        .ok_or_else(|| tesseract_install_message().to_string())
}

#[cfg(target_os = "macos")]
const fn tesseract_install_message() -> &'static str {
    "tesseract OCR is required: brew install tesseract"
}

#[cfg(target_os = "linux")]
const fn tesseract_install_message() -> &'static str {
    "tesseract OCR is required: sudo apt install tesseract-ocr"
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const fn tesseract_install_message() -> &'static str {
    "tesseract OCR is required and was not found in PATH"
}

pub fn run_ocr_file(tesseract_path: PathBuf, image_path: PathBuf) -> Result<String, String> {
    let result = run_ocr_source_file(tesseract_path, image_path.clone());
    let _ = fs::remove_file(&image_path);
    result
}

pub(crate) fn run_ocr_source_file(
    tesseract_path: PathBuf,
    image_path: PathBuf,
) -> Result<String, String> {
    let ocr_output = Command::new(tesseract_path)
        .args(["--psm", "6", "-c", "preserve_interword_spaces=1"])
        .arg(&image_path)
        .arg("stdout")
        .output()
        .map_err(|err| format!("failed to run tesseract: {err}"));

    let ocr_output = ocr_output?;
    if !ocr_output.status.success() {
        return Err(format!("tesseract failed: {}", command_stderr(&ocr_output)));
    }

    Ok(String::from_utf8_lossy(&ocr_output.stdout).to_string())
}

pub fn temporary_ocr_image_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    env::temp_dir().join(format!(
        "jumpbox-typer-ocr-{}-{timestamp}.png",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must follow Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "jumpbox-typer-ocr-{label}-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("test directory must be created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn make_fake_tesseract(path: &Path, body: &str) {
        fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("fake tesseract must be written");
        let mut permissions = fs::metadata(path)
            .expect("fake tesseract metadata must exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fake tesseract must be executable");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn finder_discovery_checks_both_homebrew_tesseract_locations() {
        assert_eq!(
            tesseract_fallback_paths(),
            vec![
                PathBuf::from("/opt/homebrew/bin/tesseract"),
                PathBuf::from("/usr/local/bin/tesseract"),
            ]
        );
    }

    #[test]
    fn missing_tesseract_reports_platform_install_guidance() {
        let error = resolve_tesseract_with(Some(OsStr::new("")), &[]).unwrap_err();

        #[cfg(target_os = "macos")]
        {
            assert!(error.contains("brew install tesseract"));
            assert!(!error.contains("apt install"));
        }

        #[cfg(target_os = "linux")]
        assert!(error.contains("sudo apt install tesseract-ocr"));
    }

    #[test]
    fn resolved_tesseract_output_is_returned_and_clipboard_image_is_removed() {
        let directory = TestDirectory::new("success");
        let executable = directory.path().join("tesseract");
        let image = directory.path().join("clipboard.png");
        make_fake_tesseract(&executable, "printf 'recognized text\\n'");
        fs::write(&image, "fake image").expect("clipboard image must be written");

        let text = run_ocr_file(executable, image.clone()).expect("OCR must succeed");

        assert_eq!(text, "recognized text\n");
        assert!(!image.exists());
    }

    #[test]
    fn source_image_is_preserved_after_success() {
        let directory = TestDirectory::new("source-success");
        let executable = directory.path().join("tesseract");
        let image = directory.path().join("source.png");
        make_fake_tesseract(&executable, "printf 'recognized text\\n'");
        fs::write(&image, "fake image").expect("source image must be written");

        let text = run_ocr_source_file(executable, image.clone()).expect("OCR must succeed");

        assert_eq!(text, "recognized text\n");
        assert!(image.exists());
    }

    #[test]
    fn source_image_is_preserved_after_tesseract_failure() {
        let directory = TestDirectory::new("source-failure");
        let executable = directory.path().join("tesseract");
        let image = directory.path().join("source.png");
        make_fake_tesseract(&executable, "printf 'engine broke\\n' >&2; exit 7");
        fs::write(&image, "fake image").expect("source image must be written");

        let error = run_ocr_source_file(executable, image.clone())
            .expect_err("Tesseract failure must be reported");

        assert!(error.contains("engine broke"));
        assert!(image.exists());
    }

    #[test]
    fn tesseract_process_failure_is_reported_and_clipboard_image_is_removed() {
        let directory = TestDirectory::new("process-failure");
        let executable = directory.path().join("tesseract");
        let image = directory.path().join("clipboard.png");
        make_fake_tesseract(&executable, "printf 'engine broke\\n' >&2; exit 7");
        fs::write(&image, "fake image").expect("clipboard image must be written");

        let error = run_ocr_file(executable, image.clone()).unwrap_err();

        assert!(error.contains("tesseract failed"));
        assert!(error.contains("engine broke"));
        assert!(!image.exists());
    }

    #[test]
    fn tesseract_launch_failure_is_reported_and_clipboard_image_is_removed() {
        let directory = TestDirectory::new("launch-failure");
        let executable = directory.path().join("missing-tesseract");
        let image = directory.path().join("clipboard.png");
        fs::write(&image, "fake image").expect("clipboard image must be written");

        let error = run_ocr_file(executable, image.clone()).unwrap_err();

        assert!(error.contains("failed to run tesseract"));
        assert!(!image.exists());
    }

    #[test]
    fn empty_tesseract_output_remains_empty_and_clipboard_image_is_removed() {
        let directory = TestDirectory::new("empty-output");
        let executable = directory.path().join("tesseract");
        let image = directory.path().join("clipboard.png");
        make_fake_tesseract(&executable, "exit 0");
        fs::write(&image, "fake image").expect("clipboard image must be written");

        let text = run_ocr_file(executable, image.clone()).expect("OCR must succeed");

        assert!(text.is_empty());
        assert!(!image.exists());
    }

    #[test]
    fn readiness_requires_resolved_tesseract_to_run_successfully() {
        let directory = TestDirectory::new("readiness");
        let ready = directory.path().join("ready-tesseract");
        let broken = directory.path().join("broken-tesseract");
        make_fake_tesseract(&ready, "exit 0");
        make_fake_tesseract(&broken, "exit 9");

        assert!(tesseract_executable_is_ready(&ready));
        assert!(!tesseract_executable_is_ready(&broken));
    }
}
