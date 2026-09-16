use crate::types::{KeyboardLayout, SystemCheck};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod unsupported;

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
use unsupported as backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessRequest {
    CheckOnly,
    RequestIfNeeded,
}

pub fn check_system(access_request: AccessRequest) -> SystemCheck {
    backend::check_system(access_request)
}

pub fn recheck_readiness_on_activation() -> bool {
    backend::recheck_readiness_on_activation()
}

pub fn prepare_typing() -> Result<(), String> {
    backend::prepare_typing()
}

pub fn type_character(ch: char, layout: KeyboardLayout) -> Result<(), String> {
    backend::type_character(ch, layout)
}
