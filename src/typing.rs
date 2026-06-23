use crate::system_check::command_stderr;
use crate::types::{KeyboardLayout, StartConfig, UiEvent};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub fn run_typing(config: StartConfig, cancel: Arc<AtomicBool>, tx: mpsc::Sender<UiEvent>) {
    let total = config.text.chars().count();

    if sleep_cancelable(
        Duration::from_secs_f64(config.delay_seconds),
        &cancel,
        |remaining| {
            let _ = tx.send(UiEvent::Status(format!(
                "Starting in {:.1} seconds. Focus the target window now.",
                remaining.as_secs_f64()
            )));
        },
    )
    .is_err()
    {
        finish_stopped(&tx, 0, total);
        return;
    }

    let interval = Duration::from_secs_f64(1.0 / config.chars_per_second);
    let enter_pause = Duration::from_secs_f64(config.enter_pause_seconds);

    for (index, ch) in config.text.chars().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            finish_stopped(&tx, index, total);
            return;
        }

        if let Err(err) = type_char(ch, config.keyboard_layout) {
            let _ = tx.send(UiEvent::Finished {
                status: format!("Error: {err}"),
                done: index,
                total,
            });
            return;
        }

        let pause = if ch == '\n' && enter_pause > interval {
            enter_pause
        } else {
            interval
        };

        if sleep_cancelable(pause, &cancel, |_| {}).is_err() {
            finish_stopped(&tx, index + 1, total);
            return;
        }

        let done = index + 1;
        if done == total || done % 25 == 0 {
            let _ = tx.send(UiEvent::Progress {
                done,
                total,
                status: format!("Typing {done} of {total} characters..."),
            });
        }
    }

    let _ = tx.send(UiEvent::Finished {
        status: format!("Done. Typed {total} characters."),
        done: total,
        total,
    });
}

fn type_char(ch: char, layout: KeyboardLayout) -> Result<(), String> {
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

fn send_sequence(sequence: Option<Vec<String>>, fallback: char) -> std::io::Result<std::process::Output> {
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
        '-' => Some(key_sequence(53, false)),
        '_' => Some(key_sequence(53, true)),
        '=' => Some(key_sequence(13, false)),
        '+' => Some(key_sequence(12, false)),
        '[' => Some(key_sequence(26, false)),
        '{' => Some(key_sequence(26, true)),
        ']' => Some(key_sequence(27, false)),
        '}' => Some(key_sequence(27, true)),
        ';' => Some(key_sequence(39, false)),
        ':' => Some(key_sequence(39, true)),
        '\'' => Some(key_sequence(43, false)),
        '"' => Some(key_sequence(43, true)),
        '`' => Some(key_sequence(13, true)),
        '~' => Some(key_sequence(41, true)),
        '\\' => Some(key_sequence(43, false)),
        '|' => Some(key_sequence(41, false)),
        ',' => Some(key_sequence(51, false)),
        '<' => Some(key_sequence(51, true)),
        '.' => Some(key_sequence(52, false)),
        '>' => Some(key_sequence(52, true)),
        '/' => Some(key_sequence(86, false)),
        '?' => Some(key_sequence(12, true)),
        '!' => Some(key_sequence(2, true)),
        '@' => Some(key_sequence(3, true)),
        '#' => Some(key_sequence(4, true)),
        '$' => Some(key_sequence(5, true)),
        '%' => Some(key_sequence(6, true)),
        '^' => Some(key_sequence(7, true)),
        '&' => Some(key_sequence(8, true)),
        '*' => Some(key_sequence(43, true)),
        '(' => Some(key_sequence(10, true)),
        ')' => Some(key_sequence(11, true)),
        _ => None,
    }
}

fn norwegian_sequence(ch: char) -> Option<Vec<String>> {
    let key = |code: u32, shifted: bool, altgr: bool| {
        Some(key_sequence_with_mods(code, shifted, altgr))
    };

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

fn sleep_cancelable(
    duration: Duration,
    cancel: &AtomicBool,
    mut tick: impl FnMut(Duration),
) -> Result<(), ()> {
    if duration.is_zero() {
        return if cancel.load(Ordering::Relaxed) { Err(()) } else { Ok(()) };
    }

    let deadline = Instant::now() + duration;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(());
        }

        let now = Instant::now();
        if now >= deadline {
            return Ok(());
        }

        let remaining = deadline - now;
        tick(remaining);
        thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}

fn finish_stopped(tx: &mpsc::Sender<UiEvent>, done: usize, total: usize) {
    let _ = tx.send(UiEvent::Finished {
        status: format!("Stopped after {done} of {total} characters."),
        done,
        total,
    });
}

pub fn progress_fraction(done: usize, total: usize) -> f64 {
    if total == 0 { 0.0 } else { done as f64 / total as f64 }
}
