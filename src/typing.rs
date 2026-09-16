use crate::platform;
use crate::types::{KeyboardLayout, StartConfig, UiEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub fn run_typing(config: StartConfig, cancel: Arc<AtomicBool>, tx: mpsc::Sender<UiEvent>) {
    run_typing_with(config, cancel, tx, platform::type_character);
}

fn run_typing_with(
    config: StartConfig,
    cancel: Arc<AtomicBool>,
    tx: mpsc::Sender<UiEvent>,
    mut type_character: impl FnMut(char, KeyboardLayout) -> Result<(), String>,
) {
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

        let typed_at = Instant::now();
        if let Err(err) = type_character(ch, config.keyboard_layout) {
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
        }
        .saturating_sub(typed_at.elapsed());

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

fn sleep_cancelable(
    duration: Duration,
    cancel: &AtomicBool,
    mut tick: impl FnMut(Duration),
) -> Result<(), ()> {
    if duration.is_zero() {
        return if cancel.load(Ordering::Relaxed) {
            Err(())
        } else {
            Ok(())
        };
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
    if total == 0 {
        0.0
    } else {
        done as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_reports_platform_error_without_counting_failed_character() {
        let config = StartConfig {
            text: "a".to_string(),
            delay_seconds: 0.0,
            chars_per_second: 1_000.0,
            enter_pause_seconds: 0.0,
            keyboard_layout: KeyboardLayout::Us,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        run_typing_with(config, cancel, tx, |_, _| {
            Err("keyboard backend unavailable".to_string())
        });

        match rx.recv().unwrap() {
            UiEvent::Finished {
                status,
                done,
                total,
            } => {
                assert_eq!(status, "Error: keyboard backend unavailable");
                assert_eq!(done, 0);
                assert_eq!(total, 1);
            }
            _ => panic!("expected typing to finish with the platform error"),
        }
    }
}
