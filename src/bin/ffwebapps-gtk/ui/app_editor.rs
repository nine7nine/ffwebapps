//! Per-app editor: an `AdwPreferencesPage` over `SiteConfig`, with Save and
//! Launch in the header. Save gathers widget state into a `core::SiteEdits`
//! (encoding the library's update semantics) and runs it off the main thread.

use std::rc::Rc;

use adw::prelude::*;
use firefoxpwa::components::site::Site;
use gtk::glib;

use crate::core;
use crate::ui::live_control;
use crate::ui::window::Ui;

/// Build the editor navigation page for `site`.
pub fn build(ui: &Rc<Ui>, site: Site) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    let launch_btn = gtk::Button::with_label("Launch");
    let uninstall_btn = gtk::Button::builder()
        .label("Uninstall")
        .css_classes(["destructive-action"])
        .build();
    let save_btn = gtk::Button::builder()
        .label("Save")
        .css_classes(["suggested-action"])
        .build();
    header.pack_start(&launch_btn);
    header.pack_start(&uninstall_btn);
    header.pack_end(&save_btn);

    let page = adw::PreferencesPage::new();

    // --- Live control (connects if the app is running) ---------------------
    let (live_group, live_conn) = live_control::build(&site);
    page.add(&live_group);

    let cfg = &site.config;

    // --- General -----------------------------------------------------------
    let general = group("General", Some("Overrides for the web app manifest. Leave blank to use the manifest value."));
    let name_row = entry_row("Name", opt_str(&cfg.name));
    let desc_row = entry_row("Description", opt_str(&cfg.description));
    let start_row = entry_row("Start URL", opt_url(&cfg.start_url));
    let icon_row = entry_row("Icon URL", opt_url(&cfg.icon_url));
    general.add(&name_row);
    general.add(&desc_row);
    general.add(&start_row);
    general.add(&icon_row);
    page.add(&general);

    // --- Behaviour ---------------------------------------------------------
    let behaviour = group("Behaviour", None);
    let login_sw = switch_row("Launch on login", Some("Autostart this web app when you log in"), cfg.launch_on_login);
    let hidden_sw = switch_row("Start hidden in tray", Some("Only affects the autostart entry"), cfg.start_hidden);
    let browser_sw = switch_row("Launch on browser launch", None, cfg.launch_on_browser);
    let ext_sw = switch_row("Open external links in browser", Some("Out-of-scope links open in your default browser"), cfg.external_links.unwrap_or(true));
    behaviour.add(&login_sw);
    behaviour.add(&hidden_sw);
    behaviour.add(&browser_sw);
    behaviour.add(&ext_sw);
    page.add(&behaviour);

    // --- Performance -------------------------------------------------------
    let performance = group("Performance", None);
    let hw_sw = switch_row("Force hardware WebRTC", Some("Maximise hardware video decode for calls (needs working VA-API)"), cfg.hardware_webrtc);
    let sw_sw = switch_row("Software rendering", Some("Disable GPU acceleration; overrides hardware WebRTC"), cfg.software_rendering);
    let sched_model = gtk::StringList::new(&[
        "Default",
        "Nice (lower priority)",
        "Round-robin (realtime)",
        "FIFO (realtime)",
        "Batch",
        "Idle",
    ]);
    let sched_combo = adw::ComboRow::builder().title("Scheduling policy").model(&sched_model).build();
    let (sched_idx, sched_prio) = parse_sched(cfg.scheduling.as_deref());
    sched_combo.set_selected(sched_idx);
    let prio_spin = adw::SpinRow::with_range(-20.0, 99.0, 1.0);
    prio_spin.set_title("Priority / nice value");
    prio_spin.set_subtitle("Used by nice / realtime policies");
    prio_spin.set_value(sched_prio);
    performance.add(&hw_sw);
    performance.add(&sw_sw);
    performance.add(&sched_combo);
    performance.add(&prio_spin);
    page.add(&performance);

    // --- Advanced ----------------------------------------------------------
    let advanced = group("Advanced", None);
    let ua_row = entry_row("Custom User-Agent", opt_str(&cfg.user_agent));
    let domains_row = entry_row("Allowed domains", &cfg.allowed_domains.join(", "));
    domains_row.set_tooltip_text(Some("Comma-separated; stay in-app even out of scope (wildcards allowed)"));
    let cats_row = entry_row("Categories", &opt_vec(&cfg.categories));
    let keys_row = entry_row("Keywords", &opt_vec(&cfg.keywords));
    let urlh_row = entry_row("Enabled URL handlers", &cfg.enabled_url_handlers.join(", "));
    let proth_row = entry_row("Enabled protocol handlers", &cfg.enabled_protocol_handlers.join(", "));
    advanced.add(&ua_row);
    advanced.add(&domains_row);
    advanced.add(&cats_row);
    advanced.add(&keys_row);
    advanced.add(&urlh_row);
    advanced.add(&proth_row);
    page.add(&advanced);

    // --- Update on save ----------------------------------------------------
    let update = group("Update on save", Some("Re-fetch from the network when saving (off = offline save)"));
    let upd_manifest_sw = switch_row("Update manifest", None, false);
    let upd_icons_sw = switch_row("Update icons", None, false);
    update.add(&upd_manifest_sw);
    update.add(&upd_icons_sw);
    page.add(&update);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));

    let page_title = core::site_display_name(&site);
    let nav_page = adw::NavigationPage::builder()
        .title(&page_title)
        .child(&toolbar)
        .build();

    // --- Save --------------------------------------------------------------
    save_btn.connect_clicked(glib::clone!(
        #[strong] ui,
        #[strong] site,
        #[weak] save_btn,
        #[weak] launch_btn,
        #[weak] name_row, #[weak] desc_row, #[weak] start_row, #[weak] icon_row,
        #[weak] login_sw, #[weak] hidden_sw, #[weak] browser_sw, #[weak] ext_sw,
        #[weak] hw_sw, #[weak] sw_sw, #[weak] sched_combo, #[weak] prio_spin,
        #[weak] ua_row, #[weak] domains_row, #[weak] cats_row, #[weak] keys_row,
        #[weak] urlh_row, #[weak] proth_row, #[weak] upd_manifest_sw, #[weak] upd_icons_sw,
        move |_| {
            let cfg = &site.config;
            let edits = core::SiteEdits {
                id: site.ulid,
                name: core::opt_opt_string(cfg.name.as_deref(), &name_row.text()),
                description: core::opt_opt_string(cfg.description.as_deref(), &desc_row.text()),
                start_url: core::opt_opt_string(cfg.start_url.as_ref().map(|u| u.as_str()), &start_row.text()),
                icon_url: core::opt_opt_string(cfg.icon_url.as_ref().map(|u| u.as_str()), &icon_row.text()),
                categories: core::vec_opt_update(&cfg.categories, &cats_row.text()),
                keywords: core::vec_opt_update(&cfg.keywords, &keys_row.text()),
                enabled_url_handlers: core::vec_plain_update(&cfg.enabled_url_handlers, &urlh_row.text()),
                enabled_protocol_handlers: core::vec_plain_update(&cfg.enabled_protocol_handlers, &proth_row.text()),
                launch_on_login: login_sw.is_active(),
                launch_on_browser: browser_sw.is_active(),
                hardware_webrtc: hw_sw.is_active(),
                software_rendering: sw_sw.is_active(),
                start_hidden: hidden_sw.is_active(),
                scheduling: core::opt_opt_string(cfg.scheduling.as_deref(), &compose_sched(sched_combo.selected(), prio_spin.value())),
                user_agent: core::opt_opt_string(cfg.user_agent.as_deref(), &ua_row.text()),
                update_manifest: upd_manifest_sw.is_active(),
                update_icons: upd_icons_sw.is_active(),
                external_links: core::external_links_value(cfg.external_links, ext_sw.is_active()),
                allowed_domains: core::parse_csv(&domains_row.text()),
            };

            save_btn.set_sensitive(false);
            launch_btn.set_sensitive(false);
            core::spawn(
                move || core::save_site(edits),
                glib::clone!(#[strong] ui, #[weak] save_btn, #[weak] launch_btn, move |res: anyhow::Result<()>| {
                    save_btn.set_sensitive(true);
                    launch_btn.set_sensitive(true);
                    match res {
                        Ok(()) => { ui.toast("Saved"); ui.refresh_list(); }
                        Err(error) => ui.toast(&format!("Save failed: {error}")),
                    }
                }),
            );
        }
    ));

    // --- Launch ------------------------------------------------------------
    let id = site.ulid;
    launch_btn.connect_clicked(glib::clone!(
        #[strong] ui,
        #[weak] launch_btn,
        move |_| {
            launch_btn.set_sensitive(false);
            core::spawn(
                move || core::launch_site(id),
                glib::clone!(#[strong] ui, #[weak] launch_btn, move |res: anyhow::Result<()>| {
                    launch_btn.set_sensitive(true);
                    match res {
                        Ok(()) => ui.toast("Launched"),
                        Err(error) => ui.toast(&format!("Launch failed: {error}")),
                    }
                }),
            );
        }
    ));

    // --- Uninstall ---------------------------------------------------------
    let uninstall_name = core::site_display_name(&site);
    uninstall_btn.connect_clicked(glib::clone!(
        #[strong] ui,
        #[strong(rename_to = name)] uninstall_name,
        move |_| {
            let heading = format!("Uninstall {name}?");
            let dialog = adw::AlertDialog::new(
                Some(heading.as_str()),
                Some("This removes the web app and its system integration. Profile data is kept."),
            );
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("uninstall", "Uninstall");
            dialog.set_response_appearance("uninstall", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            dialog.connect_response(None, glib::clone!(#[strong] ui, move |_, response| {
                if response != "uninstall" {
                    return;
                }
                core::spawn(
                    move || core::uninstall_site(id),
                    glib::clone!(#[strong] ui, move |res: anyhow::Result<()>| {
                        match res {
                            Ok(()) => {
                                ui.toast("Web app uninstalled");
                                ui.go_back();
                            }
                            Err(error) => ui.toast(&format!("Uninstall failed: {error}")),
                        }
                    }),
                );
            }));
            dialog.present(Some(ui.anchor()));
        }
    ));

    // Tear down the live connection when this editor page goes away.
    nav_page.connect_hidden(glib::clone!(#[strong] live_conn, move |_| {
        if let Some(live) = live_conn.borrow_mut().take() {
            live.close();
        }
    }));

    nav_page
}

