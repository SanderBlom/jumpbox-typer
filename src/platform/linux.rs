use super::AccessRequest;
use crate::ocr::tesseract_available;
use crate::system_check::command_stderr;
use crate::types::{KeyboardLayout, SystemCheck, SystemCheckItem};
use std::env;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub(super) fn check_system(_access_request: AccessRequest) -> SystemCheck {
    let ydotool = command_available("ydotool", "--help");
    let tesseract = tesseract_available();
    let socket_status = ydotool_socket_status();
    let uinput_status = uinput_status();

    build_system_check(ydotool, tesseract, socket_status, uinput_status)
}

pub(super) const fn recheck_readiness_on_activation() -> bool {
    false
}

fn build_system_check(
    ydotool: bool,
    tesseract: bool,
    socket_status: SocketStatus,
    uinput_status: UinputStatus,
) -> SystemCheck {
    let can_type = can_type(ydotool, &socket_status, &uinput_status);

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
                help: "ydotool is the tool that sends synthetic keystrokes. The app needs it for Start typing and OCR insertion into the target session.".to_string(),
            },
            SystemCheckItem {
                title: "ydotoold socket".to_string(),
                ok: socket_status.is_ready(),
                detail: socket_status.message,
                help: "ydotoold is the background daemon that actually talks to the input system. The socket shows that the daemon is running and reachable for typing.".to_string(),
            },
            SystemCheckItem {
                title: "/dev/uinput access".to_string(),
                ok: uinput_status.ready,
                detail: uinput_status.message,
                help: "uinput is the Linux kernel interface used to create a virtual keyboard. If the app cannot open it, ydotoold may not be able to start or recover.".to_string(),
            },
            SystemCheckItem {
                title: "tesseract OCR installed".to_string(),
                ok: tesseract,
                detail: if tesseract {
                    "tesseract OCR is installed.".to_string()
                } else {
                    "Install tesseract OCR: sudo apt install tesseract-ocr".to_string()
                },
                help: "Tesseract reads text from clipboard images and screenshots. The OCR button depends on it to turn a screenshot into editable text.".to_string(),
            },
        ],
        can_type,
        can_ocr: tesseract,
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
    path_known: bool,
    message: String,
}

impl SocketStatus {
    fn is_ready(&self) -> bool {
        self.ready
    }

    fn can_recover(&self) -> bool {
        self.path_known && !self.ready
    }
}

fn can_type(
    ydotool_installed: bool,
    socket_status: &SocketStatus,
    uinput_status: &UinputStatus,
) -> bool {
    ydotool_installed
        && (socket_status.is_ready() || (socket_status.can_recover() && uinput_status.ready))
}

