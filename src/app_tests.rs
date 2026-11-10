use super::*;
use crate::types::SystemCheckItem;
use crate::ui::dialogs::show_check_help;
use gtk::{gdk, gio};
use std::time::Instant;

#[track_caller]
fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "GTK lifecycle did not complete at {}",
            std::panic::Location::caller()
        );
        glib::MainContext::default().iteration(false);
        thread::sleep(Duration::from_millis(1));
    }
}

fn find_widget<T: IsA<gtk::Widget>>(widget: &gtk::Widget) -> Option<T> {
    if let Ok(widget) = widget.clone().downcast::<T>() {
        return Some(widget);
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_widget(&widget) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn find_button(widget: &gtk::Widget, label: &str) -> Option<Button> {
    if let Some(button) = widget.downcast_ref::<Button>()
        && button.label().as_deref() == Some(label)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(button) = find_button(&widget, label) {
            return Some(button);
        }
        child = widget.next_sibling();
    }
    None
}

fn close_dialog(app: &Application, parent: &ApplicationWindow, session: &Session) {
    let dialog = app
        .windows()
        .into_iter()
        .find(|w| w != parent.upcast_ref::<gtk::Window>())
        .unwrap();
    until(|| app.active_window().as_ref() == Some(&dialog));
    app.activate_action("close", None);
    until(|| !dialog.is_visible());
    assert!(parent.is_visible());
    assert!(
        session.state.borrow().job.is_some(),
        "dialog Close must not cancel typing"
    );
    assert!(session.is_open());
}

// This is automated GTK/native-bridge regression coverage, not proof of real
// macOS keyboard, application menu, Dock, or Accessibility behavior.
pub(crate) fn run() {
    #[cfg(target_os = "linux")]
    if let Ok(id) = std::env::var("JUMPBOX_LIFECYCLE_REMOTE_APP_ID") {
        let remote = gio::Application::builder().application_id(id).build();
        remote.register(None::<&gio::Cancellable>).unwrap();
        assert!(remote.is_remote(), "launcher must find the retained primary");
        assert_eq!(remote.run_with_args::<&str>(&[]), glib::ExitCode::SUCCESS);
        return;
    }
    if let Err(error) = gtk::init() {
        if cfg!(target_os = "macos") {
            panic!("GTK initialization failed: {error}");
        }
        eprintln!("SKIP application shortcut/lifecycle tests: {error} (display required)");
        return;
    }
    adw::init().unwrap();
    let app = Application::builder()
        .application_id(format!(
            "app.jumpbox.typer.LifecycleTest.p{}",
            std::process::id()
        ))
        .flags(if cfg!(target_os = "macos") {
            gio::ApplicationFlags::NON_UNIQUE
        } else {
            gio::ApplicationFlags::FLAGS_NONE
        })
        .build();
    let session = Rc::new(RefCell::new(None));
    let startup_session = Rc::clone(&session);
    app.connect_startup(move |app| install_actions(app, Rc::clone(&startup_session)));
    let ran = Rc::new(Cell::new(false));
    let observed_session = Rc::new(RefCell::new(std::rc::Weak::new()));
    let observed_window = Rc::new(glib::WeakRef::new());
    let observe_session = Rc::clone(&observed_session);
    let observe_window = Rc::clone(&observed_window);
    let ran_activate = Rc::clone(&ran);
    let session_activate = Rc::clone(&session);
    app.connect_activate(move |app| {
        if ran_activate.replace(true) {
            return;
        }
        let app = app.clone();
        let session = Rc::clone(&session_activate);
        let observe_session = Rc::clone(&observe_session);
        let observe_window = Rc::clone(&observe_window);
        glib::idle_add_local_once(move || {
            observe_session.replace(Rc::downgrade(session.borrow().as_ref().unwrap()));
            observe_window.set(Some(&session.borrow().as_ref().unwrap().window));
            exercise(&app, session.borrow().as_ref().unwrap());
        });
    });
    assert_eq!(app.run_with_args::<&str>(&[]), glib::ExitCode::SUCCESS);
    assert!(ran.get());
    assert!(
        session.borrow().is_none(),
        "shutdown releases session and polling source"
    );
    assert!(
        observed_session.borrow().upgrade().is_none(),
        "no callback retains an obsolete session"
    );
    assert!(
        observed_window.upgrade().is_none(),
        "shutdown releases the retained view"
    );
    println!("application shortcut/lifecycle tests passed (no system keystrokes emitted)");
}

fn exercise(app: &Application, session: &Rc<Session>) {
    let scenario = std::env::var("JUMPBOX_LIFECYCLE_SCENARIO").unwrap();
    let modifier = if cfg!(target_os = "macos") {
        gdk::ModifierType::META_MASK
    } else {
        gdk::ModifierType::CONTROL_MASK
    };
    for (action, key) in [("app.quit", gdk::Key::q), ("app.close", gdk::Key::w)] {
        let accelerators = app.accels_for_action(action);
        assert_eq!(accelerators.len(), 1);
        assert_eq!(
            gtk::accelerator_parse(&accelerators[0]),
            Some((key, modifier))
        );
    }
    #[cfg(target_os = "macos")]
    {
        let menu = app.menubar().unwrap();
        assert_eq!(menu.n_items(), 3, "File must preserve GTK Edit and Window");
        let file = menu.item_link(0, "submenu").unwrap();
        assert_eq!(
            file.item_attribute_value(0, "action", None).unwrap().str(),
            Some("app.close")
        );
        assert_eq!(
            menu.item_attribute_value(1, "label", None).unwrap().str(),
            Some("_Edit")
        );
        assert_eq!(
            menu.item_attribute_value(2, "gtk-macos-special", None)
                .unwrap()
                .str(),
            Some("window-submenu")
        );
    }
    let window = &session.window;
    until(|| app.active_window().as_ref() == Some(window.upcast_ref()));
    let editor: TextView = find_widget(window.upcast_ref()).unwrap();
    #[cfg(target_os = "linux")]
    {
        let menu_button: gtk::MenuButton = find_widget(window.upcast_ref()).unwrap();
        let menu = menu_button.menu_model().unwrap();
        for (index, action) in [(0, "app.close"), (1, "app.quit")] {
            assert_eq!(
                menu.item_attribute_value(index, "action", None).unwrap().str(),
                Some(action)
            );
        }
    }
    assert_eq!(
        editor.buffer().char_count(),
        0,
        "a new session starts empty"
    );
    let text = "Scratchpad æøå\nUnicode 🚀\n";
    editor.buffer().set_text(text);
    for _ in 0..3 {
        app.activate();
        assert_eq!(app.windows().len(), 1);
    }
    // A controlled key emitter remains blocked at a fake key-up boundary.
    let (pressed_tx, pressed_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (stopped_tx, stopped_rx) = mpsc::channel();
    session.state.borrow_mut().job = Some(TypingJob::spawn(move |cancel| {
        pressed_tx.send(std::sync::Arc::clone(&cancel)).unwrap();
        release_rx.recv().unwrap();
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
        stopped_tx.send(()).unwrap();
    }));
    let cancellation = pressed_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    show_about_window(window);
    close_dialog(app, window, session);

    assert!(
        !cancellation.load(std::sync::atomic::Ordering::Relaxed),
        "dialog Close must not request cancellation"
    );

    if scenario == "stop-worker" {
        let stop = find_button(window.upcast_ref(), "Stop").unwrap();
        until(|| stop.is_sensitive());
        stop.emit_clicked();
        assert!(cancellation.load(std::sync::atomic::Ordering::Relaxed));
        assert!(window.is_visible() && window.is_sensitive());
        release_tx.send(()).unwrap();
        stopped_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        until(|| session.state.borrow().job.is_none());
        app.activate_action("quit", None);
        return;
    }
    show_system_check_popup(window, None);
    close_dialog(app, window, session);
    show_check_help(
        window.upcast_ref(),
        &SystemCheckItem {
            title: "Test".into(),
            detail: "Test".into(),
            help: "Test".into(),
            ok: true,
        },
    );
    close_dialog(app, window, session);

    assert!(
        !cancellation.load(std::sync::atomic::Ordering::Relaxed),
        "closing system checks/help must leave typing alone"
    );

    if scenario == "quit-worker" {
        show_about_window(window);
        let dialog = app
            .windows()
            .into_iter()
            .find(|w| w != window.upcast_ref::<gtk::Window>())
            .unwrap();
        until(|| app.active_window().as_ref() == Some(&dialog));
        dialog.activate_action("app.quit", None).unwrap();
        app.activate_action("quit", None);
        app.activate();
        assert!(window.is_visible());
        assert!(!window.is_sensitive());
        assert!(!session.state.borrow().job.as_ref().unwrap().is_finished());
        release_tx.send(()).unwrap();
        stopped_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        return;
    }

    window.present();
    until(|| app.active_window().as_ref() == Some(window.upcast_ref()));
    assert!(editor.grab_focus());
    session
        .check_popup_generation
        .set(Some(session.generation.get()));
    editor.activate_action("app.close", None).unwrap();
    app.activate_action("close", None);
    app.activate(); // cannot reopen or start another session during cleanup
    assert!(
        window.is_visible(),
        "Close waits for required release attempts"
    );
    assert!(!window.is_sensitive());
    assert_eq!(app.windows().len(), 1);
    release_tx.send(()).unwrap();
    stopped_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    until(|| !window.is_visible());
    assert!(session.state.borrow().job.is_none());
    assert!(!app.is_action_enabled("close"));
    assert!(app.is_action_enabled("quit"));
    #[cfg(target_os = "linux")]
    {
        // Launch a second process on the session bus, like the desktop entry.
        // It must present the retained primary rather than create a new session.
        let mut launcher = std::process::Command::new(std::env::current_exe().unwrap())
            .env(
                "JUMPBOX_LIFECYCLE_REMOTE_APP_ID",
                app.application_id().unwrap(),
            )
            .spawn()
            .unwrap();
        until(|| launcher.try_wait().unwrap().is_some());
        assert!(launcher.wait().unwrap().success());
        until(|| window.is_visible());
        assert_eq!(app.windows().len(), 1);
        let buffer = editor.buffer();
        assert_eq!(
            buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
            text
        );
        assert!(session.state.borrow().job.is_none());
    }
    for _ in 0..3 {
        app.activate();
        until(|| window.is_visible());
        assert_eq!(app.windows().len(), 1);
        let buffer = editor.buffer();
        assert_eq!(
            buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
            text
        );
        assert!(session.state.borrow().job.is_none());
        until(|| app.active_window().as_ref() == Some(window.upcast_ref()));
        let entry: gtk::Entry = find_widget(window.upcast_ref()).unwrap();
        assert!(entry.grab_focus());
        entry.activate_action("app.close", None).unwrap();
        until(|| !window.is_visible());
    }
    if scenario == "windowless-quit" {
        assert!(!app.windows().iter().any(|window| window.is_visible()));
        assert!(!app.is_action_enabled("close"));
        app.activate_action("quit", None);
        return;
    }
    // Exercise the exact optional delegate callback we install, without sending
    // a Dock click or pretending this proves native event dispatch.
    #[cfg(target_os = "macos")]
    {
        let native = objc2_app_kit::NSApplication::sharedApplication(
            objc2::MainThreadMarker::new().unwrap(),
        );
        let delegate = native.delegate().unwrap();
        let _: objc2::runtime::Bool = unsafe {
            objc2::msg_send![&*delegate, applicationShouldHandleReopen: &*native, hasVisibleWindows: objc2::runtime::Bool::NO]
        };
        until(|| window.is_visible());
        assert_eq!(app.windows().len(), 1);
    }
    #[cfg(target_os = "linux")]
    {
        app.activate();
        until(|| window.is_visible());
    }
    // An OCR result from a prior visible generation must not overwrite text,
    // even if it is delivered after the retained view has reopened.
    session
        .events
        .send(UiEvent::OcrFinished {
            generation: session.generation.get() - 1,
            text: Some("stale text".into()),
            status: "stale".into(),
        })
        .unwrap();
    session
        .events
        .send(UiEvent::SystemCheckFinished(SystemCheck {
            items: Vec::new(),
            can_type: false,
            can_ocr: false,
        }))
        .unwrap();
    until(|| session.background.borrow().is_empty());
    let deadline = Instant::now() + Duration::from_millis(60);
    until(|| Instant::now() >= deadline);
    let buffer = editor.buffer();
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), true),
        text
    );
    assert_eq!(
        app.windows().len(),
        1,
        "an old readiness result cannot reopen a dialog"
    );

    // Background cleanup must finish before Quit, even when Quit supersedes a
    // pending Close. Use a real private temporary artifact, never real keys.
    let image = temporary_ocr_image_path().unwrap();
    let (cleanup_tx, cleanup_rx) = mpsc::channel();
    let worker_image = image.clone();
    session.background.borrow_mut().push(thread::spawn(move || {
        cleanup_rx.recv().unwrap();
        let _ = run_ocr_file("/usr/bin/true".into(), worker_image);
    }));
    window.close();
    show_about_window(window);
    let dialog = app
        .windows()
        .into_iter()
        .find(|w| w != window.upcast_ref::<gtk::Window>())
        .unwrap();
    until(|| app.active_window().as_ref() == Some(&dialog));
    dialog.activate_action("app.quit", None).unwrap();
    app.activate_action("quit", None);
    app.activate();
    assert!(image.exists());
    assert!(window.is_visible());
    assert!(!window.is_sensitive());
    cleanup_tx.send(()).unwrap();
    until(|| !image.exists());
    // Finish is processed by the main loop after this callback returns.
}
