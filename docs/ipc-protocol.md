# ffwebapps — IPC & the Runtime-Owned Window

This page documents the single Unix socket that ties a running web app to its tray and launcher: the wire protocol, the runtime-owned window model, close-to-tray, and the hard-won KWin hide/show mechanism.

## Table of Contents

1. [One socket, one invariant](#1-one-socket-one-invariant)
2. [The wire protocol](#2-the-wire-protocol)
3. [The server: nsIServerSocket in the runtime](#3-the-server-nsiserversocket-in-the-runtime)
4. [The clients](#4-the-clients)
5. [The singleton launcher](#5-the-singleton-launcher)
6. [Close-to-tray](#6-close-to-tray)
7. [Hide and show](#7-hide-and-show)
8. [Hard-won gotchas](#8-hard-won-gotchas)

---

## 1. One socket, one invariant

A running web app exposes exactly one IPC boundary: a Unix-domain socket at `$XDG_RUNTIME_DIR/ffwebapps-<ULID>.sock` (falling back to `/tmp` when `XDG_RUNTIME_DIR` is unset). The socket is **served by the Firefox runtime itself** — the privileged `_autoconfig.cfg` binds it — and consumed by thin clients: the tray, the launcher, and the GTK GUI.

The architecture rests on a single invariant:

> **The runtime is alive if and only if the socket accepts a connection.**

There are no pidfiles and no sentinel files. A leftover socket file from a crash refuses connections, so a failed connect *is* the liveness check. This one fact powers three separate behaviours — the singleton launcher (don't open a duplicate), the tray's lifetime (exit on EOF), and the GUI's running/hidden status dot — without any of them tracking a PID.

<div class="diagram-container">
<svg width="100%" viewBox="0 0 900 430" xmlns="http://www.w3.org/2000/svg">
  <style>
    .bg     { fill: #1a1b26; }
    .rt     { fill: #2a1f35; stroke: #bb9af7; stroke-width: 1.5; }
    .cl     { fill: #1a2235; stroke: #7aa2f7; stroke-width: 1.5; }
    .box    { fill: #24283b; stroke: #3b4261; stroke-width: 1; }
    .sock   { fill: #16242b; stroke: #7dcfff; stroke-width: 1.5; }
    .lbl    { fill: #c0caf5; font-size: 11px; font-family: 'JetBrains Mono', monospace; }
    .lbl-sm { fill: #c0caf5; font-size: 10px; font-family: 'JetBrains Mono', monospace; }
    .lbl-mut{ fill: #8c92b3; font-size: 9px;  font-family: 'JetBrains Mono', monospace; }
    .lbl-pur{ fill: #bb9af7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-blu{ fill: #7aa2f7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-cy { fill: #7dcfff; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .ln     { stroke: #7dcfff; stroke-width: 1.5; fill: none; }
    .ln-v   { stroke: #9ece6a; stroke-width: 1.5; fill: none; }
    .title  { fill: #7aa2f7; font-size: 14px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
  </style>
  <rect x="0" y="0" width="900" height="430" class="bg"/>
  <text x="450" y="26" text-anchor="middle" class="title">the one socket — runtime is server, everyone else is a client</text>

  <!-- runtime server -->
  <rect x="300" y="50" width="300" height="86" class="rt"/>
  <text x="450" y="72" text-anchor="middle" class="lbl-pur">Firefox runtime  (_autoconfig.cfg)</text>
  <text x="450" y="90" text-anchor="middle" class="lbl-mut">nsIServerSocket.initWithFilename(0o600)</text>
  <text x="450" y="105" text-anchor="middle" class="lbl-mut">owns: window, hide/show, unread, toggles</text>
  <text x="450" y="122" text-anchor="middle" class="lbl-mut">single source of truth</text>

  <!-- socket -->
  <rect x="280" y="172" width="340" height="40" class="sock"/>
  <text x="450" y="190" text-anchor="middle" class="lbl-cy">$XDG_RUNTIME_DIR/ffwebapps-&lt;ULID&gt;.sock</text>
  <text x="450" y="204" text-anchor="middle" class="lbl-mut">newline-delimited text; versioned "v1"</text>

  <line x1="450" y1="136" x2="450" y2="172" class="ln"/>
  <text x="460" y="158" class="lbl-mut">serves</text>

  <!-- clients -->
  <rect x="40"  y="270" width="240" height="120" class="cl"/>
  <text x="160" y="292" text-anchor="middle" class="lbl-blu">ffwebapps-tray</text>
  <text x="160" y="310" text-anchor="middle" class="lbl-mut">hello v1 tray</text>
  <text x="160" y="326" text-anchor="middle" class="lbl-mut">→ toggle / reload / quit / …</text>
  <text x="160" y="342" text-anchor="middle" class="lbl-mut">← unread / state</text>
  <text x="160" y="362" text-anchor="middle" class="lbl-mut">EOF ⇒ tray exits</text>
  <text x="160" y="378" text-anchor="middle" class="lbl-mut">draws icon + badge + menu</text>

  <rect x="330" y="270" width="240" height="120" class="cl"/>
  <text x="450" y="292" text-anchor="middle" class="lbl-blu">launcher  (site launch)</text>
  <text x="450" y="310" text-anchor="middle" class="lbl-mut">connect succeeds?</text>
  <text x="450" y="326" text-anchor="middle" class="lbl-mut">yes → hello v1 launcher; show</text>
  <text x="450" y="342" text-anchor="middle" class="lbl-mut">→ focus, don't duplicate</text>
  <text x="450" y="362" text-anchor="middle" class="lbl-mut">no → spawn the runtime</text>

  <rect x="620" y="270" width="240" height="120" class="cl"/>
  <text x="740" y="292" text-anchor="middle" class="lbl-blu">ffwebapps-gtk</text>
  <text x="740" y="310" text-anchor="middle" class="lbl-mut">hello v1 launcher</text>
  <text x="740" y="326" text-anchor="middle" class="lbl-mut">live-control panel +</text>
  <text x="740" y="342" text-anchor="middle" class="lbl-mut">per-row running dot</text>
  <text x="740" y="362" text-anchor="middle" class="lbl-mut">monitors, sends verbs</text>

  <line x1="160" y1="270" x2="160" y2="212" class="ln-v"/>
  <line x1="160" y1="212" x2="280" y2="200" class="ln-v"/>
  <line x1="450" y1="270" x2="450" y2="212" class="ln-v"/>
  <line x1="740" y1="270" x2="740" y2="212" class="ln-v"/>
  <line x1="740" y1="212" x2="620" y2="200" class="ln-v"/>
</svg>
</div>

## 2. The wire protocol

The protocol is newline-delimited UTF-8 text, versioned `v1` in every handshake. Each connection opens with the client identifying itself; the runtime immediately replies with a three-line state dump, then pushes updates on change.

**Client → runtime**

| Message | Meaning |
| --- | --- |
| `hello v1 tray` | Identify as the tray (enables close-to-tray; see §6) |
| `hello v1 launcher` | Identify as a launcher / monitor (no special handling) |
| `show` / `hide` / `toggle` | Map / unmap / flip the window |
| `quit` | Force-quit the runtime (`Services.startup.quit(eForceQuit)`) |
| `reload` | Reload the current page |
| `mute-toggle` | Flip audio mute (persisted in `ffwebapps.muted`) |
| `dnd-toggle` | Flip do-not-disturb (`dom.webnotifications.enabled`) |
| `suspend-toggle` | Flip "suspend when hidden" (`ffwebapps.suspendWhenHidden`) |
| `autostart-toggle` | Toggle the launch-on-login autostart entry |
| `copy-url` | Copy the current page URL to the clipboard |
| `open-browser` | Open the current page in the default browser |

**Runtime → client**

| Message | When |
| --- | --- |
| `hello v1 <pid>` | Once, on connect — the runtime's process ID |
| `unread <n>` | On connect and whenever the unread count changes |
| `state hidden=<0\|1> muted=<0\|1> dnd=<0\|1> suspend=<0\|1> autostart=<0\|1>` | On connect and after any toggle changes |

The unread count is scraped from the window title: a repeating 1-second timer matches a leading `(N)` in `document.title` and broadcasts `unread N` when it changes (`_autoconfig.cfg:932-952`). Because the page keeps running even while the window is hidden, the badge stays accurate in the tray.

A subtle but important detail: the tray never sends `show` or `hide` directly — it always sends `toggle` and lets the runtime decide, because the runtime is the only component that knows the true hidden state (see §8).

## 3. The server: nsIServerSocket in the runtime

The socket lives entirely inside `_autoconfig.cfg` (`377-956`). At startup the cfg derives the app ID from `MOZ_APP_REMOTINGNAME` (`ffwebapps-<ulid>` → `<ulid>`), builds the socket path, and binds:

```javascript
srv.initWithFilename(f, 0o600, -1);   // 0600 = owner-only
srv.asyncListen(_listener);
```

A few server details matter:

- **Leftover-file cleanup.** A stale socket file from a crashed run blocks the bind, so the cfg removes it first (`_autoconfig.cfg:824-828`). A *live* duplicate is impossible here because the launcher only spawns a runtime when the socket refuses connections, and Firefox's own per-profile lock backs that up.
- **Early bind with retry.** The cfg binds as early as it can so a relaunch-while-starting is deduplicated; if the socket service isn't up yet it retries on the `final-ui-startup` observer (`_autoconfig.cfg:840-851`).
- **Per-client async read loop.** Each accepted client gets an async input stream; `available() == 0` (EOF) or a throw drops the client. Bytes are accumulated and split on `\n`, each line trimmed and dispatched (`_autoconfig.cfg:762-789`).
- **Teardown on quit.** A `quit-application` observer closes the server, drops every client, and removes the socket file (`_autoconfig.cfg:853-866`).

The runtime holds the authoritative state for every client; clients never poll and never hold state of their own — they reflect `unread` / `state` pushes and forward user intent as verbs.

## 4. The clients

**The tray** (`ffwebapps-tray.rs`) connects, sends `hello v1 tray`, and runs a reader thread over `BufReader::lines()`. It mirrors the runtime's `state`/`unread` into a shared struct and redraws the icon and menu. When the runtime closes the socket the iterator ends, and the tray calls `std::process::exit(0)` — its lifetime is bound to the app's. It is documented in detail in [System Tray](tray.gen.html).

**The launcher** (`console/site.rs:29-41`) is fire-and-forget: it connects, writes `hello v1 launcher` plus optionally `show`, and never reads a reply. See §5.

**The GTK GUI** (`ffwebapps-gtk/ipc.rs`) identifies as a `launcher` (not a `tray`), so it can monitor state and send verbs *without* triggering close-to-tray semantics. Its reader runs on a `std::thread` that pushes `LiveEvent`s into an `async-channel`, which a `glib::spawn_future_local` drains onto the GTK main loop. An echo guard prevents an inbound `state` update from bouncing a verb back out. See [GTK Management GUI](gtk-gui.gen.html).

## 5. The singleton launcher

Single-instance behaviour falls straight out of the liveness invariant. When you launch an app — from the menu, the dock, or `site launch <ULID>` — the console first probes the socket (`runtime_show`, `console/site.rs:29-41`):

```text
connect $XDG_RUNTIME_DIR/ffwebapps-<ULID>.sock
  ├─ fails   → app not running → write registry + prefs, spawn the runtime
  └─ succeeds → app already running → send "show", spawn the tray if needed, return
```

If there is no explicit target URL or protocol and the connect succeeds, the launcher sends `show` to focus the existing window and exits without spawning a second Firefox (`SiteLaunchCommand::run`, `console/site.rs:93-101`). This avoids the "open in another window / use here" prompt that a duplicate taskbar-tab window would trigger for a single-page app. A `--hidden` relaunch sends only the handshake, so it doesn't un-hide a backgrounded app.

The other half of single-instance is the unique `MOZ_APP_REMOTINGNAME=ffwebapps-<ulid>` set at launch (`components/site.rs:299`): without it the app would share Firefox's default `firefox` remoting name and could both intercept external-link launches meant for the user's browser *and* let a relaunch attach to the wrong instance.

## 6. Close-to-tray

The window's X button is intercepted and turned into a hide — but only while a tray client is connected to bring it back. Otherwise the close proceeds for real, so the X can never trap the window with no way to restore it.

Taskbar-tab windows take an early return in Firefox's `warnAboutClosingWindow` and never fire `browser-lastwindow-close-requested`, so the cfg overrides the actual close path instead (`_autoconfig.cfg:875-927`). It hooks all three routes a close can travel — the compositor `close` event, `WindowIsClosing()`, and `window.close()` — and each checks one predicate:

```javascript
const intercept = () => isTab() && !_quitting && _trayConnected();
```

So a close becomes a hide only when the window is a taskbar-tab, the runtime is not already quitting, and a tray is connected. `quit` sets `_quitting = true` first, so a real quit is never vetoed. This is also where a `--hidden` autostart launch is honoured: once the window maps, a short timer hides it to the tray.

## 7. Hide and show

Hiding looks trivial but is the most-debugged part of the system. The naive approach — unmap the toplevel via `nsIBaseWindow.visibility` — works everywhere but **loses the window's position on KWin Wayland**: a remapped toplevel is a *new* window to the compositor, and KWin re-places it. So the runtime uses two mechanisms and picks per-compositor (`_autoconfig.cfg:474-580`):

| Compositor | Mechanism | Effect |
| --- | --- | --- |
| **KWin (KDE)** | Move the window off-screen by a fixed `-50000` x-offset with `skipTaskbar`/`skipSwitcher`/`skipPager` set | The surface stays mapped, so geometry, position, and rendering are preserved exactly |
| **Other** | Unmap via `nsIBaseWindow.visibility` | Hides reliably everywhere; re-show placement is left to the compositor |

On KWin the cfg writes a tiny KWin script to `$XDG_RUNTIME_DIR/ffwebapps-<id>.kwin.js`, matched to this app's window by the runtime's PID and a `webapp` resource class, and runs it over D-Bus (`qdbus6`/`qdbus`). The off-screen moves are **gated on the window's actual position** (`if(w.frameGeometry.x>-10000)…`), making them idempotent: a stale hidden-state flag can never push a visible window off-screen or pull a hidden one twice. On KDE an install-time `kwinrulesrc` "Remember position" rule (`positionrule=4`) further helps KWin restore placement — see [Desktop Integration](desktop-integration.gen.html).

An optional "suspend when hidden" mode gives the page background-tab semantics while hidden — `docShellIsActive = false` and `renderLayers = false` (`_autoconfig.cfg:602-612`) — throttling timers and stopping layer rendering to save CPU/GPU, *without* killing WebSockets, so chat stays connected. It is fully restored on show.

## 8. Hard-won gotchas

The comments in `_autoconfig.cfg` and `HANDOFF.md` record several traps that shaped the current design:

1. **`nsIBaseWindow.visibility` cannot be read back.** `AppWindow::GetVisibility` hardcodes `true` (Mozilla bug 306245). The runtime therefore tracks `_hiddenFlag` itself; trusting the getter would make `toggle` always choose "hide".
2. **Unmap/remap loses position on KWin Wayland.** Empirically `500,300 → remap → 1165,811`. Hence the off-screen-move mechanism that keeps the surface mapped.
3. **KWin runs loaded scripts asynchronously.** The `Script.run` D-Bus call returns *before* KWin executes the body, so the old "load && run && unload" sequence raced — the trailing unload tore the script down before it ran, and a single toggle did nothing. The fix uses a stable per-app plugin name, unloads any leftover *first* (forcing a fresh read), loads, runs, and leaves it loaded for the next call to clean up. Under XWayland the unmap fallback masked this; under native Wayland the fallback is a no-op, so the bug surfaced as dead hide/show (commit `a908e16`).
4. **Mixed binary versions cause "app relaunches after quit" bugs.** Always install one version and make sure `.desktop` `Exec` lines point at the installed binary, not `target/debug`.

These are the reason the runtime owns its own window: every workaround above needs to read or change Firefox-internal state, which only privileged in-process JS can do reliably.
