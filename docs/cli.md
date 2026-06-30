# ffwebapps — CLI & Command Model

This page documents the `ffwebapps` command-line interface and the small command pattern behind it: the clap command tree, the `Run` trait, and the three-state update semantics that let `site update` tell "leave unchanged" apart from "reset to the manifest default".

## Table of Contents

1. [The command tree](#1-the-command-tree)
2. [The Run trait](#2-the-run-trait)
3. [site install](#3-site-install)
4. [site launch](#4-site-launch)
5. [site update](#5-site-update)
6. [site uninstall](#6-site-uninstall)
7. [profile and runtime](#7-profile-and-runtime)
8. [Update-value semantics](#8-update-value-semantics)
9. [HTTP client options](#9-http-client-options)

---

## 1. The command tree

The CLI is a thin clap front-end over the library. `ffwebapps.rs` is essentially `App::parse().run()`; everything else is command structs in `src/console/`. The root `App` (`app.rs:9-24`) has three subcommands, each a small group:

```text
ffwebapps
├── site
│   ├── install <MANIFEST_URL> [--document-url --name --profile …]
│   ├── launch  <ULID> [--url | --protocol] [--hidden]
│   ├── update  <ULID> [many Option<Option<…>> flags]
│   └── uninstall <ULID> [-q]
├── profile
│   ├── list
│   ├── create [--name --description --template]
│   ├── update <ULID> [--name --description --template]
│   └── remove <ULID> [-q]
└── runtime
    ├── install [--link]
    ├── patch
    └── uninstall
```

Each binary in the project calls these same structs in-process — the GUI builds a `SiteUpdateCommand` and calls `.run()` exactly as the CLI does, which is why behaviour, manifest fetching, and system integration are identical across surfaces (see [GTK Management GUI](gtk-gui.gen.html)).

## 2. The Run trait

Dispatch is uniform through a one-method trait (`console/mod.rs:53-55`):

```rust
pub trait Run {
    fn run(&self) -> Result<()>;
}
```

The subcommand enums (`App`, `SiteCommand`, `ProfileCommand`, `RuntimeCommand`) implement `Run` as pure dispatch tables that match the variant and call the inner command's `run()`. Two commands also need to *return a value* — the ULID they just created — so they have an inherent `_run()` alongside the trait method:

| Command | `run()` | `_run()` |
| --- | --- | --- |
| `SiteInstallCommand` | calls `_run()`, discards the ULID | returns `Result<Ulid>` (`site.rs:230-317`) |
| `ProfileCreateCommand` | calls `_run()`, discards the ULID | returns `Result<Ulid>` (`profile.rs:90-108`) |

The CLI path takes `run()` and prints; the GUI calls `_run()` to capture the new ULID (e.g. to select the freshly installed app). Every other command implements `run()` directly.

## 3. site install

`site install` (`app.rs:67-138`) is the heaviest command: it fetches the manifest and icons, builds a `SiteConfig`, writes the `Site`, and runs desktop integration. The positional argument is the manifest URL; `--document-url` defaults to the manifest URL's parent.

| Flag | Purpose |
| --- | --- |
| `--document-url <URL>` | The page itself (defaults to `manifest_url.join(".")`) |
| `--profile <ULID>` | Target profile (defaults to the Nil "Default") |
| `--name` / `--description` | Override manifest metadata |
| `--start-url` / `--icon-url` | Override manifest start URL / icon |
| `--categories` / `--keywords` | Override manifest categories / keywords |
| `--launch-on-login <bool>` / `--launch-on-browser <bool>` | Autostart hooks |
| `--hardware-webrtc` | Force/maximise hardware WebRTC video decode (flag) |
| `--software-rendering` | Disable hardware acceleration entirely (flag) |
| `--scheduling <spec>` | `nice:-5` / `rr:5` / `fifo:5` / `batch` / `idle` |
| `--launch-now` | Launch immediately after install |
| `--no-system-integration` | Skip `.desktop` / icon generation |

On install, `--hardware-webrtc` and `--software-rendering` are plain boolean flags; `--user-agent` and `--start-hidden` are *not* available here (they default to unset/false and are set later via `site update`). The `SiteConfig` is populated in `_run` (`site.rs:246-271`), including the freshly minted `webapp_id` (a UUID v4).

## 4. site launch

`site launch <ULID>` (`app.rs:41-65`) starts or focuses an app:

| Flag | Purpose |
| --- | --- |
| `--url <URL>...` | Override the start URL(s); conflicts with `--protocol` |
| `--protocol [<URL>]` | Launch for a protocol-handler URL; conflicts with `--url` |
| `--hidden` | Start hidden in the system tray |
| trailing args | Forwarded verbatim to the Firefox runtime |

The launch path is where single-instance lives: with no explicit target it first probes the app's socket and, if the app is already running, sends `show` and exits instead of opening a duplicate. The full mechanism — socket probe, tray spawn, env vars, the `firefox -taskbar-tab` argv — is in [IPC & the Runtime-Owned Window](ipc-protocol.gen.html).

## 5. site update

`site update <ULID>` (`app.rs:154-234`) is the editing command. Most of its flags are `Option<Option<T>>` so the command can express three states (see §8). It is the only command that exposes `--user-agent` and `--start-hidden`:

| Flag | Type | Notes |
| --- | --- | --- |
| `--name` / `--description` / `--start-url` / `--icon-url` | `Option<Option<T>>` | leave / clear / set |
| `--categories` / `--keywords` / `--enabled-url-handlers` / `--enabled-protocol-handlers` | `Option<Vec<String>>` | empty element clears |
| `--launch-on-login <bool>` / `--launch-on-browser <bool>` | `Option<bool>` | |
| `--hardware-webrtc <bool>` / `--software-rendering <bool>` | `Option<bool>` | |
| `--scheduling [<spec>]` | `Option<Option<String>>` | empty clears |
| `--user-agent [<ua>]` | `Option<Option<String>>` | empty clears |
| `--start-hidden <bool>` | `Option<bool>` | start in tray when launched on login |
| `--no-manifest-updates` | flag | skip re-fetching the manifest (`update_manifest = false`) |
| `--no-icon-updates` | flag | skip re-fetching icons (`update_icons = false`) |

`--no-manifest-updates --no-icon-updates` is the network-free way to regenerate launchers and KWin rules — useful when a site's manifest fetch is flaky (Teams) and would otherwise abort a script. This is exactly what the runtime's `autostart-toggle` shells out to (`_autoconfig.cfg:643-648`).

## 6. site uninstall

`site uninstall <ULID>` (`app.rs:140-152`) removes the app: its `Site` record, profile data, `.desktop` launcher, icons, autostart entry, and KWin rule. `-q`/`--quiet` skips the confirmation prompt (the GUI always passes `quiet: true` and shows its own confirm dialog), and `--no-system-integration` leaves the desktop files in place.

## 7. profile and runtime

**Profiles** (`profile.rs`):

| Command | Effect |
| --- | --- |
| `profile list` | Lists profiles with their apps and ULIDs |
| `profile create [--name --description --template <DIR>]` | Creates a profile; a template directory's contents are copied in |
| `profile update <ULID> [--name --description --template]` | Edits a profile |
| `profile remove <ULID> [-q]` | Removes a profile (the Nil Default is only cleared, never deleted) |

**Runtime** (`runtime.rs`):

| Command | Effect |
| --- | --- |
| `runtime install [--link]` | Download a Mozilla build, or `--link` the system Firefox (Linux) |
| `runtime patch` | Re-copy the autoconfig payload into the runtime |
| `runtime uninstall` | Empty the runtime directory |

Both install and uninstall are compiled out under the `immutable-runtime` feature.

## 8. Update-value semantics

The trickiest part of the command model is distinguishing **"the user didn't mention this field"** from **"the user explicitly cleared it back to the manifest default"**. clap models the first as `Option<Option<T>> = None` and the second as `Some(None)`. Two macros (`console/mod.rs:11-48`) apply that to the stored config:

`store_value!` — for `Option<Option<T>>` → `Option<T>`:

| Source | Result |
| --- | --- |
| `None` | leave the stored value unchanged |
| `Some(None)` | clear to `None` (use the manifest/default value) |
| `Some(Some(v))` | set to `Some(v)` |

`store_value_vec!` — for `Option<Vec<T>>`, where a single empty-string element is the reset sentinel (a CLI/serve-compat hack):

| Source | Result |
| --- | --- |
| `None` | leave unchanged |
| `Some(vec![""])` | clear to `None` (manifest default) |
| `Some(vec![a, b, …])` | set |

The GTK editor reproduces this exact three-state logic in plain Rust (`opt_opt_string`, `vec_opt_update`) by diffing each widget's current text against the original config value, so an untouched field is never sent and an emptied field is actively cleared. That correspondence is what lets the GUI and CLI share one update command without the GUI clobbering fields it didn't touch.

## 9. HTTP client options

`site install` and `site update` flatten an `HTTPClientConfig` (`app.rs:327-350`) that tunes the manifest/icon fetch, kept deliberately separate from the per-app browsing UA:

| Flag | Purpose |
| --- | --- |
| `--client-user-agent <ua>` | UA used when fetching the manifest and icons (not the app's browsing UA) |
| `--tls-root-certificates-der` / `--tls-root-certificates-pem <FILE>...` | Extra trust roots |
| `--tls-danger-accept-invalid-certs` / `--tls-danger-accept-invalid-hostnames` | Relax TLS verification |

Note the two distinct user-agent concepts: `--client-user-agent` (install-time fetch) is unrelated to `site update --user-agent` (the app's runtime `general.useragent.override`). Confusing the two caused an early bug (`c4e2676`).
