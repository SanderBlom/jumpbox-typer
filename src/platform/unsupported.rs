use super::AccessRequest;
use crate::types::{KeyboardLayout, SystemCheck, SystemCheckItem};

pub(super) fn check_system(_access_request: AccessRequest) -> SystemCheck {
    SystemCheck {
        items: vec![SystemCheckItem {
            title: "Keyboard event posting".to_string(),
            ok: false,
            detail: "Typing is not supported on this operating system yet.".to_string(),
            help: "A native typing backend is required before Jumpbox Typer can send keyboard events on this operating system.".to_string(),
        }],
        can_type: false,
        can_ocr: false,
    }
}

pub(super) const fn recheck_readiness_on_activation() -> bool {
    false
}

pub(super) fn prepare_typing() -> Result<(), String> {
    Err("typing is not supported on this operating system yet".to_string())
}

pub(super) fn type_character(_ch: char, _layout: KeyboardLayout) -> Result<(), String> {
    Err("typing is not supported on this operating system yet".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_exposes_disabled_typing_capability() {
        let check = check_system(AccessRequest::CheckOnly);

        assert!(!check.can_type);
        assert_eq!(check.items.len(), 1);
        assert_eq!(check.items[0].title, "Keyboard event posting");
        assert!(!check.items[0].ok);
    }

    #[test]
    fn unsupported_platform_returns_platform_neutral_typing_error() {
        assert_eq!(
            prepare_typing().unwrap_err(),
            "typing is not supported on this operating system yet"
        );
    }
}
