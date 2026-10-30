use crate::clipboard::copied_image_file;
use crate::ocr::{resolve_tesseract, run_ocr_file, run_ocr_source_file, temporary_ocr_image_path};
use crate::platform::{AccessRequest, prepare_typing, recheck_readiness_on_activation};
use crate::settings::{load_app_config, read_app_config, read_config, save_app_config};
use crate::system_check::queue_system_check;
use crate::types::{AppState, SystemCheck, UiEvent};
use crate::typing::{TypingJob, progress_fraction};
use crate::ui::dialogs::{show_about_window, show_system_check_popup};
use crate::ui::widgets::{action_row, numeric_entry};
use adw::prelude::*;
use adw::{Application, ApplicationWindow, HeaderBar, ToolbarView};
use gtk::glib;
use gtk::{
    Align, Box as GtkBox, Button, DropDown, Label, Orientation, ProgressBar, ScrolledWindow,
    TextView, WrapMode,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

enum OcrInput {
    SourceFile(std::path::PathBuf),
    TemporaryFile(std::path::PathBuf),
}

pub fn setup_actions(app: &Application) {
    install_actions(app, Rc::new(RefCell::new(None)));
}

fn install_actions(app: &Application, session: Rc<RefCell<Option<Rc<Session>>>>) {
    let quit_session = Rc::clone(&session);
    app.add_action_entries([gtk::gio::ActionEntry::builder("quit")
        .activate(move |app: &Application, _, _| {
            if let Some(session) = quit_session.borrow().as_ref() {
                session.request_transition(true);
            } else {
                app.quit();
            }
        })
        .build()]);

    // A global menu cannot use the window widget action directly for plain
    // GtkWindows (our dialogs). Resolve the active window, then use GTK close.
    let close = gtk::gio::SimpleAction::new("close", None);
    close.set_enabled(false);
    close.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| {
            if let Some(window) = app.active_window().filter(|w| w.is_visible()) {
                window.close();
            }
        }
    ));
    app.add_action(&close);
    app.connect_active_window_notify(glib::clone!(
        #[weak]
        close,
        move |app| {
            close.set_enabled(app.active_window().is_some_and(|w| w.is_visible()));
        }
    ));
    app.connect_window_added(glib::clone!(
        #[weak]
        close,
        move |_, window| {
            window.connect_visible_notify(glib::clone!(
                #[weak]
                close,
                move |window| {
                    if let Some(app) = window.application() {
                        close.set_enabled(app.active_window().is_some_and(|w| w.is_visible()));
                    }
                }
            ));
        }
    ));

    #[cfg(target_os = "macos")]
    {
        // Reuse GTK's installed default model rather than replacing its native
        // Edit/Window menus. The application menu (Hide/Services/Quit) is separate.
        let builder = gtk::Builder::from_resource("/org/gtk/libgtk/ui/gtkapplication-quartz.ui");
        let menu: gtk::gio::Menu = builder
            .object("default-menu")
            .expect("GTK native default menu");
        let file = gtk::gio::Menu::new();
        file.append(Some("Close"), Some("app.close"));
        menu.prepend_submenu(Some("_File"), &file);
        app.set_menubar(Some(&menu));
        crate::platform::install_dock_reopen(app);
    }

    let activate_session = Rc::clone(&session);
    app.connect_activate(move |app| {
        let existing = activate_session.borrow().clone();
        if let Some(session) = existing {
            if matches!(
                session.transition.get(),
                Transition::Open | Transition::Closed
            ) {
                session.transition.set(Transition::Open);
                session.window.set_sensitive(true);
                session.window.present();
            }
        } else {
            activate_session.borrow_mut().replace(build_ui(app));
        }
    });
    app.connect_shutdown(move |_| {
        if let Some(session) = session.borrow_mut().take() {
            if let Some(source) = session.poll.borrow_mut().take() {
                source.remove();
            }
            session.hold.borrow_mut().take();
            session.window.destroy();
        }
    });

    let (quit, close) = if cfg!(target_os = "macos") {
        ("<Meta>q", "<Meta>w")
    } else {
        ("<Control>q", "<Control>w")
    };
    app.set_accels_for_action("app.quit", &[quit]);
    app.set_accels_for_action("app.close", &[close]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transition {
    Open,
    Closing,
    Closed,
    Quitting,
}

struct Session {
    window: ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    transition: Cell<Transition>,
    generation: Cell<u64>,
    poll: RefCell<Option<glib::SourceId>>,
    hold: RefCell<Option<gtk::gio::ApplicationHoldGuard>>,
    background: RefCell<Vec<thread::JoinHandle<()>>>,
    clipboard: RefCell<Option<gtk::gio::Cancellable>>,
    check_pending: Cell<bool>,
    ocr_pending: Cell<bool>,
    check_popup_generation: Cell<Option<u64>>,
    events: mpsc::Sender<UiEvent>,
}

impl Session {
    fn is_open(&self) -> bool {
        self.transition.get() == Transition::Open
    }

    fn request_transition(&self, quit: bool) {
        if self.transition.get() == Transition::Quitting {
            return;
        }
        if quit || self.is_open() {
            self.transition.set(if quit {
                Transition::Quitting
            } else {
                Transition::Closing
            });
            self.generation.set(self.generation.get() + 1);
            self.window.set_sensitive(false);
            self.ocr_pending.set(false);
            if let Some(cancel) = self.clipboard.borrow_mut().take() {
                cancel.cancel();
            }
            if let Some(job) = &self.state.borrow().job {
                job.cancel();
            }
        }
    }

    fn check(&self, request: AccessRequest) {
        if self.is_open() && !self.check_pending.replace(true) {
            self.background
                .borrow_mut()
                .push(queue_system_check(self.events.clone(), request));
        }
    }

    fn reap(&self) {
        let finished = self
            .state
            .borrow()
            .job
            .as_ref()
            .is_some_and(TypingJob::is_finished);
        if finished {
            // Finished events can arrive just before the thread returns. Only
            // joining the finished handle proves the key emitter has stopped.
            if self
                .state
                .borrow_mut()
                .job
                .take()
                .unwrap()
                .finish()
                .is_err()
            {
                eprintln!("Typing worker stopped unexpectedly.");
            }
        }
        let mut background = self.background.borrow_mut();
        let mut index = 0;
        while index < background.len() {
            if background[index].is_finished() {
                if background.swap_remove(index).join().is_err() {
                    eprintln!("Background worker stopped unexpectedly.");
                }
            } else {
                index += 1;
            }
        }
    }

    fn finish_transition(&self) -> bool {
        if self.state.borrow().job.is_some() || !self.background.borrow().is_empty() {
            return false;
        }
        match self.transition.get() {
            Transition::Closing => {
                if let Some(app) = self.window.application() {
                    for window in app.windows() {
                        if window != self.window.upcast_ref::<gtk::Window>().clone() {
                            window.destroy();
                        }
                    }
                    if cfg!(target_os = "macos") {
                        self.window.set_visible(false);
                        self.transition.set(Transition::Closed);
                    } else {
                        self.window.destroy();
                        app.quit();
                        return true;
                    }
                }
            }
            Transition::Quitting => {
                self.hold.borrow_mut().take();
                if let Some(app) = self.window.application() {
                    app.quit();
                }
                return true;
            }
            _ => {}
        }
        false
    }
}

fn queue_ocr(
    tx: mpsc::Sender<UiEvent>,
    tesseract_path: std::path::PathBuf,
    input: OcrInput,
    generation: u64,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let result = match input {
            OcrInput::SourceFile(path) => run_ocr_source_file(tesseract_path, path),
            OcrInput::TemporaryFile(path) => run_ocr_file(tesseract_path, path),
        };
        let event = match result {
            Ok(text) if text.trim().is_empty() => UiEvent::OcrFinished {
                generation,
                status: "No text found in clipboard image.".to_string(),
                text: None,
            },
            Ok(text) => {
                let character_count = text.chars().count();
                UiEvent::OcrFinished {
                    generation,
                    status: format!("Inserted {character_count} OCR characters."),
                    text: Some(text),
                }
            }
            Err(message) => UiEvent::OcrFinished {
                generation,
                status: message,
                text: None,
            },
        };

        let _ = tx.send(event);
    })
}

