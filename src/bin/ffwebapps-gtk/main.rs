//! ffwebapps-gtk — GTK4/libadwaita management GUI for ffwebapps.
//!
//! Lives inside the `firefoxpwa` crate (behind the `gui` feature) so it can call
//! the library in-process: `Storage::load`/`write` for reads and direct edits,
//! and the existing command structs for mutations. The core structs are
//! `#[non_exhaustive]`, so this same-crate placement is required.
//!
//! P0: a read-only window listing installed web apps grouped by profile.

mod core;
mod ui;

use adw::prelude::*;
use gtk::glib;

/// Reverse-DNS application id (matches the GitHub repo `nine7nine/ffwebapps`).
/// Reused later for the `.desktop` launcher and single-instance behaviour.
const APP_ID: &str = "io.github.nine7nine.ffwebapps";

fn main() -> glib::ExitCode {
    init_logging();

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(ui::window::build);
    app.run()
}

/// Route the `log` crate (the core command structs log through it) to a file in
/// the user data dir; otherwise those messages would vanish. A later phase will
/// also surface them in a status pane.
fn init_logging() {
    use simplelog::{ConfigBuilder, LevelFilter, WriteLogger};

    let Some(path) = core::project_dirs().ok().map(|d| d.userdata.join("ffwebapps-gui.log"))
    else {
        return;
    };

    if let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = WriteLogger::init(LevelFilter::Info, ConfigBuilder::new().build(), file);
    }
}
