//! Compatibility for GTK versions without applicationShouldHandleReopen.
//! GTK retains its delegate, termination policy, and all other native behavior.
use adw::prelude::*;
use gtk::glib;
use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::NSApplication;
use std::cell::RefCell;

thread_local! {
    static APPLICATION: RefCell<glib::WeakRef<adw::Application>> = RefCell::default();
}

unsafe extern "C-unwind" fn reopen(
    _delegate: &AnyObject,
    _selector: Sel,
    _application: &NSApplication,
    _has_visible_windows: Bool,
) -> Bool {
    APPLICATION.with(|application| {
        if let Some(application) = application.borrow().upgrade() {
            application.activate();
        }
    });
    Bool::YES
}

pub(crate) fn install_dock_reopen(app: &adw::Application) {
    let mtm = MainThreadMarker::new().expect("GTK startup must run on the main thread");
    let native = NSApplication::sharedApplication(mtm);
    let delegate = native
        .delegate()
        .expect("GTK installs its native delegate at startup");
    let object: &AnyObject = (*delegate).as_ref();
    let class = object.class();
    let selector = sel!(applicationShouldHandleReopen:hasVisibleWindows:);
    // Newer GTK owns this event already. Never override an implementation.
    if class.instance_method(selector).is_some() {
        return;
    }
    assert_eq!(
        class.name(),
        c"GtkApplicationQuartzDelegate",
        "unexpected native delegate"
    );
    APPLICATION.with(|application| application.borrow_mut().set(Some(app)));
    app.connect_shutdown(|_| APPLICATION.with(|application| application.borrow_mut().set(None)));

    // SAFETY: startup and AppKit delegate callbacks run on the OS main thread.
    // Add only a missing optional delegate method. Its ABI is BOOL(id, SEL,
    // NSApplication*, BOOL); BOOL is 'B' on the supported arm64 macOS baseline.
    // No delegate replacement/swizzling, retained raw app pointer, or new Quit
    // implementation is involved. class_addMethod refuses an existing method.
    let added = unsafe {
        objc2::ffi::class_addMethod(
            class as *const AnyClass as *mut AnyClass,
            selector,
            std::mem::transmute::<
                unsafe extern "C-unwind" fn(&AnyObject, Sel, &NSApplication, Bool) -> Bool,
                objc2::runtime::Imp,
            >(reopen),
            c"B@:@B".as_ptr(),
        )
    };
    assert!(
        added.as_bool(),
        "could not install GTK Dock reopen compatibility"
    );
}
