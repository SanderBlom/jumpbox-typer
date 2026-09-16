use crate::platform::{self, AccessRequest};
use crate::types::UiEvent;
use std::sync::mpsc;
use std::thread;

pub fn queue_system_check(tx: mpsc::Sender<UiEvent>, access_request: AccessRequest) {
    thread::spawn(move || {
        let check = platform::check_system(access_request);
        let _ = tx.send(UiEvent::SystemCheckFinished(check));
    });
}

pub fn command_stderr(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        output.status.to_string()
    } else {
        stderr.replace('\n', " ")
    }
}
