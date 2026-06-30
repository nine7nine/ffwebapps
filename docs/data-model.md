# ffwebapps — Data Model & Storage

This page documents how ffwebapps describes a web app on disk: the `Storage` / `Config` / `Site` / `Profile` / `Runtime` types, the ULID and UUID identifiers, and the directory layout that makes an app fully file-described.

## Table of Contents

1. [Everything is a file](#1-everything-is-a-file)
2. [Storage and config.json](#2-storage-and-configjson)
3. [Config: global runtime options](#3-config-global-runtime-options)
4. [Site and SiteConfig](#4-site-and-siteconfig)
5. [Profiles](#5-profiles)
6. [The Runtime](#6-the-runtime)
7. [Identifiers: ULID vs UUID](#7-identifiers-ulid-vs-uuid)
8. [On-disk layout](#8-on-disk-layout)
9. [ProjectDirs and overrides](#9-projectdirs-and-overrides)

---

## 1. Everything is a file

ffwebapps has no database and no daemon holding state. A web app is described entirely by a record in `config.json` plus the files generated from it (the Firefox profile, the `.desktop` launcher, the icons). Delete those and the app is gone; copy `config.json` and the profile directory and the app moves with you.

The model is small and lives in two places: `src/storage.rs` (the top-level `Storage` and global `Config`) and `src/components/` (`Site`, `Profile`, `Runtime`). Every core type is `#[non_exhaustive]`, which forbids other crates from constructing or exhaustively matching them — the reason the GUI had to be a binary *in the same crate* (see [Architecture](architecture.gen.html)).

<div class="diagram-container">
<svg width="100%" viewBox="0 0 900 410" xmlns="http://www.w3.org/2000/svg">
  <style>
    .bg     { fill: #1a1b26; }
    .stor   { fill: #1a2235; stroke: #7aa2f7; stroke-width: 1.5; }
    .site   { fill: #1a2a1a; stroke: #9ece6a; stroke-width: 1.5; }
    .prof   { fill: #2a1f35; stroke: #bb9af7; stroke-width: 1.5; }
    .box    { fill: #24283b; stroke: #3b4261; stroke-width: 1; }
    .lbl    { fill: #c0caf5; font-size: 11px; font-family: 'JetBrains Mono', monospace; }
    .lbl-sm { fill: #c0caf5; font-size: 10px; font-family: 'JetBrains Mono', monospace; }
    .lbl-mut{ fill: #8c92b3; font-size: 9px;  font-family: 'JetBrains Mono', monospace; }
    .lbl-blu{ fill: #7aa2f7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-grn{ fill: #9ece6a; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-pur{ fill: #bb9af7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .ln     { stroke: #7dcfff; stroke-width: 1.5; fill: none; }
    .title  { fill: #7aa2f7; font-size: 14px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
  </style>
  <rect x="0" y="0" width="900" height="410" class="bg"/>
  <text x="450" y="26" text-anchor="middle" class="title">config.json  →  Storage</text>

  <rect x="280" y="44" width="340" height="92" class="stor"/>
  <text x="450" y="64" text-anchor="middle" class="lbl-blu">Storage</text>
  <text x="450" y="82" text-anchor="middle" class="lbl-mut">profiles: BTreeMap&lt;Ulid, Profile&gt;</text>
  <text x="450" y="96" text-anchor="middle" class="lbl-mut">sites: BTreeMap&lt;Ulid, Site&gt;</text>
  <text x="450" y="110" text-anchor="middle" class="lbl-mut">config: Config</text>
  <text x="450" y="126" text-anchor="middle" class="lbl-mut">arguments: Vec&lt;String&gt;   variables: BTreeMap</text>

  <!-- sites -->
  <rect x="40" y="190" width="260" height="180" class="site"/>
  <text x="170" y="210" text-anchor="middle" class="lbl-grn">Site</text>
  <text x="170" y="228" text-anchor="middle" class="lbl-mut">ulid: Ulid          (the app ID)</text>
  <text x="170" y="242" text-anchor="middle" class="lbl-mut">profile: Ulid       (→ a Profile)</text>
  <text x="170" y="256" text-anchor="middle" class="lbl-mut">manifest: WebAppManifest</text>
  <rect x="56" y="266" width="228" height="92" class="box"/>
  <text x="170" y="284" text-anchor="middle" class="lbl-sm">config: SiteConfig</text>
  <text x="170" y="300" text-anchor="middle" class="lbl-mut">document_url, manifest_url</text>
  <text x="170" y="314" text-anchor="middle" class="lbl-mut">webapp_id: Uuid, scheduling,</text>
  <text x="170" y="328" text-anchor="middle" class="lbl-mut">external_links, allowed_domains,</text>
  <text x="170" y="344" text-anchor="middle" class="lbl-mut">hardware_webrtc, user_agent, …</text>

  <!-- config -->
  <rect x="320" y="190" width="260" height="100" class="stor"/>
  <text x="450" y="210" text-anchor="middle" class="lbl-blu">Config  (global)</text>
  <text x="450" y="228" text-anchor="middle" class="lbl-mut">always_patch</text>
  <text x="450" y="242" text-anchor="middle" class="lbl-mut">runtime_enable_wayland</text>
  <text x="450" y="256" text-anchor="middle" class="lbl-mut">runtime_use_xinput2 / _portals</text>
  <text x="450" y="270" text-anchor="middle" class="lbl-mut">use_linked_runtime</text>

  <!-- profiles -->
  <rect x="600" y="190" width="260" height="120" class="prof"/>
  <text x="730" y="210" text-anchor="middle" class="lbl-pur">Profile</text>
  <text x="730" y="228" text-anchor="middle" class="lbl-mut">ulid: Ulid  (nil = "Default")</text>
  <text x="730" y="242" text-anchor="middle" class="lbl-mut">name: Option&lt;String&gt;</text>
  <text x="730" y="256" text-anchor="middle" class="lbl-mut">description: Option&lt;String&gt;</text>
  <text x="730" y="270" text-anchor="middle" class="lbl-mut">sites: Vec&lt;Ulid&gt;</text>
  <text x="730" y="292" text-anchor="middle" class="lbl-mut">one Firefox profile dir per Profile</text>

  <line x1="370" y1="136" x2="170" y2="190" class="ln"/>
  <line x1="450" y1="136" x2="450" y2="190" class="ln"/>
  <line x1="540" y1="136" x2="730" y2="190" class="ln"/>
  <line x1="300" y1="248" x2="320" y2="248" class="ln"/>
  <text x="300" y="180" class="lbl-mut">Site.profile → Profile.ulid; Profile.sites → Site.ulid</text>
</svg>
</div>

## 2. Storage and config.json

`Storage` (`storage.rs:58-77`) is the whole serialized model, read from and written to `~/.local/share/ffwebapps/config.json`:

| Field | Type | Holds |
| --- | --- | --- |
| `profiles` | `BTreeMap<Ulid, Profile>` | Every profile, keyed by ULID; defaults to one entry — the Nil-ULID "Default" |
| `sites` | `BTreeMap<Ulid, Site>` | Every web app, keyed by ULID |
| `config` | `Config` | The global runtime options |
| `arguments` | `Vec<String>` | Extra argv appended to every Firefox launch |
| `variables` | `BTreeMap<String, String>` | Extra environment variables for every launch |

`Storage::load` (`storage.rs:80-93`) returns `Self::default()` when the file is absent — so a first run already has the Default profile. `Storage::write` (`storage.rs:95-105`) truncates and rewrites the whole file: pretty JSON in debug builds, compact in release. There is no locking; because both `BTreeMap`s are keyed by time-ordered ULIDs, the JSON keys come out sorted and stable.

## 3. Config: global runtime options

`Config` (`storage.rs:18-56`) holds options that apply to every app's runtime launch. All default to `false`.

| Field | Effect at launch |
| --- | --- |
| `always_patch` | Re-patch the runtime and profile on every launch (no effect on macOS) |
| `runtime_enable_wayland` | Sets `MOZ_ENABLE_WAYLAND=1` |
| `runtime_use_xinput2` | Sets `MOZ_USE_XINPUT2=1` |
| `runtime_use_portals` | Sets `GTK_USE_PORTAL=1` (XDG Desktop Portals) |
| `use_linked_runtime` | Linux only: use the symlinked system Firefox instead of a downloaded copy |

`use_linked_runtime` is not toggled directly — it is set as a side effect of `runtime install --link` vs `runtime install` (see [The Runtime](#6-the-runtime)). The Wayland/XInput2/portal flags are consumed in `Site::launch` (`site.rs:301-309`).

## 4. Site and SiteConfig

A `Site` (`site.rs:176-195`) is one installed app: a `ulid`, the `profile` it belongs to, the parsed web-app `manifest`, and its `config`. The interesting surface is `SiteConfig` (`site.rs:33-145`), which mixes **required** anchors with **optional** overrides of manifest-provided values.

**Required**

| Field | Type | Meaning |
| --- | --- | --- |
| `document_url` | `Url` | The site's main page |
| `manifest_url` | `Url` | The web-app manifest — may be a `data:` URL for non-PWA sites |

**Overrides (unset = use the manifest)**

| Field | Type |
| --- | --- |
| `name`, `description` | `Option<String>` |
| `start_url`, `icon_url` | `Option<Url>` |
| `categories`, `keywords` | `Option<Vec<String>>` |

**Behaviour & identity**

| Field | Type | Meaning |
| --- | --- | --- |
| `webapp_id` | `Option<Uuid>` | Stable Taskbar-Tabs registry ID; also the Wayland `app_id` and `.desktop` `StartupWMClass` |
| `enabled_url_handlers` | `Vec<String>` | URL scopes intercepted into the app window |
| `enabled_protocol_handlers` | `Vec<String>` | Protocol schemes registered with the OS |
| `custom_protocol_handlers` | `Vec<ProtocolHandlerResource>` | Schemes registered via `registerProtocolHandler` |
| `launch_on_login` | `bool` | Write an autostart entry |
| `launch_on_browser` | `bool` | Launch when the browser launches |
| `start_hidden` | `bool` | Autostart launch goes straight to the tray |
| `external_links` | `Option<bool>` | Route out-of-scope links to the browser (**unset ⇒ on**) |
| `allowed_domains` | `Vec<String>` | Wildcard domains kept in-window (empty ⇒ scope-derived default) |

**Performance**

| Field | Type | Meaning |
| --- | --- | --- |
| `hardware_webrtc` | `bool` | Force HW video decode past Firefox's blocklist + HW VP8 path |
| `software_rendering` | `bool` | Disable all GPU use — **overrides `hardware_webrtc`** |
| `scheduling` | `Option<String>` | Process scheduling policy applied at launch |
| `user_agent` | `Option<String>` | UA override (unset ⇒ Firefox default) |

The `scheduling` string has a small grammar realized by `scheduling_launcher` (`site.rs:151-174`): `nice:<n>` → `nice -n <n>`, `rr:<p>` / `fifo:<p>` → `chrt -r/-f <p>`, `batch` → `chrt -b 0`, `idle` → `chrt -i 0`. The launcher wraps the runtime as `sh -c '<sched> "$@" || exec "$@"'`, so an unprivileged RT request falls back to a normal launch rather than failing. These knobs are explained in [Performance Tuning](performance.gen.html).

`Site` also carries small resolver methods that fall back through config → manifest → derived defaults: `url()`, `domain()`, `name()`, `description()`, `icons()`, `categories()`, `keywords()` (`site.rs:320-417`). Notably the GTK GUI deliberately *avoids* `name()`/`domain()` because they `unreachable!`-panic on a malformed manifest, using non-panicking equivalents instead.

## 5. Profiles

A `Profile` (`profile.rs:11-33`) groups apps that share a Firefox profile — and therefore cookies, storage, and per-profile CSS/JS injection. It is just a `ulid`, an optional `name` and `description`, and a `sites: Vec<Ulid>` back-reference.

The **Nil-ULID profile** is special (`profile.rs:36-46`): a profile with `Ulid::nil()`, named "Default", always exists and is the default target for new apps. It cannot be fully removed — removing it clears its apps but the profile stays. Its directory on disk is `profiles/00000000000000000000000000/`.

`Profile::patch` (`profile.rs:54-74`) refreshes the profile's chrome assets: it copies `sysdata/userchrome/profile/` (the `chrome/userChrome.css`) into the profile directory, after removing `startupCache/` and `chrome/pwa/`. It deliberately does **not** touch `user.js` or the `ffwebapps.css`/`ffwebapps.js` injection files, so those persist across patches. Profile **templates** (a CLI/GUI feature) copy the contents of a user-supplied directory into a new profile at creation time.

## 6. The Runtime

`Runtime` (`runtime.rs:152-235`) describes the Firefox install ffwebapps owns: a `version` (only `Some` when actually installed, parsed from `application.ini`'s `[app] version`), the `directory`, the `executable` (`firefox`), and the `config` (`application.ini`). `Runtime::new` prefers `userdata/runtime/`, falling back to `sysdata/runtime/`.

| Operation | What it does |
| --- | --- |
| `install` | Download an official Mozilla build, unpack it, and replace the runtime directory (`runtime.rs:237-342`) |
| `link` (Linux) | Symlink the system Firefox into the runtime dir (copy only `firefox`/`firefox-bin`), set `use_linked_runtime = true` (`runtime.rs:344-399`) |
| `patch` | Copy `sysdata/userchrome/runtime/` (autoconfig) into the runtime and fix permissions (`runtime.rs:410-496`) |
| `uninstall` | Empty the runtime directory (`runtime.rs:401-408`) |
| `run` | Spawn the runtime, optionally under a `launcher` wrapper (the scheduling prefix) (`runtime.rs:498-530`) |

`version == None` means no runtime is installed, and launches bail — the GUI gates its UI on this and offers the install action. See [The Firefox Runtime & Autoconfig](runtime.gen.html) for the link/patch mechanics.

## 7. Identifiers: ULID vs UUID

ffwebapps uses two different identifier types for two different jobs, and the distinction matters:

| ID | Type | Identifies | Generated |
| --- | --- | --- | --- |
| App ID | `Ulid` | A `Site` — the canonical internal handle, profile directory name, and `MOZ_APP_REMOTINGNAME` suffix | `Ulid::new()` at install |
| Profile ID | `Ulid` | A `Profile` — directory name; `Ulid::nil()` is the Default | `Ulid::new()` at create |
| `webapp_id` | `Uuid` (v4) | The Firefox Taskbar-Tabs registry entry, Wayland `app_id`, and `.desktop` `StartupWMClass` | `Uuid::new_v4()` at install |

ULIDs are used everywhere ffwebapps refers to its own objects — they are lexicographically time-ordered, so `BTreeMap` keys and the on-disk `.desktop`/icon filenames (`FFPWA-<ulid>`) sort by creation time. The `webapp_id` is a separate UUID purely because that is the shape Firefox's Web Apps registry and window `app_id` expect. An app installed before `webapp_id` existed gets one lazily back-filled on its next launch.

## 8. On-disk layout

Under `~/.local/share/ffwebapps/` (the `userdata` dir):

```text
config.json                         the serialized Storage
runtime/                            the Firefox runtime (symlinks when --link)
profiles/
  00000000000000000000000000/       the Nil "Default" profile
    user.js                         per-app prefs (regenerated at launch)
    chrome/userChrome.css           chromeless titlebar (from patch)
    taskbartabs/taskbartabs.json    Web Apps registry (scope + start URL)
    ffwebapps.css / ffwebapps.js     optional per-profile injection
  01J.../                            a per-app profile (by profile ULID)
```

And the desktop-integration files, written outside the ffwebapps data dir into standard XDG locations:

```text
~/.local/share/applications/FFPWA-<ulid>.desktop      the launcher
~/.local/share/icons/hicolor/<size>/apps/FFPWA-<ulid>.png   icons
~/.config/autostart/FFPWA-<ulid>.desktop              only if launch-on-login
~/.config/kwinrulesrc  [ffwebapps-FFPWA-<ulid>]       only on KDE
$XDG_RUNTIME_DIR/ffwebapps-<ULID>.sock                 only while running
```

These are documented in [Desktop Integration](desktop-integration.gen.html).

## 9. ProjectDirs and overrides

`ProjectDirs` (`directories.rs:30-99`) resolves the three roots ffwebapps uses:

| Field | Default (Linux) | Holds | Env override |
| --- | --- | --- | --- |
| `executables` | `/usr/bin` | The `ffwebapps` / `ffwebapps-tray` binaries | `FFPWA_EXECUTABLES` |
| `sysdata` | `/usr/share/ffwebapps` | The `userchrome/` assets copied into profiles and the runtime | `FFPWA_SYSDATA` |
| `userdata` | `~/.local/share/ffwebapps` | `config.json`, `runtime/`, `profiles/` | `FFPWA_USERDATA` |

Each path is resolved first from a build-time `option_env!`, then — unless the build set `FFPWA_STATIC_DIRS=1` — overridden by the matching run-time environment variable, with a leading `~` expanded against the home directory (`directories.rs:102-214`). This is how a development checkout points the binaries at an in-tree `userchrome/` and a scratch data dir without touching `/usr`. `ProjectDirs::new` also `create_dir_all`s `userdata` so the data directory always exists.
