# ffwebapps — Build, Install & Packaging

This page documents how ffwebapps is built and installed: the cargo features and binaries, the one-time runtime link step, the on-disk footprint, and the Arch package that wires it all together.

## Table of Contents

1. [Requirements](#1-requirements)
2. [The crate and its features](#2-the-crate-and-its-features)
3. [Building from source](#3-building-from-source)
4. [The runtime link step](#4-the-runtime-link-step)
5. [The installed footprint](#5-the-installed-footprint)
6. [The Arch package](#6-the-arch-package)
7. [Completions and the build script](#7-completions-and-the-build-script)
8. [Development checkout](#8-development-checkout)

---

## 1. Requirements

| Requirement | Why |
| --- | --- |
| **Firefox ≥ 151** | Provides the Web Apps / Taskbar Tabs modules; used as the linked runtime |
| **Rust / Cargo** | To build the binaries |
| **GTK4 + libadwaita + GtkSourceView 5** | Only for the `ffwebapps-gtk` management GUI |
| **KDE Plasma (KWin, `qdbus6`)** | Tray hide/show and window control currently target KWin |

The CLI and tray themselves only need Firefox; the GTK deps are pulled in only when you build the GUI. On other desktops the tray icon appears but hide/restore won't work yet, since that path targets KWin.

## 2. The crate and its features

The whole project is one crate (`firefoxpwa`) producing four binaries. Cargo features gate the optional pieces (`Cargo.toml:22-35`):

| Feature | Effect |
| --- | --- |
| `gui` | Builds `ffwebapps-gtk`; pulls in `gtk4`, `libadwaita`, `async-channel`, `sourceview5` |
| `static` | Vendored TLS and static `xz2` for a self-contained build |
| `immutable-runtime` | Compiles out `runtime install`/`uninstall` (for a system-managed runtime) |
| `portable` | Portable-install support (Windows-oriented) |

| Binary | Built by | Role |
| --- | --- | --- |
| `ffwebapps` | default | The CLI |
| `ffwebapps-tray` | default | The tray helper |
| `ffwebapps-gtk` | `--features gui` | The management GUI |
| `firefoxpwa-connector` | default | Inherited native-messaging host (not used by the CLI flow) |

The release profile is tuned for size and speed — `lto = true`, `codegen-units = 1` (`Cargo.toml:18-20`).

## 3. Building from source

The CLI binaries and the GUI are built in two separate invocations, deliberately, so the CLI binaries don't link GTK:

```bash
# CLI + tray (no GTK)
cargo build --release --bin ffwebapps --bin ffwebapps-tray

# Management GUI (pulls in gtk4 + libadwaita)
cargo build --release --features gui --bin ffwebapps-gtk
```

Then put `ffwebapps` and `ffwebapps-tray` on your `PATH`, and copy `./userchrome` to `/usr/share/ffwebapps/userchrome` (or point `FFPWA_SYSDATA` at the in-tree copy). The `userchrome` bundle is required — it carries the autoconfig and the chromeless CSS the runtime needs (see [The Firefox Runtime & Autoconfig](runtime.gen.html)).

## 4. The runtime link step

Before installing any app, link your system Firefox as the runtime:

```bash
ffwebapps runtime install --link
```

On Linux this is the recommended mode: instead of downloading a second copy of Firefox, it symlinks the system install into `~/.local/share/ffwebapps/runtime/` (copying only `firefox` and `firefox-bin` as real files) and records `use_linked_runtime = true`. The runtime then tracks your distro's Firefox updates and costs only a few hundred KB. The alternative, `runtime install` without `--link`, downloads an official Mozilla build instead.

Installing an app is then a single command:

```bash
ffwebapps site install <MANIFEST_URL> --document-url <PAGE_URL> --name "App Name"
```

It appears in your application menu as a chromeless window with a tray icon. The full command surface is in [CLI & Command Model](cli.gen.html).

## 5. The installed footprint

A system install (via the package) places files in standard locations:

| Path | Contents |
| --- | --- |
| `/usr/bin/ffwebapps`, `ffwebapps-tray`, `ffwebapps-gtk` | The three binaries |
| `/usr/share/ffwebapps/userchrome/` | The autoconfig + chromeless CSS bundle (`sysdata`) |
| `/usr/share/applications/io.github.nine7nine.ffwebapps.desktop` | The GUI's own launcher |
| `/usr/share/icons/hicolor/<size>/apps/io.github.nine7nine.ffwebapps.*` | The GUI's icon (SVG + 16–256 PNGs) |
| `/usr/share/bash-completion/…`, `fish/…`, `zsh/…` | Shell completions |

Per-user state — `config.json`, profiles, the linked runtime, app launchers, and icons — lives under `~/.local/share/ffwebapps/` and the XDG dirs, never in `/usr` (see [Data Model & Storage](data-model.gen.html)).

## 6. The Arch package

`packages/arch/PKGBUILD` is a VCS package that builds the current commit. Its shape captures the whole install in one place:

- **`depends`**: `firefox`, `gtk4`, `libadwaita`, `gtksourceview5`.
- **`optdepends`**: `qt6-tools` (for `qdbus6`) and `plasma-workspace` — the KDE tray and KWin control.
- **`build()`**: the same two-invocation build as above (CLI binaries, then the `gui` feature).
- **`package()`**: installs the three binaries, the GUI launcher + icon set, the `userchrome` bundle into `/usr/share/ffwebapps`, the completions, and the license.

```bash
cd packages/arch
makepkg -si
```

The PKGBUILD ships both a scalable SVG icon *and* rasterized 16–256 PNGs for the GUI, because KDE's Qt SVG renderer is picky and the fixed-size PNGs always load. After installing or upgrading, existing apps should be re-run through `site update` (with `--no-manifest-updates --no-icon-updates` to avoid network) so their `.desktop` `Exec` lines point at `/usr/bin` and their KWin rules are refreshed — mixed binary locations are a known source of "relaunch after quit" bugs.

## 7. Completions and the build script

`build.rs` does two jobs at compile time:

1. **Shell completions** — it reflects over the clap `App` definition and generates Bash, Elvish, Fish, PowerShell, and Zsh completions into `target/release/completions/`, which the package then installs. Completions therefore always match the actual command tree.
2. **cfg aliases** — it defines `platform_linux` / `platform_macos` / `platform_windows` / `platform_bsd` via `cfg_aliases`, the predicates the source uses to gate platform-specific code (`directories.rs`, `runtime.rs`, `integrations/`).

## 8. Development checkout

ffwebapps supports running from a repo checkout without touching `/usr`, via the `FFPWA_*` environment overrides (`directories.rs`):

| Variable | Points at |
| --- | --- |
| `FFPWA_SYSDATA` | An in-tree `userchrome/` instead of `/usr/share/ffwebapps` |
| `FFPWA_USERDATA` | A scratch data dir instead of `~/.local/share/ffwebapps` |
| `FFPWA_EXECUTABLES` | The directory holding the built binaries |

When integration writes a `.desktop` launcher for a non-standard install, it bakes these overrides into the `Exec` line so the launched binary finds its data (see [Desktop Integration](desktop-integration.gen.html)). The project's stated working style favours a single installed version over churn: build and install once, then launch and test the real installed binaries rather than running mixed debug/release copies against a live session.
