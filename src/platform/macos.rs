use super::AccessRequest;
use crate::ocr::tesseract_available;
use crate::types::{KeyboardLayout, SystemCheck, SystemCheckItem};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGPreflightPostEventAccess, CGRequestPostEventAccess,
};

pub(super) fn check_system(access_request: AccessRequest) -> SystemCheck {
    let posting_access = posting_access(
        access_request,
        || CGPreflightPostEventAccess(),
        || CGRequestPostEventAccess(),
    );
    let tesseract = tesseract_available();

    build_system_check(posting_access, tesseract)
}

pub(super) const fn recheck_readiness_on_activation() -> bool {
    true
}

pub(super) fn prepare_typing() -> Result<(), String> {
    prepare_typing_with(CGPreflightPostEventAccess())
}

pub(super) fn type_character(ch: char, layout: KeyboardLayout) -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok_or_else(|| {
        "Core Graphics could not create a keyboard event source; verify Accessibility permission in System Settings > Privacy & Security"
            .to_string()
    })?;

    match character_input(ch, layout) {
        CharacterInput::Physical(sequence) => {
            post_sequence_with(&sequence, |transition, modifiers| {
                let event = CGEvent::new_keyboard_event(
                    Some(&source),
                    transition.key_code,
                    transition.key_down,
                )
                .ok_or_else(|| {
                    format!(
                        "Core Graphics could not create keyboard event for key code {}",
                        transition.key_code
                    )
                })?;

                let mut flags = CGEventFlags::empty();
                if modifiers.shift {
                    flags |= CGEventFlags::MaskShift;
                }
                if modifiers.option {
                    flags |= CGEventFlags::MaskAlternate;
                }
                CGEvent::set_flags(Some(&event), flags);
                CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
                Ok(())
            })
        }
        CharacterInput::Unicode(utf16) => post_unicode_with(&utf16, |key_down, utf16| {
            let event =
                CGEvent::new_keyboard_event(Some(&source), 0, key_down).ok_or_else(|| {
                    format!("Core Graphics could not create Unicode keyboard event for {ch:?}")
                })?;

            unsafe {
                CGEvent::keyboard_set_unicode_string(
                    Some(&event),
                    utf16.len() as u64,
                    utf16.as_ptr(),
                );
            }
            CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
            Ok(())
        }),
    }
}

fn prepare_typing_with(posting_access: bool) -> Result<(), String> {
    if posting_access {
        Ok(())
    } else {
        Err("Keyboard event posting access is required. Allow Jumpbox Typer in System Settings > Privacy & Security > Accessibility, then run Check system again.".to_string())
    }
}

fn posting_access(
    access_request: AccessRequest,
    mut preflight: impl FnMut() -> bool,
    mut request: impl FnMut() -> bool,
) -> bool {
    let allowed = preflight();
    if !allowed && access_request == AccessRequest::RequestIfNeeded {
        request()
    } else {
        allowed
    }
}

