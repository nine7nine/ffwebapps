//! Thin, GUI-facing wrappers around the `firefoxpwa` library.
//!
//! Reads go through `Storage::load`; mutations (later phases) will construct the
//! library command structs and run them off the GTK main thread. This module
//! keeps the UI layer free of library-internal details and provides
//! non-panicking display helpers (the library's `Site::name`/`domain` can
//! `unreachable!`-panic on a malformed manifest — unacceptable in a GUI).

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use firefoxpwa::components::runtime::Runtime;
use firefoxpwa::components::site::Site;
use firefoxpwa::console::Run;
use firefoxpwa::console::app::{
    HTTPClientConfig,
    ProfileCreateCommand,
    ProfileRemoveCommand,
    ProfileUpdateCommand,
    RuntimeInstallCommand,
    RuntimePatchCommand,
    RuntimeUninstallCommand,
    SiteInstallCommand,
    SiteLaunchCommand,
    SiteUninstallCommand,
    SiteUpdateCommand,
};
use firefoxpwa::directories::ProjectDirs;
use firefoxpwa::storage::Storage;
use gtk::{gio, glib};
use serde::{Deserialize, Serialize};
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
        client: default_client(),
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

// ---------------------------------------------------------------------------
// Lifecycle: install / uninstall / profiles / runtime (run via `spawn`)
// ---------------------------------------------------------------------------

fn default_client() -> HTTPClientConfig {
    HTTPClientConfig {
        user_agent: None,
        tls_root_certificates_der: None,
        tls_root_certificates_pem: None,
        tls_danger_accept_invalid_certs: false,
        tls_danger_accept_invalid_hostnames: false,
    }
}

/// Inputs for installing a new web app.
pub struct InstallParams {
    pub manifest_url: String,
    pub document_url: Option<String>,
    pub profile: Option<Ulid>,
    pub name: Option<String>,
    pub launch_on_login: bool,
    pub launch_on_browser: bool,
    pub hardware_webrtc: bool,
    pub software_rendering: bool,
}

/// Install a web app (fetches the manifest + icons, writes system integration).
/// Blocking — call via `spawn`. Returns the new web app's ULID.
pub fn install_site(params: InstallParams) -> Result<Ulid> {
    let manifest_url = Url::parse(params.manifest_url.trim()).context("Invalid manifest URL")?;
    let document_url = match params.document_url.as_deref().map(str::trim) {
        Some(text) if !text.is_empty() => Some(Url::parse(text).context("Invalid document URL")?),
        _ => None,
    };

    SiteInstallCommand {
        manifest_url,
        document_url,
        profile: params.profile,
        start_url: None,
        icon_url: None,
        name: params.name,
        description: None,
        categories: None,
        keywords: None,
        launch_on_login: Some(params.launch_on_login),
        launch_on_browser: Some(params.launch_on_browser),
        launch_now: false,
        hardware_webrtc: params.hardware_webrtc,
        software_rendering: params.software_rendering,
        scheduling: None,
        system_integration: true,
        client: default_client(),
    }
    ._run()
    .context("Failed to install web app")
}

/// Uninstall a web app (quiet — no stdin prompt). Blocking — call via `spawn`.
pub fn uninstall_site(id: Ulid) -> Result<()> {
    SiteUninstallCommand { id, quiet: true, system_integration: true }
        .run()
        .context("Failed to uninstall web app")
}

/// Create a profile. Blocking — call via `spawn`. Returns the new ULID.
pub fn create_profile(name: Option<String>, description: Option<String>) -> Result<Ulid> {
    ProfileCreateCommand { name, description, template: None }
        ._run()
        .context("Failed to create profile")
}

/// Update a profile's name/description. `Some(text)` sets, `None` clears (the
/// editor always shows current values, so we always set or clear — never leave).
pub fn update_profile(id: Ulid, name: Option<String>, description: Option<String>) -> Result<()> {
    ProfileUpdateCommand { id, name: Some(name), description: Some(description), template: None }
        .run()
        .context("Failed to update profile")
}

/// Remove a profile (quiet). The nil/Default profile is only cleared, not
/// removed (handled by the command). Blocking — call via `spawn`.
pub fn remove_profile(id: Ulid) -> Result<()> {
    ProfileRemoveCommand { id, quiet: true }.run().context("Failed to remove profile")
}

/// The installed runtime version, or `None` if the runtime isn't installed.
pub fn runtime_version(dirs: &ProjectDirs) -> Option<String> {
    Runtime::new(dirs).ok().and_then(|runtime| runtime.version)
}

/// Install the runtime (downloads from Mozilla, or links the system Firefox when
/// `link` is set). Slow + network — call via `spawn`.
pub fn install_runtime(link: bool) -> Result<()> {
    RuntimeInstallCommand { link }.run().context("Failed to install runtime")
}

/// Re-patch the installed runtime. Blocking — call via `spawn`.
pub fn patch_runtime() -> Result<()> {
    RuntimePatchCommand {}.run().context("Failed to patch runtime")
}

/// Uninstall the runtime. Blocking — call via `spawn`.
pub fn uninstall_runtime() -> Result<()> {
    RuntimeUninstallCommand {}.run().context("Failed to uninstall runtime")
}

