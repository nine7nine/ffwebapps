//! Thin, GUI-facing wrappers around the `firefoxpwa` library.
//!
//! Reads go through `Storage::load`; mutations (later phases) will construct the
//! library command structs and run them off the GTK main thread. This module
//! keeps the UI layer free of library-internal details and provides
//! non-panicking display helpers (the library's `Site::name`/`domain` can
//! `unreachable!`-panic on a malformed manifest — unacceptable in a GUI).

use std::path::PathBuf;

use anyhow::{Context, Result};
use firefoxpwa::components::site::Site;
use firefoxpwa::console::Run;
use firefoxpwa::console::app::{HTTPClientConfig, SiteLaunchCommand, SiteUpdateCommand};
use firefoxpwa::directories::ProjectDirs;
use firefoxpwa::storage::Storage;
use gtk::{gio, glib};
use ulid::Ulid;
use url::Url;

/// Resolve the project directories (`~/.local/share/ffwebapps`, etc.).
pub fn project_dirs() -> Result<ProjectDirs> {
    Ok(ProjectDirs::new()?)
}

/// Load the on-disk storage (`config.json`): profiles, sites, global config.
pub fn load_storage(dirs: &ProjectDirs) -> Result<Storage> {
    Ok(Storage::load(dirs)?)
}

/// A non-panicking display name for a site.
///
/// Mirrors `Site::name`'s preference order (custom name → manifest name →
/// short name) but falls back to the URL host instead of `Site::domain`, which
/// panics via `unreachable!` when the manifest scope isn't an absolute URL.
pub fn site_display_name(site: &Site) -> String {
    let candidates = [
        site.config.name.as_deref(),
        site.manifest.name.as_deref(),
        site.manifest.short_name.as_deref(),
    ];

    for candidate in candidates.into_iter().flatten() {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }

    site_domain(site)
}

/// A non-panicking host string for a site, used as the list-row subtitle.
pub fn site_domain(site: &Site) -> String {
    // The manifest URL host is always present and absolute for installed apps;
    // for data-URL installs the document URL carries the real host.
    let url = if site.config.manifest_url.scheme() != "data" {
        &site.config.manifest_url
    } else {
        &site.config.document_url
    };

    url.host_str().unwrap_or_default().to_owned()
}

/// Locate the largest installed PNG icon for a site, if any.
///
/// System integration writes these to
/// `~/.local/share/icons/hicolor/<size>/apps/FFPWA-<ulid>.png`. `userdata` is
/// `~/.local/share/ffwebapps`, so its parent is the `~/.local/share` data root.
pub fn site_icon_path(dirs: &ProjectDirs, site: &Site) -> Option<PathBuf> {
    let hicolor = dirs.userdata.parent()?.join("icons").join("hicolor");
    let filename = format!("FFPWA-{}.png", site.ulid);

    ["256x256", "128x128", "96x96", "64x64", "48x48", "32x32"]
        .into_iter()
        .map(|size| hicolor.join(size).join("apps").join(&filename))
        .find(|path| path.exists())
}

// ---------------------------------------------------------------------------
// Off-thread execution
// ---------------------------------------------------------------------------

/// Run a blocking `work` closure on a worker thread and deliver its result to
/// `on_done` back on the GTK main loop. Used for every library call that does
/// network I/O or spawns a process — never block the UI thread.
pub fn spawn<T, W, D>(work: W, on_done: D)
where
    T: Send + 'static,
    W: FnOnce() -> Result<T> + Send + 'static,
    D: FnOnce(Result<T>) + 'static,
{
    let (tx, rx) = async_channel::bounded(1);
    gio::spawn_blocking(move || {
        let _ = tx.send_blocking(work());
    });
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            on_done(result);
        }
    });
}

// ---------------------------------------------------------------------------
// Mutations (run these via `spawn`)
// ---------------------------------------------------------------------------

/// Everything the per-app editor can change, gathered from the widgets. The
/// `Option<Option<_>>` / `Option<Vec<_>>` fields already encode the library's
/// update semantics (see the field-diff helpers below).
pub struct SiteEdits {
    pub id: Ulid,
    pub name: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub start_url: Option<Option<String>>,
    pub icon_url: Option<Option<String>>,
    pub categories: Option<Vec<String>>,
    pub keywords: Option<Vec<String>>,
    pub enabled_url_handlers: Option<Vec<String>>,
    pub enabled_protocol_handlers: Option<Vec<String>>,
    pub launch_on_login: bool,
    pub launch_on_browser: bool,
    pub hardware_webrtc: bool,
    pub software_rendering: bool,
    pub start_hidden: bool,
    pub scheduling: Option<Option<String>>,
    pub user_agent: Option<Option<String>>,
    pub update_manifest: bool,
    pub update_icons: bool,
    // Direct Storage edits (no `SiteUpdateCommand` field for these):
    pub external_links: Option<bool>,
    pub allowed_domains: Vec<String>,
}

