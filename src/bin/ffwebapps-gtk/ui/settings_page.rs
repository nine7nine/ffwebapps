//! Global settings tab: the `Config` runtime toggles plus extra runtime
//! arguments and environment variables. Direct `Storage` edits; take effect on
//! the next web-app launch. Save lives in the body (the window CSD is static).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::core;
use crate::ui::widgets;
use crate::ui::window::Ui;

/// Build the settings tab content.
pub fn build_content(ui: &Rc<Ui>) -> gtk::ScrolledWindow {
    let config = core::load_config(ui.dirs()).unwrap_or_else(|_| empty_config());

    let appearance_group = build_appearance_group();

    let runtime_group = adw::PreferencesGroup::builder()
        .title("Runtime")
        .description("Takes effect on next launch")
        .build();
    let wayland_sw = switch("Enable Wayland", "Sets MOZ_ENABLE_WAYLAND=1", config.runtime_enable_wayland);
    let xinput_sw = switch("Use XInput2", "Sets MOZ_USE_XINPUT2=1", config.runtime_use_xinput2);
    let portals_sw = switch("Use XDG portals", "Sets GTK_USE_PORTAL=1", config.runtime_use_portals);
    let linked_sw = switch("Use system runtime", "Use the system Firefox instead of a downloaded runtime (experimental)", config.use_linked_runtime);
    let patch_sw = switch("Always patch", "Re-patch the runtime and profile on every launch", config.always_patch);
    runtime_group.add(&wayland_sw);
    runtime_group.add(&xinput_sw);
    runtime_group.add(&portals_sw);
    runtime_group.add(&linked_sw);
    runtime_group.add(&patch_sw);

    let args_group = adw::PreferencesGroup::builder()
        .title("Extra runtime arguments")
        .description("One argument per line, passed to every runtime launch")
        .build();
    let args_view = text_view(&config.arguments.join("\n"), false);
    args_group.add(&text_area(&args_view));

    let vars_group = adw::PreferencesGroup::builder()
        .title("Extra environment variables")
        .description("One KEY=VALUE per line")
        .build();
    let vars_view = text_view(&variables_to_text(&config.variables), true);
    vars_group.add(&text_area(&vars_view));

    let save_btn = gtk::Button::builder()
        .label("Save settings")
        .css_classes(["suggested-action", "pill"])
        .halign(gtk::Align::Center)
        .build();

    save_btn.connect_clicked(glib::clone!(
        #[strong] ui, #[weak] save_btn,
        #[weak] wayland_sw, #[weak] xinput_sw, #[weak] portals_sw, #[weak] linked_sw, #[weak] patch_sw,
        #[weak] args_view, #[weak] vars_view,
        move |_| {
            let edits = core::ConfigEdits {
                always_patch: patch_sw.is_active(),
                runtime_enable_wayland: wayland_sw.is_active(),
                runtime_use_xinput2: xinput_sw.is_active(),
                runtime_use_portals: portals_sw.is_active(),
                use_linked_runtime: linked_sw.is_active(),
                arguments: text_to_arguments(&view_text(&args_view)),
                variables: text_to_variables(&view_text(&vars_view)),
            };
            save_btn.set_sensitive(false);
            core::spawn(
                move || core::save_config(edits),
                glib::clone!(#[strong] ui, #[weak] save_btn, move |res: anyhow::Result<()>| {
                    save_btn.set_sensitive(true);
                    match res {
                        Ok(()) => ui.toast("Settings saved"),
                        Err(error) => ui.toast(&format!("Save failed: {error}")),
                    }
                }),
            );
        }
    ));

    let (scroll, body) = widgets::content();
    body.append(&appearance_group);
    body.append(&runtime_group);
    body.append(&args_group);
    body.append(&vars_group);
    body.append(&save_btn);
    scroll
}

/// Appearance controls (window opacity / glass tint / accent) that apply live
/// and self-persist — like the Poxicle configurator's Preferences page.
fn build_appearance_group() -> adw::PreferencesGroup {
    let appearance = Rc::new(RefCell::new(core::load_appearance()));
    let group = adw::PreferencesGroup::builder()
        .title("Appearance")
        .description("Applies live")
        .build();

    let opacity_row = adw::SpinRow::with_range(40.0, 100.0, 1.0);
    opacity_row.set_title("Window opacity");
    opacity_row.set_value(f64::from(appearance.borrow().opacity));
    opacity_row.connect_value_notify(glib::clone!(#[strong] appearance, move |row| {
        appearance.borrow_mut().opacity = row.value().round() as u8;
        push_appearance(&appearance);
    }));
    group.add(&opacity_row);

    group.add(&color_row("Glass color", &appearance.borrow().glass, &appearance, |a, hex| {
        a.glass = hex;
    }));
    group.add(&color_row("Accent color", &appearance.borrow().accent, &appearance, |a, hex| {
        a.accent = hex;
    }));

    group
}

/// A row with a colour-picker suffix that writes its hex into the appearance via
/// `set`, then applies + persists.
fn color_row(
    title: &str,
    initial: &str,
    appearance: &Rc<RefCell<core::Appearance>>,
    set: impl Fn(&mut core::Appearance, String) + 'static,
) -> adw::ActionRow {
    let button = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
    button.set_valign(gtk::Align::Center);
    if let Ok(rgba) = gtk::gdk::RGBA::parse(initial) {
        button.set_rgba(&rgba);
    }
    button.connect_rgba_notify(glib::clone!(#[strong] appearance, move |button| {
        set(&mut appearance.borrow_mut(), rgba_to_hex(&button.rgba()));
        push_appearance(&appearance);
    }));

    let row = adw::ActionRow::builder().title(title).build();
    row.add_suffix(&button);
    row
}

fn push_appearance(appearance: &Rc<RefCell<core::Appearance>>) {
    let appearance = appearance.borrow();
    widgets::apply_appearance(&appearance);
    core::save_appearance(&appearance);
}

fn rgba_to_hex(color: &gtk::gdk::RGBA) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (color.red() * 255.0 + 0.5) as u8,
        (color.green() * 255.0 + 0.5) as u8,
        (color.blue() * 255.0 + 0.5) as u8,
    )
}

fn switch(title: &str, subtitle: &str, active: bool) -> adw::SwitchRow {
    adw::SwitchRow::builder().title(title).subtitle(subtitle).active(active).build()
}

fn text_view(text: &str, monospace: bool) -> gtk::TextView {
    let view = gtk::TextView::builder()
        .monospace(monospace)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(8)
        .right_margin(8)
        .build();
    view.buffer().set_text(text);
    view
}

fn text_area(view: &gtk::TextView) -> gtk::Frame {
    let scroll = gtk::ScrolledWindow::builder().min_content_height(88).child(view).build();
    let frame = gtk::Frame::builder().child(&scroll).build();
    frame.add_css_class("text-field");
    frame
}

fn view_text(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    let (start, end) = buffer.bounds();
    buffer.text(&start, &end, false).to_string()
}

fn variables_to_text(variables: &BTreeMap<String, String>) -> String {
    variables.iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join("\n")
}

fn text_to_variables(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

fn text_to_arguments(text: &str) -> Vec<String> {
    text.lines().map(|line| line.trim().to_owned()).filter(|line| !line.is_empty()).collect()
}

fn empty_config() -> core::ConfigEdits {
    core::ConfigEdits {
        always_patch: false,
        runtime_enable_wayland: false,
        runtime_use_xinput2: false,
        runtime_use_portals: false,
        use_linked_runtime: false,
        arguments: Vec::new(),
        variables: BTreeMap::new(),
    }
}
