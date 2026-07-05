use crate::types::{
    KeyboardLayout, StartConfig, DEFAULT_CHARS_PER_SECOND, DEFAULT_DELAY_SECONDS,
    DEFAULT_ENTER_PAUSE_SECONDS, MAX_CHARS_PER_SECOND,
};
use gtk::{Entry, TextView};
use gtk::prelude::{EditableExt, TextBufferExt, TextViewExt};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

const CONFIG_DIR_NAME: &str = "jumpbox-typer";
const CONFIG_FILE_NAME: &str = "config.txt";
const LEGACY_KEYBOARD_LAYOUT_FILE_NAME: &str = "keyboard-layout.txt";

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub delay_seconds: f64,
    pub chars_per_second: f64,
    pub enter_pause_seconds: f64,
    pub keyboard_layout: KeyboardLayout,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            delay_seconds: DEFAULT_DELAY_SECONDS,
            chars_per_second: DEFAULT_CHARS_PER_SECOND,
            enter_pause_seconds: DEFAULT_ENTER_PAUSE_SECONDS,
            keyboard_layout: KeyboardLayout::Norwegian,
        }
    }
}

impl AppConfig {
    pub const fn keyboard_layout_index(&self) -> u32 {
        match self.keyboard_layout {
            KeyboardLayout::Us => 1,
            KeyboardLayout::Norwegian => 0,
        }
    }
}

pub fn read_config(
    text_view: &TextView,
    delay: &Entry,
    speed: &Entry,
    enter_pause: &Entry,
    layout_index: u32,
) -> Result<StartConfig, String> {
    let buffer = text_view.buffer();
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    let text = buffer.text(&start, &end, true).to_string();
    if text.is_empty() {
        return Err("Paste or write text before starting.".to_string());
    }

    Ok(StartConfig {
        text,
        delay_seconds: read_float(&delay.text(), "start delay", 0.0, 300.0)?,
        chars_per_second: read_float(&speed.text(), "typing speed", 0.1, MAX_CHARS_PER_SECOND)?,
        enter_pause_seconds: read_float(&enter_pause.text(), "pause after Enter", 0.0, 10.0)?,
        keyboard_layout: match layout_index {
            1 => KeyboardLayout::Us,
            _ => KeyboardLayout::Norwegian,
        },
    })
}

pub fn read_app_config(
    delay: &Entry,
    speed: &Entry,
    enter_pause: &Entry,
    layout_index: u32,
) -> Result<AppConfig, String> {
    Ok(AppConfig {
        delay_seconds: read_float(&delay.text(), "start delay", 0.0, 300.0)?,
        chars_per_second: read_float(&speed.text(), "typing speed", 0.1, MAX_CHARS_PER_SECOND)?,
        enter_pause_seconds: read_float(&enter_pause.text(), "pause after Enter", 0.0, 10.0)?,
        keyboard_layout: keyboard_layout_from_index(layout_index),
    })
}

pub fn load_app_config() -> AppConfig {
    load_app_config_from(config_path(), legacy_keyboard_layout_path())
}

pub fn load_app_config_from(
    path: impl AsRef<Path>,
    legacy_keyboard_layout_path: impl AsRef<Path>,
) -> AppConfig {
    let mut config = AppConfig::default();
    let mut saw_keyboard_layout = false;

    if let Ok(text) = fs::read_to_string(path) {
        for line in text.lines() {
            let Some((raw_key, raw_value)) = line.split_once('=') else {
                continue;
            };

            let key = raw_key.trim();
            let value = raw_value.trim();

            match key {
                "delay_seconds" => {
                    if let Ok(parsed) = read_float(value, "start delay", 0.0, 300.0) {
                        config.delay_seconds = parsed;
                    }
                }
                "chars_per_second" => {
                    if let Ok(parsed) = read_float(value, "typing speed", 0.1, MAX_CHARS_PER_SECOND)
                    {
                        config.chars_per_second = parsed;
                    }
                }
                "enter_pause_seconds" => {
                    if let Ok(parsed) = read_float(value, "pause after Enter", 0.0, 10.0) {
                        config.enter_pause_seconds = parsed;
                    }
                }
                "keyboard_layout" => {
                    if let Some(layout) = parse_keyboard_layout(value) {
                        config.keyboard_layout = layout;
                        saw_keyboard_layout = true;
                    }
                }
                _ => {}
            }
        }
    }

    if !saw_keyboard_layout {
        config.keyboard_layout = load_keyboard_layout(legacy_keyboard_layout_path.as_ref());
    }

    config
}

