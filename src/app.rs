use crate::ocr::{run_ocr_file, temporary_ocr_image_path};
use crate::system_check::{ensure_ydotool_ready, queue_system_check, require_command};
use crate::types::{
    AppState, KeyboardLayout, StartConfig, SystemCheck, SystemCheckItem, UiEvent,
    DEFAULT_CHARS_PER_SECOND, DEFAULT_DELAY_SECONDS, DEFAULT_ENTER_PAUSE_SECONDS,
};
use crate::typing::{progress_fraction, run_typing};
use adw::prelude::*;
use adw::{AboutWindow, Application, ApplicationWindow, HeaderBar, ToolbarView};
use gtk::glib;
use gtk::{
    Align, Box as GtkBox, Button, DropDown, Entry, Image, Label, Orientation, ProgressBar,
    ScrolledWindow, TextView, WrapMode,
};
use std::cell::{Cell, RefCell};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Jumpbox Typer")
        .default_width(920)
        .default_height(660)
        .build();
    window.set_icon_name(Some("dev.sander.jumpbox_typer"));

    let header_bar = HeaderBar::new();
    let toolbar_view = ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);

    let text_view = TextView::builder()
        .monospace(true)
        .wrap_mode(WrapMode::None)
        .vexpand(true)
        .hexpand(true)
        .build();

    let scroller = ScrolledWindow::builder()
        .child(&text_view)
        .min_content_height(360)
        .vexpand(true)
        .hexpand(true)
        .build();

    let editor_title = Label::new(Some("Text to type"));
    editor_title.add_css_class("heading");
    editor_title.set_halign(Align::Start);

    let editor = GtkBox::new(Orientation::Vertical, 6);
    editor.append(&editor_title);
    editor.append(&scroller);

    let delay = numeric_entry(&format!("{DEFAULT_DELAY_SECONDS:.1}"));
    let speed = numeric_entry(&format!("{DEFAULT_CHARS_PER_SECOND:.0}"));
    let enter_pause = numeric_entry(&format!("{DEFAULT_ENTER_PAUSE_SECONDS:.2}"));
    let keyboard_layout = DropDown::from_strings(&["Norwegian", "US"]);
    keyboard_layout.set_selected(load_keyboard_layout_index());

    let start = Button::with_label("Start typing");
    start.add_css_class("suggested-action");
    start.set_sensitive(false);
    let stop = Button::with_label("Stop");
    stop.set_sensitive(false);
    let clear = Button::with_label("Clear");
    clear.add_css_class("destructive-action");
    let extract_clipboard_image = Button::with_label("Extract clipboard image text");
    extract_clipboard_image.set_sensitive(false);
    let check_system = Button::with_label("Check system");

    let status = Label::new(Some("Ready"));
    status.set_halign(Align::Start);
    status.set_wrap(true);
    let progress = ProgressBar::new();

    let title = adw::WindowTitle::new(
        "Jumpbox Typer",
        "Type pasted or OCR text into focused remote sessions",
    );
    let title_widget = GtkBox::new(Orientation::Horizontal, 10);
    let app_icon = Image::from_file(app_icon_path());
    app_icon.set_pixel_size(28);
    title_widget.append(&app_icon);
    title_widget.append(&title);
    header_bar.set_title_widget(Some(&title_widget));

    let about_button = Button::builder()
        .icon_name("help-about-symbolic")
        .tooltip_text("About Jumpbox Typer")
        .build();
    header_bar.pack_end(&about_button);

    {
        let window = window.clone();
        about_button.connect_clicked(move |_| {
            show_about_window(&window);
        });
    }

    let settings = adw::PreferencesGroup::builder()
        .title("Typing Settings")
        .description("Tune the delay and typing pace for the target remote session")
        .build();
    settings.add(&action_row("Start Delay", "Seconds before typing begins", &delay));
    settings.add(&action_row("Typing Speed", "Characters per second", &speed));
    settings.add(&action_row(
        "Enter Pause",
        "Seconds to wait after pressing Enter",
        &enter_pause,
    ));
    settings.add(&action_row(
        "Keyboard Layout",
        "Choose the layout used for special characters",
        &keyboard_layout,
    ));

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    actions.set_hexpand(true);
    actions.append(&start);
    actions.append(&stop);
    actions.append(&clear);
    actions.append(&extract_clipboard_image);
    actions.append(&check_system);

    let actions_group = adw::PreferencesGroup::builder()
        .title("Actions")
        .description("Start typing, stop a running job, or OCR an image from the clipboard")
        .build();
    actions_group.add(&actions);

    let controls = GtkBox::new(Orientation::Vertical, 10);
    controls.append(&settings);
    controls.append(&actions_group);

    let footer = GtkBox::new(Orientation::Vertical, 6);
    footer.append(&status);
    footer.append(&progress);

    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.append(&controls);
    root.append(&editor);
    root.append(&footer);

    toolbar_view.set_content(Some(&root));
    window.set_content(Some(&toolbar_view));

    let state = Rc::new(RefCell::new(AppState {
        cancel: None,
        progress: 0,
        total: 0,
        can_type: false,
        can_ocr: false,
    }));
    let (tx, rx) = mpsc::channel::<UiEvent>();
    let latest_check: Rc<RefCell<Option<SystemCheck>>> = Rc::new(RefCell::new(None));
    let show_check_popup_on_finish = Rc::new(Cell::new(false));

    {
        let keyboard_layout = keyboard_layout.clone();
        keyboard_layout.connect_selected_notify(move |dropdown| {
            save_keyboard_layout_index(dropdown.selected());
        });
    }

    queue_system_check(tx.clone());

    {
        let state = Rc::clone(&state);
        let text_view = text_view.clone();
        let delay = delay.clone();
        let speed = speed.clone();
        let enter_pause = enter_pause.clone();
        let keyboard_layout = keyboard_layout.clone();
        let start = start.clone();
        let stop = stop.clone();
        let status = status.clone();
        let progress = progress.clone();
        let tx = tx.clone();
        let start_for_callback = start.clone();

        start.connect_clicked(move |_| {
            if !start_for_callback.is_sensitive() {
                return;
            }

            let config = match read_config(
                &text_view,
                &delay,
                &speed,
                &enter_pause,
                keyboard_layout.selected(),
            ) {
                Ok(config) => config,
                Err(message) => {
                    status.set_text(&message);
                    return;
                }
            };

            if let Err(message) = ensure_ydotool_ready() {
                status.set_text(&message);
                return;
            }

            let cancel = Arc::new(AtomicBool::new(false));
            {
                let mut state = state.borrow_mut();
                if state.cancel.is_some() {
                    return;
                }
                state.progress = 0;
                state.total = config.text.chars().count();
                state.cancel = Some(Arc::clone(&cancel));
            }

            start_for_callback.set_sensitive(false);
            stop.set_sensitive(true);
            progress.set_fraction(0.0);
            status.set_text(&format!(
                "Starting in {:.1} seconds. Focus the target window now.",
                config.delay_seconds
            ));

            let worker_tx = tx.clone();
            thread::spawn(move || run_typing(config, cancel, worker_tx));
        });
    }

    {
        let state = Rc::clone(&state);
        let status = status.clone();
        stop.connect_clicked(move |_| {
            if let Some(cancel) = &state.borrow().cancel {
                cancel.store(true, Ordering::Relaxed);
                status.set_text("Stopping...");
            }
        });
    }

    {
        let text_view = text_view.clone();
        let status = status.clone();
        clear.connect_clicked(move |_| {
            text_view.buffer().set_text("");
            status.set_text("Cleared text.");
        });
    }

    {
        let tx = tx.clone();
        let status = status.clone();
        let extract_clipboard_image_for_callback = extract_clipboard_image.clone();
        let clipboard = window.clipboard();

        extract_clipboard_image.connect_clicked(move |_| {
            if let Err(message) = require_command(
                "tesseract",
                "tesseract OCR is required: sudo apt install tesseract-ocr",
            ) {
                status.set_text(&message);
                return;
            }

            status.set_text("Reading image from clipboard...");
            extract_clipboard_image_for_callback.set_sensitive(false);

            let worker_tx = tx.clone();
            clipboard.read_texture_async(None::<&gtk::gio::Cancellable>, move |result| {
                let texture = match result {
                    Ok(Some(texture)) => texture,
                    Ok(None) => {
                        let _ = worker_tx.send(UiEvent::OcrFinished {
                            status: "Clipboard does not contain an image.".to_string(),
                            text: None,
                        });
                        return;
                    }
                    Err(err) => {
                        let _ = worker_tx.send(UiEvent::OcrFinished {
                            status: format!("Failed to read clipboard image: {err}"),
                            text: None,
                        });
                        return;
                    }
                };

                let image_path = temporary_ocr_image_path();
                if let Err(err) = texture.save_to_png(&image_path) {
                    let _ = fs::remove_file(&image_path);
                    let _ = worker_tx.send(UiEvent::OcrFinished {
                        status: format!("Failed to save clipboard image: {err}"),
                        text: None,
                    });
                    return;
                }

                thread::spawn(move || {
                    let event = match run_ocr_file(image_path) {
                        Ok(text) if text.trim().is_empty() => UiEvent::OcrFinished {
                            status: "No text found in clipboard image.".to_string(),
                            text: None,
                        },
                        Ok(text) => {
                            let character_count = text.chars().count();
                            UiEvent::OcrFinished {
                                status: format!("Inserted {character_count} OCR characters."),
                                text: Some(text),
                            }
                        }
                        Err(message) => UiEvent::OcrFinished {
                            status: message,
                            text: None,
                        },
                    };

                    let _ = worker_tx.send(event);
                });
            });
        });
    }

    {
        let tx = tx.clone();
        let status = status.clone();
        let check_system_for_callback = check_system.clone();
        let latest_check = Rc::clone(&latest_check);
        let show_check_popup_on_finish = Rc::clone(&show_check_popup_on_finish);

        check_system.connect_clicked(move |_| {
            status.set_text("Checking system requirements...");
            check_system_for_callback.set_sensitive(false);
            latest_check.borrow_mut().take();
            show_check_popup_on_finish.set(true);
            queue_system_check(tx.clone());
        });
    }

    {
        let state = Rc::clone(&state);
        let start = start.clone();
        let stop = stop.clone();
        let extract_clipboard_image = extract_clipboard_image.clone();
        let check_system = check_system.clone();
        let text_view = text_view.clone();
        let status = status.clone();
        let progress = progress.clone();
        let latest_check = Rc::clone(&latest_check);
        let window = window.clone();

        glib::timeout_add_local(Duration::from_millis(100), move || {
            while let Ok(event) = rx.try_recv() {
                match event {
                    UiEvent::Status(message) => status.set_text(&message),
                    UiEvent::Progress {
                        done,
                        total,
                        status: message,
                    } => {
                        {
                            let mut state = state.borrow_mut();
                            state.progress = done;
                            state.total = total;
                        }
                        status.set_text(&message);
                        progress.set_fraction(progress_fraction(done, total));
                    }
                    UiEvent::Finished {
                        status: message,
                        done,
                        total,
                    } => {
                        {
                            let mut state = state.borrow_mut();
                            state.cancel = None;
                            state.progress = done;
                            state.total = total;
                        }
                        status.set_text(&message);
                        progress.set_fraction(progress_fraction(done, total));
                        start.set_sensitive(state.borrow().can_type);
                        stop.set_sensitive(false);
                    }
                    UiEvent::OcrFinished {
                        status: message,
                        text,
                    } => {
                        if let Some(text) = text {
                            text_view.buffer().insert_at_cursor(&text);
                        }
                        status.set_text(&message);
                        extract_clipboard_image.set_sensitive(state.borrow().can_ocr);
                    }
                    UiEvent::SystemCheckFinished(check) => {
                        let should_show_popup = show_check_popup_on_finish.replace(false);
                        {
                            let mut state = state.borrow_mut();
                            state.can_type = check.can_type;
                            state.can_ocr = check.can_ocr;
                        }
                        latest_check.borrow_mut().replace(check.clone());
                        start.set_sensitive(check.can_type && state.borrow().cancel.is_none());
                        extract_clipboard_image.set_sensitive(check.can_ocr);
                        check_system.set_sensitive(true);
                        status.set_text(if check.can_type {
                            "Ready to type."
                        } else {
                            "Some system checks failed."
                        });
                        if should_show_popup {
                            show_system_check_popup(&window, latest_check.borrow().clone());
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    window.present();
}

fn action_row(title: &str, subtitle: &str, child: &impl IsA<gtk::Widget>) -> adw::ActionRow {
    child.set_halign(Align::End);
    child.set_valign(Align::Center);

    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .activatable_widget(child)
        .build();
    row.add_suffix(child);
    row
}

fn app_icon_path() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let candidate = dir.join("assets/jumpbox-typer.svg");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    PathBuf::from("assets/jumpbox-typer.svg")
}

fn show_about_window(parent: &ApplicationWindow) {
    let about = AboutWindow::builder()
        .transient_for(parent)
        .modal(true)
        .application_name("Jumpbox Typer")
        .application_icon("dev.sander.jumpbox_typer")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("Sander Blomvagnes")
        .comments("Jumpbox Typer makes remote-session text entry less painful across locked-down jump hosts such as AVD, Citrix, Horizon, and similar environments. It combines existing tools for keystroke injection and OCR. If a proper zero-trust setup like Boundary, Tailscale, or Twingate were already in place, this app would probably not need to exist.")
        .copyright("Copyright © 2026 Sander Blomvagnes")
        .license_type(gtk::License::MitX11)
        .website("https://github.com/SanderBlom/jumpbox-typer")
        .issue_url("https://github.com/SanderBlom/jumpbox-typer/issues")
        .developers(["Sander Blomvagnes", "sanderblom (GitHub)"])
        .build();

    about.set_translator_credits(
        "Credit to the maintainers of the underlying typing and OCR tools, and to gpt-5.4-mini for help shaping the implementation.",
    );
    about.add_link("ydotool", "https://github.com/ReimuNotMoe/ydotool");
    about.add_link("Tesseract OCR", "https://github.com/tesseract-ocr/tesseract");

    about.present();
}

fn show_system_check_popup(parent: &ApplicationWindow, check: Option<SystemCheck>) {
    let popup = gtk::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("System checks")
        .default_width(640)
        .default_height(420)
        .build();

    let body = GtkBox::new(Orientation::Vertical, 12);
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);

    let summary = Label::new(None);
    summary.set_wrap(true);
    summary.set_halign(Align::Start);
    body.append(&summary);

    let rows = GtkBox::new(Orientation::Vertical, 8);
    body.append(&rows);

    if let Some(check) = check {
        let has_failures = check.items.iter().any(|item| !item.ok);
        summary.set_text(if has_failures {
            "Fix the red items before starting."
        } else {
            "Everything looks ready."
        });
        rebuild_check_rows(&rows, &check.items);
    } else {
        summary.set_text("Running system checks...");
    }

    let close = Button::with_label("Close");
    let popup_clone = popup.clone();
    close.connect_clicked(move |_| popup_clone.close());
    body.append(&close);

    popup.set_child(Some(&body));
    popup.present();
}

fn rebuild_check_rows(group: &GtkBox, items: &[SystemCheckItem]) {
    while let Some(child) = group.first_child() {
        group.remove(&child);
    }

    for item in items {
        group.append(&system_check_row(item));
    }
}

fn system_check_row(item: &SystemCheckItem) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.add_css_class("card");
    row.set_margin_top(4);
    row.set_margin_bottom(4);
    row.set_margin_start(4);
    row.set_margin_end(4);

    let icon_name = if item.ok {
        "emblem-ok-symbolic"
    } else {
        "dialog-error-symbolic"
    };

    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(18);
    icon.add_css_class(if item.ok { "success" } else { "error" });

    let text = GtkBox::new(Orientation::Vertical, 2);
    let title = Label::new(Some(&item.title));
    title.set_halign(Align::Start);
    title.add_css_class("heading");
    let detail = Label::new(Some(&item.detail));
    detail.set_halign(Align::Start);
    detail.set_wrap(true);
    detail.set_wrap_mode(gtk::pango::WrapMode::WordChar);

    text.append(&title);
    text.append(&detail);

    let info = Button::builder()
        .icon_name("dialog-information-symbolic")
        .tooltip_text("What does this check mean?")
        .build();
    info.connect_clicked({
        let item = item.clone();
        move |_| show_check_help(&item)
    });

    row.append(&icon);
    row.append(&text);
    row.append(&info);
    row
}

fn show_check_help(item: &SystemCheckItem) {
    let dialog = gtk::Window::builder()
        .modal(true)
        .title(&item.title)
        .default_width(520)
        .default_height(220)
        .build();

    let body = GtkBox::new(Orientation::Vertical, 12);
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);

    let title = Label::new(Some(&item.title));
    title.add_css_class("heading");
    title.set_halign(Align::Start);

    let text = Label::new(Some(&item.help));
    text.set_halign(Align::Start);
    text.set_wrap(true);
    text.set_wrap_mode(gtk::pango::WrapMode::WordChar);

    let close = Button::with_label("Close");
    let dialog_clone = dialog.clone();
    close.connect_clicked(move |_| dialog_clone.close());

    body.append(&title);
    body.append(&text);
    body.append(&close);

    dialog.set_child(Some(&body));
    dialog.present();
}

fn numeric_entry(value: &str) -> Entry {
    let entry = Entry::new();
    entry.set_text(value);
    entry.set_width_chars(10);
    entry
}

fn read_config(
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
        chars_per_second: read_float(&speed.text(), "typing speed", 0.1, 120.0)?,
        enter_pause_seconds: read_float(&enter_pause.text(), "pause after Enter", 0.0, 10.0)?,
        keyboard_layout: match layout_index {
            1 => KeyboardLayout::Us,
            _ => KeyboardLayout::Norwegian,
        },
    })
}

fn load_keyboard_layout_index() -> u32 {
    let path = keyboard_layout_path();
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };

    match text.trim() {
        "US" => 1,
        _ => 0,
    }
}

fn save_keyboard_layout_index(index: u32) {
    let label = if index == 1 { "US" } else { "Norwegian" };
    let path = keyboard_layout_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(mut file) = File::create(path) {
        let _ = writeln!(file, "{label}");
    }
}

fn keyboard_layout_path() -> std::path::PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| std::path::PathBuf::from(".") )
        .join("jumpbox-typer/keyboard-layout.txt")
}

fn read_float(value: &str, label: &str, min: f64, max: f64) -> Result<f64, String> {
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
