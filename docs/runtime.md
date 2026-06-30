# ffwebapps — The Firefox Runtime & Autoconfig

This page documents the running app itself: how ffwebapps drives Firefox's first-party Web Apps feature, and the privileged `_autoconfig.cfg` JavaScript that turns a plain Firefox into a chromeless, socket-served, link-routing web-app runtime.

## Table of Contents

1. [Web Apps, not chrome patching](#1-web-apps-not-chrome-patching)
2. [How a runtime is assembled](#2-how-a-runtime-is-assembled)
3. [The autoconfig loader chain](#3-the-autoconfig-loader-chain)
4. [The taskbartabs registry](#4-the-taskbartabs-registry)
5. [Profile prefs: user.js](#5-profile-prefs-userjs)
6. [The chromeless titlebar](#6-the-chromeless-titlebar)
7. [What _autoconfig.cfg does](#7-what-_autoconfigcfg-does)
8. [Per-app CSS and JS injection](#8-per-app-css-and-js-injection)
9. [Why this is robust](#9-why-this-is-robust)

---

## 1. Web Apps, not chrome patching

Firefox ≥ 151 ships a first-party **Web Apps (Taskbar Tabs)** feature. Launch it with `firefox -taskbar-tab <id>` and Firefox opens a standalone, minimal-UI window whose Wayland `app_id` is `org.mozilla.firefox.webapp-<id>` — a window the desktop treats as its own application. ffwebapps' entire approach is to *enable and configure* that feature rather than replace Firefox's UI.

The contrast with the usual site-specific-browser approach is the whole point. Instead of overwriting `browser.xhtml` and re-applying that patch against every Firefox release, ffwebapps uses three supported extension points, each owned by a different file:

| Mechanism | File | Responsibility |
| --- | --- | --- |
| **Autoconfig** (enterprise policy JS) | `_autoconfig.cfg` | Enable the feature; serve the tray socket; own hide/show; route links; inject CSS/JS |
| **Profile `user.js`** | `<profile>/user.js` | Per-app prefs: link allow-list, performance knobs, UA override |
| **`userChrome.css`** | `<profile>/chrome/userChrome.css` | Strip the last toolbar pixels for a chromeless titlebar |

Nothing here is a private API or a binary patch. Autoconfig is the mechanism enterprises use to lock down Firefox; `user.js` and `userChrome.css` are documented customization hooks.

## 2. How a runtime is assembled

The "runtime" is a Firefox install that ffwebapps owns under `~/.local/share/ffwebapps/runtime/`. On Linux the preferred mode is `--link`, which avoids a second copy of Firefox entirely (`Runtime::link`, `runtime.rs:344-399`):

- `defaults/` → a `defaults/pref/` directory is created and `channel-prefs.js` is symlinked.
- `firefox` and `firefox-bin` → **copied** as real files (a symlinked main binary misbehaves).
- everything else under `/usr/lib/firefox/` → **symlinked** into the runtime directory.

The result is a Firefox that tracks the system package's updates but lives in ffwebapps' directory, where it can carry the autoconfig. The alternative, `runtime install` without `--link`, downloads an official Mozilla build and unpacks it instead. `Config::use_linked_runtime` records which mode is active.

`Runtime::patch` (`runtime.rs:410-496`) copies `sysdata/userchrome/runtime/` into the runtime directory and (on Linux) chmods the files to `0o644`. That copied payload is what makes the Firefox an ffwebapps runtime: the autoconfig loader and the autoconfig itself.

<div class="diagram-container">
<svg width="100%" viewBox="0 0 920 470" xmlns="http://www.w3.org/2000/svg">
  <style>
    .bg     { fill: #1a1b26; }
    .src    { fill: #16242b; stroke: #7dcfff; stroke-width: 1.5; }
    .rt     { fill: #1a2235; stroke: #7aa2f7; stroke-width: 1.5; }
    .prof   { fill: #1a2a1a; stroke: #9ece6a; stroke-width: 1.5; }
    .box    { fill: #24283b; stroke: #3b4261; stroke-width: 1; }
    .hot    { fill: #2a2438; stroke: #e0af68; stroke-width: 1.5; }
    .lbl    { fill: #c0caf5; font-size: 11px; font-family: 'JetBrains Mono', monospace; }
    .lbl-sm { fill: #c0caf5; font-size: 10px; font-family: 'JetBrains Mono', monospace; }
    .lbl-mut{ fill: #8c92b3; font-size: 9px;  font-family: 'JetBrains Mono', monospace; }
    .lbl-cy { fill: #7dcfff; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-blu{ fill: #7aa2f7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-grn{ fill: #9ece6a; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-yel{ fill: #e0af68; font-size: 10px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .ln     { stroke: #7dcfff; stroke-width: 1.5; fill: none; }
    .title  { fill: #7aa2f7; font-size: 14px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
  </style>
  <rect x="0" y="0" width="920" height="470" class="bg"/>
  <text x="460" y="26" text-anchor="middle" class="title">runtime patch: how config reaches Firefox</text>

  <!-- source -->
  <rect x="20" y="48" width="270" height="150" class="src"/>
  <text x="36" y="68" class="lbl-cy">sysdata/userchrome/runtime/</text>
  <rect x="36" y="80" width="238" height="34" class="box"/>
  <text x="155" y="95" text-anchor="middle" class="lbl-sm">defaults/pref/autoconfig.js</text>
  <text x="155" y="108" text-anchor="middle" class="lbl-mut">points Firefox at _autoconfig.cfg</text>
  <rect x="36" y="122" width="238" height="34" class="hot"/>
  <text x="155" y="137" text-anchor="middle" class="lbl-yel">_autoconfig.cfg</text>
  <text x="155" y="150" text-anchor="middle" class="lbl-mut">the privileged runtime JS</text>
  <rect x="36" y="164" width="238" height="26" class="box"/>
  <text x="155" y="181" text-anchor="middle" class="lbl-mut">(profile/chrome/userChrome.css)</text>

  <!-- runtime -->
  <rect x="330" y="48" width="270" height="150" class="rt"/>
  <text x="346" y="68" class="lbl-blu">userdata/runtime/  (Firefox)</text>
  <rect x="346" y="82" width="238" height="30" class="box"/>
  <text x="465" y="101" text-anchor="middle" class="lbl-sm">firefox / firefox-bin  (copied)</text>
  <rect x="346" y="118" width="238" height="30" class="box"/>
  <text x="465" y="137" text-anchor="middle" class="lbl-mut">everything else → symlinked /usr/lib/firefox</text>
  <rect x="346" y="154" width="238" height="34" class="hot"/>
  <text x="465" y="169" text-anchor="middle" class="lbl-yel">+ autoconfig.js + _autoconfig.cfg</text>
  <text x="465" y="182" text-anchor="middle" class="lbl-mut">copied in by Runtime::patch</text>

  <line x1="290" y1="139" x2="330" y2="139" class="ln"/>
  <text x="296" y="132" class="lbl-mut">patch</text>

  <!-- profile -->
  <rect x="640" y="48" width="260" height="150" class="prof"/>
  <text x="656" y="68" class="lbl-grn">userdata/profiles/&lt;ulid&gt;/</text>
  <rect x="656" y="82" width="228" height="28" class="box"/>
  <text x="770" y="100" text-anchor="middle" class="lbl-sm">user.js  (write_profile_prefs)</text>
  <rect x="656" y="116" width="228" height="28" class="box"/>
  <text x="770" y="134" text-anchor="middle" class="lbl-sm">chrome/userChrome.css</text>
  <rect x="656" y="150" width="228" height="38" class="box"/>
  <text x="770" y="166" text-anchor="middle" class="lbl-sm">taskbartabs/taskbartabs.json</text>
  <text x="770" y="180" text-anchor="middle" class="lbl-mut">scope + start URL (sync_registry)</text>

  <!-- launch -->
  <rect x="20" y="238" width="880" height="62" class="hot"/>
  <text x="460" y="258" text-anchor="middle" class="lbl-yel">firefox -profile &lt;profile&gt; -taskbar-tab &lt;webapp_id&gt;</text>
  <text x="460" y="273" text-anchor="middle" class="lbl-yel">-new-window &lt;start_url&gt; -container 0</text>
  <text x="460" y="290" text-anchor="middle" class="lbl-mut">env MOZ_APP_REMOTINGNAME=ffwebapps-&lt;ulid&gt;  =  single instance</text>

  <line x1="465" y1="198" x2="465" y2="238" class="ln"/>
  <line x1="770" y1="198" x2="770" y2="218" class="ln"/>
  <line x1="770" y1="218" x2="465" y2="218" class="ln"/>

  <!-- running window -->
  <rect x="20" y="312" width="880" height="138" class="rt"/>
  <text x="460" y="332" text-anchor="middle" class="lbl-blu">the running Web App window  (Firefox loads _autoconfig.cfg at startup)</text>
  <rect x="40"  y="346" width="200" height="90" class="box"/>
  <text x="140" y="364" text-anchor="middle" class="lbl-sm">socket server</text>
  <text x="140" y="380" text-anchor="middle" class="lbl-mut">nsIServerSocket on</text>
  <text x="140" y="392" text-anchor="middle" class="lbl-mut">ffwebapps-&lt;ULID&gt;.sock</text>
  <text x="140" y="410" text-anchor="middle" class="lbl-mut">hide/show, close-to-tray,</text>
  <text x="140" y="422" text-anchor="middle" class="lbl-mut">unread, quit</text>
  <rect x="256" y="346" width="200" height="90" class="box"/>
  <text x="356" y="364" text-anchor="middle" class="lbl-sm">link router</text>
  <text x="356" y="380" text-anchor="middle" class="lbl-mut">content frame script +</text>
  <text x="356" y="392" text-anchor="middle" class="lbl-mut">http-on-modify-request</text>
  <text x="356" y="410" text-anchor="middle" class="lbl-mut">out-of-scope → xdg-open</text>
  <rect x="472" y="346" width="200" height="90" class="box"/>
  <text x="572" y="364" text-anchor="middle" class="lbl-sm">injector</text>
  <text x="572" y="380" text-anchor="middle" class="lbl-mut">ffwebapps.css → USER_SHEET</text>
  <text x="572" y="392" text-anchor="middle" class="lbl-mut">ffwebapps.js → sandbox</text>
  <text x="572" y="410" text-anchor="middle" class="lbl-mut">at DOMContentLoaded</text>
  <rect x="688" y="346" width="192" height="90" class="box"/>
  <text x="784" y="364" text-anchor="middle" class="lbl-sm">prefs in effect</text>
  <text x="784" y="380" text-anchor="middle" class="lbl-mut">taskbarTabs.enabled</text>
  <text x="784" y="392" text-anchor="middle" class="lbl-mut">cookieBehavior=0</text>
  <text x="784" y="410" text-anchor="middle" class="lbl-mut">processCount caps</text>

  <line x1="465" y1="300" x2="465" y2="312" class="ln"/>
</svg>
</div>

## 3. The autoconfig loader chain

Firefox's autoconfig is a two-file handshake. The small `autoconfig.js` is a normal pref file that lives in `defaults/pref/` and tells Firefox which autoconfig to load and how to read it:

```javascript
pref('general.config.filename', '_autoconfig.cfg');
pref('general.config.obscure_value', 0);
pref('general.config.sandbox_enabled', false);
```

`obscure_value = 0` means the cfg is read as plain text (autoconfig historically byte-rotated the file), and `sandbox_enabled = false` grants it full chrome privileges — it runs with access to `Components`, `Services`, the window manager, sockets, and processes. That is exactly the power ffwebapps needs and the reason all of the runtime's behaviour can live in one JS file.

The cfg opens with a set of `defaultPref` calls that establish the runtime baseline (`_autoconfig.cfg:7-30`):

| Pref | Value | Why |
| --- | --- | --- |
| `browser.taskbarTabs.enabled` | `true` | Enables the Web Apps feature itself |
| `toolkit.legacyUserProfileCustomizations.stylesheets` | `true` | Lets the profile's `userChrome.css` load |
| `media.hardware-video-decoding.enabled` | `true` | Reaffirm GPU video decode per app |
| `media.ffvpx-hw.enabled` | `true` | Hardware ffvpx path |
| `network.cookie.cookieBehavior` | `0` | **Disables Total Cookie Protection** so M365/SSO silent re-auth via hidden cross-origin iframes works |
| `dom.ipc.processCount` | `4` | Cap content processes — a single-site app doesn't need general-browsing counts |
| `dom.ipc.processCount.webIsolated` | `1` | Cap isolated cross-origin process pool |

The `cookieBehavior = 0` line is a deliberate trade-off documented in the source: an app profile is single-app, not general browsing, and partitioning third-party cookies breaks the silent token renewal that Microsoft 365 and many SSO providers do through a hidden iframe (the "Sign in does nothing" symptom).

## 4. The taskbartabs registry

For `-taskbar-tab <id>` to resolve, the ID must be registered in the profile's `taskbartabs/taskbartabs.json`, whose shape is validated by Firefox against its own `TaskbarTabs.1.schema.json` on every load. `taskbartabs::sync_registry` (`taskbartabs.rs:74-110`) writes one entry per app:

```json
{
  "version": 1,
  "taskbarTabs": [
    {
      "id": "<webapp_id UUID>",
      "scopes": [{ "hostname": "teams.microsoft.com", "prefix": "/v2" }],
      "userContextId": 0,
      "startUrl": "https://teams.microsoft.com/v2/",
      "name": "Microsoft Teams"
    }
  ]
}
```

The scope is derived from the manifest (`scope_from_site`, `taskbartabs.rs:59-70`): the hostname is the site's domain, and the optional path prefix comes from the manifest `scope` path (dropped when it is just `/`). Entries are upserted by `id`, and the registry tolerates a missing or corrupt file by falling back to a fresh one. This file is rewritten on every launch, so config changes always reach Firefox.

## 5. Profile prefs: user.js

`taskbartabs::write_profile_prefs` (`taskbartabs.rs:178-233`) regenerates the profile's `user.js` at launch. It is owned by ffwebapps — the header literally says *"Managed by ffwebapps — do not edit"* — and carries the per-app behaviour the autoconfig reads back at runtime:

- `ffwebapps.externalLinks.enabled` — from `SiteConfig::external_links` (`unwrap_or(true)`).
- `ffwebapps.allowedDomains` — the comma-joined in-app allow-list, either the user's list or a scope-derived default (see [Link Routing & Scope](link-routing.gen.html)).
- **Software rendering** (when `software_rendering` is set): forces `gfx.webrender.software`, disables `layers.acceleration`, and turns off every hardware video-decode path. This branch *takes precedence over* `hardware_webrtc`.
- **Hardware WebRTC** (when `hardware_webrtc` is set and software rendering is not): forces decode past Firefox's GPU blocklist and enables the hardware VP8 path used by WhatsApp/Meet.
- **User-Agent override** (when `user_agent` is non-empty): writes `general.useragent.override`, with quotes and backslashes escaped.

Because these are written fresh on each launch and read once at startup, a config change "applies on next relaunch" — there is no live pref-watching. See [Performance Tuning](performance.gen.html) for the rendering and scheduling knobs.

## 6. The chromeless titlebar

A taskbar-tab window is already minimal-UI: a slim toolbar with a read-only address pill plus navigation and extension buttons. `userChrome.css` removes the rest while keeping one thing Firefox can't replace. The core rule hides the whole customizable toolbar area except the window controls and the URL container:

```css
:root[taskbartab] #nav-bar-customization-target > *:not(.titlebar-buttonbox-container):not(#urlbar-container) {
  display: none !important;
}
```

The stylesheet then recolours the chrome dark (the manifest's light `theme_color` clashes with dark app UIs), removes Firefox's 40px titlebar spacers (the normal rule that strips them is inert when tabs are hidden), and — importantly — **keeps the site-identity / permission cluster**. Hiding the entire urlbar removed the anchor that camera/mic/geolocation prompts drop down from, so they never appeared and grants could never be made. The fix collapses the urlbar to just `#identity-box` (which contains the permission anchors and the notification-popup box) and strips the address pill, the page-action buttons, and the "remove tab from taskbar" button — leaving a single clickable lock icon, like a Chromium installed-app window.

## 7. What _autoconfig.cfg does

Past the prefs, the bulk of `_autoconfig.cfg` is runtime behaviour. It is organized into independent `try` blocks so a failure in one never disables the others:

| Block | Lines | Responsibility |
| --- | --- | --- |
| External-link backstop | `145-210` | A `http-on-modify-request` observer that cancels out-of-scope top-level loads and hands them to the default browser |
| Content-side router | `217-297` | A frame script that intercepts `target="_blank"` / middle-click / ctrl-click at the source and routes out-of-scope links before Firefox opens a window |
| CSS/JS injection | `299-372` | Reads `ffwebapps.css` / `ffwebapps.js` from the profile and injects them userscript-style |
| Tray IPC + window control | `377-956` | Serves the Unix socket, owns hide/show, close-to-tray, the unread badge, and the persisted toggles |

The link-routing blocks are covered in [Link Routing & Scope](link-routing.gen.html); the socket server, the runtime-owned window, and the KWin hide/show mechanism are covered in [IPC & the Runtime-Owned Window](ipc-protocol.gen.html). The unifying idea is that **the runtime decides everything about its own window** — there is no external script reaching in to move or close it.

One small shared object, `_ffwaShared`, is exported from the link block (`_autoconfig.cfg:34`, `141`) and reused by the socket block so the tray's "Open page in browser" command and the link router invoke the *same* `xdg-open`-based external opener.

## 8. Per-app CSS and JS injection

For Ferdium/WebCatalog-style customization, the runtime injects user files if they exist in the profile (`_autoconfig.cfg:299-372`):

- `ffwebapps.css` is loaded as a **`USER_SHEET`** via `windowUtils.loadSheetUsingURIString` — it is CSP-immune and needs no DOM mutation.
- `ffwebapps.js` runs in a **content-principal sandbox** (`Cu.Sandbox` + `evalInSandbox`) at `DOMContentLoaded` — userscript-style, also not subject to the page's CSP.

The files are read once at startup and baked into a frame script (the contents are JSON-encoded into the script source), so editing them requires an app restart. Crucially this is **per-profile**, not per-app: the files live at the profile root, so every app sharing a profile shares the injection. The GTK GUI surfaces this under the profile with a banner saying as much (see [GTK Management GUI](gtk-gui.gen.html)).

## 9. Why this is robust

The design's durability comes from leaning on maintained Mozilla code and from a few defensive habits in the cfg:

- **No chrome patch to maintain.** The chromeless look is CSS against documented `:root[taskbartab]` selectors; the behaviour is autoconfig JS. A Firefox update doesn't invalidate a binary patch because there isn't one.
- **Independent `try` blocks.** Link routing, injection, and the socket server each fail closed without taking the others down.
- **Config is regenerated at launch.** `taskbartabs.json` and `user.js` are rewritten every launch from `config.json`, so they cannot drift out of sync with the stored `Site`.
- **The runtime is replaceable.** With `--link` the runtime is mostly symlinks to system Firefox; uninstalling and re-linking is cheap and tracks the distro's Firefox updates.
