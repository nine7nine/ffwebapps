//! Main window.
//!
//! Layout: `AdwApplicationWindow` → `AdwToastOverlay` → `AdwToolbarView` with a
//! **static** header (app name + window controls only), a centred row of
//! switcher-style toggle buttons as a second bar, and an `AdwCarousel` of the
//! four sections (Web Apps / Profiles / Runtime / Settings) so the pages can be
//! swiped with a touchpad. `AdwViewSwitcher` can only drive an `AdwViewStack`,
//! never a carousel, so the tab buttons are hand-built and kept in two-way sync
//! with the carousel's page. Per-section actions live in the body; detail
//! editors (per-app, injection) open as dialogs. `Ui` (in an `Rc`) owns the
//! shared widgets and the app-list refresh.

use std::cell::{Cell, RefCell};
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
    /// Per-row "● running" indicators, polled so apps launched (or closed) while
    /// the window is open update live without a manual refresh.
    running_rows: RefCell<Vec<(Ulid, gtk::Label)>>,
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
        running_rows: RefCell::new(Vec::new()),
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

    // The pages live in an AdwCarousel (its swipe tracker gives finger-tracked
    // touchpad swipes; it only claims the horizontal axis, so each page's list
    // still scrolls vertically). Order matches the tab buttons below.
    let carousel = adw::Carousel::new();
    carousel.set_vexpand(true);

    let page_widgets: Vec<gtk::Widget> = vec![
        apps_scroll.upcast(),
        profiles_content.upcast(),
        runtime_content.upcast(),
        settings_content.upcast(),
    ];
    for page in &page_widgets {
        carousel.append(page);
    }

    // Hand-built switcher: a centred row of toggle buttons (one group, so exactly
    // one is active) styled to match the old AdwViewSwitcher. `AdwViewSwitcher`
    // can't drive a carousel, hence the manual version.
    const TABS: [(&str, &str); 4] = [
        ("applications-internet-symbolic", "Web Apps"),
        ("system-users-symbolic", "Profiles"),
        ("system-run-symbolic", "Runtime"),
        ("preferences-system-symbolic", "Settings"),
    ];
    let tabs = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .halign(gtk::Align::Center)
        .margin_top(8)
        .margin_bottom(6)
        .build();
    tabs.add_css_class("swipe-tabs");

    let buttons: Vec<gtk::ToggleButton> = TABS
        .iter()
        .map(|(icon, title)| {
            let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            inner.set_halign(gtk::Align::Center);
            inner.append(&gtk::Image::from_icon_name(icon));
            inner.append(&gtk::Label::new(Some(title)));
            gtk::ToggleButton::builder().child(&inner).build()
        })
        .collect();
    // Chain every button into the first one's group (radio behaviour).
    for pair in buttons.windows(2) {
        pair[1].set_group(Some(&pair[0]));
    }

    // A guard so the two sync directions (button → carousel, carousel → button)
    // don't feed back into each other.
    let syncing = Rc::new(Cell::new(false));

    // Mark the first tab active before wiring handlers, so the carousel's
    // initial page (0) and the tab bar agree without firing a scroll.
    buttons[0].set_active(true);

    for (index, button) in buttons.iter().enumerate() {
        tabs.append(button);
        let page = page_widgets[index].clone();
        button.connect_toggled(glib::clone!(
            #[weak] carousel,
            #[strong] syncing,
            move |button| {
                if button.is_active() && !syncing.get() {
                    carousel.scroll_to(&page, true);
                }
            }
        ));
    }

    // Swiping (or the animation after a tab click) settles on a page → light up
    // its tab. The guard keeps `set_active` from bouncing back into a scroll.
    let sync_buttons = buttons.clone();
    let sync_flag = syncing.clone();
    carousel.connect_page_changed(move |_, index| {
        sync_flag.set(true);
        if let Some(button) = sync_buttons.get(index as usize) {
            button.set_active(true);
        }
        sync_flag.set(false);
    });

    toasts.set_child(Some(&carousel));
    toasts.set_vexpand(true);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&tabs);
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

    // Poll each visible web app's runtime socket so the "● running" indicator
    // tracks apps launched or closed while the window stays open. Cheap (a local
    // socket connect per app); a weak ref lets the timer stop when the window is
    // gone.
    let weak = Rc::downgrade(&ui);
    glib::timeout_add_seconds_local(2, move || match weak.upgrade() {
        Some(ui) => {
            ui.poll_running();
            glib::ControlFlow::Continue
        }
        None => glib::ControlFlow::Break,
    });
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

    /// Refresh the live "● running" indicators against each app's runtime socket.
    fn poll_running(&self) {
        for (id, label) in self.running_rows.borrow().iter() {
            label.set_visible(crate::ipc::is_running(*id));
        }
    }

    /// Rebuild the profile-grouped app list (kept below the Install row).
    pub fn refresh_list(self: &Rc<Self>) {
        for group in self.apps_groups.borrow_mut().drain(..) {
            self.apps_body.remove(&group);
        }
        self.running_rows.borrow_mut().clear();

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

        // Always present; visibility tracks the live poll (see `poll_running`).
        let running = gtk::Label::new(Some("● running"));
        running.add_css_class("success");
        running.add_css_class("caption");
        running.set_visible(crate::ipc::is_running(site.ulid));
        row.add_suffix(&running);
        self.running_rows.borrow_mut().push((site.ulid, running));

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
