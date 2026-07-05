use crate::types::{SystemCheck, SystemCheckItem};
use crate::ui::widgets::system_check_row;
use adw::{AboutWindow, ApplicationWindow};
use adw::prelude::*;
use gtk::{Align, Box as GtkBox, Button, Label, Orientation};

pub fn show_about_window(parent: &ApplicationWindow) {
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
        .developers(["GPT-5.4 model"])
        .build();

    about.add_credit_section(
        Some("Acknowledgements"),
        &["Maintainers of ydotool and Tesseract OCR"],
    );
    about.add_link("ydotool", "https://github.com/ReimuNotMoe/ydotool");
    about.add_link("Tesseract OCR", "https://github.com/tesseract-ocr/tesseract");

    about.present();
}

pub fn show_system_check_popup(parent: &ApplicationWindow, check: Option<SystemCheck>) {
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

pub fn show_check_help(item: &SystemCheckItem) {
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

fn rebuild_check_rows(group: &GtkBox, items: &[SystemCheckItem]) {
    while let Some(child) = group.first_child() {
        group.remove(&child);
    }

    for item in items {
        group.append(&system_check_row(item, show_check_help));
    }
}
