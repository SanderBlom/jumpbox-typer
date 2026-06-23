use gtk::gdk;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Keybind {
    key: String,
    modifiers: gdk::ModifierType,
}

pub fn keybind_matches(
    configured: &str,
    pressed_key: gdk::Key,
    pressed_state: gdk::ModifierType,
) -> Result<bool, String> {
    let configured = parse_keybind(configured)?;
    let pressed = Keybind {
        key: normalize_key_name(&pressed_key.name().unwrap_or_default()),
        modifiers: shortcut_modifiers(pressed_state),
    };

    Ok(configured == pressed)
}

fn parse_keybind(value: &str) -> Result<Keybind, String> {
    let parts = value
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    let Some((key, modifiers)) = parts.split_last() else {
        return Err("Set a start/stop keybind, for example F8 or Ctrl+Alt+S.".to_string());
    };

    let mut modifier_mask = gdk::ModifierType::empty();
    for modifier in modifiers {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifier_mask |= gdk::ModifierType::CONTROL_MASK,
            "alt" | "option" => modifier_mask |= gdk::ModifierType::ALT_MASK,
            "shift" => modifier_mask |= gdk::ModifierType::SHIFT_MASK,
            "super" | "cmd" | "win" | "windows" => modifier_mask |= gdk::ModifierType::SUPER_MASK,
            unknown => return Err(format!("Unknown keybind modifier: {unknown}.")),
        }
    }

    let key = normalize_key_name(key);
    if key.is_empty() {
        return Err("Set a start/stop keybind, for example F8 or Ctrl+Alt+S.".to_string());
    }

    Ok(Keybind {
        key,
        modifiers: modifier_mask,
    })
}

fn normalize_key_name(key: &str) -> String {
    match key.trim().to_ascii_lowercase().as_str() {
        "esc" | "escape" => "ESCAPE".to_string(),
        "return" | "enter" => "ENTER".to_string(),
        "space" => "SPACE".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

fn shortcut_modifiers(state: gdk::ModifierType) -> gdk::ModifierType {
    state
        & (gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SHIFT_MASK
            | gdk::ModifierType::SUPER_MASK)
}