pub fn save_app_config(config: &AppConfig) {
    save_app_config_to(config_path(), config);
}

pub fn save_app_config_to(path: impl AsRef<Path>, config: &AppConfig) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(mut file) = File::create(path) {
        let _ = writeln!(file, "delay_seconds={}", config.delay_seconds);
        let _ = writeln!(file, "chars_per_second={}", config.chars_per_second);
        let _ = writeln!(file, "enter_pause_seconds={}", config.enter_pause_seconds);
        let _ = writeln!(file, "keyboard_layout={}", config.keyboard_layout.label());
    }
}

pub fn load_keyboard_layout_index() -> u32 {
    load_app_config().keyboard_layout_index()
}

pub fn load_keyboard_layout_index_from(path: impl AsRef<Path>) -> u32 {
    match load_keyboard_layout(path.as_ref()) {
        KeyboardLayout::Us => 1,
        KeyboardLayout::Norwegian => 0,
    }
}

pub fn save_keyboard_layout_index(index: u32) {
    let mut config = load_app_config();
    config.keyboard_layout = keyboard_layout_from_index(index);
    save_app_config(&config);
}

pub fn save_keyboard_layout_index_to(path: impl AsRef<Path>, index: u32) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(mut file) = File::create(path) {
        let _ = writeln!(file, "{}", keyboard_layout_from_index(index).label());
    }
}

pub fn config_path() -> PathBuf {
    config_path_from(std::env::var_os("XDG_CONFIG_HOME"), std::env::var_os("HOME"))
}

pub fn config_path_from(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    config_dir_from(xdg_config_home, home).join(CONFIG_FILE_NAME)
}

pub fn keyboard_layout_path() -> PathBuf {
    legacy_keyboard_layout_path()
}

pub fn keyboard_layout_path_from(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    config_dir_from(xdg_config_home, home).join(LEGACY_KEYBOARD_LAYOUT_FILE_NAME)
}

fn legacy_keyboard_layout_path() -> PathBuf {
    keyboard_layout_path_from(std::env::var_os("XDG_CONFIG_HOME"), std::env::var_os("HOME"))
}

fn config_dir_from(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    xdg_config_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR_NAME)
}

fn load_keyboard_layout(path: &Path) -> KeyboardLayout {
    let Ok(text) = fs::read_to_string(path) else {
        return KeyboardLayout::Norwegian;
    };

    parse_keyboard_layout(text.trim()).unwrap_or(KeyboardLayout::Norwegian)
}

fn keyboard_layout_from_index(index: u32) -> KeyboardLayout {
    match index {
        1 => KeyboardLayout::Us,
        _ => KeyboardLayout::Norwegian,
    }
}

fn parse_keyboard_layout(value: &str) -> Option<KeyboardLayout> {
    match value.trim() {
        "US" => Some(KeyboardLayout::Us),
        "Norwegian" => Some(KeyboardLayout::Norwegian),
        _ => None,
    }
}