// ---------------------------------------------------------------------------
// GUI appearance (glass tint / opacity / accent) — like the Poxicle configurator
// ---------------------------------------------------------------------------

/// The GUI's own glass appearance. Persisted separately from the app config in
/// `~/.local/share/ffwebapps/gui-appearance.json`; applied live.
#[derive(Clone, Serialize, Deserialize)]
pub struct Appearance {
    /// Window opacity, 0–100.
    pub opacity: u8,
    /// Glass tint colour, `#rrggbb`.
    pub glass: String,
    /// Accent colour, `#rrggbb`.
    pub accent: String,
}

impl Default for Appearance {
    fn default() -> Self {
        // Matches the static glass stylesheet's defaults.
        Self { opacity: 92, glass: "#14141a".into(), accent: "#3584e4".into() }
    }
}

fn appearance_path() -> Option<PathBuf> {
    ProjectDirs::new().ok().map(|dirs| dirs.userdata.join("gui-appearance.json"))
}

/// Load the saved appearance, or defaults if absent/unreadable.
pub fn load_appearance() -> Appearance {
    appearance_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Persist the appearance (best-effort).
pub fn save_appearance(appearance: &Appearance) {
    if let Some(path) = appearance_path()
        && let Ok(json) = serde_json::to_string_pretty(appearance)
    {
        let _ = std::fs::write(path, json);
    }
}

/// Profiles as `(ulid, display name)` in storage order (nil/Default first).
pub fn list_profiles(dirs: &ProjectDirs) -> Result<Vec<(Ulid, String)>> {
    let storage = Storage::load(dirs)?;
    Ok(storage
        .profiles
        .values()
        .map(|profile| {
            let name = profile.name.clone().unwrap_or_else(|| "Unnamed profile".into());
            (profile.ulid, name)
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Global config + runtime argv/env (no CLI command — direct Storage edits)
// ---------------------------------------------------------------------------

/// The global `Config` plus the runtime argv/env, for the Settings page.
pub struct ConfigEdits {
    pub always_patch: bool,
    pub runtime_enable_wayland: bool,
    pub runtime_use_xinput2: bool,
    pub runtime_use_portals: bool,
    pub use_linked_runtime: bool,
    pub arguments: Vec<String>,
    pub variables: BTreeMap<String, String>,
}

/// Load the current global config + runtime argv/env.
pub fn load_config(dirs: &ProjectDirs) -> Result<ConfigEdits> {
    let storage = Storage::load(dirs)?;
    Ok(ConfigEdits {
        always_patch: storage.config.always_patch,
        runtime_enable_wayland: storage.config.runtime_enable_wayland,
        runtime_use_xinput2: storage.config.runtime_use_xinput2,
        runtime_use_portals: storage.config.runtime_use_portals,
        use_linked_runtime: storage.config.use_linked_runtime,
        arguments: storage.arguments.clone(),
        variables: storage.variables.clone(),
    })
}

/// Persist the global config + runtime argv/env. Takes effect on next launch.
/// Blocking — call via `spawn`.
pub fn save_config(edits: ConfigEdits) -> Result<()> {
    let dirs = ProjectDirs::new()?;
    let mut storage = Storage::load(&dirs)?;
    storage.config.always_patch = edits.always_patch;
    storage.config.runtime_enable_wayland = edits.runtime_enable_wayland;
    storage.config.runtime_use_xinput2 = edits.runtime_use_xinput2;
    storage.config.runtime_use_portals = edits.runtime_use_portals;
    storage.config.use_linked_runtime = edits.use_linked_runtime;
    storage.arguments = edits.arguments;
    storage.variables = edits.variables;
    storage.write(&dirs).context("Failed to save settings")
}

// ---------------------------------------------------------------------------
// Per-profile CSS/JS injection (files at the profile root, read at launch)
// ---------------------------------------------------------------------------

fn injection_paths(dirs: &ProjectDirs, profile: Ulid) -> (PathBuf, PathBuf) {
    let dir = dirs.userdata.join("profiles").join(profile.to_string());
    (dir.join("ffwebapps.css"), dir.join("ffwebapps.js"))
}

/// Read a profile's `ffwebapps.css` / `ffwebapps.js` (empty if absent).
pub fn read_injection(dirs: &ProjectDirs, profile: Ulid) -> (String, String) {
    let (css, js) = injection_paths(dirs, profile);
    (
        std::fs::read_to_string(css).unwrap_or_default(),
        std::fs::read_to_string(js).unwrap_or_default(),
    )
}

/// Write a profile's `ffwebapps.css` / `ffwebapps.js`. Applies on next launch.
/// Blocking — call via `spawn`.
pub fn write_injection(dirs: &ProjectDirs, profile: Ulid, css: &str, js: &str) -> Result<()> {
    let (css_path, js_path) = injection_paths(dirs, profile);
    if let Some(parent) = css_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create profile directory")?;
    }
    std::fs::write(&css_path, css).context("Failed to write ffwebapps.css")?;
    std::fs::write(&js_path, js).context("Failed to write ffwebapps.js")?;
    Ok(())
}