/// Apply `edits` to a web app: run `SiteUpdateCommand` (reuses manifest fetch +
/// system integration + storage write) and then direct-edit the two
/// `SiteConfig` fields the command doesn't cover. Blocking — call via `spawn`.
pub fn save_site(edits: SiteEdits) -> Result<()> {
    let start_url = parse_opt_opt_url(edits.start_url, "start")?;
    let icon_url = parse_opt_opt_url(edits.icon_url, "icon")?;

    let command = SiteUpdateCommand {
        id: edits.id,
        start_url,
        icon_url,
        name: edits.name,
        description: edits.description,
        categories: edits.categories,
        keywords: edits.keywords,
        enabled_url_handlers: edits.enabled_url_handlers,
        enabled_protocol_handlers: edits.enabled_protocol_handlers,
        launch_on_login: Some(edits.launch_on_login),
        launch_on_browser: Some(edits.launch_on_browser),
        update_manifest: edits.update_manifest,
        update_icons: edits.update_icons,
        hardware_webrtc: Some(edits.hardware_webrtc),
        software_rendering: Some(edits.software_rendering),
        scheduling: edits.scheduling,
        user_agent: edits.user_agent,
        start_hidden: Some(edits.start_hidden),
        system_integration: true,
        client: HTTPClientConfig {
            user_agent: None,
            tls_root_certificates_der: None,
            tls_root_certificates_pem: None,
            tls_danger_accept_invalid_certs: false,
            tls_danger_accept_invalid_hostnames: false,
        },
    };
    command.run().context("Failed to save web app settings")?;

    // `external_links` and `allowed_domains` have no command flag — edit Storage
    // directly. Done after the command's own write, so they aren't clobbered.
    let dirs = ProjectDirs::new()?;
    let mut storage = Storage::load(&dirs)?;
    let site = storage.sites.get_mut(&edits.id).context("Web app no longer exists")?;
    site.config.external_links = edits.external_links;
    site.config.allowed_domains = edits.allowed_domains;
    storage.write(&dirs)?;

    Ok(())
}

/// Launch a web app via the normal launch path (singleton check, runtime patch,
/// taskbar-tab registry, tray). Blocking — call via `spawn`.
pub fn launch_site(id: Ulid) -> Result<()> {
    SiteLaunchCommand { id, arguments: vec![], url: vec![], protocol: None, hidden: false }
        .run()
        .context("Failed to launch web app")
}

fn parse_opt_opt_url(value: Option<Option<String>>, field: &str) -> Result<Option<Option<Url>>> {
    match value {
        None => Ok(None),
        Some(None) => Ok(Some(None)),
        Some(Some(text)) => {
            let url = Url::parse(text.trim())
                .with_context(|| format!("Invalid {field} URL: {text}"))?;
            Ok(Some(Some(url)))
        }
    }
}

// ---------------------------------------------------------------------------
// Field-diff helpers — translate widget state into the library's update model.
//
//   Option<Option<T>>: None = leave unchanged, Some(None) = clear to manifest
//   default, Some(Some(v)) = set. Vec (store_value_vec!): Some(vec![""]) = clear,
//   Some(vec![..]) = set, None = leave.
// ---------------------------------------------------------------------------

/// Split a comma-separated entry into trimmed, non-empty items.
pub fn parse_csv(text: &str) -> Vec<String> {
    text.split(',').map(|item| item.trim().to_owned()).filter(|item| !item.is_empty()).collect()
}

/// Diff an override string field (e.g. name) against its original value.
pub fn opt_opt_string(original: Option<&str>, text: &str) -> Option<Option<String>> {
    let text = text.trim();
    match (original, text) {
        (None, "") => None,                       // no override, still none → leave
        (Some(orig), cur) if orig == cur => None, // unchanged → leave
        (_, "") => Some(None),                    // had an override, now cleared
        (_, cur) => Some(Some(cur.to_owned())),   // set / changed
    }
}

/// Diff `external_links` (`Option<bool>`, unset = on). Preserve "unset" when the
/// user leaves it at the default so we don't hard-code a value needlessly.
pub fn external_links_value(original: Option<bool>, switch_on: bool) -> Option<bool> {
    if switch_on && original.is_none() { None } else { Some(switch_on) }
}

/// Diff an `Option<Vec<String>>` override field (categories/keywords).
pub fn vec_opt_update(original: &Option<Vec<String>>, text: &str) -> Option<Vec<String>> {
    let items = parse_csv(text);
    let current = original.clone().unwrap_or_default();
    if items == current {
        None
    } else if items.is_empty() {
        Some(vec![String::new()]) // store_value_vec!: clear → manifest default
    } else {
        Some(items)
    }
}

/// Diff a plain `Vec<String>` field (URL/protocol handlers, applied via
/// store_value!). `Some(items)` sets it (empty vec clears); `None` leaves it.
pub fn vec_plain_update(original: &[String], text: &str) -> Option<Vec<String>> {
    let items = parse_csv(text);
    if items == original { None } else { Some(items) }
}
