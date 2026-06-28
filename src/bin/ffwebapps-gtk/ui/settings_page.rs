//! Global settings: the `Config` runtime toggles plus extra runtime arguments
//! and environment variables. All are direct `Storage` edits and take effect on
//! the next web-app launch.

use std::collections::BTreeMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::core;
use crate::ui::window::Ui;

/// Build the settings navigation page.
pub fn build(ui: &Rc<Ui>) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    let save_btn = gtk::Button::builder().label("Save").css_classes(["suggested-action"]).build();
    header.pack_end(&save_btn);

    let config = core::load_config(ui.dirs()).unwrap_or_else(|_| empty_config());

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

    let page = adw::PreferencesPage::new();
    page.add(&runtime_group);
    page.add(&args_group);
    page.add(&vars_group);

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

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    adw::NavigationPage::builder().title("Settings").child(&toolbar).build()
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

/// Wrap a text view in a scrolled, framed area suitable for a preferences group.
fn text_area(view: &gtk::TextView) -> gtk::Frame {
    let scroll = gtk::ScrolledWindow::builder().min_content_height(120).child(view).build();
    gtk::Frame::builder().child(&scroll).build()
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
