//! Per-profile CSS/JS injection editor, shown as a dialog. Edits
//! `ffwebapps.css` / `ffwebapps.js` at the profile root; the runtime reads them
//! once at startup, so changes apply on the next launch. Per-profile, not per-app.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use ulid::Ulid;

use crate::core;
use crate::ui::widgets;
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
    let css_view = code_view(&css);
    css_group.add(&code_area(&css_view));

    let js_group = adw::PreferencesGroup::builder()
        .title("ffwebapps.js")
        .description("User script (CSP-immune, injected at startup)")
        .build();
    let js_view = code_view(&js);
    js_group.add(&code_area(&js_view));

    let (scroll, body) = widgets::content();
    body.append(&css_group);
    body.append(&js_group);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&banner);
    toolbar.set_content(Some(&scroll));

    let title = format!("Injection · {profile_name}");
    let dialog = adw::Dialog::builder().title(&title).content_width(720).content_height(640).build();
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

fn code_view(text: &str) -> gtk::TextView {
    let view = gtk::TextView::builder()
        .monospace(true)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(8)
        .right_margin(8)
        .build();
    view.buffer().set_text(text);
    view
}

fn code_area(view: &gtk::TextView) -> gtk::Frame {
    let scroll = gtk::ScrolledWindow::builder().min_content_height(200).child(view).build();
    gtk::Frame::builder().child(&scroll).build()
}

fn view_text(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    let (start, end) = buffer.bounds();
    buffer.text(&start, &end, false).to_string()
}
