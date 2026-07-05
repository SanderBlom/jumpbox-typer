use crate::ocr::{run_ocr_file, temporary_ocr_image_path};
use crate::settings::{load_app_config, read_app_config, read_config, save_app_config};
use crate::system_check::{ensure_ydotool_ready, queue_system_check, require_command};
use crate::ui::dialogs::{show_about_window, show_system_check_popup};
use crate::ui::widgets::{action_row, numeric_entry};
use crate::types::{AppState, SystemCheck, UiEvent};
use crate::typing::{progress_fraction, run_typing};
use adw::prelude::*;
use adw::{Application, ApplicationWindow, HeaderBar, ToolbarView};
use gtk::glib;
use gtk::{
    Align, Box as GtkBox, Button, DropDown, Label, Orientation, ProgressBar, ScrolledWindow,
    TextView, WrapMode,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub fn build_ui(app: &Application) {
    let stored_config = load_app_config();

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

    let delay = numeric_entry(&stored_config.delay_seconds.to_string());
    let speed = numeric_entry(&stored_config.chars_per_second.to_string());
    let enter_pause = numeric_entry(&stored_config.enter_pause_seconds.to_string());
    let keyboard_layout = DropDown::from_strings(&["Norwegian", "US"]);
    keyboard_layout.set_selected(stored_config.keyboard_layout_index());

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
    header_bar.set_title_widget(Some(&title));

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
        let delay_for_signal = delay.clone();
        let delay = delay.clone();
        let speed = speed.clone();
        let enter_pause = enter_pause.clone();
        let keyboard_layout = keyboard_layout.clone();

        delay_for_signal.connect_changed(move |_| {
            if let Ok(config) = read_app_config(&delay, &speed, &enter_pause, keyboard_layout.selected()) {
                save_app_config(&config);
            }
        });
    }

    {
        let speed_for_signal = speed.clone();
        let delay = delay.clone();
        let speed = speed.clone();
        let enter_pause = enter_pause.clone();
        let keyboard_layout = keyboard_layout.clone();

        speed_for_signal.connect_changed(move |_| {
            if let Ok(config) = read_app_config(&delay, &speed, &enter_pause, keyboard_layout.selected()) {
                save_app_config(&config);
            }
        });
    }

    {
        let enter_pause_for_signal = enter_pause.clone();
        let delay = delay.clone();
        let speed = speed.clone();
        let enter_pause = enter_pause.clone();
        let keyboard_layout = keyboard_layout.clone();

        enter_pause_for_signal.connect_changed(move |_| {
            if let Ok(config) = read_app_config(&delay, &speed, &enter_pause, keyboard_layout.selected()) {
                save_app_config(&config);
            }
        });
    }

    {
        let keyboard_layout_for_signal = keyboard_layout.clone();
        let delay = delay.clone();
        let speed = speed.clone();
        let enter_pause = enter_pause.clone();

        keyboard_layout_for_signal.connect_selected_notify(move |dropdown| {
            if let Ok(config) = read_app_config(&delay, &speed, &enter_pause, dropdown.selected()) {
                save_app_config(&config);
            }
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
                    let _ = std::fs::remove_file(&image_path);
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