pub fn read_float(value: &str, label: &str, min: f64, max: f64) -> Result<f64, String> {
    let number = value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| format!("{label} must be a number."))?;

    if !(min..=max).contains(&number) {
        return Err(format!("{label} must be between {min:.1} and {max:.1}."));
    }

    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_path(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        std::env::temp_dir().join(format!("jumpbox-typer-{name}-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn read_float_accepts_comma_decimal() {
        let value = read_float(" 12,5 ", "speed", 0.0, 20.0).unwrap();
        assert_eq!(value, 12.5);
    }

    #[test]
    fn read_float_rejects_out_of_range() {
        let err = read_float("0", "speed", 0.1, 20.0).unwrap_err();
        assert_eq!(err, "speed must be between 0.1 and 20.0.");
    }

    #[test]
    fn read_float_accepts_high_typing_speed() {
        let value = read_float("1000", "typing speed", 0.1, MAX_CHARS_PER_SECOND).unwrap();
        assert_eq!(value, 1000.0);
    }

    #[test]
    fn keyboard_layout_path_prefers_xdg_config_home() {
        let path = keyboard_layout_path_from(
            Some(std::ffi::OsString::from("/tmp/config-base")),
            Some(std::ffi::OsString::from("/tmp/home-base")),
        );

        assert_eq!(path, PathBuf::from("/tmp/config-base/jumpbox-typer/keyboard-layout.txt"));
    }

    #[test]
    fn keyboard_layout_path_falls_back_to_home_config() {
        let path = keyboard_layout_path_from(None, Some(std::ffi::OsString::from("/tmp/home-base")));

        assert_eq!(path, PathBuf::from("/tmp/home-base/.config/jumpbox-typer/keyboard-layout.txt"));
    }

    #[test]
    fn config_path_prefers_xdg_config_home() {
        let path = config_path_from(
            Some(std::ffi::OsString::from("/tmp/config-base")),
            Some(std::ffi::OsString::from("/tmp/home-base")),
        );

        assert_eq!(path, PathBuf::from("/tmp/config-base/jumpbox-typer/config.txt"));
    }

    #[test]
    fn load_app_config_uses_defaults_when_missing() {
        let config = load_app_config_from(
            unique_temp_path("missing-config"),
            unique_temp_path("missing-layout"),
        );

        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn load_app_config_reads_saved_values() {
        let config_path = unique_temp_path("app-config");
        let legacy_path = unique_temp_path("legacy-layout");
        fs::write(
            &config_path,
            "delay_seconds=3\nchars_per_second=25\nenter_pause_seconds=0.5\nkeyboard_layout=US\n",
        )
        .unwrap();

        let config = load_app_config_from(&config_path, &legacy_path);

        assert_eq!(
            config,
            AppConfig {
                delay_seconds: 3.0,
                chars_per_second: 25.0,
                enter_pause_seconds: 0.5,
                keyboard_layout: KeyboardLayout::Us,
            }
        );

        let _ = fs::remove_file(&config_path);
    }

    #[test]
    fn load_app_config_falls_back_to_legacy_layout() {
        let config_path = unique_temp_path("app-config-no-layout");
        let legacy_path = unique_temp_path("legacy-layout");
        fs::write(&config_path, "delay_seconds=3\n").unwrap();
        fs::write(&legacy_path, "US\n").unwrap();

        let config = load_app_config_from(&config_path, &legacy_path);

        assert_eq!(config.delay_seconds, 3.0);
        assert_eq!(config.keyboard_layout, KeyboardLayout::Us);

        let _ = fs::remove_file(&config_path);
        let _ = fs::remove_file(&legacy_path);
    }

    #[test]
    fn save_app_config_writes_all_values() {
        let config_path = unique_temp_path("saved-app-config");
        let config = AppConfig {
            delay_seconds: 3.0,
            chars_per_second: 25.0,
            enter_pause_seconds: 0.5,
            keyboard_layout: KeyboardLayout::Us,
        };

        save_app_config_to(&config_path, &config);

        let saved = fs::read_to_string(&config_path).unwrap();
        assert!(saved.contains("delay_seconds=3"));
        assert!(saved.contains("chars_per_second=25"));
        assert!(saved.contains("enter_pause_seconds=0.5"));
        assert!(saved.contains("keyboard_layout=US"));

        let _ = fs::remove_file(&config_path);
    }

    #[test]
    fn keyboard_layout_index_round_trips() {
        let path = unique_temp_path("keyboard-layout");

        save_keyboard_layout_index_to(&path, 1);

        assert_eq!(load_keyboard_layout_index_from(&path), 1);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn keyboard_layout_index_defaults_on_invalid_file() {
        let path = unique_temp_path("keyboard-layout-invalid");
        fs::write(&path, "maybe").unwrap();

        assert_eq!(load_keyboard_layout_index_from(&path), 0);

        let _ = fs::remove_file(&path);
    }
}
