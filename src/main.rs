use adw::prelude::*;
use adw::Application;
use jumpbox_typer::{app, types::APP_ID};

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(app::build_ui);
    app.run();
}
