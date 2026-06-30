# ffwebapps — Architecture Overview

This page is the system map for ffwebapps: the binaries, the shared library they all call, the Firefox runtime they drive, and the single socket that ties a running app to its tray and launcher.

## Table of Contents

1. [What ffwebapps is](#1-what-ffwebapps-is)
2. [The core idea: drive, don't patch](#2-the-core-idea-drive-dont-patch)
3. [Component model](#3-component-model)
4. [The shared library](#4-the-shared-library)
5. [On-disk state](#5-on-disk-state)
6. [Lifecycle of an app](#6-lifecycle-of-an-app)
7. [The one IPC boundary](#7-the-one-ipc-boundary)
8. [Source layout](#8-source-layout)
9. [Design rules](#9-design-rules)
10. [Document index](#10-document-index)

---

## 1. What ffwebapps is

ffwebapps runs any website as a **native, chromeless desktop app** on Linux: its own window with no tabs or address bar, its own taskbar/dock identity, a system-tray icon with an unread badge, close-to-tray, and out-of-scope links that open in your real browser. It is a CLI-driven fork of [PWAsForFirefox](https://github.com/filips123/PWAsForFirefox)'s native component, re-architected to drive Firefox's first-party **Web Apps (Taskbar Tabs)** feature rather than patch the browser chrome at runtime.

The whole project is one Rust crate (`firefoxpwa`) that produces several binaries plus a small bundle of privileged Firefox configuration. There is no long-lived ffwebapps daemon: the *running app is a Firefox process*, and everything else is either a one-shot CLI command or a thin client of the socket that Firefox process serves.

<div class="diagram-container">
<svg width="100%" viewBox="0 0 980 700" xmlns="http://www.w3.org/2000/svg">
  <style>
    .bg      { fill: #1a1b26; }
    .layer-u { fill: #1a2a1a; stroke: #9ece6a; stroke-width: 1.5; }
    .layer-s { fill: #1a2235; stroke: #7aa2f7; stroke-width: 1.5; }
    .layer-p { fill: #2a1f35; stroke: #bb9af7; stroke-width: 1.5; }
    .layer-f { fill: #16242b; stroke: #7dcfff; stroke-width: 1.5; }
    .box     { fill: #24283b; stroke: #3b4261; stroke-width: 1; }
    .box-hot { fill: #2a2438; stroke: #e0af68; stroke-width: 1.5; }
    .sys     { fill: #1f2535; stroke: #565f89; stroke-width: 1; }
    .lbl     { fill: #c0caf5; font-size: 11px; font-family: 'JetBrains Mono', monospace; }
    .lbl-sm  { fill: #c0caf5; font-size: 10px; font-family: 'JetBrains Mono', monospace; }
    .lbl-mut { fill: #8c92b3; font-size: 9px;  font-family: 'JetBrains Mono', monospace; }
    .lbl-grn { fill: #9ece6a; font-size: 12px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-blu { fill: #7aa2f7; font-size: 12px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-pur { fill: #bb9af7; font-size: 12px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-cy  { fill: #7dcfff; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-yel { fill: #e0af68; font-size: 10px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .ln      { stroke: #7dcfff; stroke-width: 1.5; fill: none; }
    .bound   { stroke: #6b7398; stroke-width: 1.2; stroke-dasharray: 6,4; fill: none; }
    .title   { fill: #7aa2f7; font-size: 14px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
  </style>

  <rect x="0" y="0" width="980" height="700" class="bg"/>
  <text x="490" y="26" text-anchor="middle" class="title">ffwebapps component architecture</text>

  <!-- Control surfaces -->
  <rect x="20" y="44" width="940" height="84" class="layer-u"/>
  <text x="40" y="64" class="lbl-grn">control surfaces  --  link the library in-process</text>
  <text x="40" y="78" class="lbl-mut">no shelling out, no CLI-text parsing: each constructs command structs and calls .run()</text>

  <rect x="40"  y="86" width="280" height="34" class="box"/>
  <text x="180" y="107" text-anchor="middle" class="lbl-sm">ffwebapps  (CLI)</text>
  <rect x="332" y="86" width="280" height="34" class="box"/>
  <text x="472" y="107" text-anchor="middle" class="lbl-sm">ffwebapps-gtk  (GTK4 / libadwaita GUI)</text>
  <rect x="624" y="86" width="316" height="34" class="box"/>
  <text x="782" y="100" text-anchor="middle" class="lbl-sm">firefoxpwa-connector</text>
  <text x="782" y="113" text-anchor="middle" class="lbl-mut">inherited native-messaging host (unused by CLI flow)</text>

  <!-- surfaces -> library -->
  <line x1="180" y1="128" x2="180" y2="158" class="ln"/>
  <line x1="472" y1="128" x2="472" y2="158" class="ln"/>

  <!-- Library core -->
  <rect x="20" y="162" width="940" height="104" class="layer-s"/>
  <text x="40" y="182" class="lbl-blu">firefoxpwa  --  the shared library (one crate, called in-process)</text>
  <text x="40" y="196" class="lbl-mut">manifest fetch, system integration, storage; the only code that mutates app state</text>

  <rect x="40"  y="206" width="210" height="50" class="box"/>
  <text x="145" y="226" text-anchor="middle" class="lbl-sm">storage / components</text>
  <text x="145" y="240" text-anchor="middle" class="lbl-mut">Site / Profile / Runtime / Config</text>
  <text x="145" y="252" text-anchor="middle" class="lbl-mut">over config.json</text>

  <rect x="262" y="206" width="210" height="50" class="box"/>
  <text x="367" y="226" text-anchor="middle" class="lbl-sm">taskbartabs</text>
  <text x="367" y="240" text-anchor="middle" class="lbl-mut">writes taskbartabs.json +</text>
  <text x="367" y="252" text-anchor="middle" class="lbl-mut">per-app user.js prefs</text>

  <rect x="484" y="206" width="210" height="50" class="box"/>
  <text x="589" y="226" text-anchor="middle" class="lbl-sm">integrations</text>
  <text x="589" y="240" text-anchor="middle" class="lbl-mut">.desktop launchers, icons,</text>
  <text x="589" y="252" text-anchor="middle" class="lbl-mut">autostart, KWin rules</text>

  <rect x="706" y="206" width="234" height="50" class="box"/>
  <text x="823" y="226" text-anchor="middle" class="lbl-sm">console (clap commands)</text>
  <text x="823" y="240" text-anchor="middle" class="lbl-mut">site / profile / runtime;</text>
  <text x="823" y="252" text-anchor="middle" class="lbl-mut">launch builds Firefox argv</text>

  <!-- library -> disk -->
  <line x1="145" y1="266" x2="145" y2="300" class="ln"/>
  <text x="157" y="288" class="lbl-mut">read / write</text>

  <!-- Disk state -->
  <rect x="20" y="304" width="450" height="86" class="sys"/>
  <text x="40" y="324" class="lbl-cy">on-disk state  (~/.local/share/ffwebapps)</text>
  <rect x="40"  y="334" width="130" height="46" class="box"/>
  <text x="105" y="353" text-anchor="middle" class="lbl-mut">config.json</text>
  <text x="105" y="367" text-anchor="middle" class="lbl-mut">sites + profiles</text>
  <rect x="182" y="334" width="130" height="46" class="box"/>
  <text x="247" y="353" text-anchor="middle" class="lbl-mut">profiles/&lt;ulid&gt;/</text>
  <text x="247" y="367" text-anchor="middle" class="lbl-mut">Firefox profile</text>
  <rect x="324" y="334" width="130" height="46" class="box"/>
  <text x="389" y="353" text-anchor="middle" class="lbl-mut">runtime/</text>
  <text x="389" y="367" text-anchor="middle" class="lbl-mut">linked Firefox</text>

  <!-- XDG integration files -->
  <rect x="490" y="304" width="450" height="86" class="sys"/>
  <text x="510" y="324" class="lbl-cy">desktop integration  (XDG dirs)</text>
  <rect x="510" y="334" width="130" height="46" class="box"/>
  <text x="575" y="353" text-anchor="middle" class="lbl-mut">applications/</text>
  <text x="575" y="367" text-anchor="middle" class="lbl-mut">FFPWA-&lt;ulid&gt;.desktop</text>
  <rect x="652" y="334" width="130" height="46" class="box"/>
  <text x="717" y="353" text-anchor="middle" class="lbl-mut">icons/hicolor/</text>
  <text x="717" y="367" text-anchor="middle" class="lbl-mut">app icons</text>
  <rect x="794" y="334" width="130" height="46" class="box"/>
  <text x="859" y="353" text-anchor="middle" class="lbl-mut">autostart/</text>
  <text x="859" y="367" text-anchor="middle" class="lbl-mut">launch-on-login</text>

  <!-- launch boundary -->
  <line x1="20" y1="406" x2="960" y2="406" class="bound"/>
  <text x="490" y="401" text-anchor="middle" class="lbl-yel">firefox -profile … -taskbar-tab &lt;webapp_id&gt;  --  launch boundary</text>

  <!-- console -> firefox -->
  <line x1="823" y1="256" x2="823" y2="430" class="ln"/>
  <text x="835" y="350" class="lbl-mut">spawns</text>

  <!-- The running app -->
  <rect x="20" y="430" width="940" height="130" class="layer-p"/>
  <text x="40" y="450" class="lbl-pur">the running app  --  a system Firefox process (Web App window)</text>
  <text x="40" y="464" class="lbl-mut">owns its window and lifecycle; configured entirely by files in its profile</text>

  <rect x="40"  y="474" width="300" height="74" class="box-hot"/>
  <text x="190" y="493" text-anchor="middle" class="lbl-yel">_autoconfig.cfg  (privileged JS)</text>
  <text x="190" y="509" text-anchor="middle" class="lbl-mut">serves the Unix socket, owns hide/show,</text>
  <text x="190" y="522" text-anchor="middle" class="lbl-mut">routes out-of-scope links to the browser,</text>
  <text x="190" y="535" text-anchor="middle" class="lbl-mut">injects per-app CSS / JS, tracks unread</text>

  <rect x="352" y="474" width="290" height="74" class="box"/>
  <text x="497" y="493" text-anchor="middle" class="lbl-sm">profile config (read at launch)</text>
  <text x="497" y="509" text-anchor="middle" class="lbl-mut">user.js  -- link allow-list, perf prefs, UA</text>
  <text x="497" y="522" text-anchor="middle" class="lbl-mut">chrome/userChrome.css  -- chromeless titlebar</text>
  <text x="497" y="535" text-anchor="middle" class="lbl-mut">taskbartabs/taskbartabs.json  -- scope</text>

  <rect x="654" y="474" width="286" height="74" class="box"/>
  <text x="797" y="493" text-anchor="middle" class="lbl-sm">window identity</text>
  <text x="797" y="509" text-anchor="middle" class="lbl-mut">app_id  org.mozilla.firefox.webapp-&lt;id&gt;</text>
  <text x="797" y="522" text-anchor="middle" class="lbl-mut">MOZ_APP_REMOTINGNAME  ffwebapps-&lt;ulid&gt;</text>
  <text x="797" y="535" text-anchor="middle" class="lbl-mut">= distinct dock entry + single instance</text>

  <!-- socket boundary -->
  <line x1="190" y1="548" x2="190" y2="592" class="ln"/>
  <text x="202" y="575" class="lbl-cy">serves</text>

  <!-- Socket + clients -->
  <rect x="20" y="596" width="940" height="92" class="layer-f"/>
  <text x="40" y="616" class="lbl-cy">$XDG_RUNTIME_DIR/ffwebapps-&lt;ULID&gt;.sock  --  the one IPC boundary (newline protocol)</text>
  <text x="40" y="630" class="lbl-mut">runtime alive  iff  socket accepts; no pidfiles, no sentinels</text>

  <rect x="40"  y="640" width="280" height="38" class="box"/>
  <text x="180" y="659" text-anchor="middle" class="lbl-sm">ffwebapps-tray</text>
  <text x="180" y="671" text-anchor="middle" class="lbl-mut">StatusNotifierItem: icon, badge, menu</text>

  <rect x="332" y="640" width="280" height="38" class="box"/>
  <text x="472" y="659" text-anchor="middle" class="lbl-sm">launcher  (site launch)</text>
  <text x="472" y="671" text-anchor="middle" class="lbl-mut">connect succeeds = focus, don't duplicate</text>

  <rect x="624" y="640" width="316" height="38" class="box"/>
  <text x="782" y="659" text-anchor="middle" class="lbl-sm">ffwebapps-gtk live control</text>
  <text x="782" y="671" text-anchor="middle" class="lbl-mut">running / hidden / unread + show / hide / quit</text>
</svg>
</div>

## 2. The core idea: drive, don't patch

The upstream project, and most "site-specific browser" tools, achieve a chromeless window by *patching the browser's chrome* at runtime — replacing `browser.xhtml`, injecting a custom UI, and maintaining that patch against every Firefox update. ffwebapps takes the opposite stance: it leans on a feature Mozilla now ships and maintains itself.

Firefox ≥ 151 includes **Web Apps (Taskbar Tabs)**: pass `firefox -taskbar-tab <id>` and Firefox opens a standalone, minimal-UI window with its own Wayland `app_id`. ffwebapps' job is reduced to three things:

1. **Register** the app in the profile's `taskbartabs.json` so the ID resolves to a scope and a start URL.
2. **Configure** the runtime through first-party mechanisms — a privileged `_autoconfig.cfg`, a profile `user.js`, and `userChrome.css` — to enable the feature, strip the last toolbar pixels, route links, and serve a tray socket.
3. **Integrate** with the desktop — generate the `.desktop` launcher, icons, and (on KDE) a window rule.

Nothing monkeypatches Firefox's UI code. The chromeless look is `userChrome.css`; the behaviour is `_autoconfig.cfg`. Both are supported, documented Firefox extension points. See [The Firefox Runtime & Autoconfig](runtime.gen.html) for the full mechanism.

## 3. Component model

ffwebapps is built from four binaries and one configuration bundle. Only the first three binaries matter for the Linux web-app flow; the connector is inherited from upstream.

| Component | Kind | Role |
| --- | --- | --- |
| `ffwebapps` | CLI binary | The primary interface: install / launch / update / uninstall apps, profiles, and the runtime |
| `ffwebapps-tray` | Tray binary | A `StatusNotifierItem` that shows the icon, unread badge, and menu, and drives the window over the socket |
| `ffwebapps-gtk` | GUI binary | A GTK4 / libadwaita management app behind the `gui` cargo feature; same crate, calls the library in-process |
| `firefoxpwa-connector` | Helper binary | Native-messaging host inherited from PWAsForFirefox; not part of the CLI-driven flow |
| `userchrome/` | Config bundle | `_autoconfig.cfg`, `autoconfig.js`, and `userChrome.css` installed into the runtime / profile |

The crucial property: **there is no ffwebapps daemon**. A web app that is "running" is a Firefox process. The CLI and GUI are short-lived; the tray is a thin remote that exits when its app does. State lives on disk and in that one Firefox process.

## 4. The shared library

Everything except the privileged JS lives in the `firefoxpwa` crate, and `src/lib.rs` re-exports each module so every binary calls the *same* code in-process:

```rust
pub mod components;   // Site, Profile, Runtime, taskbartabs registry
pub mod connector;    // inherited native-messaging protocol
pub mod console;      // clap command structs + the Run trait
pub mod directories;  // ProjectDirs — the on-disk layout
pub mod integrations; // .desktop / icons / autostart / KWin rules
pub mod storage;      // Storage + Config over config.json
pub mod utils;
```

The GTK GUI was deliberately built as a second binary *in the same crate* rather than a separate program. The core types (`Site`, `Config`, `Storage`, `ProjectDirs`, `Profile`) are `#[non_exhaustive]`, which forbids construction from *other* crates but not from within this one — so the GUI can build command structs and call `.run()` directly, reusing manifest fetching, system integration, and storage writes instead of re-implementing or shelling out. See [Data Model & Storage](data-model.gen.html) and [CLI & Command Model](cli.gen.html).

## 5. On-disk state

A web app is fully described by files. There is no hidden runtime database; deleting the directories below removes the app.

| Path | Holds |
| --- | --- |
| `~/.local/share/ffwebapps/config.json` | The `Storage`: every `Site`, every `Profile`, and the global `Config` |
| `~/.local/share/ffwebapps/profiles/<profile-ulid>/` | The Firefox profile for that profile's apps (`user.js`, `chrome/`, `taskbartabs/`, `ffwebapps.css/js`) |
| `~/.local/share/ffwebapps/runtime/` | The runtime — symlinks to the system Firefox when installed with `--link` |
| `~/.local/share/applications/FFPWA-<ulid>.desktop` | The launcher that appears in the app menu |
| `~/.local/share/icons/hicolor/<size>/apps/FFPWA-<ulid>.png` | Rasterised app icons |
| `~/.config/autostart/FFPWA-<ulid>.desktop` | Present only when *launch on login* is enabled |
| `$XDG_RUNTIME_DIR/ffwebapps-<ULID>.sock` | The live IPC socket — exists only while the app runs |

Profiles are the unit of isolation: several apps can share one Firefox profile (and thus cookies, storage, and per-profile CSS/JS), or each can have its own. The Nil-ULID profile is the shared "Default".

## 6. Lifecycle of an app

A single `site install` followed by a launch touches most of the system. The sequence makes the boundaries concrete:

1. **Install** (`ffwebapps site install <MANIFEST_URL> --document-url <PAGE>`): the library fetches the web-app manifest and icons, mints a ULID and a `webapp_id` (a UUID), writes the `Site` into `config.json`, generates the `.desktop` launcher and icons, and — on KDE — a `kwinrulesrc` position rule.
2. **Launch** (from the menu or `site launch <ULID>`): the console first **probes the socket**. If it connects, the app is already running, so it sends `show` and exits — the single-instance guarantee. Otherwise it writes `taskbartabs.json` and `user.js`, builds the Firefox argv, and spawns the runtime, optionally under a scheduling policy.
3. **Run**: Firefox loads `_autoconfig.cfg`, which binds the socket, takes ownership of the window's hide/show, starts routing out-of-scope links, injects any per-app CSS/JS, and begins exporting the unread count.
4. **Tray**: the runtime (via the launch path) spawns `ffwebapps-tray`, which connects to the socket, draws the icon, and relays menu actions back as verbs.
5. **Close to tray**: the window's X is intercepted and turned into a hide *while a tray client is connected*; otherwise it really closes. **Quit** (from the tray menu) sends `quit`, the runtime tears the socket down, and the tray sees EOF and exits.

## 7. The one IPC boundary

The entire live surface is a single Unix-domain socket, **served by the Firefox runtime** and consumed by thin clients. It uses a newline-delimited text protocol:

```text
client -> runtime : hello v1 tray | hello v1 launcher
                    show | hide | toggle | quit | reload
                    mute-toggle | dnd-toggle | suspend-toggle
                    autostart-toggle | copy-url | open-browser
runtime -> client : hello v1 <pid>            (once, on connect)
                    unread <n>                (on change)
                    state hidden=… muted=… dnd=… suspend=… autostart=…
```

There are **no pidfiles and no sentinel files**: the runtime is alive if and only if the socket accepts a connection. That single invariant powers the singleton launcher, the tray's liveness detection, and the GUI's running/hidden status dot. The full protocol, the runtime-owned-window model, and the KWin hide/show mechanism are documented in [IPC & the Runtime-Owned Window](ipc-protocol.gen.html).

## 8. Source layout

| Path | Purpose |
| --- | --- |
| `src/bin/ffwebapps.rs` | CLI entrypoint — `App::parse().run()` |
| `src/bin/ffwebapps-tray.rs` | The `ksni` tray helper |
| `src/bin/ffwebapps-gtk/` | The GTK4 management GUI (its own `core` / `ipc` / `ui` modules) |
| `src/components/` | `Site`, `Profile`, `Runtime`, and the `taskbartabs` registry writer |
| `src/console/` | clap command structs (`app`, `profile`, `runtime`) and the `Run` trait |
| `src/integrations/` | Desktop integration; `implementation/linux.rs` is the Linux backend |
| `src/storage.rs`, `src/directories.rs` | `Storage` / `Config` and the on-disk path resolver |
| `userchrome/runtime/_autoconfig.cfg` | The privileged runtime JS — the heart of the running app |
| `userchrome/profile/chrome/userChrome.css` | The chromeless titlebar styling |

## 9. Design rules

The architecture holds together because of a few deliberate choices, several of them learned the hard way (see the gotchas in each page):

- **Drive Firefox's first-party features; never patch its chrome.** The chromeless look and all behaviour come from supported extension points.
- **The runtime owns its window and lifecycle.** Hide/show, close-to-tray, and quit are decided *inside* the Firefox process, not by an external window-manager script reaching in.
- **One socket, one invariant.** Liveness is "the socket accepts"; no pidfiles, no polling, no sentinels.
- **The tray spawns nothing.** It is a pure remote — zero `Command::new` calls — so it can never launch, relaunch, or strand a process.
- **Config is files, owned by ffwebapps.** A web app's profile `user.js` and registry are managed by the library and regenerated at launch; nothing is hidden in opaque daemon state.
- **Call the library in-process.** The GUI reuses the CLI's code paths rather than parsing CLI text or duplicating logic.

## 10. Document index

| Document | Covers |
| --- | --- |
| [The Firefox Runtime & Autoconfig](runtime.gen.html) | Driving Web Apps / Taskbar Tabs, the privileged `_autoconfig.cfg`, `userChrome.css`, and profile prefs |
| [IPC & the Runtime-Owned Window](ipc-protocol.gen.html) | The socket protocol, close-to-tray, and the KWin hide/show mechanism |
| [Data Model & Storage](data-model.gen.html) | `Storage`, `Config`, `Site`, `Profile`, `Runtime`, ULIDs, and the on-disk layout |
| [CLI & Command Model](cli.gen.html) | The clap command tree, the `Run` trait, and the update-value semantics |
| [System Tray](tray.gen.html) | The `ksni` `StatusNotifierItem`, the flock singleton, and the menu |
| [GTK Management GUI](gtk-gui.gen.html) | The `ffwebapps-gtk` app: pages, off-thread workers, and live control |
| [Link Routing & Scope](link-routing.gen.html) | Two-layer out-of-scope interception and the in-app allow-list |
| [Desktop Integration](desktop-integration.gen.html) | `.desktop` launchers, icons, autostart, window identity, and KWin rules |
| [Performance Tuning](performance.gen.html) | `--hardware-webrtc`, `--scheduling`, `--software-rendering`, and memory |
| [Build, Install & Packaging](install.gen.html) | Building, the runtime link step, and packaging |
