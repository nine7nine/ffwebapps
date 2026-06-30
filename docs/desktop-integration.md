# ffwebapps — Desktop Integration

This page documents how an installed web app becomes a first-class desktop citizen on Linux: the `.desktop` launcher, the icon set, the autostart entry, the window identity that ties them together, and the KDE window rule.

## Table of Contents

1. [What "integration" writes](#1-what-integration-writes)
2. [The .desktop launcher](#2-the-desktop-launcher)
3. [Window identity: the app_id chain](#3-window-identity-the-app_id-chain)
4. [Icons](#4-icons)
5. [Categories and shortcuts](#5-categories-and-shortcuts)
6. [Autostart and start-hidden](#6-autostart-and-start-hidden)
7. [The KWin position rule](#7-the-kwin-position-rule)
8. [Cache refresh and uninstall](#8-cache-refresh-and-uninstall)

---

## 1. What "integration" writes

Desktop integration is the step that turns a stored `Site` into files the desktop environment understands. It runs on `site install` and `site update`, and is implemented per-platform; the Linux backend is `src/integrations/implementation/linux.rs`. Every file is keyed by the same identifier, `classid = FFPWA-<ulid>` (`linux.rs:67`), so an app's launcher, icons, autostart entry, and KDE rule all share one stem.

| File | Path |
| --- | --- |
| Launcher | `~/.local/share/applications/FFPWA-<ulid>.desktop` |
| Icons | `~/.local/share/icons/hicolor/<size>/apps/FFPWA-<ulid>.png` (+ `scalable/`, `symbolic/`) |
| Autostart | `~/.config/autostart/FFPWA-<ulid>.desktop` (only when launch-on-login) |
| KDE rule | `~/.config/kwinrulesrc` → section `[ffwebapps-FFPWA-<ulid>]` (KDE only) |

The base directories come from the `directories` crate (`~/.local/share`, `~/.config`), not from ffwebapps' own `userdata` — these live in the standard XDG locations so the desktop picks them up.

## 2. The .desktop launcher

`create_desktop_entry` (`linux.rs:199-303`) writes the launcher. Its template is straightforward, but two lines carry the weight:

```ini
[Desktop Entry]
Type=Application
Name={name}
Comment={description}
Keywords={keywords};
Categories=GTK;{categories};
Icon=FFPWA-<ulid>
Exec={exe} site launch <ulid> --protocol %u
Actions={actions}
MimeType={protocols}
Terminal=false
StartupNotify=true
StartupWMClass=org.mozilla.firefox.webapp-<webapp_id>
```

The **`Exec`** line launches the app *through ffwebapps itself* — `ffwebapps site launch <ulid> --protocol %u` — not Firefox directly. That routes every menu/dock launch through the singleton check and tray spawn (see [IPC & the Runtime-Owned Window](ipc-protocol.gen.html)). The `%u` lets the launcher carry a protocol-handler URL when the app is invoked as a handler.

The `{exe}` token is usually just the bare `ffwebapps` binary path, but for non-standard installs (a dev checkout, not `/usr/bin` + `/usr/share/ffwebapps`) it is wrapped to bake in the resolved directories so the launched binary finds its data:

```ini
Exec=env FFPWA_USERDATA=<userdata> FFPWA_SYSDATA=<sysdata> <bin> site launch <ulid> --protocol %u
```

## 3. Window identity: the app_id chain

For the desktop to treat a web app as its own application — distinct dock entry, correct icon, its own alt-tab slot — three identities have to line up. This is the single most important detail of the integration, and it is why `webapp_id` exists.

<div class="diagram-container">
<svg width="100%" viewBox="0 0 900 300" xmlns="http://www.w3.org/2000/svg">
  <style>
    .bg     { fill: #1a1b26; }
    .box    { fill: #24283b; stroke: #3b4261; stroke-width: 1; }
    .hot    { fill: #2a2438; stroke: #e0af68; stroke-width: 1.5; }
    .blu    { fill: #1a2235; stroke: #7aa2f7; stroke-width: 1.5; }
    .lbl    { fill: #c0caf5; font-size: 11px; font-family: 'JetBrains Mono', monospace; }
    .lbl-sm { fill: #c0caf5; font-size: 10px; font-family: 'JetBrains Mono', monospace; }
    .lbl-mut{ fill: #8c92b3; font-size: 9px;  font-family: 'JetBrains Mono', monospace; }
    .lbl-yel{ fill: #e0af68; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-blu{ fill: #7aa2f7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .ln     { stroke: #7dcfff; stroke-width: 1.5; fill: none; }
    .title  { fill: #7aa2f7; font-size: 14px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
  </style>
  <rect x="0" y="0" width="900" height="300" class="bg"/>
  <text x="450" y="26" text-anchor="middle" class="title">webapp_id (UUID) threads three identities together</text>

  <rect x="40" y="60" width="250" height="80" class="hot"/>
  <text x="165" y="82" text-anchor="middle" class="lbl-yel">SiteConfig.webapp_id</text>
  <text x="165" y="102" text-anchor="middle" class="lbl-mut">a UUID v4, minted at install</text>
  <text x="165" y="120" text-anchor="middle" class="lbl-mut">(distinct from the app's ULID)</text>

  <rect x="330" y="40" width="250" height="56" class="box"/>
  <text x="455" y="62" text-anchor="middle" class="lbl-sm">launch argv</text>
  <text x="455" y="80" text-anchor="middle" class="lbl-mut">firefox -taskbar-tab &lt;webapp_id&gt;</text>

  <rect x="330" y="112" width="250" height="56" class="box"/>
  <text x="455" y="134" text-anchor="middle" class="lbl-sm">taskbartabs.json</text>
  <text x="455" y="152" text-anchor="middle" class="lbl-mut">id: &lt;webapp_id&gt; → scope</text>

  <rect x="330" y="184" width="250" height="56" class="box"/>
  <text x="455" y="206" text-anchor="middle" class="lbl-sm">.desktop StartupWMClass</text>
  <text x="455" y="224" text-anchor="middle" class="lbl-mut">org.mozilla.firefox.webapp-&lt;id&gt;</text>

  <rect x="620" y="112" width="240" height="80" class="blu"/>
  <text x="740" y="138" text-anchor="middle" class="lbl-blu">the live window</text>
  <text x="740" y="158" text-anchor="middle" class="lbl-mut">Wayland app_id =</text>
  <text x="740" y="172" text-anchor="middle" class="lbl-mut">org.mozilla.firefox.webapp-&lt;id&gt;</text>
  <text x="740" y="186" text-anchor="middle" class="lbl-mut">→ matches StartupWMClass</text>

  <line x1="290" y1="100" x2="330" y2="68" class="ln"/>
  <line x1="290" y1="100" x2="330" y2="140" class="ln"/>
  <line x1="290" y1="100" x2="330" y2="212" class="ln"/>
  <line x1="580" y1="152" x2="620" y2="152" class="ln"/>
  <line x1="580" y1="212" x2="600" y2="212" class="ln"/>
  <line x1="600" y1="212" x2="600" y2="170" class="ln"/>
  <line x1="600" y1="170" x2="620" y2="170" class="ln"/>
</svg>
</div>

The chain (`linux.rs:226-229`, `components/site.rs:284-299`):

- The launch passes `-taskbar-tab <webapp_id>`, so Firefox gives the window the Wayland `app_id` `org.mozilla.firefox.webapp-<webapp_id>`.
- The `.desktop` file sets `StartupWMClass=org.mozilla.firefox.webapp-<webapp_id>` to that *same* string.
- The desktop matches the window's `app_id` to the launcher's `StartupWMClass`, so the window groups under the right launcher with the right icon.

Separately, `MOZ_APP_REMOTINGNAME=ffwebapps-<ulid>` gives each app a unique Firefox *remoting* identity, which is what makes a relaunch focus the existing window instead of spawning a duplicate (see [IPC & the Runtime-Owned Window](ipc-protocol.gen.html)). When `webapp_id` is absent (a very old app not yet relaunched), `StartupWMClass` falls back to `FFPWA-<ulid>`.

## 4. Icons

`store_icons` (`linux.rs:92-186`) writes the icon set into the hicolor theme, keyed on `classid`:

| Source | Destination |
| --- | --- |
| SVG, purpose `Any` | `icons/hicolor/scalable/apps/FFPWA-<ulid>.svg` |
| SVG, purpose `Monochrome` | `icons/hicolor/symbolic/apps/FFPWA-<ulid>-symbolic.svg` |
| Raster (`Any`) | `icons/hicolor/<w>x<h>/apps/FFPWA-<ulid>.png` at native size |

A 48×48 PNG is mandatory under the Icon Theme Spec; if the manifest supplies none, ffwebapps generates one. Icons are fetched and rendered by `integrations/utils.rs`: manifest icons are normalized and ranked (exact-size → `Any`-size → nearest-larger → nearest-smaller), SVGs are rasterized with `resvg`/`usvg`/`tiny_skia` (with external resource loading disabled), and rasters are resized with Lanczos3. When everything fails, `generate_fallback_icon` renders the app name's first letter in white on a gray background using the bundled `Metropolis-SemiBold.otf`.

## 5. Categories and shortcuts

`Categories=` is built by mapping the site's W3C/manifest categories to FreeDesktop menu categories through a compile-time `phf` map (`categories.rs`). For example `music → Audio;AudioVideo`, `webdevelopment → WebDevelopment;Development;Network`. Matches are accumulated, sorted, de-duplicated, and emitted with a leading `GTK;`.

Manifest **shortcuts** become `[Desktop Action <i>]` blocks appended to the `.desktop` file (`linux.rs:270-296`), each with its own `Exec={exe} site launch <ulid> --url "<url>"` and, optionally, its own icon stored under `FFPWA-<ulid>-<i>`. These are the right-click "jump list" entries a launcher shows for the app.

## 6. Autostart and start-hidden

`create_startup_entry` (`linux.rs:305-333`) manages launch-on-login. When `launch_on_login` is set, it copies the already-written `applications/FFPWA-<ulid>.desktop` into `~/.config/autostart/`. If `start_hidden` is also set, it rewrites the copied `Exec` line, appending `--hidden` so the login launch goes straight to the tray rather than popping the window:

```ini
Exec=… site launch <ulid> --hidden
```

When `launch_on_login` is false, any existing autostart entry is removed. The `--hidden` flag flows through to the runtime as `FFWEBAPPS_START_HIDDEN=1`, which the autoconfig reads to hide the window once it first maps (see [The Firefox Runtime & Autoconfig](runtime.gen.html)). The tray's "Start on login" checkbox toggles exactly this state via `site update --launch-on-login --start-hidden`.

## 7. The KWin position rule

On KDE, `write_kwin_rule` (`linux.rs:372-446`) adds a per-app rule to `~/.config/kwinrulesrc` so KWin restores the window's position across hide/show cycles. It surgically upserts a section and keeps `[General]`'s `rules=`/`count=` in sync:

```ini
[ffwebapps-FFPWA-<ulid>]
Description=ffwebapps: remember window position for <name>
positionrule=4
wmclass=org.mozilla.firefox.webapp-<webapp_id>
wmclassmatch=1
types=1
```

`positionrule=4` is KWin's "Remember" policy: it saves and restores the position of the matching window. No initial `position=` is written, so the first launch gets normal placement; KWin learns the position afterward. After writing, ffwebapps asks KWin to reload its config over D-Bus (`qdbus6` → `org.kde.KWin.reconfigure`).

This rule only acts when a `webapp_id` exists, and only on KDE (`XDG_CURRENT_DESKTOP` contains `KDE`). It complements — but is independent of — the runtime's own off-screen-move hide mechanism, which preserves position even without the rule. The handoff notes flag the rule as somewhat inert in practice and a possible cleanup candidate; the runtime mechanism is what actually guarantees position today.

## 8. Cache refresh and uninstall

After writing (or removing) files, `update_application_cache` (`linux.rs:41-48`) nudges the desktop to notice: it touches the icon directories and runs `update-desktop-database`, `update-mime-database`, `gtk-update-icon-cache`, and `xdg-desktop-menu forceupdate`. Without this, a freshly installed app might not appear in the menu until the next login.

Uninstall is the mirror image (`linux.rs:522-543`): remove the icons (globbed as `…/apps/FFPWA-<ulid>*`), the launcher, the autostart entry, and the KWin rule section, then refresh the caches. Because every artifact shares the `FFPWA-<ulid>` stem, the removal is exact and complete.