// --- small builders --------------------------------------------------------

fn group(title: &str, description: Option<&str>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).build();
    if let Some(description) = description {
        group.set_description(Some(description));
    }
    group
}

fn entry_row(title: &str, text: &str) -> adw::EntryRow {
    let row = adw::EntryRow::builder().title(title).build();
    row.set_text(text);
    row
}

fn switch_row(title: &str, subtitle: Option<&str>, active: bool) -> adw::SwitchRow {
    let builder = adw::SwitchRow::builder().title(title).active(active);
    let builder = match subtitle {
        Some(subtitle) => builder.subtitle(subtitle),
        None => builder,
    };
    builder.build()
}

// --- value formatting / scheduling ----------------------------------------

fn opt_str(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("")
}

fn opt_url(value: &Option<url::Url>) -> &str {
    value.as_ref().map(|u| u.as_str()).unwrap_or("")
}

fn opt_vec(value: &Option<Vec<String>>) -> String {
    value.as_ref().map(|v| v.join(", ")).unwrap_or_default()
}

/// Parse a scheduling spec into (combo index, priority) for the editor.
fn parse_sched(spec: Option<&str>) -> (u32, f64) {
    match spec {
        Some(s) if s.starts_with("nice:") => (1, s[5..].trim().parse().unwrap_or(0.0)),
        Some(s) if s.starts_with("rr:") => (2, s[3..].trim().parse().unwrap_or(1.0)),
        Some(s) if s.starts_with("fifo:") => (3, s[5..].trim().parse().unwrap_or(1.0)),
        Some("batch") => (4, 0.0),
        Some("idle") => (5, 0.0),
        _ => (0, 0.0),
    }
}

/// Compose a scheduling spec string from the editor's combo index + priority.
/// Empty string = "no policy" (the diff helper turns it into leave/clear).
fn compose_sched(index: u32, priority: f64) -> String {
    let priority = priority.round() as i64;
    match index {
        1 => format!("nice:{priority}"),
        2 => format!("rr:{priority}"),
        3 => format!("fifo:{priority}"),
        4 => "batch".to_owned(),
        5 => "idle".to_owned(),
        _ => String::new(),
    }
}
