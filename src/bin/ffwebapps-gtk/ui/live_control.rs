//! Live control panel for the per-app editor: connects to a running web app's
//! IPC socket, shows running/window/unread state, and sends verbs
//! (show/hide/reload/quit/copy-url/open-browser) plus toggles
//! (mute/dnd/suspend/autostart). Toggle switches reflect the runtime's state;
//! an `applying` guard stops the inbound state update from echoing a verb back.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use firefoxpwa::components::site::Site;
use gtk::glib;

use crate::ipc::{self, LiveConn, LiveEvent};

/// Shared handle to the live connection (None when the app isn't running).
pub type Conn = Rc<RefCell<Option<LiveConn>>>;

/// Build the "Live control" group and connect to the app if it's running.
/// Returns the group plus the connection handle so the caller can close it when
/// the editor page goes away.
pub fn build(site: &Site) -> (adw::PreferencesGroup, Conn) {
    let id = site.ulid;
    let group = adw::PreferencesGroup::builder().title("Live control").build();

    let status_row = adw::ActionRow::builder().title("Status").subtitle("Not running").build();
    let show_btn = action_btn("Show");
    let hide_btn = action_btn("Hide");
    status_row.add_suffix(&show_btn);
    status_row.add_suffix(&hide_btn);

    let unread_row = adw::ActionRow::builder().title("Unread").subtitle("0").build();

    let window_row = adw::ActionRow::builder().title("Window").build();
    let reload_btn = action_btn("Reload");
    let quit_btn = action_btn("Quit");
    quit_btn.add_css_class("destructive-action");
    window_row.add_suffix(&reload_btn);
    window_row.add_suffix(&quit_btn);

    let page_row = adw::ActionRow::builder().title("Current page").build();
    let copy_btn = action_btn("Copy URL");
    let open_btn = action_btn("Open in browser");
    page_row.add_suffix(&copy_btn);
    page_row.add_suffix(&open_btn);

    let mute_sw = switch("Mute");
    let dnd_sw = switch("Do not disturb");
    let suspend_sw = switch("Suspend when hidden");
    let autostart_sw = switch("Start on login");

    group.add(&status_row);
    group.add(&unread_row);
    group.add(&window_row);
    group.add(&page_row);
    group.add(&mute_sw);
    group.add(&dnd_sw);
    group.add(&suspend_sw);
    group.add(&autostart_sw);

    let conn: Conn = Rc::new(RefCell::new(None));
    let applying = Rc::new(Cell::new(false));

    wire_verb(&show_btn, &conn, "show");
    wire_verb(&hide_btn, &conn, "hide");
    wire_verb(&reload_btn, &conn, "reload");
    wire_verb(&quit_btn, &conn, "quit");
    wire_verb(&copy_btn, &conn, "copy-url");
    wire_verb(&open_btn, &conn, "open-browser");

    wire_toggle(&mute_sw, &conn, &applying, "mute-toggle");
    wire_toggle(&dnd_sw, &conn, &applying, "dnd-toggle");
    wire_toggle(&suspend_sw, &conn, &applying, "suspend-toggle");
    wire_toggle(&autostart_sw, &conn, &applying, "autostart-toggle");

    // Interactive controls are disabled until we know the app is running.
    let controls: Rc<Vec<gtk::Widget>> = Rc::new(vec![
        show_btn.upcast(),
        hide_btn.upcast(),
        reload_btn.upcast(),
        quit_btn.upcast(),
        copy_btn.upcast(),
        open_btn.upcast(),
        mute_sw.clone().upcast(),
        dnd_sw.clone().upcast(),
        suspend_sw.clone().upcast(),
        autostart_sw.clone().upcast(),
    ]);
    set_enabled(&controls, false);

    let on_event = glib::clone!(
        #[strong] applying, #[strong] controls,
        #[strong] status_row, #[strong] unread_row,
        #[strong] mute_sw, #[strong] dnd_sw, #[strong] suspend_sw, #[strong] autostart_sw,
        move |event: LiveEvent| match event {
            LiveEvent::Hello(pid) => status_row.set_subtitle(&format!("Running · pid {pid}")),
            LiveEvent::Unread(count) => unread_row.set_subtitle(&count.to_string()),
            LiveEvent::State(flags) => {
                applying.set(true);
                mute_sw.set_active(flags.muted);
                dnd_sw.set_active(flags.dnd);
                suspend_sw.set_active(flags.suspend);
                autostart_sw.set_active(flags.autostart);
                applying.set(false);
                status_row.set_subtitle(if flags.hidden {
                    "Running · window hidden"
                } else {
                    "Running · window shown"
                });
            }
            LiveEvent::Disconnected => {
                status_row.set_subtitle("Not running");
                set_enabled(&controls, false);
            }
            LiveEvent::Other => {}
        }
    );

    if let Some(live) = ipc::connect_live(id, on_event) {
        *conn.borrow_mut() = Some(live);
        status_row.set_subtitle("Running");
        set_enabled(&controls, true);
    }

    (group, conn)
}

fn set_enabled(controls: &[gtk::Widget], enabled: bool) {
    for widget in controls {
        widget.set_sensitive(enabled);
    }
}

fn wire_verb(button: &gtk::Button, conn: &Conn, verb: &'static str) {
    button.connect_clicked(glib::clone!(#[strong] conn, move |_| {
        if let Some(live) = conn.borrow().as_ref() {
            live.send(verb);
        }
    }));
}

fn wire_toggle(switch: &adw::SwitchRow, conn: &Conn, applying: &Rc<Cell<bool>>, verb: &'static str) {
    switch.connect_active_notify(glib::clone!(#[strong] conn, #[strong] applying, move |_| {
        if applying.get() {
            return; // inbound state update, not a user action
        }
        if let Some(live) = conn.borrow().as_ref() {
            live.send(verb);
        }
    }));
}

fn action_btn(label: &str) -> gtk::Button {
    gtk::Button::builder().label(label).valign(gtk::Align::Center).css_classes(["flat"]).build()
}

fn switch(title: &str) -> adw::SwitchRow {
    adw::SwitchRow::builder().title(title).build()
}
