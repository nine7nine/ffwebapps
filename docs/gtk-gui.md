# ffwebapps — GTK Management GUI

This page documents `ffwebapps-gtk`: a GTK4 / libadwaita management app that lives in the same crate as the CLI, calls the library in-process, and drives a running app over the same IPC socket the tray uses.

## Table of Contents

1. [Same crate, second binary](#1-same-crate-second-binary)
2. [Off-thread by construction](#2-off-thread-by-construction)
3. [The window: carousel and hand-built tabs](#3-the-window-carousel-and-hand-built-tabs)
4. [The pages](#4-the-pages)
5. [The app editor and three-state edits](#5-the-app-editor-and-three-state-edits)
6. [Live control over the socket](#6-live-control-over-the-socket)
7. [Appearance and the glass theme](#7-appearance-and-the-glass-theme)
8. [Data flow](#8-data-flow)

---

## 1. Same crate, second binary

`ffwebapps-gtk` is a Linux-only binary behind the `gui` cargo feature, declared in the same `firefoxpwa` crate as the CLI (`Cargo.toml:32-35`). That placement is forced by the data model: the core types are `#[non_exhaustive]` and only constructible *in-crate*, so a separate program could not build a `SiteUpdateCommand` or a `SiteConfig` at all. By living in the same crate, the GUI reuses the CLI's exact code paths — manifest fetching, system integration, storage writes — rather than shelling out or parsing CLI text.

`main.rs` is a 43-line bootstrap: it sets the reverse-DNS app id `io.github.nine7nine.ffwebapps`, routes the `log` crate (which the library commands log through) into `~/.local/share/ffwebapps/ffwebapps-gui.log` via `simplelog::WriteLogger`, builds an `adw::Application`, and wires `ui::window::build` as the `activate` handler. The `gui` feature pulls in four optional deps: `gtk4`, `libadwaita`, `async-channel`, and `sourceview5`.

The module layout mirrors the responsibilities:

| Module | Responsibility |
| --- | --- |
| `main.rs` | Bootstrap: app id, logger, `adw::Application` |
| `core.rs` | The off-thread wrapper layer over the library, plus field-diff helpers and appearance persistence |
| `ipc.rs` | Async Unix-socket client for a running app's runtime socket |
| `ui/window.rs` | Main window, carousel + tab switcher, app list, live-running poll |
| `ui/widgets.rs` | The glass CSS, dark-mode forcing, live appearance provider |
| `ui/*_page.rs`, `ui/*_dialog.rs` | The individual pages and dialogs |

## 2. Off-thread by construction

The cardinal rule of a GTK app is that the main loop must never block — and the library's install/update/launch paths do blocking network I/O and spawn processes. The GUI funnels every such call through one primitive (`core::spawn`, `core.rs:103-118`):

```rust
pub fn spawn<T, W, D>(work: W, on_done: D)
where W: FnOnce() -> Result<T> + Send + 'static,
      D: FnOnce(Result<T>) + 'static {
    let (tx, rx) = async_channel::bounded(1);
    gio::spawn_blocking(move || { let _ = tx.send_blocking(work()); });
    glib::spawn_future_local(async move { on_done(rx.recv().await.unwrap()); });
}
```

`work` runs on a `gio::spawn_blocking` worker thread; its `Result` is marshalled back over an `async-channel` and delivered to `on_done` on the GTK main loop. Every mutating wrapper in `core.rs` follows this shape:

| Wrapper | Library call |
| --- | --- |
| `install_site` | optionally `ProfileCreateCommand._run()`, then `SiteInstallCommand._run()` → new `Ulid` |
| `save_site` | `SiteUpdateCommand.run()`, then a direct `Storage` edit for the two fields the command can't express |
| `launch_site` / `uninstall_site` | `SiteLaunchCommand.run()` / `SiteUninstallCommand{ quiet: true }.run()` |
| `create_profile` / `update_profile` / `remove_profile` | the `Profile*Command`s |
| `install_runtime` / `patch_runtime` / `uninstall_runtime` | the `Runtime*Command`s |
| `save_config` | no command exists — direct `Storage::load` → mutate `Config` → `storage.write` |

`save_site` is a good example of mixing the two paths: it runs `SiteUpdateCommand` for everything the command covers, then does a *direct* `Storage` edit for `external_links` and `allowed_domains` (the two `SiteConfig` fields with no CLI flag) — deliberately *after* the command, so the command's own write doesn't clobber them.

## 3. The window: carousel and hand-built tabs

The window is an `AdwApplicationWindow` → `AdwToolbarView` with a static `AdwHeaderBar` on top and, below it, a hand-built tab bar over an `adw::Carousel` of four pages: **Apps**, **Profiles**, **Runtime**, **Settings** (`window.rs`). The carousel gives real swipeable pages (commit `3c9b671`).

There is a wrinkle worth recording: `AdwViewSwitcher` can only drive an `AdwViewStack`, never a carousel. So the tab bar is built by hand — a centred row of four `gtk::ToggleButton`s chained into one radio group — and kept in sync with the carousel through a re-entrancy guard (`Rc<Cell<bool>>`): a button toggle scrolls the carousel, a carousel page-change sets the matching button active, and the guard stops the two from echoing each other. The carousel claims only the horizontal axis so each page's list still scrolls vertically.

The **Apps** page lists one `AdwPreferencesGroup` per profile, each with an activatable row per site carrying a 32px icon, a "● running" indicator, and a chevron. A 2-second `glib::timeout` poll calls `ipc::is_running(id)` (a cheap socket-connect probe) to light each row's running dot. Activating a row opens the app editor; a separate "Install web app" row opens the install dialog.

## 4. The pages

| Page / dialog | What it does |
| --- | --- |
| **Install dialog** | Manifest URL + optional document URL / name, a profile combo whose last entry is "New profile…", and option switches. Calls `core::install_site` |
| **App editor** | A grouped `AdwDialog` over `SiteConfig` (see §5) with Launch / Uninstall / Save in its header and a live-control group |
| **Profiles page** | Create / edit / remove profiles, each row offering CSS/JS injection. The Nil "Default" shows "Clear", not "Remove" |
| **Injection editor** | A GtkSourceView 5 editor for the per-**profile** `ffwebapps.css` / `ffwebapps.js`, with a banner noting it's shared by every app in the profile and applies on next launch |
| **Runtime page** | Install / use-system-Firefox / patch / uninstall, with the detected version. Actions are body rows (not header buttons) so the window CSD stays static |
| **Settings page** | Appearance (see §7), the global `Config` toggles, and the extra runtime arguments / environment variables |

The injection editor is explicitly per-profile, matching the runtime: the `ffwebapps.css` / `ffwebapps.js` files live at the profile root and are injected into every app that shares the profile (see [The Firefox Runtime & Autoconfig](runtime.gen.html)). It uses a dark GtkSourceView style scheme picked from the first available of `Adwaita-dark` / `oblivion` / `classic-dark` / `solarized-dark`.

## 5. The app editor and three-state edits

The app editor is an `AdwDialog` of grouped sections — Live control, General, Behaviour, Performance, Advanced, and "Update on save" — each mapping to `SiteConfig` fields. Save gathers a `SiteEdits` struct and runs `core::save_site` off-thread, disabling Save and Launch while in flight and toasting the result.

The interesting part is that the editor faithfully reproduces the CLI's three-state update semantics (see [CLI & Command Model](cli.gen.html)) in plain Rust. `opt_opt_string` (`core.rs:228-236`) diffs each widget's current text against the *original* config value:

| Original | Current widget text | Result |
| --- | --- | --- |
| none | empty | `None` — leave (was empty, still empty) |
| `Some(x)` | `x` | `None` — leave (unchanged) |
| any | empty | `Some(None)` — clear to manifest default |
| any | `y` | `Some(Some(y))` — set |

That distinction — "the field was already empty, don't touch it" versus "the user emptied a previously-set field, so actively clear it" — is what lets the GUI send a minimal, correct update instead of blindly overwriting every field. Vec overrides (categories/keywords) use the empty-element reset convention; `external_links` (an `Option<bool>` where unset means on) preserves "unset" rather than hard-coding `true`.

## 6. Live control over the socket

When the app editor opens, it connects to the app's runtime socket and turns its Live-control group into a remote — the same protocol the tray speaks (`ipc.rs`, `live_control.rs`). The GUI identifies as `hello v1 launcher` (not `tray`), so it can monitor and send verbs *without* affecting close-to-tray behaviour.

The socket reader runs on a `std::thread` (a `BufReader::lines()` loop) that pushes `LiveEvent`s — `Hello(pid)`, `Unread(n)`, `State(flags)`, `Disconnected` — into an `async-channel`; a `glib::spawn_future_local` drains them onto the main loop and updates the controls. The ten controls (Show/Hide, Reload, Quit, Copy URL, Open in browser, and the Mute / DND / Suspend / Start-on-login switches) start disabled and enable only once the connection succeeds. An echo guard (`Rc<Cell<bool>> applying`) suppresses the verb that an inbound `state` update would otherwise bounce back when it calls `set_active` on the switches. Closing the dialog calls `LiveConn::close()`, which shuts the stream down and ends the reader thread.

## 7. Appearance and the glass theme

The GUI forces dark mode (`adw::ColorScheme::ForceDark`) and installs a "glass" stylesheet adapted from the Poxicle configurator (commit `451ee43`). The mechanism: the `window` gets one translucent tint (`rgba(20,20,26,0.92)`) and every container stacked on it (`box`, `list`, `row`, `headerbar`, …) is forced transparent so the tint shows through uniformly.

A second CSS provider at `USER` priority (above the static `APPLICATION` sheet) lets the user retune the look live (commit `50fc393`): an `Appearance { opacity, glass, accent }` model, persisted separately at `~/.local/share/ffwebapps/gui-appearance.json`, redefines `@accent_bg_color` / `@accent_color` and the window tint, so every static rule that references the accent re-resolves for free. The settings page wires an opacity spin row and two `ColorDialogButton`s to apply changes both live and to disk.

Several small theming fixes are recorded in the history and worth noting as cautionary detail: the switch knob is forced white because styling it with the accent made it invisible on the on-state track (`e306ea7`); dialogs get an opaque surface so they read as a distinct sheet over the dimmed backdrop (`104ee6b`); and the accent is applied only to a section's *title* label, not every row header (`a060526` / `0347f0a`).

## 8. Data flow

Storage is the single source of truth, and every page follows the same one-directional cycle:

```text
Storage::load ─▶ populate Adwaita widgets ─▶ user edits
      ▲                                          │
      │                                          ▼
   refresh_list ◀── on_done: toast + refresh ── core::spawn(work)
                                                   │ runs .run()/._run()
                                                   │ or Storage::write
                                                   ▼ on a worker thread
```

The only genuinely live, non-Storage channel is the IPC socket, which streams runtime state into the editor's Live-control group and feeds the per-row "● running" dots. Everything else is: load from `config.json`, edit widgets, gather into a plain struct, run a library command off-thread, toast the result, and reload the list.