fn build_ui(app: &Application) -> Rc<Session> {
    let stored_config = load_app_config();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Jumpbox Typer")
        .default_width(920)
        .default_height(660)
        .build();
    window.set_icon_name(Some(crate::types::APP_ID));

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
        about_button.connect_clicked(glib::clone!(
            #[weak]
            window,
            move |_| {
                show_about_window(&window);
            }
        ));
    }

    let settings = adw::PreferencesGroup::builder()
        .title("Typing Settings")
        .description("Tune the delay and typing pace for the target remote session")
        .build();
    settings.add(&action_row(
        "Start Delay",
        "Seconds before typing begins",
        &delay,
    ));
    settings.add(&action_row("Typing Speed", "Characters per second", &speed));
    settings.add(&action_row(
        "Enter Pause",
        "Seconds to wait after pressing Enter",
        &enter_pause,
    ));
    settings.add(&action_row(
        "Keyboard Layout",
        "Match this layout to the active system input source",
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
        job: None,
        progress: 0,
        total: 0,
        can_type: false,
        can_ocr: false,
    }));
    let (tx, rx) = mpsc::channel::<UiEvent>();
    let session = Rc::new(Session {
        window: window.clone(),
        state: Rc::clone(&state),
        transition: Cell::new(Transition::Open),
        generation: Cell::new(0),
        poll: RefCell::new(None),
        hold: RefCell::new(if cfg!(target_os = "macos") {
            Some(app.hold())
        } else {
            None
        }),
        background: RefCell::new(Vec::new()),
        clipboard: RefCell::new(None),
        check_pending: Cell::new(false),
        ocr_pending: Cell::new(false),
        check_popup_generation: Cell::new(None),
        events: tx.clone(),
    });
    window.connect_close_request(glib::clone!(
        #[weak]
        session,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_| {
            session.request_transition(false);
            glib::Propagation::Stop
        }
    ));
    let latest_check: Rc<RefCell<Option<SystemCheck>>> = Rc::new(RefCell::new(None));

    {
        let delay_for_signal = delay.clone();
        let delay = delay.clone();
        let speed = speed.clone();
        let enter_pause = enter_pause.clone();
        let keyboard_layout = keyboard_layout.clone();

        delay_for_signal.connect_changed(move |_| {
            if let Ok(config) =
                read_app_config(&delay, &speed, &enter_pause, keyboard_layout.selected())
            {
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
            if let Ok(config) =
                read_app_config(&delay, &speed, &enter_pause, keyboard_layout.selected())
            {
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
            if let Ok(config) =
                read_app_config(&delay, &speed, &enter_pause, keyboard_layout.selected())
            {
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

    if recheck_readiness_on_activation() {
        app.connect_active_window_notify(glib::clone!(
            #[weak]
            session,
            move |app| {
                if app.active_window().is_some() {
                    session.check(AccessRequest::CheckOnly);
                }
            }
        ));
    }

    session.check(AccessRequest::CheckOnly);

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

        start.connect_clicked(glib::clone!(
            #[weak]
            session,
            move |_| {
                if !session.is_open() || !start_for_callback.is_sensitive() {
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

                start_for_callback.set_sensitive(false);
                if let Err(message) = prepare_typing() {
                    state.borrow_mut().can_type = false;
                    status.set_text(&message);
                    return;
                }

                {
                    let mut state = state.borrow_mut();
                    if state.job.is_some() {
                        return;
                    }
                    state.progress = 0;
                    state.total = config.text.chars().count();
                }

                stop.set_sensitive(true);
                progress.set_fraction(0.0);
                status.set_text(&format!(
                    "Starting in {:.1} seconds. Focus the target window now.",
                    config.delay_seconds
                ));

                state.borrow_mut().job = Some(TypingJob::start(config, tx.clone()));
            }
        ));
    }

    {
        let state = Rc::clone(&state);
        let status = status.clone();
        stop.connect_clicked(move |_| {
            if let Some(job) = &state.borrow().job {
                job.cancel();
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

        extract_clipboard_image.connect_clicked(glib::clone!(
            #[weak]
            session,
            move |_| {
                if !session.is_open() || session.ocr_pending.get() {
                    return;
                }
                let generation = session.generation.get();
                let tesseract_path = match resolve_tesseract() {
                    Ok(path) => path,
                    Err(message) => {
                        status.set_text(&message);
                        return;
                    }
                };

                status.set_text("Reading image from clipboard...");
                session.ocr_pending.set(true);
                extract_clipboard_image_for_callback.set_sensitive(false);

                let worker_tx = tx.clone();
                match copied_image_file() {
                    Ok(Some(image_path)) => {
                        session.background.borrow_mut().push(queue_ocr(
                            worker_tx,
                            tesseract_path,
                            OcrInput::SourceFile(image_path),
                            generation,
                        ));
                        return;
                    }
                    Ok(None) => {}
                    Err(message) => {
                        let _ = worker_tx.send(UiEvent::OcrFinished {
                            generation,
                            status: message,
                            text: None,
                        });
                        return;
                    }
                }

                let cancellable = gtk::gio::Cancellable::new();
                session.clipboard.borrow_mut().replace(cancellable.clone());
                let session = Rc::downgrade(&session);
                clipboard.read_texture_async(Some(&cancellable), move |result| {
                    let Some(session) = session.upgrade() else {
                        return;
                    };
                    if !session.is_open() || session.generation.get() != generation {
                        return;
                    }
                    session.clipboard.borrow_mut().take();
                    let texture = match result {
                        Ok(Some(texture)) => texture,
                        Ok(None) => {
                            let _ = worker_tx.send(UiEvent::OcrFinished {
                                generation,
                                status: "Clipboard does not contain an image.".to_string(),
                                text: None,
                            });
                            return;
                        }
                        Err(err) => {
                            let _ = worker_tx.send(UiEvent::OcrFinished {
                                generation,
                                status: format!("Failed to read clipboard image: {err}"),
                                text: None,
                            });
                            return;
                        }
                    };

                    let image_path = match temporary_ocr_image_path() {
                        Ok(path) => path,
                        Err(message) => {
                            let _ = worker_tx.send(UiEvent::OcrFinished {
                                generation,
                                status: message,
                                text: None,
                            });
                            return;
                        }
                    };
                    if let Err(err) = texture.save_to_png(&image_path) {
                        let _ = std::fs::remove_file(&image_path);
                        let _ = worker_tx.send(UiEvent::OcrFinished {
                            generation,
                            status: format!("Failed to save clipboard image: {err}"),
                            text: None,
                        });
                        return;
                    }

                    session.background.borrow_mut().push(queue_ocr(
                        worker_tx,
                        tesseract_path,
                        OcrInput::TemporaryFile(image_path),
                        generation,
                    ));
                });
            }
        ));
    }

    {
        let status = status.clone();
        let check_system_for_callback = check_system.clone();
        let latest_check = Rc::clone(&latest_check);

        check_system.connect_clicked(glib::clone!(
            #[weak]
            session,
            move |_| {
                if !session.is_open() || session.check_pending.get() {
                    return;
                }
                status.set_text("Checking system requirements...");
                check_system_for_callback.set_sensitive(false);
                latest_check.borrow_mut().take();
                session
                    .check_popup_generation
                    .set(Some(session.generation.get()));
                session.check(AccessRequest::RequestIfNeeded);
            }
        ));
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

        let source = glib::timeout_add_local(
            Duration::from_millis(25),
            glib::clone!(
                #[weak]
                session,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    // Reap before draining: all messages from a joined worker
                    // must be consumed before Start can be enabled for a new job.
                    session.reap();
                    while let Ok(event) = rx.try_recv() {
                        // Readiness state may outlive a visible view, but no background
                        // callback may modify text or present a dialog after Close.
                        if matches!(event, UiEvent::SystemCheckFinished(_)) {
                            session.check_pending.set(false);
                        }
                        if !session.is_open() {
                            session.check_popup_generation.set(None);
                            continue;
                        }
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
                                    state.progress = done;
                                    state.total = total;
                                }
                                status.set_text(&message);
                                progress.set_fraction(progress_fraction(done, total));
                            }
                            UiEvent::OcrFinished {
                                generation,
                                status: message,
                                text,
                            } => {
                                if generation != session.generation.get() {
                                    continue;
                                }
                                session.ocr_pending.set(false);
                                if let Some(text) = text {
                                    text_view.buffer().insert_at_cursor(&text);
                                }
                                status.set_text(&message);
                                extract_clipboard_image.set_sensitive(state.borrow().can_ocr);
                            }
                            UiEvent::SystemCheckFinished(check) => {
                                let should_show_popup = session.check_popup_generation.take()
                                    == Some(session.generation.get());
                                {
                                    let mut state = state.borrow_mut();
                                    state.can_type = check.can_type;
                                    state.can_ocr = check.can_ocr;
                                }
                                latest_check.borrow_mut().replace(check.clone());
                                start.set_sensitive(check.can_type && state.borrow().job.is_none());
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
                    start.set_sensitive(
                        session.is_open()
                            && state.borrow().can_type
                            && state.borrow().job.is_none(),
                    );
                    stop.set_sensitive(session.is_open() && state.borrow().job.is_some());
                    extract_clipboard_image.set_sensitive(
                        session.is_open() && state.borrow().can_ocr && !session.ocr_pending.get(),
                    );
                    check_system.set_sensitive(session.is_open() && !session.check_pending.get());
                    if session.finish_transition() {
                        session.poll.borrow_mut().take();
                        return glib::ControlFlow::Break;
                    }
                    glib::ControlFlow::Continue
                }
            ),
        );
        session.poll.borrow_mut().replace(source);
    }

    window.present();
    session
}

#[cfg(test)]
#[allow(dead_code)] // Invoked by the OS-main-thread integration harness.
#[path = "app_tests.rs"]
pub(crate) mod tests;
