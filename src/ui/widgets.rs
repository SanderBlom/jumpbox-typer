use crate::types::SystemCheckItem;
use adw::prelude::*;
use gtk::{Align, Box as GtkBox, Button, Image, Label, Orientation};

pub fn action_row(title: &str, subtitle: &str, child: &impl IsA<gtk::Widget>) -> adw::ActionRow {
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

pub fn numeric_entry(value: &str) -> gtk::Entry {
    let entry = gtk::Entry::new();
    entry.set_text(value);
    entry.set_width_chars(10);
    entry
}

pub fn system_check_row(
    item: &SystemCheckItem,
    on_help_clicked: impl Fn(&SystemCheckItem) + 'static,
) -> GtkBox {
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
        move |_| on_help_clicked(&item)
    });

    row.append(&icon);
    row.append(&text);
    row.append(&info);
    row
}
