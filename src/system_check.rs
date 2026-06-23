use crate::types::{SystemCheck, SystemCheckItem, UiEvent};
use std::env;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub fn queue_system_check(tx: mpsc::Sender<UiEvent>) {
    thread::spawn(move || {
        let _ = tx.send(UiEvent::SystemCheckFinished(run_system_check()));
    });
}

fn run_system_check() -> SystemCheck {
    let ydotool = command_available("ydotool", "--help");
    let tesseract = command_available("tesseract", "--version");
    let socket_status = ydotool_socket_status();
    let uinput_status = uinput_status();

    let can_type = ydotool && socket_status.is_ready();
    let can_ocr = tesseract;

    SystemCheck {
        items: vec![
            SystemCheckItem {
                title: "ydotool installed".to_string(),
                ok: ydotool,
                detail: if ydotool {
                    "ydotool is installed.".to_string()
                } else {
                    "Install ydotool: sudo apt install ydotool".to_string()
                },
            },
            SystemCheckItem {
                title: "ydotoold socket".to_string(),
                ok: socket_status.is_ready(),
                detail: socket_status.message,
            },
            SystemCheckItem {
                title: "/dev/uinput access".to_string(),
                ok: uinput_status.ready,
                detail: uinput_status.message,
            },
            SystemCheckItem {
                title: "tesseract OCR installed".to_string(),
                ok: tesseract,
                detail: if tesseract {
                    "tesseract OCR is installed.".to_string()
                } else {
                    "Install tesseract OCR: sudo apt install tesseract-ocr".to_string()
                },
            },
        ],
        can_type,
        can_ocr,
    }
}

#[derive(Debug)]
struct UinputStatus {
    ready: bool,
    message: String,
}

#[derive(Debug)]
struct SocketStatus {
    ready: bool,
    message: String,
}

impl SocketStatus {
    fn is_ready(&self) -> bool {
        self.ready
    }
}

fn ydotool_socket_status() -> SocketStatus {
    match ydotool_socket_path() {
        Some(socket) if socket.exists() => SocketStatus {
            ready: true,
            message: format!("OK ydotoold socket found at {}.", socket.display()),
        },
        Some(socket) => SocketStatus {
            ready: false,
            message: format!(
                "ydotoold is not running or its socket is missing at {}. Start/Check will not type until ydotoold is running. Try: systemctl --user start ydotool.service",
                socket.display()
            ),
        },
        None => SocketStatus {
            ready: false,
            message:
                "Could not determine the ydotoold socket path because XDG_RUNTIME_DIR is not set."
                    .to_string(),
        },
    }
}

fn uinput_status() -> UinputStatus {
    let path = PathBuf::from("/dev/uinput");
    if !path.exists() {
        return UinputStatus {
            ready: false,
            message: "/dev/uinput is missing. ydotoold cannot create a virtual keyboard without the uinput kernel device.".to_string(),
        };
    }

    if OpenOptions::new().read(true).write(true).open(&path).is_ok() {
        return UinputStatus {
            ready: true,
            message: "OK current user can open /dev/uinput if ydotoold needs to start.".to_string(),
        };
    }

    if user_groups().is_some_and(|groups| groups.iter().any(|group| group == "input")) {
        UinputStatus {
            ready: false,
            message: "/dev/uinput exists, but this process cannot open it. If ydotoold is already running, typing may still work; otherwise log out and back in if the input group was just added.".to_string(),
        }
    } else {
        UinputStatus {
            ready: false,
            message: "/dev/uinput exists, but this user cannot open it. If ydotoold is not already running, common fix: sudo usermod -aG input $USER, then log out and back in.".to_string(),
        }
    }
}

fn user_groups() -> Option<Vec<String>> {
    let output = Command::new("id").arg("-nG").output().ok()?;
    if !output.status.success() {
        return None;
    }

    Some(
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .map(ToString::to_string)
            .collect(),
    )
}

fn command_available(binary: &str, version_arg: &str) -> bool {
    Command::new(binary)
        .arg(version_arg)
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn require_command(binary: &str, install_message: &str) -> Result<(), String> {
    Command::new(binary)
        .arg("--version")
        .output()
        .map(|_| ())
        .map_err(|_| install_message.to_string())
}

pub fn ensure_ydotool_ready() -> Result<(), String> {
    if Command::new("ydotool").arg("--help").output().is_err() {
        return Err("ydotool is required: sudo apt install ydotool".to_string());
    }

    if ydotool_socket_path().is_some_and(|socket| socket.exists()) {
        return Ok(());
    }

    let _ = Command::new("systemctl")
        .args(["--user", "reset-failed", "ydotool.service"])
        .status();

    let start_output = Command::new("systemctl")
        .args(["--user", "start", "ydotool.service"])
        .output()
        .map_err(|err| format!("failed to start ydotool.service: {err}"))?;

    if !start_output.status.success() {
        return Err(format!(
            "failed to start ydotool.service: {}",
            command_stderr(&start_output)
        ));
    }

    for _ in 0..10 {
        if ydotool_socket_path().is_some_and(|socket| socket.exists()) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err("ydotool.service started but no socket appeared. If ydotoold cannot open /dev/uinput, run: sudo usermod -aG input $USER, then log out and back in.".to_string())
}

fn ydotool_socket_path() -> Option<PathBuf> {
    if let Ok(socket) = env::var("YDOTOOL_SOCKET") {
        return Some(PathBuf::from(socket));
    }
    env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(|runtime_dir| PathBuf::from(runtime_dir).join(".ydotool_socket"))
}

pub fn command_stderr(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        output.status.to_string()
    } else {
        stderr.replace('\n', " ")
    }
}
