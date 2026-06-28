//! Runtime page: show the installed runtime version and offer
//! install / link-system-Firefox / patch / uninstall. All operations run off
//! the main thread (install in particular downloads Firefox and is slow).

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::core;
use crate::ui::window::Ui;

/// Build the runtime navigation page.
pub fn build(ui: &Rc<Ui>) -> adw::NavigationPage {
    let status_group = adw::PreferencesGroup::new();
    let status_row = adw::ActionRow::builder().title("Runtime").build();
    status_group.add(&status_row);
    set_status(ui, &status_row);

    let actions = adw::PreferencesGroup::builder().title("Actions").build();
    let install_row = action_row("Install runtime", "Download the Firefox runtime from Mozilla");
    let link_row = action_row("Use system Firefox", "Link the installed system Firefox instead of downloading");
    let patch_row = action_row("Patch runtime", "Re-apply the ffwebapps runtime patches");
    let uninstall_row = action_row("Uninstall runtime", "Remove the downloaded runtime");
    actions.add(&install_row);
    actions.add(&link_row);
    actions.add(&patch_row);
    actions.add(&uninstall_row);

    install_row.connect_activated(glib::clone!(#[strong] ui, #[weak] status_row, move |_| {
        ui.toast("Installing runtime… this may take a while");
        run_action(&ui, &status_row, "Runtime installed", || core::install_runtime(false));
    }));
    link_row.connect_activated(glib::clone!(#[strong] ui, #[weak] status_row, move |_| {
        ui.toast("Linking system Firefox…");
        run_action(&ui, &status_row, "Linked system Firefox", || core::install_runtime(true));
    }));
    patch_row.connect_activated(glib::clone!(#[strong] ui, #[weak] status_row, move |_| {
        run_action(&ui, &status_row, "Runtime patched", core::patch_runtime);
    }));
    uninstall_row.connect_activated(glib::clone!(#[strong] ui, #[weak] status_row, move |_| {
        confirm_uninstall(&ui, &status_row);
    }));

    let page = adw::PreferencesPage::new();
    page.add(&status_group);
    page.add(&actions);

    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    adw::NavigationPage::builder().title("Runtime").child(&toolbar).build()
}

/// Run a runtime operation off-thread, then toast + refresh the status line.
fn run_action<F>(ui: &Rc<Ui>, status_row: &adw::ActionRow, success: &'static str, work: F)
where
    F: FnOnce() -> anyhow::Result<()> + Send + 'static,
{
    core::spawn(work, glib::clone!(#[strong] ui, #[strong] status_row, move |res: anyhow::Result<()>| {
        match res {
            Ok(()) => ui.toast(success),
            Err(error) => ui.toast(&format!("Failed: {error}")),
        }
        set_status(&ui, &status_row);
    }));
}

fn confirm_uninstall(ui: &Rc<Ui>, status_row: &adw::ActionRow) {
    let dialog = adw::AlertDialog::new(
        Some("Uninstall runtime?"),
        Some("Web apps can't launch until a runtime is installed again."),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("uninstall", "Uninstall");
    dialog.set_response_appearance("uninstall", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    dialog.connect_response(None, glib::clone!(#[strong] ui, #[strong] status_row, move |_, response| {
        if response != "uninstall" {
            return;
        }
        run_action(&ui, &status_row, "Runtime uninstalled", core::uninstall_runtime);
    }));

    dialog.present(Some(ui.anchor()));
}

fn set_status(ui: &Ui, status_row: &adw::ActionRow) {
    let text = match core::runtime_version(ui.dirs()) {
        Some(version) => format!("Installed · version {version}"),
        None => "Not installed".to_owned(),
    };
    status_row.set_subtitle(&text);
}

fn action_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).subtitle(subtitle).activatable(true).build();
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    row
}
