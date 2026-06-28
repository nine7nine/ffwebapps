//! Per-profile CSS/JS injection editor, shown as a dialog. Edits
//! `ffwebapps.css` / `ffwebapps.js` at the profile root; the runtime reads them
//! once at startup, so changes apply on the next launch. Per-profile, not per-app.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use sourceview5::prelude::BufferExt;
use ulid::Ulid;

use crate::core;
use crate::ui::window::Ui;

/// Present the injection editor dialog for a profile.
pub fn present(ui: &Rc<Ui>, profile: Ulid, profile_name: String) {
    let (css, js) = core::read_injection(ui.dirs(), profile);

    let header = adw::HeaderBar::new();
    let save_btn = gtk::Button::builder().label("Save").css_classes(["suggested-action"]).build();
    header.pack_end(&save_btn);

    let banner = adw::Banner::builder()
        .title("Injection is shared by every web app in this profile and applies on next launch")
        .revealed(true)
        .build();

    let css_group = adw::PreferencesGroup::builder()
        .title("ffwebapps.css")
        .description("User stylesheet (CSP-immune, injected at startup)")
        .build();
    let css_view = code_view(&css, "css");
    css_group.add(&code_area(&css_view));

    let js_group = adw::PreferencesGroup::builder()
        .title("ffwebapps.js")
        .description("User script (CSP-immune, injected at startup)")
        .build();
    let js_view = code_view(&js, "js");
    js_group.add(&code_area(&js_view));

    // The editor should fill the dialog (the two code areas share the height),
    // so use a plain expanding box rather than the natural-height scroller.
    css_group.set_vexpand(true);
    js_group.set_vexpand(true);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    body.append(&css_group);
    body.append(&js_group);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_css_class("dialog-surface");
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&banner);
    toolbar.set_content(Some(&body));

    // Open at ~95% of the window in both dimensions (this editor benefits from
    // as much room as possible, unlike the smaller form dialogs).
    let (width, height) = dialog_size(ui);
    let title = format!("Injection · {profile_name}");
    let dialog = adw::Dialog::builder().title(&title).content_width(width).content_height(height).build();
    dialog.set_child(Some(&toolbar));

    save_btn.connect_clicked(glib::clone!(
        #[strong] ui, #[weak] save_btn, #[weak] css_view, #[weak] js_view,
        move |_| {
            let css = view_text(&css_view);
            let js = view_text(&js_view);
            save_btn.set_sensitive(false);
            core::spawn(
                move || {
                    let dirs = core::project_dirs()?;
                    core::write_injection(&dirs, profile, &css, &js)
                },
                glib::clone!(#[strong] ui, #[weak] save_btn, move |res: anyhow::Result<()>| {
                    save_btn.set_sensitive(true);
                    match res {
                        Ok(()) => ui.toast("Injection saved — applies on next launch"),
                        Err(error) => ui.toast(&format!("Save failed: {error}")),
                    }
                }),
            );
        }
    ));

    dialog.present(Some(ui.anchor()));
}

/// A syntax-highlighted source editor for `language_id` (`"css"` / `"js"`),
/// styled with a dark scheme and line numbers.
fn code_view(text: &str, language_id: &str) -> sourceview5::View {
    let buffer = sourceview5::Buffer::new(None);
    if let Some(language) = sourceview5::LanguageManager::default().language(language_id) {
        buffer.set_language(Some(&language));
    }
    if let Some(scheme) = dark_scheme() {
        buffer.set_style_scheme(Some(&scheme));
    }
    buffer.set_text(text);

    sourceview5::View::builder()
        .buffer(&buffer)
        .monospace(true)
        .show_line_numbers(true)
        .highlight_current_line(true)
        .auto_indent(true)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(8)
        .right_margin(8)
        .build()
}

/// The first available dark style scheme (Adwaita-dark ships with GtkSourceView
/// 5.4+; the others are long-standing fallbacks).
fn dark_scheme() -> Option<sourceview5::StyleScheme> {
    let manager = sourceview5::StyleSchemeManager::default();
    ["Adwaita-dark", "oblivion", "classic-dark", "solarized-dark"]
        .into_iter()
        .find_map(|id| manager.scheme(id))
}

fn code_area(view: &sourceview5::View) -> gtk::Frame {
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_height(180)
        .vexpand(true)
        .child(view)
        .build();
    let frame = gtk::Frame::builder().child(&scroll).vexpand(true).build();
    frame.add_css_class("text-field");
    frame
}

/// 95% of the main window's current size (falls back to its default size if the
/// window hasn't been allocated yet).
fn dialog_size(ui: &Rc<Ui>) -> (i32, i32) {
    let (mut width, mut height) = (1000, 820);
    if let Some(window) = ui.anchor().root().and_downcast::<gtk::Window>() {
        let (root_width, root_height) = (window.width(), window.height());
        if root_width > 0 && root_height > 0 {
            width = root_width;
            height = root_height;
        }
    }
    (width * 95 / 100, height * 95 / 100)
}

fn view_text(view: &sourceview5::View) -> String {
    let buffer = view.buffer();
    let (start, end) = buffer.bounds();
    buffer.text(&start, &end, false).to_string()
}