fn ydotool_socket_status() -> SocketStatus {
    match ydotool_socket_path() {
        Some(socket) if socket.exists() => SocketStatus {
            ready: true,
            path_known: true,
            message: format!("OK ydotoold socket found at {}.", socket.display()),
        },
        Some(socket) => SocketStatus {
            ready: false,
            path_known: true,
            message: format!(
                "ydotoold is not running or its socket is missing at {}. Start typing can still try to launch ydotoold automatically, or you can run: systemctl --user start ydotool.service",
                socket.display()
            ),
        },
        None => SocketStatus {
            ready: false,
            path_known: false,
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

    if OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .is_ok()
    {
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

pub(super) fn prepare_typing() -> Result<(), String> {
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

pub(super) fn type_character(ch: char, layout: KeyboardLayout) -> Result<(), String> {
    let output = match layout {
        KeyboardLayout::Norwegian => send_sequence(norwegian_sequence(ch), ch),
        KeyboardLayout::Us => send_sequence(us_sequence(ch), ch),
    }
    .map_err(|err| format!("failed to run ydotool: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "ydotool exited with {}: {}",
            output.status,
            command_stderr(&output)
        ))
    }
}

fn send_sequence(
    sequence: Option<Vec<String>>,
    fallback: char,
) -> std::io::Result<std::process::Output> {
    match sequence {
        Some(keys) => {
            let refs = keys.iter().map(String::as_str).collect::<Vec<_>>();
            Command::new("ydotool").args(["key"]).args(refs).output()
        }
        None => Command::new("ydotool")
            .args(["type"])
            .arg(fallback.to_string())
            .output(),
    }
}

fn us_sequence(ch: char) -> Option<Vec<String>> {
    match ch {
        '\n' => Some(vec!["28:1".into(), "28:0".into()]),
        '\t' => Some(vec!["15:1".into(), "15:0".into()]),
        ' ' => Some(vec!["57:1".into(), "57:0".into()]),
        'a' => Some(key_sequence(30, false)),
        'b' => Some(key_sequence(48, false)),
        'c' => Some(key_sequence(46, false)),
        'd' => Some(key_sequence(32, false)),
        'e' => Some(key_sequence(18, false)),
        'f' => Some(key_sequence(33, false)),
        'g' => Some(key_sequence(34, false)),
        'h' => Some(key_sequence(35, false)),
        'i' => Some(key_sequence(23, false)),
        'j' => Some(key_sequence(36, false)),
        'k' => Some(key_sequence(37, false)),
        'l' => Some(key_sequence(38, false)),
        'm' => Some(key_sequence(50, false)),
        'n' => Some(key_sequence(49, false)),
        'o' => Some(key_sequence(24, false)),
        'p' => Some(key_sequence(25, false)),
        'q' => Some(key_sequence(16, false)),
        'r' => Some(key_sequence(19, false)),
        's' => Some(key_sequence(31, false)),
        't' => Some(key_sequence(20, false)),
        'u' => Some(key_sequence(22, false)),
        'v' => Some(key_sequence(47, false)),
        'w' => Some(key_sequence(17, false)),
        'x' => Some(key_sequence(45, false)),
        'y' => Some(key_sequence(21, false)),
        'z' => Some(key_sequence(44, false)),
        'A' => Some(key_sequence(30, true)),
        'B' => Some(key_sequence(48, true)),
        'C' => Some(key_sequence(46, true)),
        'D' => Some(key_sequence(32, true)),
        'E' => Some(key_sequence(18, true)),
        'F' => Some(key_sequence(33, true)),
        'G' => Some(key_sequence(34, true)),
        'H' => Some(key_sequence(35, true)),
        'I' => Some(key_sequence(23, true)),
        'J' => Some(key_sequence(36, true)),
        'K' => Some(key_sequence(37, true)),
        'L' => Some(key_sequence(38, true)),
        'M' => Some(key_sequence(50, true)),
        'N' => Some(key_sequence(49, true)),
        'O' => Some(key_sequence(24, true)),
        'P' => Some(key_sequence(25, true)),
        'Q' => Some(key_sequence(16, true)),
        'R' => Some(key_sequence(19, true)),
        'S' => Some(key_sequence(31, true)),
        'T' => Some(key_sequence(20, true)),
        'U' => Some(key_sequence(22, true)),
        'V' => Some(key_sequence(47, true)),
        'W' => Some(key_sequence(17, true)),
        'X' => Some(key_sequence(45, true)),
        'Y' => Some(key_sequence(21, true)),
        'Z' => Some(key_sequence(44, true)),
        '1' => Some(key_sequence(2, false)),
        '2' => Some(key_sequence(3, false)),
        '3' => Some(key_sequence(4, false)),
        '4' => Some(key_sequence(5, false)),
        '5' => Some(key_sequence(6, false)),
        '6' => Some(key_sequence(7, false)),
        '7' => Some(key_sequence(8, false)),
        '8' => Some(key_sequence(9, false)),
        '9' => Some(key_sequence(10, false)),
        '0' => Some(key_sequence(11, false)),
        '-' => Some(key_sequence(12, false)),
        '_' => Some(key_sequence(12, true)),
        '=' => Some(key_sequence(13, false)),
        '+' => Some(key_sequence(13, true)),
        '[' => Some(key_sequence(26, false)),
        '{' => Some(key_sequence(26, true)),
        ']' => Some(key_sequence(27, false)),
        '}' => Some(key_sequence(27, true)),
        ';' => Some(key_sequence(39, false)),
        ':' => Some(key_sequence(39, true)),
        '\'' => Some(key_sequence(40, false)),
        '"' => Some(key_sequence(40, true)),
        '`' => Some(key_sequence(41, false)),
        '~' => Some(key_sequence(41, true)),
        '\\' => Some(key_sequence(43, false)),
        '|' => Some(key_sequence(43, true)),
        ',' => Some(key_sequence(51, false)),
        '<' => Some(key_sequence(51, true)),
        '.' => Some(key_sequence(52, false)),
        '>' => Some(key_sequence(52, true)),
        '/' => Some(key_sequence(53, false)),
        '?' => Some(key_sequence(53, true)),
        '!' => Some(key_sequence(2, true)),
        '@' => Some(key_sequence(3, true)),
        '#' => Some(key_sequence(4, true)),
        '$' => Some(key_sequence(5, true)),
        '%' => Some(key_sequence(6, true)),
        '^' => Some(key_sequence(7, true)),
        '&' => Some(key_sequence(8, true)),
        '*' => Some(key_sequence(9, true)),
        '(' => Some(key_sequence(10, true)),
        ')' => Some(key_sequence(11, true)),
        _ => None,
    }
}

fn norwegian_sequence(ch: char) -> Option<Vec<String>> {
    let key =
        |code: u32, shifted: bool, altgr: bool| Some(key_sequence_with_mods(code, shifted, altgr));

    match ch {
        '\n' => Some(vec!["28:1".into(), "28:0".into()]),
        '\t' => Some(vec!["15:1".into(), "15:0".into()]),
        ' ' => Some(vec!["57:1".into(), "57:0".into()]),
        'a' | 'A' => Some(letter_sequence(30, ch.is_uppercase())),
        'b' | 'B' => Some(letter_sequence(48, ch.is_uppercase())),
        'c' | 'C' => Some(letter_sequence(46, ch.is_uppercase())),
        'd' | 'D' => Some(letter_sequence(32, ch.is_uppercase())),
        'e' | 'E' => Some(letter_sequence(18, ch.is_uppercase())),
        'f' | 'F' => Some(letter_sequence(33, ch.is_uppercase())),
        'g' | 'G' => Some(letter_sequence(34, ch.is_uppercase())),
        'h' | 'H' => Some(letter_sequence(35, ch.is_uppercase())),
        'i' | 'I' => Some(letter_sequence(23, ch.is_uppercase())),
        'j' | 'J' => Some(letter_sequence(36, ch.is_uppercase())),
        'k' | 'K' => Some(letter_sequence(37, ch.is_uppercase())),
        'l' | 'L' => Some(letter_sequence(38, ch.is_uppercase())),
        'm' | 'M' => Some(letter_sequence(50, ch.is_uppercase())),
        'n' | 'N' => Some(letter_sequence(49, ch.is_uppercase())),
        'o' | 'O' => Some(letter_sequence(24, ch.is_uppercase())),
        'p' | 'P' => Some(letter_sequence(25, ch.is_uppercase())),
        'q' | 'Q' => Some(letter_sequence(16, ch.is_uppercase())),
        'r' | 'R' => Some(letter_sequence(19, ch.is_uppercase())),
        's' | 'S' => Some(letter_sequence(31, ch.is_uppercase())),
        't' | 'T' => Some(letter_sequence(20, ch.is_uppercase())),
        'u' | 'U' => Some(letter_sequence(22, ch.is_uppercase())),
        'v' | 'V' => Some(letter_sequence(47, ch.is_uppercase())),
        'w' | 'W' => Some(letter_sequence(17, ch.is_uppercase())),
        'x' | 'X' => Some(letter_sequence(45, ch.is_uppercase())),
        'y' | 'Y' => Some(letter_sequence(21, ch.is_uppercase())),
        'z' | 'Z' => Some(letter_sequence(44, ch.is_uppercase())),
        '1' => key(2, false, false),
        '!' => key(2, true, false),
        '2' => key(3, false, false),
        '"' => key(3, true, false),
        '@' => key(3, false, true),
        '3' => key(4, false, false),
        '#' => key(4, true, false),
        '4' => key(5, false, false),
        '$' => key(5, false, true),
        '5' => key(6, false, false),
        '%' => key(6, true, false),
        '6' => key(7, false, false),
        '&' => key(7, true, false),
        '7' => key(8, false, false),
        '/' => key(8, true, false),
        '{' => key(8, false, true),
        '8' => key(9, false, false),
        '(' => key(9, true, false),
        '[' => key(9, false, true),
        '9' => key(10, false, false),
        ')' => key(10, true, false),
        ']' => key(10, false, true),
        '0' => key(11, false, false),
        '=' => key(11, true, false),
        '}' => key(11, false, true),
        '+' => key(12, false, false),
        '?' => key(12, true, false),
        '\\' => key(13, false, false),
        '`' => key(13, true, false),
        '\'' => key(43, false, false),
        '*' => key(43, true, false),
        '|' => key(41, false, false),
        '§' => key(41, true, false),
        ',' => key(51, false, false),
        ';' => key(51, true, false),
        '.' => key(52, false, false),
        ':' => key(52, true, false),
        '-' => key(53, false, false),
        '_' => key(53, true, false),
        '<' => key(86, false, false),
        '>' => key(86, true, false),
        'å' => key(26, false, false),
        'Å' => key(26, true, false),
        '^' => key(27, true, false),
        '~' => key(27, false, true),
        'ø' => key(39, false, false),
        'Ø' => key(39, true, false),
        'æ' => key(40, false, false),
        'Æ' => key(40, true, false),
        '€' => key(18, false, true),
        _ => None,
    }
}

fn key_sequence(keycode: u32, shifted: bool) -> Vec<String> {
    key_sequence_with_mods(keycode, shifted, false)
}

fn key_sequence_with_mods(keycode: u32, shifted: bool, altgr: bool) -> Vec<String> {
    let mut sequence = Vec::new();
    if altgr {
        sequence.push("100:1".to_string());
    }
    if shifted {
        sequence.push("42:1".to_string());
    }
    sequence.push(format!("{}:1", keycode));
    sequence.push(format!("{}:0", keycode));
    if shifted {
        sequence.push("42:0".to_string());
    }
    if altgr {
        sequence.push("100:0".to_string());
    }
    sequence
}

fn letter_sequence(keycode: u32, upper: bool) -> Vec<String> {
    key_sequence(keycode, upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn socket_status(ready: bool, path_known: bool) -> SocketStatus {
        SocketStatus {
            ready,
            path_known,
            message: String::new(),
        }
    }

    fn uinput_status(ready: bool) -> UinputStatus {
        UinputStatus {
            ready,
            message: String::new(),
        }
    }

    #[test]
    fn system_check_reports_linux_capabilities_and_items() {
        let check = build_system_check(true, true, socket_status(true, true), uinput_status(false));

        assert!(check.can_type);
        assert!(check.can_ocr);
        assert_eq!(
            check
                .items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            vec![
                "ydotool installed",
                "ydotoold socket",
                "/dev/uinput access",
                "tesseract OCR installed",
            ]
        );
    }

    #[test]
    fn can_type_when_socket_is_ready() {
        assert!(can_type(
            true,
            &socket_status(true, true),
            &uinput_status(false)
        ));
    }

    #[test]
    fn can_type_when_socket_can_be_started() {
        assert!(can_type(
            true,
            &socket_status(false, true),
            &uinput_status(true)
        ));
    }

    #[test]
    fn cannot_type_without_known_socket_path() {
        assert!(!can_type(
            true,
            &socket_status(false, false),
            &uinput_status(true)
        ));
    }

    #[test]
    fn cannot_type_without_ydotool_installed() {
        assert!(!can_type(
            false,
            &socket_status(true, true),
            &uinput_status(true)
        ));
    }

    #[test]
    fn us_sequence_maps_minus_and_underscore_to_minus_key() {
        assert_eq!(us_sequence('-'), Some(key_sequence(12, false)));
        assert_eq!(us_sequence('_'), Some(key_sequence(12, true)));
    }

    #[test]
    fn us_sequence_maps_equals_and_plus_to_equals_key() {
        assert_eq!(us_sequence('='), Some(key_sequence(13, false)));
        assert_eq!(us_sequence('+'), Some(key_sequence(13, true)));
    }

    #[test]
    fn us_sequence_maps_quote_and_backslash_to_distinct_keys() {
        assert_eq!(us_sequence('\''), Some(key_sequence(40, false)));
        assert_eq!(us_sequence('"'), Some(key_sequence(40, true)));
        assert_eq!(us_sequence('\\'), Some(key_sequence(43, false)));
        assert_eq!(us_sequence('|'), Some(key_sequence(43, true)));
    }

    #[test]
    fn us_sequence_maps_grave_and_slash_keys_correctly() {
        assert_eq!(us_sequence('`'), Some(key_sequence(41, false)));
        assert_eq!(us_sequence('~'), Some(key_sequence(41, true)));
        assert_eq!(us_sequence('/'), Some(key_sequence(53, false)));
        assert_eq!(us_sequence('?'), Some(key_sequence(53, true)));
    }

    #[test]
    fn us_sequence_maps_star_to_shifted_eight() {
        assert_eq!(us_sequence('*'), Some(key_sequence(9, true)));
    }
}
