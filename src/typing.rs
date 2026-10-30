use crate::platform;
use crate::types::{KeyboardLayout, StartConfig, UiEvent};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Owns cancellation and completion. Callers poll rather than blocking the UI
/// while the current character finishes its key-release attempts.
#[derive(Debug)]
pub struct TypingJob {
    cancel: Arc<AtomicBool>,
    worker: thread::JoinHandle<()>,
}

impl TypingJob {
    pub fn start(config: StartConfig, tx: mpsc::Sender<UiEvent>) -> Self {
        Self::spawn(move |cancel| run_typing(config, cancel, tx))
    }

    pub(crate) fn spawn(work: impl FnOnce(Arc<AtomicBool>) + Send + 'static) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        Self {
            cancel,
            worker: thread::spawn(move || work(worker_cancel)),
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }

    /// Only call after `is_finished`, so joining cannot hold up the event loop.
    pub fn finish(self) -> thread::Result<()> {
        self.worker.join()
    }
}

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

    fn config(text: &str) -> StartConfig {
        StartConfig {
            text: text.into(),
            delay_seconds: 0.0,
            chars_per_second: 1_000.0,
            enter_pause_seconds: 0.0,
            keyboard_layout: KeyboardLayout::Us,
        }
    }

    #[test]
    fn cancellation_during_character_waits_for_release_and_never_starts_next_character() {
        let (pressed_tx, pressed_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let job = TypingJob::spawn(move |cancel| {
            run_typing_with(config("ab"), cancel, events_tx, |ch, _| {
                assert_eq!(ch, 'a');
                pressed_tx.send(()).unwrap();
                release_rx.recv().unwrap(); // fake key-up boundary
                Ok(())
            });
        });
        pressed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        job.cancel();
        job.cancel();
        assert!(!job.is_finished(), "cancellation alone is not completion");
        assert!(events_rx.try_recv().is_err());
        release_tx.send(()).unwrap();
        job.finish().unwrap();
        assert!(matches!(
            events_rx.recv().unwrap(),
            UiEvent::Finished { done: 1, .. }
        ));
        assert!(events_rx.try_recv().is_err());
    }

    #[test]
    fn cancellation_during_initial_delay_never_emits_a_character() {
        let mut config = config("a");
        config.delay_seconds = 60.0;
        let (tx, rx) = mpsc::channel();
        let job = TypingJob::spawn(move |cancel| {
            run_typing_with(config, cancel, tx, |_, _| panic!("must not type"));
        });
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            UiEvent::Status(_)
        ));
        job.cancel();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            UiEvent::Finished { done: 0, .. }
        ));
        job.finish().unwrap();
    }

    #[test]
    fn cancellation_interrupts_pacing_and_enter_pause() {
        for text in ["ab", "\nb"] {
            let mut config = config(text);
            config.chars_per_second = 1.0 / 60.0;
            config.enter_pause_seconds = 120.0;
            let (emitted_tx, emitted_rx) = mpsc::channel();
            let (tx, rx) = mpsc::channel();
            let job = TypingJob::spawn(move |cancel| {
                run_typing_with(config, cancel, tx, |ch, _| {
                    emitted_tx.send(ch).unwrap();
                    Ok(())
                });
            });
            assert_eq!(
                emitted_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
                text.chars().next().unwrap()
            );
            job.cancel();
            assert!(matches!(
                rx.recv_timeout(Duration::from_secs(2)).unwrap(),
                UiEvent::Finished { done: 1, .. }
            ));
            job.finish().unwrap();
            assert!(emitted_rx.try_recv().is_err());
        }
    }

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
