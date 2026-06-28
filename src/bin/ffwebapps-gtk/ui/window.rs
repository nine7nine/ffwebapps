//! Main window.
//!
//! Layout: `AdwApplicationWindow` → `AdwToastOverlay` → `AdwToolbarView` with a
//! **static** header (app name + window controls only), an `AdwViewSwitcher`
//! (tabs) as a second bar, and an `AdwViewStack` of the four sections
//! (Web Apps / Profiles / Runtime / Settings). Per-section actions live in the
//! body; detail editors (per-app, injection) open as dialogs. `Ui` (in an `Rc`)
//! owns the shared widgets and the app-list refresh.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use firefoxpwa::components::site::Site;
use firefoxpwa::directories::ProjectDirs;
use gtk::glib;
use ulid::Ulid;

use crate::core;
use crate::ui::{app_editor, injection_page, install_dialog, profiles_page, runtime_page, settings_page, widgets};

pub struct Ui {
    dirs: ProjectDirs,
    toasts: adw::ToastOverlay,
    apps_body: gtk::Box,
    apps_groups: RefCell<Vec<adw::PreferencesGroup>>,
}

/// Build and present the main window. Wired to `Application::activate`.
pub fn build(app: &adw::Application) {
    widgets::apply_app_style();
    widgets::apply_appearance(&core::load_appearance());

    let dirs = match core::project_dirs() {
        Ok(dirs) => dirs,
        Err(error) => return present_fatal(app, &error.to_string()),
    };

    let toasts = adw::ToastOverlay::new();
    let (apps_scroll, apps_body) = widgets::content();

    let ui = Rc::new(Ui {
        dirs,
        toasts: toasts.clone(),
        apps_body: apps_body.clone(),
        apps_groups: RefCell::new(Vec::new()),
    });

    // Install action at the top of the Web Apps tab.
    let install_group = adw::PreferencesGroup::new();
    let install_row = adw::ActionRow::builder()
        .title("Install web app")
        .subtitle("Add a site as a web app")
        .activatable(true)
        .build();
    install_row.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
    install_group.add(&install_row);
    apps_body.append(&install_group);
    install_row.connect_activated(glib::clone!(#[strong] ui, move |_| ui.open_install()));

    ui.refresh_list();

    // The other three tabs.
    let profiles_content = profiles_page::build_content(&ui);
    let runtime_content = runtime_page::build_content(&ui);
    let settings_content = settings_page::build_content(&ui);

    let stack = adw::ViewStack::new();
    stack.add_titled_with_icon(&apps_scroll, Some("apps"), "Web Apps", "applications-internet-symbolic");
    stack.add_titled_with_icon(&profiles_content, Some("profiles"), "Profiles", "system-users-symbolic");
    stack.add_titled_with_icon(&runtime_content, Some("runtime"), "Runtime", "system-run-symbolic");
    stack.add_titled_with_icon(&settings_content, Some("settings"), "Settings", "preferences-system-symbolic");

    // Centred tab switcher in the body (not the window decoration).
    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .halign(gtk::Align::Center)
        .margin_top(8)
        .margin_bottom(6)
        .build();

    toasts.set_child(Some(&stack));
    toasts.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&switcher);
    content.append(&toasts);

    // Static header: centred app name + window controls only.
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("ffwebapps", "")));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("ffwebapps")
        .default_width(1000)
        .default_height(820)
        .content(&toolbar)
        .build();
    window.present();
}

impl Ui {
    /// The resolved project directories.
    pub fn dirs(&self) -> &ProjectDirs {
        &self.dirs
    }

    /// A widget inside the window, used as the anchor for `AdwDialog::present`.
    pub fn anchor(&self) -> &adw::ToastOverlay {
        &self.toasts
    }

    /// Show a transient toast.
    pub fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    /// Rebuild the profile-grouped app list (kept below the Install row).
    pub fn refresh_list(self: &Rc<Self>) {
        for group in self.apps_groups.borrow_mut().drain(..) {
            self.apps_body.remove(&group);
        }

        let storage = match core::load_storage(&self.dirs) {
            Ok(storage) => storage,
            Err(error) => {
                let group = adw::PreferencesGroup::builder().title("Failed to load web apps").build();
                group.set_description(Some(&error.to_string()));
                self.apps_body.append(&group);
                self.apps_groups.borrow_mut().push(group);
                return;
            }
        };

        let mut groups = self.apps_groups.borrow_mut();
        for profile in storage.profiles.values() {
            let group = adw::PreferencesGroup::new();
            let title = profile.name.clone().unwrap_or_else(|| "Unnamed profile".into());
            let subtitle = app_count_label(profile.sites.len());
            group.set_title(&title);
            group.set_description(Some(&subtitle));

            for site_id in &profile.sites {
                if let Some(site) = storage.sites.get(site_id) {
                    group.add(&self.site_row(site));
                }
            }

            self.apps_body.append(&group);
            groups.push(group);
        }
    }

    /// An activatable row that opens the editor for `site`.
    fn site_row(self: &Rc<Self>, site: &Site) -> adw::ActionRow {
        let name = glib::markup_escape_text(&core::site_display_name(site));
        let domain = glib::markup_escape_text(&core::site_domain(site));
        let row = adw::ActionRow::builder()
            .title(name.as_str())
            .subtitle(domain.as_str())
            .activatable(true)
            .build();

        let image = match core::site_icon_path(&self.dirs, site) {
            Some(path) => gtk::Image::from_file(path),
            None => gtk::Image::from_icon_name("application-x-executable-symbolic"),
        };
        image.set_pixel_size(32);
        row.add_prefix(&image);

        if crate::ipc::is_running(site.ulid) {
            let running = gtk::Label::new(Some("● running"));
            running.add_css_class("success");
            running.add_css_class("caption");
            row.add_suffix(&running);
        }

        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));

        let site = site.clone();
        row.connect_activated(glib::clone!(#[strong(rename_to = ui)] self, move |_| {
            ui.open_editor(site.clone());
        }));
        row
    }

    fn open_install(self: &Rc<Self>) {
        install_dialog::present(self);
    }

    fn open_editor(self: &Rc<Self>, site: Site) {
        app_editor::present(self, site);
    }

    /// Open the per-profile CSS/JS injection editor (called from the profiles tab).
    pub fn open_injection(self: &Rc<Self>, profile: Ulid, name: String) {
        injection_page::present(self, profile, name);
    }
}

/// Fallback window when we can't even resolve the data directories.
fn present_fatal(app: &adw::Application, message: &str) {
    let status = adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Cannot start ffwebapps")
        .description(message)
        .build();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("ffwebapps")
        .default_width(480)
        .default_height(220)
        .content(&status)
        .build();
    window.present();
}

fn app_count_label(count: usize) -> String {
    match count {
        0 => "No web apps".to_owned(),
        1 => "1 web app".to_owned(),
        n => format!("{n} web apps"),
    }
}