fn build_system_check(posting_access: bool, tesseract: bool) -> SystemCheck {
    SystemCheck {
        items: vec![
            SystemCheckItem {
                title: "Keyboard event posting access".to_string(),
                ok: posting_access,
                detail: if posting_access {
                    "Jumpbox Typer can post keyboard events.".to_string()
                } else {
                    "Allow Jumpbox Typer in System Settings > Privacy & Security > Accessibility, then return to the app.".to_string()
                },
                help: "macOS requires Accessibility permission before Jumpbox Typer can send physical keyboard events to the focused application.".to_string(),
            },
            SystemCheckItem {
                title: "tesseract OCR installed".to_string(),
                ok: tesseract,
                detail: if tesseract {
                    "tesseract OCR is installed.".to_string()
                } else {
                    "Install tesseract OCR with Homebrew: brew install tesseract".to_string()
                },
                help: "Tesseract reads text from clipboard images and screenshots. The OCR button depends on it to turn a screenshot into editable text.".to_string(),
            },
        ],
        can_type: posting_access,
        can_ocr: tesseract,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyTransition {
    key_code: u16,
    key_down: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CharacterInput {
    Physical(Vec<KeyTransition>),
    Unicode(Vec<u16>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModifierState {
    shift: bool,
    option: bool,
}

impl ModifierState {
    const NONE: Self = Self {
        shift: false,
        option: false,
    };
    #[cfg(test)]
    const SHIFT: Self = Self {
        shift: true,
        option: false,
    };
    #[cfg(test)]
    const OPTION: Self = Self {
        shift: false,
        option: true,
    };
}

impl std::ops::BitOr for ModifierState {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            shift: self.shift || rhs.shift,
            option: self.option || rhs.option,
        }
    }
}

const SHIFT_KEY_CODE: u16 = 56;
const OPTION_KEY_CODE: u16 = 58;

fn character_input(ch: char, layout: KeyboardLayout) -> CharacterInput {
    let physical = match layout {
        KeyboardLayout::Us => us_sequence(ch),
        KeyboardLayout::Norwegian => norwegian_sequence(ch),
    };

    physical.map_or_else(
        || CharacterInput::Unicode(ch.encode_utf16(&mut [0; 2]).to_vec()),
        CharacterInput::Physical,
    )
}

fn us_sequence(ch: char) -> Option<Vec<KeyTransition>> {
    let (key_code, shifted) = match ch {
        '\n' => (36, false),
        '\t' => (48, false),
        ' ' => (49, false),
        '1' => (18, false),
        '2' => (19, false),
        '3' => (20, false),
        '4' => (21, false),
        '5' => (23, false),
        '6' => (22, false),
        '7' => (26, false),
        '8' => (28, false),
        '9' => (25, false),
        '0' => (29, false),
        '!' => (18, true),
        '"' => (39, true),
        '#' => (20, true),
        '$' => (21, true),
        '%' => (23, true),
        '&' => (26, true),
        '\'' => (39, false),
        '(' => (25, true),
        ')' => (29, true),
        '*' => (28, true),
        '+' => (24, true),
        ',' => (43, false),
        '-' => (27, false),
        '.' => (47, false),
        '/' => (44, false),
        ':' => (41, true),
        ';' => (41, false),
        '<' => (43, true),
        '=' => (24, false),
        '>' => (47, true),
        '?' => (44, true),
        '@' => (19, true),
        '[' => (33, false),
        '\\' => (42, false),
        ']' => (30, false),
        '^' => (22, true),
        '_' => (27, true),
        '`' => (50, false),
        '{' => (33, true),
        '|' => (42, true),
        '}' => (30, true),
        '~' => (50, true),
        _ if ch.is_ascii_alphabetic() => {
            let key_code = match ch.to_ascii_lowercase() {
                'a' => 0,
                'b' => 11,
                'c' => 8,
                'd' => 2,
                'e' => 14,
                'f' => 3,
                'g' => 5,
                'h' => 4,
                'i' => 34,
                'j' => 38,
                'k' => 40,
                'l' => 37,
                'm' => 46,
                'n' => 45,
                'o' => 31,
                'p' => 35,
                'q' => 12,
                'r' => 15,
                's' => 1,
                't' => 17,
                'u' => 32,
                'v' => 9,
                'w' => 13,
                'x' => 7,
                'y' => 16,
                'z' => 6,
                _ => unreachable!(),
            };
            (key_code, ch.is_ascii_uppercase())
        }
        _ => return None,
    };

    Some(key_sequence(key_code, shifted))
}

fn norwegian_sequence(ch: char) -> Option<Vec<KeyTransition>> {
    let (key_code, shifted, option) = match ch {
        '\n' => (36, false, false),
        '\t' => (48, false, false),
        ' ' => (49, false, false),
        '1' => (18, false, false),
        '!' => (18, true, false),
        '2' => (19, false, false),
        '"' => (19, true, false),
        '@' => (42, false, false),
        '3' => (20, false, false),
        '#' => (20, true, false),
        '4' => (21, false, false),
        '$' => (21, true, false),
        '€' => (21, false, true),
        '5' => (23, false, false),
        '%' => (23, true, false),
        '6' => (22, false, false),
        '&' => (22, true, false),
        '7' => (26, false, false),
        '/' => (26, true, false),
        '|' => (26, false, true),
        '\\' => (26, true, true),
        '8' => (28, false, false),
        '(' => (28, true, false),
        '[' => (28, false, true),
        '{' => (28, true, true),
        '9' => (25, false, false),
        ')' => (25, true, false),
        ']' => (25, false, true),
        '}' => (25, true, true),
        '0' => (29, false, false),
        '=' => (29, true, false),
        '+' => (27, false, false),
        '?' => (27, true, false),
        '`' => (24, true, false),
        '\'' => (10, false, false),
        '§' => (10, true, false),
        '*' => (42, true, false),
        ',' => (43, false, false),
        ';' => (43, true, false),
        '.' => (47, false, false),
        ':' => (47, true, false),
        '-' => (44, false, false),
        '_' => (44, true, false),
        '<' => (50, false, false),
        '>' => (50, true, false),
        'å' => (33, false, false),
        'Å' => (33, true, false),
        '^' => (30, true, false),
        '~' => (30, false, true),
        'ø' => (41, false, false),
        'Ø' => (41, true, false),
        'æ' => (39, false, false),
        'Æ' => (39, true, false),
        _ if ch.is_ascii_alphabetic() => {
            let key_code = match ch.to_ascii_lowercase() {
                'a' => 0,
                'b' => 11,
                'c' => 8,
                'd' => 2,
                'e' => 14,
                'f' => 3,
                'g' => 5,
                'h' => 4,
                'i' => 34,
                'j' => 38,
                'k' => 40,
                'l' => 37,
                'm' => 46,
                'n' => 45,
                'o' => 31,
                'p' => 35,
                'q' => 12,
                'r' => 15,
                's' => 1,
                't' => 17,
                'u' => 32,
                'v' => 9,
                'w' => 13,
                'x' => 7,
                'y' => 16,
                'z' => 6,
                _ => unreachable!(),
            };
            (key_code, ch.is_ascii_uppercase(), false)
        }
        _ => return None,
    };

    Some(key_sequence_with_modifiers(key_code, shifted, option))
}

fn key_sequence(key_code: u16, shifted: bool) -> Vec<KeyTransition> {
    key_sequence_with_modifiers(key_code, shifted, false)
}

fn key_sequence_with_modifiers(key_code: u16, shifted: bool, option: bool) -> Vec<KeyTransition> {
    let mut sequence = Vec::new();
    if option {
        sequence.push(KeyTransition {
            key_code: OPTION_KEY_CODE,
            key_down: true,
        });
    }
    if shifted {
        sequence.push(KeyTransition {
            key_code: SHIFT_KEY_CODE,
            key_down: true,
        });
    }
    sequence.extend([
        KeyTransition {
            key_code,
            key_down: true,
        },
        KeyTransition {
            key_code,
            key_down: false,
        },
    ]);
    if shifted {
        sequence.push(KeyTransition {
            key_code: SHIFT_KEY_CODE,
            key_down: false,
        });
    }
    if option {
        sequence.push(KeyTransition {
            key_code: OPTION_KEY_CODE,
            key_down: false,
        });
    }
    sequence
}

fn post_sequence_with(
    sequence: &[KeyTransition],
    mut post: impl FnMut(KeyTransition, ModifierState) -> Result<(), String>,
) -> Result<(), String> {
    let mut modifiers = ModifierState::NONE;
    let mut first_error = None;

    for &transition in sequence {
        if transition.key_code == SHIFT_KEY_CODE {
            modifiers.shift = transition.key_down;
        } else if transition.key_code == OPTION_KEY_CODE {
            modifiers.option = transition.key_down;
        }

        if let Err(error) = post(transition, modifiers) {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    first_error.map_or(Ok(()), Err)
}

fn post_unicode_with(
    utf16: &[u16],
    mut post: impl FnMut(bool, &[u16]) -> Result<(), String>,
) -> Result<(), String> {
    let mut first_error = None;

    for key_down in [true, false] {
        if let Err(error) = post(key_down, utf16) {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }

    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn missing_posting_access_reports_macos_readiness_without_linux_concepts() {
        let check = build_system_check(false, true);

        assert!(!check.can_type);
        assert!(check.can_ocr);
        assert_eq!(
            check
                .items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Keyboard event posting access", "tesseract OCR installed"]
        );
        assert!(check.items[0].detail.contains("Privacy & Security"));
        assert!(check.items[0].detail.contains("Accessibility"));
        assert!(!check.items.iter().any(|item| {
            item.title.contains("ydotool")
                || item.detail.contains("ydotool")
                || item.detail.contains("uinput")
                || item.detail.contains("systemctl")
        }));
    }

    #[test]
    fn startup_check_preflights_without_requesting_access() {
        let request_count = Cell::new(0);

        let allowed = posting_access(
            AccessRequest::CheckOnly,
            || false,
            || {
                request_count.set(request_count.get() + 1);
                false
            },
        );

        assert!(!allowed);
        assert_eq!(request_count.get(), 0);
    }

    #[test]
    fn user_check_requests_missing_access_once() {
        let request_count = Cell::new(0);

        let allowed = posting_access(
            AccessRequest::RequestIfNeeded,
            || false,
            || {
                request_count.set(request_count.get() + 1);
                true
            },
        );

        assert!(allowed);
        assert_eq!(request_count.get(), 1);
    }

    #[test]
    fn us_letters_map_to_physical_keys_with_shift_for_uppercase() {
        let key_codes = [
            ('a', 0),
            ('b', 11),
            ('c', 8),
            ('d', 2),
            ('e', 14),
            ('f', 3),
            ('g', 5),
            ('h', 4),
            ('i', 34),
            ('j', 38),
            ('k', 40),
            ('l', 37),
            ('m', 46),
            ('n', 45),
            ('o', 31),
            ('p', 35),
            ('q', 12),
            ('r', 15),
            ('s', 1),
            ('t', 17),
            ('u', 32),
            ('v', 9),
            ('w', 13),
            ('x', 7),
            ('y', 16),
            ('z', 6),
        ];

        for (lowercase, key_code) in key_codes {
            assert_eq!(us_sequence(lowercase), Some(key_sequence(key_code, false)));
            assert_eq!(
                us_sequence(lowercase.to_ascii_uppercase()),
                Some(key_sequence(key_code, true))
            );
        }
    }

    #[test]
    fn us_digits_and_whitespace_controls_map_to_physical_keys() {
        let mappings = [
            ('1', 18),
            ('2', 19),
            ('3', 20),
            ('4', 21),
            ('5', 23),
            ('6', 22),
            ('7', 26),
            ('8', 28),
            ('9', 25),
            ('0', 29),
            ('\n', 36),
            ('\t', 48),
            (' ', 49),
        ];

        for (character, key_code) in mappings {
            assert_eq!(
                us_sequence(character),
                Some(key_sequence(key_code, false)),
                "wrong mapping for {character:?}"
            );
        }
    }

    #[test]
    fn us_printable_punctuation_maps_to_physical_keys() {
        let mappings = [
            ('!', 18, true),
            ('"', 39, true),
            ('#', 20, true),
            ('$', 21, true),
            ('%', 23, true),
            ('&', 26, true),
            ('\'', 39, false),
            ('(', 25, true),
            (')', 29, true),
            ('*', 28, true),
            ('+', 24, true),
            (',', 43, false),
            ('-', 27, false),
            ('.', 47, false),
            ('/', 44, false),
            (':', 41, true),
            (';', 41, false),
            ('<', 43, true),
            ('=', 24, false),
            ('>', 47, true),
            ('?', 44, true),
            ('@', 19, true),
            ('[', 33, false),
            ('\\', 42, false),
            (']', 30, false),
            ('^', 22, true),
            ('_', 27, true),
            ('`', 50, false),
            ('{', 33, true),
            ('|', 42, true),
            ('}', 30, true),
            ('~', 50, true),
        ];

        for (character, key_code, shifted) in mappings {
            assert_eq!(
                us_sequence(character),
                Some(key_sequence(key_code, shifted)),
                "wrong mapping for {character:?}"
            );
        }
    }

    #[test]
    fn norwegian_layout_maps_linux_supported_characters_to_physical_keys() {
        let mappings = [
            ('1', 18, false, false),
            ('!', 18, true, false),
            ('2', 19, false, false),
            ('"', 19, true, false),
            ('@', 42, false, false),
            ('3', 20, false, false),
            ('#', 20, true, false),
            ('4', 21, false, false),
            ('$', 21, true, false),
            ('5', 23, false, false),
            ('%', 23, true, false),
            ('6', 22, false, false),
            ('&', 22, true, false),
            ('7', 26, false, false),
            ('/', 26, true, false),
            ('|', 26, false, true),
            ('\\', 26, true, true),
            ('8', 28, false, false),
            ('(', 28, true, false),
            ('[', 28, false, true),
            ('{', 28, true, true),
            ('9', 25, false, false),
            (')', 25, true, false),
            (']', 25, false, true),
            ('}', 25, true, true),
            ('0', 29, false, false),
            ('=', 29, true, false),
            ('+', 27, false, false),
            ('?', 27, true, false),
            ('`', 24, true, false),
            ('\'', 10, false, false),
            ('§', 10, true, false),
            ('*', 42, true, false),
            (',', 43, false, false),
            (';', 43, true, false),
            ('.', 47, false, false),
            (':', 47, true, false),
            ('-', 44, false, false),
            ('_', 44, true, false),
            ('<', 50, false, false),
            ('>', 50, true, false),
            ('å', 33, false, false),
            ('Å', 33, true, false),
            ('^', 30, true, false),
            ('~', 30, false, true),
            ('ø', 41, false, false),
            ('Ø', 41, true, false),
            ('æ', 39, false, false),
            ('Æ', 39, true, false),
            ('€', 21, false, true),
        ];

        for (character, key_code, shifted, option) in mappings {
            assert_eq!(
                norwegian_sequence(character),
                Some(key_sequence_with_modifiers(key_code, shifted, option)),
                "wrong mapping for {character:?}"
            );
        }
    }

    #[test]
    fn norwegian_letters_and_whitespace_map_to_physical_keys() {
        let key_codes = [
            ('a', 0),
            ('b', 11),
            ('c', 8),
            ('d', 2),
            ('e', 14),
            ('f', 3),
            ('g', 5),
            ('h', 4),
            ('i', 34),
            ('j', 38),
            ('k', 40),
            ('l', 37),
            ('m', 46),
            ('n', 45),
            ('o', 31),
            ('p', 35),
            ('q', 12),
            ('r', 15),
            ('s', 1),
            ('t', 17),
            ('u', 32),
            ('v', 9),
            ('w', 13),
            ('x', 7),
            ('y', 16),
            ('z', 6),
        ];

        for (lowercase, key_code) in key_codes {
            assert_eq!(
                norwegian_sequence(lowercase),
                Some(key_sequence(key_code, false))
            );
            assert_eq!(
                norwegian_sequence(lowercase.to_ascii_uppercase()),
                Some(key_sequence(key_code, true))
            );
        }

        for (character, key_code) in [('\n', 36), ('\t', 48), (' ', 49)] {
            assert_eq!(
                norwegian_sequence(character),
                Some(key_sequence(key_code, false))
            );
        }
    }

    #[test]
    fn every_guaranteed_norwegian_sequence_releases_modifiers() {
        let corpus = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!\"@#$%&/|\\()[]{}=+?`'§*,;.:_-<>åÅ^~øØæÆ€"
            .chars()
            .chain(['\n', '\t', ' ']);

        for character in corpus {
            let sequence = norwegian_sequence(character).expect("guaranteed character must map");
            let mut shift_depth = 0_i32;
            let mut option_depth = 0_i32;

            for transition in sequence {
                let depth = if transition.key_code == SHIFT_KEY_CODE {
                    &mut shift_depth
                } else if transition.key_code == OPTION_KEY_CODE {
                    &mut option_depth
                } else {
                    continue;
                };
                *depth += if transition.key_down { 1 } else { -1 };
                assert!(
                    *depth >= 0,
                    "modifier released before press for {character:?}"
                );
            }

            assert_eq!(shift_depth, 0, "Shift left held for {character:?}");
            assert_eq!(option_depth, 0, "Option left held for {character:?}");
        }
    }

    #[test]
    fn every_guaranteed_us_sequence_releases_shift() {
        let corpus = (b' '..=b'~').map(char::from).chain(['\n', '\t']);

        for character in corpus {
            let sequence = us_sequence(character).expect("guaranteed character must map");
            let mut shift_depth = 0_i32;

            for transition in sequence {
                if transition.key_code == SHIFT_KEY_CODE {
                    shift_depth += if transition.key_down { 1 } else { -1 };
                    assert!(
                        shift_depth >= 0,
                        "Shift released before press for {character:?}"
                    );
                }
            }

            assert_eq!(shift_depth, 0, "Shift left held for {character:?}");
        }
    }

    #[test]
    fn posting_failure_still_attempts_modifier_release() {
        let sequence = key_sequence(0, true);
        let mut attempted = Vec::new();

        let error = post_sequence_with(&sequence, |transition, modifiers| {
            attempted.push((transition, modifiers));
            if transition.key_code == 0 && transition.key_down {
                Err("could not create keyboard event".to_string())
            } else {
                Ok(())
            }
        })
        .unwrap_err();

        assert_eq!(error, "could not create keyboard event");
        assert_eq!(
            attempted,
            vec![
                (sequence[0], ModifierState::SHIFT),
                (sequence[1], ModifierState::SHIFT),
                (sequence[2], ModifierState::SHIFT),
                (sequence[3], ModifierState::NONE),
            ]
        );
    }

    #[test]
    fn norwegian_posting_tracks_and_releases_shift_and_option() {
        let sequence = key_sequence_with_modifiers(26, true, true);
        let mut attempted = Vec::new();

        let error = post_sequence_with(&sequence, |transition, modifiers| {
            attempted.push((transition, modifiers));
            if transition.key_code == 26 && transition.key_down {
                Err("could not create keyboard event".to_string())
            } else {
                Ok(())
            }
        })
        .unwrap_err();

        assert_eq!(error, "could not create keyboard event");
        assert_eq!(attempted[0].1, ModifierState::OPTION);
        assert_eq!(attempted[1].1, ModifierState::OPTION | ModifierState::SHIFT);
        assert_eq!(attempted[2].1, ModifierState::OPTION | ModifierState::SHIFT);
        assert_eq!(attempted[3].1, ModifierState::OPTION | ModifierState::SHIFT);
        assert_eq!(attempted[4].1, ModifierState::OPTION);
        assert_eq!(attempted[5].1, ModifierState::NONE);
    }

    #[test]
    fn unmapped_characters_use_bmp_and_surrogate_pair_unicode_input() {
        assert_eq!(
            character_input('é', KeyboardLayout::Us),
            CharacterInput::Unicode(vec![0x00e9])
        );
        assert_eq!(
            character_input('🚀', KeyboardLayout::Norwegian),
            CharacterInput::Unicode(vec![0xd83d, 0xde80])
        );
    }

    #[test]
    fn unicode_event_creation_failure_is_reported() {
        let mut attempted = Vec::new();

        let error = post_unicode_with(&[0xd83d, 0xde80], |key_down, utf16| {
            attempted.push((key_down, utf16.to_vec()));
            Err("Core Graphics could not create Unicode keyboard event".to_string())
        })
        .unwrap_err();

        assert_eq!(
            error,
            "Core Graphics could not create Unicode keyboard event"
        );
        assert_eq!(
            attempted,
            vec![(true, vec![0xd83d, 0xde80]), (false, vec![0xd83d, 0xde80])]
        );
    }

    #[test]
    fn typing_preparation_rejects_revoked_posting_access() {
        let error = prepare_typing_with(false).unwrap_err();

        assert!(error.contains("Privacy & Security"));
        assert!(error.contains("Accessibility"));
    }

    #[test]
    fn macos_rechecks_readiness_when_application_returns_to_foreground() {
        assert!(recheck_readiness_on_activation());
    }
}
