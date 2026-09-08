#!/usr/bin/env bash
# Manually install ffwebapps (CLI + tray + GTK GUI) to /usr, mirroring the Arch
# PKGBUILD's package() step. Run with sudo AFTER building the release binaries:
#
#   cargo build --release --bin ffwebapps --bin ffwebapps-tray
#   cargo build --release --features gui --bin ffwebapps-gtk
#   sudo ./install-to-usr.sh
#
# Then (as your normal user, NOT root) run the user-level cleanup printed at the end.
set -euo pipefail

SRC="$(cd "$(dirname "$0")" && pwd)"          # this repo (the build tree)
ICON=io.github.nine7nine.ffwebapps

if [[ $EUID -ne 0 ]]; then
  echo "Run with sudo: sudo $0" >&2
  exit 1
fi

for b in ffwebapps ffwebapps-tray ffwebapps-gtk; do
  [[ -x "$SRC/target/release/$b" ]] || {
    echo "missing target/release/$b — build first:" >&2
    echo "  cargo build --release --bin ffwebapps --bin ffwebapps-tray" >&2
    echo "  cargo build --release --features gui --bin ffwebapps-gtk" >&2
    exit 1
  }
done

# --- executables ---
install -Dm755 "$SRC/target/release/ffwebapps"      /usr/bin/ffwebapps
install -Dm755 "$SRC/target/release/ffwebapps-tray" /usr/bin/ffwebapps-tray
install -Dm755 "$SRC/target/release/ffwebapps-gtk"  /usr/bin/ffwebapps-gtk

# --- runtime + profile assets (autoconfig + userChrome) ---
rm -rf /usr/share/ffwebapps/userchrome
install -dm755 /usr/share/ffwebapps
cp -r "$SRC/userchrome" /usr/share/ffwebapps/userchrome
find /usr/share/ffwebapps -type d -exec chmod 755 {} +
find /usr/share/ffwebapps -type f -exec chmod 644 {} +

# --- shell completions ---
install -Dm644 "$SRC/target/release/completions/ffwebapps.bash" /usr/share/bash-completion/completions/ffwebapps
install -Dm644 "$SRC/target/release/completions/ffwebapps.fish" /usr/share/fish/vendor_completions.d/ffwebapps.fish
install -Dm644 "$SRC/target/release/completions/_ffwebapps"     /usr/share/zsh/site-functions/_ffwebapps

# --- GUI: desktop launcher + icons (scalable SVG + rasterized PNGs) ---
install -Dm644 "$SRC/packages/gui/$ICON.desktop" "/usr/share/applications/$ICON.desktop"
install -Dm644 "$SRC/packages/gui/$ICON.svg"     "/usr/share/icons/hicolor/scalable/apps/$ICON.svg"
for s in 16 24 32 48 64 128 256; do
  install -Dm644 "$SRC/packages/gui/icons/${s}x${s}/$ICON.png" \
    "/usr/share/icons/hicolor/${s}x${s}/apps/$ICON.png"
done

# --- refresh system caches ---
gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
update-desktop-database /usr/share/applications 2>/dev/null || true

echo
echo "Installed to /usr. Now, as your NORMAL user (not root), run:"
echo "  rm -f ~/.local/share/applications/$ICON.desktop"
echo "  rm -f ~/.local/share/icons/hicolor/scalable/apps/$ICON.svg ~/.local/share/icons/hicolor/*/apps/$ICON.png"
echo "  kbuildsycoca6"
echo "...to drop the earlier user-level shim and refresh KDE, then relaunch ffwebapps-gtk."
