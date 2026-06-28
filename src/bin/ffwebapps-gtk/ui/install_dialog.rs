//! Install-a-web-app dialog: manifest URL + target profile + a few options,
//! confirmed into `SiteInstallCommand._run()` off the main thread.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use ulid::Ulid;

use crate::core;
use crate::ui::window::Ui;

/// Present the install dialog.
pub fn present(ui: &Rc<Ui>) {
    let profiles = core::list_profiles(ui.dirs()).unwrap_or_default();
    let profile_ids: Vec<Ulid> = profiles.iter().map(|(id, _)| *id).collect();
    // Existing profiles first (Default stays the selected default), then a
    // trailing "New profile…" entry that reveals the name field below.
    let new_profile_index = profile_ids.len() as u32;
    let mut profile_names: Vec<&str> = profiles.iter().map(|(_, name)| name.as_str()).collect();
    profile_names.push("New profile…");

    let manifest_row = adw::EntryRow::builder().title("Manifest URL").build();
    let doc_row = adw::EntryRow::builder().title("Document URL (optional)").build();
    let name_row = adw::EntryRow::builder().title("Name (optional)").build();

    let profile_combo = adw::ComboRow::builder().title("Profile").build();
    profile_combo.set_model(Some(&gtk::StringList::new(&profile_names)));

    // Name for the to-be-created profile; only shown when "New profile…" is the
    // selected entry.
    let new_profile_row = adw::EntryRow::builder().title("New profile name").visible(false).build();
    profile_combo.connect_selected_notify(glib::clone!(#[weak] new_profile_row, move |combo| {
        new_profile_row.set_visible(combo.selected() == new_profile_index);
    }));

    let general = adw::PreferencesGroup::new();
    general.add(&manifest_row);
    general.add(&doc_row);
    general.add(&name_row);
    general.add(&profile_combo);
    general.add(&new_profile_row);

    let options = adw::PreferencesGroup::builder().title("Options").build();
    let login_sw = switch("Launch on login", false);
    let browser_sw = switch("Launch on browser launch", false);
    let hw_sw = switch("Force hardware WebRTC", false);
    let sw_sw = switch("Software rendering", false);
    options.add(&login_sw);
    options.add(&browser_sw);
    options.add(&hw_sw);
    options.add(&sw_sw);

    let page = adw::PreferencesPage::new();
    page.add(&general);
    page.add(&options);

    let header = adw::HeaderBar::new();
    let cancel_btn = gtk::Button::with_label("Cancel");
    let install_btn = gtk::Button::builder().label("Install").css_classes(["suggested-action"]).build();
    header.pack_start(&cancel_btn);
    header.pack_end(&install_btn);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_css_class("dialog-surface");
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));

    let dialog = adw::Dialog::builder()
        .title("Install web app")
        .content_width(520)
        .content_height(560)
        .build();
    dialog.set_child(Some(&toolbar));

    cancel_btn.connect_clicked(glib::clone!(#[weak] dialog, move |_| {
        dialog.close();
    }));

    install_btn.connect_clicked(glib::clone!(
        #[strong] ui,
        #[weak] dialog,
        #[weak] manifest_row, #[weak] doc_row, #[weak] name_row, #[weak] profile_combo, #[weak] new_profile_row,
        #[weak] login_sw, #[weak] browser_sw, #[weak] hw_sw, #[weak] sw_sw,
        #[weak] install_btn, #[weak] cancel_btn,
        move |_| {
            let manifest = manifest_row.text().trim().to_owned();
            if manifest.is_empty() {
                ui.toast("Manifest URL is required");
                return;
            }

            let selected = profile_combo.selected();
            // Trailing entry = create a new profile; require a name for it.
            let (profile, new_profile_name) = if selected == new_profile_index {
                let new_name = new_profile_row.text().trim().to_owned();
                if new_name.is_empty() {
                    ui.toast("New profile name is required");
                    return;
                }
                (None, Some(new_name))
            } else {
                (profile_ids.get(selected as usize).copied(), None)
            };
            let params = core::InstallParams {
                manifest_url: manifest,
                document_url: opt(&doc_row.text()),
                profile,
                new_profile_name,
                name: opt(&name_row.text()),
                launch_on_login: login_sw.is_active(),
                launch_on_browser: browser_sw.is_active(),
                hardware_webrtc: hw_sw.is_active(),
                software_rendering: sw_sw.is_active(),
            };

            install_btn.set_sensitive(false);
            cancel_btn.set_sensitive(false);
            ui.toast("Installing… fetching the manifest");
            core::spawn(
                move || core::install_site(params),
                glib::clone!(#[strong] ui, #[weak] dialog, #[weak] install_btn, #[weak] cancel_btn,
                    move |res: anyhow::Result<Ulid>| {
                        install_btn.set_sensitive(true);
                        cancel_btn.set_sensitive(true);
                        match res {
                            Ok(_) => {
                                ui.toast("Web app installed");
                                ui.refresh_list();
                                dialog.close();
                            }
                            Err(error) => ui.toast(&format!("Install failed: {error}")),
                        }
                    }),
            );
        }
    ));

    dialog.present(Some(ui.anchor()));
}

fn switch(title: &str, active: bool) -> adw::SwitchRow {
    adw::SwitchRow::builder().title(title).active(active).build()
}

fn opt(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() { None } else { Some(text.to_owned()) }
}
