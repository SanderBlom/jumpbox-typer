use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub const APP_ID: &str = "dev.sander.jumpbox_typer";
pub const DEFAULT_DELAY_SECONDS: f64 = 5.0;
pub const DEFAULT_CHARS_PER_SECOND: f64 = 18.0;
pub const MAX_CHARS_PER_SECOND: f64 = 1000.0;
pub const DEFAULT_ENTER_PAUSE_SECONDS: f64 = 0.12;

#[derive(Debug, Clone)]
pub struct StartConfig {
    pub text: String,
    pub delay_seconds: f64,
    pub chars_per_second: f64,
    pub enter_pause_seconds: f64,
    pub keyboard_layout: KeyboardLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardLayout {
    Norwegian,
    Us,
}

impl KeyboardLayout {
    pub const fn label(self) -> &'static str {
        match self {
            KeyboardLayout::Norwegian => "Norwegian",
            KeyboardLayout::Us => "US",
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub cancel: Option<Arc<AtomicBool>>,
    pub progress: usize,
    pub total: usize,
    pub can_type: bool,
    pub can_ocr: bool,
}

#[derive(Debug, Clone)]
pub struct SystemCheck {
    pub items: Vec<SystemCheckItem>,
    pub can_type: bool,
    pub can_ocr: bool,
}

#[derive(Debug, Clone)]
pub struct SystemCheckItem {
    pub title: String,
    pub ok: bool,
    pub detail: String,
    pub help: String,
}

#[derive(Debug)]
pub enum UiEvent {
    Status(String),
    Progress {
        done: usize,
        total: usize,
        status: String,
    },
    Finished {
        status: String,
        done: usize,
        total: usize,
    },
    OcrFinished {
        status: String,
        text: Option<String>,
    },
    SystemCheckFinished(SystemCheck),
}
