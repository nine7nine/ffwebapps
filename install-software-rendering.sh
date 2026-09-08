#!/usr/bin/env bash
# Build + install ffwebapps from this working tree and switch the Teams
# web app to software rendering (no PKGBUILD).
set -euo pipefail

REPO=/home/ninez/Claude/ffwebapps
TEAMS_ULID=01KTVNB6PT0YSDPX9X083P5Z6N
TEAMS_PROFILE=01KTVNB66WTBEMWMHHFQQM39YT
SOCK="${XDG_RUNTIME_DIR:-/run/user/$UID}/ffwebapps-${TEAMS_ULID}.sock"

echo "==> Building release binaries"
cd "$REPO"
cargo build --release --bin ffwebapps --bin ffwebapps-tray

echo "==> Quitting Teams runtime (if running)"
if [[ -S "$SOCK" ]]; then
    python3 - "$SOCK" <<'EOF'
import socket, sys
s = socket.socket(socket.AF_UNIX)
try:
    s.settimeout(3)
    s.connect(sys.argv[1])
    s.sendall(b"hello v1 launcher\nquit\n")
except OSError:
    pass  # socket stale or runtime already gone
EOF
    for _ in $(seq 1 10); do [[ -S "$SOCK" ]] || break; sleep 0.5; done
    rm -f "$SOCK"
fi

echo "==> Installing to /usr (sudo)"
sudo install -Dm755 target/release/ffwebapps      /usr/bin/ffwebapps
sudo install -Dm755 target/release/ffwebapps-tray /usr/bin/ffwebapps-tray
sudo rm -rf /usr/share/ffwebapps/userchrome
sudo cp -r userchrome /usr/share/ffwebapps/userchrome
sudo find /usr/share/ffwebapps -type d -exec chmod 755 {} +
sudo find /usr/share/ffwebapps -type f -exec chmod 644 {} +

echo "==> Switching Teams to software rendering"
ffwebapps site update "$TEAMS_ULID" \
    --hardware-webrtc false --software-rendering true \
    --no-manifest-updates --no-icon-updates

echo "==> Resulting user.js:"
cat "$HOME/.local/share/ffwebapps/profiles/${TEAMS_PROFILE}/user.js"

echo
echo "Done. Relaunch Teams from the app menu (prefs are read at startup)."
