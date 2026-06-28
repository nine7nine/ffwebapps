//! Main window and navigation controller.
//!
//! Layout: `AdwApplicationWindow` → `AdwToastOverlay` → `AdwNavigationView`.
//! The root page lists web apps grouped by profile; activating a row pushes the
//! per-app editor. `Ui` (held in an `Rc`) owns the shared widgets and the
//! list-refresh / editor-navigation logic.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use firefoxpwa::components::site::Site;
use firefoxpwa::directories::ProjectDirs;
use gtk::glib;

use crate::core;
use crate::ui::app_editor;

pub struct Ui {
    dirs: ProjectDirs,
    toasts: adw::ToastOverlay,
    nav: adw::NavigationView,
    list_page: adw::PreferencesPage,
    list_groups: RefCell<Vec<adw::PreferencesGroup>>,
}

/// Build and present the main window. Wired to `Application::activate`.
pub fn build(app: &adw::Application) {
    let dirs = match core::project_dirs() {
        Ok(dirs) => dirs,
        Err(error) => return present_fatal(app, &error.to_string()),
    };

    let nav = adw::NavigationView::new();
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&nav));

    let list_page = adw::PreferencesPage::new();

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("ffwebapps", "Web App Manager")));
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&list_page));
    let root = adw::NavigationPage::builder().title("Web Apps").child(&toolbar).build();
    nav.push(&root);

    let ui = Rc::new(Ui {
        dirs,
        toasts: toasts.clone(),
        nav: nav.clone(),
        list_page,
        list_groups: RefCell::new(Vec::new()),
    });
    ui.refresh_list();

    // Refresh the list when returning to it (e.g. after a rename in the editor).
    nav.connect_popped(glib::clone!(#[strong] ui, move |_, _| ui.refresh_list()));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("ffwebapps")
        .default_width(860)
        .default_height(760)
        .content(&toasts)
        .build();
    window.present();
}

impl Ui {
    /// Show a transient toast.
    pub fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    /// Rebuild the profile-grouped app list from current storage.
    pub fn refresh_list(self: &Rc<Self>) {
        for group in self.list_groups.borrow_mut().drain(..) {
            self.list_page.remove(&group);
        }

        let storage = match core::load_storage(&self.dirs) {
            Ok(storage) => storage,
            Err(error) => {
                let group = adw::PreferencesGroup::builder().title("Failed to load web apps").build();
                group.set_description(Some(&error.to_string()));
                self.list_page.add(&group);
                self.list_groups.borrow_mut().push(group);
                return;
            }
        };

        let mut groups = self.list_groups.borrow_mut();
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

            self.list_page.add(&group);
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
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));

        let site = site.clone();
        row.connect_activated(glib::clone!(#[strong(rename_to = ui)] self, move |_| {
            ui.open_editor(site.clone());
        }));
        row
    }

    fn open_editor(self: &Rc<Self>, site: Site) {
        self.nav.push(&app_editor::build(self, site));
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
