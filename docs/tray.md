# ffwebapps — System Tray

This page documents `ffwebapps-tray`: a `StatusNotifierItem` that shows each app's icon, unread badge, and menu, and is built around one deliberate constraint — it can spawn nothing and holds no authoritative state of its own.

## Table of Contents

1. [A tray that does almost nothing](#1-a-tray-that-does-almost-nothing)
2. [The flock singleton](#2-the-flock-singleton)
3. [Connecting to the runtime](#3-connecting-to-the-runtime)
4. [State is pushed, never polled](#4-state-is-pushed-never-polled)
5. [The menu](#5-the-menu)
6. [Quit and the SIGKILL fallback](#6-quit-and-the-sigkill-fallback)
7. [Surviving a Plasma restart](#7-surviving-a-plasma-restart)
8. [How it gets launched](#8-how-it-gets-launched)

---

## 1. A tray that does almost nothing

`ffwebapps-tray` (`src/bin/ffwebapps-tray.rs`) is a `StatusNotifierItem` built on the `ksni` crate. Its entire job is to draw an icon with an unread badge and a menu, and to translate clicks into socket verbs. The defining property is what it *cannot* do:

> **The tray spawns nothing.** There is no `std::process::Command` anywhere in the file. The only process-control primitive is a single `libc::kill` syscall used by Quit.

This is a direct response to a class of bugs where a tray "helpfully" relaunched its app and produced orphans or zombie windows. Because the tray can never launch anything, it can never relaunch, duplicate, or strand a process. Every user action — show, hide, reload, mute, quit — is a pure socket write; the runtime does the actual work.

The tray is a thin remote of the runtime. The runtime is the single source of truth for window visibility and every toggle; the tray merely reflects what the runtime pushes and forwards what the user clicks.

<div class="diagram-container">
<svg width="100%" viewBox="0 0 900 360" xmlns="http://www.w3.org/2000/svg">
  <style>
    .bg     { fill: #1a1b26; }
    .tray   { fill: #1a2235; stroke: #7aa2f7; stroke-width: 1.5; }
    .rt     { fill: #2a1f35; stroke: #bb9af7; stroke-width: 1.5; }
    .box    { fill: #24283b; stroke: #3b4261; stroke-width: 1; }
    .lbl    { fill: #c0caf5; font-size: 11px; font-family: 'JetBrains Mono', monospace; }
    .lbl-sm { fill: #c0caf5; font-size: 10px; font-family: 'JetBrains Mono', monospace; }
    .lbl-mut{ fill: #8c92b3; font-size: 9px;  font-family: 'JetBrains Mono', monospace; }
    .lbl-blu{ fill: #7aa2f7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-pur{ fill: #bb9af7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-cy { fill: #7dcfff; font-size: 10px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .ln     { stroke: #7dcfff; stroke-width: 1.5; fill: none; }
    .title  { fill: #7aa2f7; font-size: 14px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
  </style>
  <rect x="0" y="0" width="900" height="360" class="bg"/>
  <text x="450" y="26" text-anchor="middle" class="title">tray = StatusNotifierItem + reader thread, both over one socket</text>

  <rect x="40" y="56" width="360" height="250" class="tray"/>
  <text x="220" y="78" text-anchor="middle" class="lbl-blu">ffwebapps-tray  (flock singleton)</text>
  <rect x="60" y="92" width="320" height="58" class="box"/>
  <text x="220" y="112" text-anchor="middle" class="lbl-sm">ksni StatusNotifierItem</text>
  <text x="220" y="128" text-anchor="middle" class="lbl-mut">icon + "mail-unread" overlay badge</text>
  <text x="220" y="142" text-anchor="middle" class="lbl-mut">menu built from the latest state snapshot</text>
  <rect x="60" y="160" width="320" height="54" class="box"/>
  <text x="220" y="180" text-anchor="middle" class="lbl-sm">reader thread</text>
  <text x="220" y="196" text-anchor="middle" class="lbl-mut">BufReader::lines() → State (RwLock)</text>
  <text x="220" y="208" text-anchor="middle" class="lbl-mut">EOF ⇒ std::process::exit(0)</text>
  <rect x="60" y="224" width="320" height="58" class="box"/>
  <text x="220" y="244" text-anchor="middle" class="lbl-sm">send(verb)</text>
  <text x="220" y="260" text-anchor="middle" class="lbl-mut">write "&lt;verb&gt;\n" — the ONLY outward action</text>
  <text x="220" y="274" text-anchor="middle" class="lbl-mut">no Command::new anywhere</text>

  <rect x="520" y="110" width="340" height="150" class="rt"/>
  <text x="690" y="132" text-anchor="middle" class="lbl-pur">Firefox runtime  (socket server)</text>
  <text x="690" y="154" text-anchor="middle" class="lbl-mut">authoritative: hidden / muted / dnd /</text>
  <text x="690" y="168" text-anchor="middle" class="lbl-mut">suspend / autostart / unread</text>
  <text x="690" y="190" text-anchor="middle" class="lbl-mut">pushes: hello v1 &lt;pid&gt;, unread, state</text>
  <text x="690" y="212" text-anchor="middle" class="lbl-mut">receives: toggle, reload, quit,</text>
  <text x="690" y="226" text-anchor="middle" class="lbl-mut">mute/dnd/suspend/autostart-toggle, …</text>

  <line x1="400" y1="187" x2="520" y2="170" class="ln"/>
  <text x="410" y="160" class="lbl-cy">unread / state  ←</text>
  <line x1="400" y1="253" x2="520" y2="210" class="ln"/>
  <text x="410" y="280" class="lbl-cy">verbs  →</text>
</svg>
</div>

## 2. The flock singleton

Exactly one tray may exist per app. The guard is an advisory file lock (`acquire_singleton`, `ffwebapps-tray.rs:92-98`): the tray opens `$XDG_RUNTIME_DIR/ffwebapps-tray-<id>.lock` and takes `flock(fd, LOCK_EX | LOCK_NB)`. If the lock is already held, a tray already owns this app and the new process exits immediately. The lock file is held for the whole process lifetime.

`flock` was chosen because the kernel releases the lock automatically when the process exits or dies — there are no stale lock files to clean up and no PID-reuse hazard. It is the same "let the OS track liveness" philosophy as the runtime's socket.

## 3. Connecting to the runtime

The tray derives the socket path purely from the ULID it was given — `$XDG_RUNTIME_DIR/ffwebapps-<id>.sock` — no path is ever passed in. Because the tray is usually spawned right after Firefox, before Firefox has bound the socket, `connect()` retries (`ffwebapps-tray.rs:104-113`):

- Poll every **250 ms** for up to **30 seconds**.
- On success, send `hello v1 tray\n` and start the reader.
- On timeout, log "runtime never came up → tray exiting" and exit.

The `hello v1 tray` handshake is what tells the runtime to treat this connection as a tray — which is what enables close-to-tray (the runtime only intercepts a window close *while a tray client is connected*; see [IPC & the Runtime-Owned Window](ipc-protocol.gen.html)).

## 4. State is pushed, never polled

A dedicated reader thread consumes the socket line-by-line (`ffwebapps-tray.rs:329-374`) and updates a shared `State` behind an `RwLock`. It parses three message kinds:

- `hello v1 <pid>` → records the runtime PID (needed for the Quit fallback).
- `unread <n>` → updates the badge, redrawing only when the count changed.
- `state hidden=… muted=… dnd=… suspend=… autostart=…` → updates the five toggle flags and refreshes the menu.

The unread badge is rendered as a quiet `mail-unread` overlay icon (no pulsing) and surfaced again in the tooltip as "N unread". The tray holds no state of its own and never asks the runtime for anything — it only reacts to pushes. When the runtime closes the socket, `BufReader::lines()` ends, and the thread logs "runtime closed the connection → tray exiting" and calls `std::process::exit(0)`. The flock then releases automatically. This is what binds the tray's lifetime to its app's.

## 5. The menu

The menu (`ffwebapps-tray.rs:210-283`) is rebuilt from the latest state snapshot on each draw, so labels and checkmarks always reflect the runtime:

| Item | Sends | Notes |
| --- | --- | --- |
| **Show / Hide** | `toggle` | Label flips on the `hidden` flag |
| **Reload** | `reload` | |
| **Mute** | `mute-toggle` | Checkmark = `muted` |
| **Do not disturb** | `dnd-toggle` | Checkmark = `dnd` |
| **Suspend when hidden** | `suspend-toggle` | Checkmark = `suspend` |
| **Start on login** | `autostart-toggle` | Checkmark = `autostart` |
| **Copy URL** | `copy-url` | |
| **Open page in browser** | `open-browser` | |
| **Quit** | (see §6) | |

Note that Show/Hide always sends `toggle`, never `show`/`hide` directly — only the runtime knows the true visibility, so it makes the decision. The checkable items are interesting because the runtime *persists* most of them as Firefox prefs (`ffwebapps.muted`, `dom.webnotifications.enabled`, `ffwebapps.suspendWhenHidden`), and "Start on login" is backed by the actual existence of the autostart `.desktop` file — so the checkmark can never lie about reality.

## 6. Quit and the SIGKILL fallback

Quit is the one place the tray reaches past the socket, and it does so with a syscall, not a subprocess (`quit_app`, `ffwebapps-tray.rs:159-169`):

1. Snapshot the runtime PID (learned from the `hello v1 <pid>` line).
2. Send `quit` over the socket.
3. Sleep **5 seconds** to let the runtime tear down cleanly.
4. If the PID is known, `libc::kill(pid, SIGKILL)` as a guaranteed teardown.
5. `std::process::exit(0)`.

The fallback exists so Quit always wins: even if the runtime hangs during shutdown, the app cannot survive as a trayless, unreachable window. Using a direct `kill` syscall (rather than spawning `pkill`) keeps the "tray spawns nothing" invariant intact — and avoids the classic `pkill -f ffwebapps-tray` footgun of matching one's own shell.

## 7. Surviving a Plasma restart

`ksni`'s `service.run()` returns when the StatusNotifier host (e.g. `plasmashell`) goes away — which happens on a Plasma restart. The tray wraps it in a loop (`ffwebapps-tray.rs:380-389`): on return it waits 500 ms and re-creates the service, so the icon reappears once the new host is up. The live `Handle` is swapped into a shared slot so the reader thread always pushes badge/menu refreshes to the current generation.

## 8. How it gets launched

The tray is spawned only by `spawn_tray` (`console/site.rs:45-68`), from two places: after a normal launch, and on the already-running focus path (so a relaunch that just focuses an existing window still guarantees a tray exists). It is invoked as:

```text
ffwebapps-tray --id <ULID> --name <App Name> --icon FFPWA-<ULID>
```

The binary is discovered next to the running `ffwebapps` (via `current_exe()`), falling back to `dirs.executables`. That ordering is deliberate: the `.desktop` launcher does not set `FFPWA_EXECUTABLES`, so a menu/taskbar launch would otherwise resolve the tray to the wrong directory and silently fail to start. `--wmclass` and `--exec` are still accepted but ignored, for compatibility with older launchers.
