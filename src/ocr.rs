use crate::system_check::command_stderr;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run_ocr_file(image_path: PathBuf) -> Result<String, String> {
    let ocr_output = Command::new("tesseract")
        .args(["--psm", "6", "-c", "preserve_interword_spaces=1"])
        .arg(&image_path)
        .arg("stdout")
        .output()
        .map_err(|err| format!("failed to run tesseract: {err}"));

    let _ = fs::remove_file(&image_path);

    let ocr_output = ocr_output?;
    if !ocr_output.status.success() {
        return Err(format!(
            "tesseract failed: {}",
            command_stderr(&ocr_output)
        ));
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
