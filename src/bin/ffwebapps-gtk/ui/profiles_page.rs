//! Profiles management page: list profiles with create / edit / remove.
//! The nil "Default" profile can only be cleared (its apps removed), not deleted.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use firefoxpwa::directories::ProjectDirs;
use gtk::glib;
use ulid::Ulid;

use crate::core;
use crate::ui::window::Ui;

/// Rows currently shown, so we can clear them on rebuild.
type Rows = Rc<RefCell<Vec<adw::ActionRow>>>;

/// Build the profiles navigation page.
pub fn build(ui: &Rc<Ui>) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    let add_btn = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("New profile")
        .build();
    header.pack_end(&add_btn);

    let group = adw::PreferencesGroup::new();
    let page = adw::PreferencesPage::new();
    page.add(&group);

    let rows: Rows = Rc::new(RefCell::new(Vec::new()));
    populate(ui, &group, &rows);

    add_btn.connect_clicked(glib::clone!(#[strong] ui, #[weak] group, #[strong] rows, move |_| {
        present_form(&ui, &group, &rows, None);
    }));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    adw::NavigationPage::builder().title("Profiles").child(&toolbar).build()
}

/// Clear and rebuild the list of profile rows from current storage.
fn populate(ui: &Rc<Ui>, group: &adw::PreferencesGroup, rows: &Rows) {
    for row in rows.borrow_mut().drain(..) {
        group.remove(&row);
    }

    let storage = match core::load_storage(ui.dirs()) {
        Ok(storage) => storage,
        Err(error) => {
            ui.toast(&format!("Failed to load profiles: {error}"));
            return;
        }
    };

    for profile in storage.profiles.values() {
        let name = profile.name.clone().unwrap_or_else(|| "Unnamed profile".into());
        let count = profile.sites.len();
        let subtitle = match &profile.description {
            Some(description) if !description.is_empty() => format!("{description} · {count} apps"),
            _ => format!("{count} apps"),
        };

        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&name).as_str())
            .subtitle(glib::markup_escape_text(&subtitle).as_str())
            .build();

        let id = profile.ulid;
        let is_default = id == Ulid::nil();

        let edit_btn = icon_button("document-edit-symbolic", "Edit");
        edit_btn.connect_clicked(glib::clone!(#[strong] ui, #[weak] group, #[strong] rows, move |_| {
            present_form(&ui, &group, &rows, Some(id));
        }));
        row.add_suffix(&edit_btn);

        let remove_btn = icon_button("user-trash-symbolic", if is_default { "Clear" } else { "Remove" });
        remove_btn.add_css_class("error");
        remove_btn.connect_clicked(glib::clone!(#[strong] ui, #[weak] group, #[strong] rows, #[strong] name, move |_| {
            confirm_remove(&ui, &group, &rows, id, &name, is_default);
        }));
        row.add_suffix(&remove_btn);

        group.add(&row);
        rows.borrow_mut().push(row);
    }
}

/// Present the create/edit form. `edit_id = None` creates a new profile.
fn present_form(ui: &Rc<Ui>, group: &adw::PreferencesGroup, rows: &Rows, edit_id: Option<Ulid>) {
    let (title, action_label, name0, desc0) = match edit_id {
        Some(id) => {
            let (name, description) = load_profile_fields(ui.dirs(), id);
            ("Edit profile", "Save", name, description)
        }
        None => ("New profile", "Create", String::new(), String::new()),
    };

    let name_row = adw::EntryRow::builder().title("Name").build();
    name_row.set_text(&name0);
    let desc_row = adw::EntryRow::builder().title("Description").build();
    desc_row.set_text(&desc0);

    let form = adw::PreferencesGroup::new();
    form.add(&name_row);
    form.add(&desc_row);
    let page = adw::PreferencesPage::new();
    page.add(&form);

    let header = adw::HeaderBar::new();
    let cancel_btn = gtk::Button::with_label("Cancel");
    let ok_btn = gtk::Button::builder().label(action_label).css_classes(["suggested-action"]).build();
    header.pack_start(&cancel_btn);
    header.pack_end(&ok_btn);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));

    let dialog = adw::Dialog::builder().title(title).content_width(420).build();
    dialog.set_child(Some(&toolbar));

    cancel_btn.connect_clicked(glib::clone!(#[weak] dialog, move |_| {
        dialog.close();
    }));

    ok_btn.connect_clicked(glib::clone!(
        #[strong] ui, #[weak] group, #[strong] rows, #[weak] dialog,
        #[weak] name_row, #[weak] desc_row, #[weak] ok_btn,
        move |_| {
            let name = opt(&name_row.text());
            let description = opt(&desc_row.text());
            ok_btn.set_sensitive(false);
            let work = move || -> anyhow::Result<()> {
                match edit_id {
                    Some(id) => core::update_profile(id, name, description),
                    None => core::create_profile(name, description).map(|_| ()),
                }
            };
            core::spawn(work, glib::clone!(
                #[strong] ui, #[weak] group, #[strong] rows, #[weak] dialog, #[weak] ok_btn,
                move |res: anyhow::Result<()>| {
                    ok_btn.set_sensitive(true);
                    match res {
                        Ok(()) => {
                            ui.toast("Profile saved");
                            populate(&ui, &group, &rows);
                            ui.refresh_list();
                            dialog.close();
                        }
                        Err(error) => ui.toast(&format!("Failed: {error}")),
                    }
                }));
        }
    ));

    dialog.present(Some(ui.anchor()));
}

/// Confirm + perform a profile removal/clear.
fn confirm_remove(
    ui: &Rc<Ui>,
    group: &adw::PreferencesGroup,
    rows: &Rows,
    id: Ulid,
    name: &str,
    is_default: bool,
) {
    let heading = if is_default { format!("Clear “{name}”?") } else { format!("Remove “{name}”?") };
    let body = if is_default {
        "Removes all web apps and data in the Default profile. The profile itself stays."
    } else {
        "Permanently removes the profile and all of its web apps and data."
    };

    let dialog = adw::AlertDialog::new(Some(heading.as_str()), Some(body));
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("remove", if is_default { "Clear" } else { "Remove" });
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    dialog.connect_response(None, glib::clone!(#[strong] ui, #[weak] group, #[strong] rows, move |_, response| {
        if response != "remove" {
            return;
        }
        core::spawn(move || core::remove_profile(id), glib::clone!(
            #[strong] ui, #[weak] group, #[strong] rows,
            move |res: anyhow::Result<()>| {
                match res {
                    Ok(()) => {
                        ui.toast("Profile removed");
                        populate(&ui, &group, &rows);
                        ui.refresh_list();
                    }
                    Err(error) => ui.toast(&format!("Failed: {error}")),
                }
            }));
    }));

    dialog.present(Some(ui.anchor()));
}

fn load_profile_fields(dirs: &ProjectDirs, id: Ulid) -> (String, String) {
    if let Ok(storage) = core::load_storage(dirs)
        && let Some(profile) = storage.profiles.get(&id)
    {
        return (
            profile.name.clone().unwrap_or_default(),
            profile.description.clone().unwrap_or_default(),
        );
    }
    (String::new(), String::new())
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build()
}

fn opt(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() { None } else { Some(text.to_owned()) }
}
