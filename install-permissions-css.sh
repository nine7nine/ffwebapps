#!/usr/bin/env bash
# Install the updated chromeless userChrome.css (adds Firefox's native
# site-identity / permission cluster to the app titlebar) and quit the Teams
# runtime so the next launch re-patches its profile.
#
# Data-only change: userChrome.css is an asset, not compiled. No `cargo build`.
# The runtime re-copies userchrome into the profile and clears startupCache on
# every FRESH launch (SiteLaunchCommand: should_patch = true), so all we do is
# refresh /usr/share/ffwebapps/userchrome and make sure the app is fully quit.
set -euo pipefail

REPO=/home/ninez/Claude/ffwebapps
TEAMS_ULID=01KTVNB6PT0YSDPX9X083P5Z6N
SOCK="${XDG_RUNTIME_DIR:-/run/user/$UID}/ffwebapps-${TEAMS_ULID}.sock"

echo "==> Installing userchrome assets to /usr/share/ffwebapps (sudo)"
sudo rm -rf /usr/share/ffwebapps/userchrome
sudo cp -r "$REPO/userchrome" /usr/share/ffwebapps/userchrome
sudo find /usr/share/ffwebapps/userchrome -type d -exec chmod 755 {} +
sudo find /usr/share/ffwebapps/userchrome -type f -exec chmod 644 {} +

echo "==> Quitting Teams runtime (if running) so the next launch re-patches"
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

cat <<'MSG'

Done. Relaunch Teams FROM THE APP MENU (a fresh launch re-patches the profile
and clears startupCache; the singleton "show existing window" path does not).

What to check:
  - A small lock/site icon now sits at the LEFT of the black titlebar.
  - Start a call (or visit https://webcamtests.com) → the camera/mic "Allow?"
    prompt should now drop down from that icon. Click Allow.
  - After granting, click the icon → Firefox's per-site panel lets you
    review / revoke camera, mic, etc. (the small dot = a granted permission).

Other apps (WhatsApp, Bitwarden) pick up the new titlebar the next time they
are fully quit and relaunched.
MSG
