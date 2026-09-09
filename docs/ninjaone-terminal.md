# NinjaOne as a Web App, with a Pop-Out Terminal

A worked example of two ffwebapps features: installing a site that ships **no
web-app manifest**, and using **per-app JS injection** to restructure the page.

The result is NinjaOne running as a chromeless desktop app whose PowerShell/CMD
terminal can be popped out into its own borderless window — keeping its live
session, scrollback and titlebar buttons.

Commands below use the `vicinity.rmmservices.net` tenant; substitute your own
host throughout.

**Requires:** Firefox >= 151, Rust/Cargo. Steps 6 and 7 are KDE Plasma only.

---

## 1. Build and install ffwebapps

```bash
git clone https://github.com/nine7nine/ffwebapps
cd ffwebapps
cargo build --release --bin ffwebapps --bin ffwebapps-tray
cargo build --release --features gui --bin ffwebapps-gtk
sudo ./install-to-usr.sh
```

Then run the cleanup commands the script prints at the end, **as your normal
user, not root**.

## 2. Link the runtime (one time)

```bash
ffwebapps runtime install --link
```

Symlinks your system Firefox as the runtime, so it tracks Firefox updates
instead of bundling a second copy. Skip if you have used ffwebapps before.

## 3. Re-register existing apps

Only if you already had apps installed before this update. The launcher's
`StartupWMClass` fix is applied when system integration runs, so existing
`.desktop` files keep the old, non-matching class until they are rewritten:

```bash
for id in $(ffwebapps profile list | grep -oE '\(([0-9A-Z]{26})\)' | tr -d '()'); do
  ffwebapps site update "$id"
done
kbuildsycoca6
```

Skip on a fresh install.

## 4. Install the app

NinjaOne is a plain React SPA. It has no `<link rel="manifest">`, and
`/manifest.json`, `/manifest.webmanifest` and `/site.webmanifest` all 404 — so
there is no manifest URL to give `site install`. Supply a `data:` URL instead,
which is ffwebapps' route for non-PWA sites. `--document-url` becomes required
when the manifest URL is a data URL.

```bash
MANIFEST=$(python3 <<'PY'
import base64, json
h = "https://vicinity.rmmservices.net"
m = {"name": "Vicinity RMM", "short_name": "Vicinity",
     "description": "NinjaOne RMM console - Vicinity tenant",
     "start_url": h + "/", "scope": h + "/", "display": "standalone",
     "background_color": "#000000", "theme_color": "#00a0c6",
     "categories": ["utilities", "system"],
     "icons": [{"src": h + "/img/favicon.png", "sizes": "512x512",
                "type": "image/png", "purpose": "any"}]}
print("data:application/manifest+json;base64," +
      base64.b64encode(json.dumps(m).encode()).decode())
PY
)

PROFILE=$(ffwebapps profile create --name "Vicinity RMM" \
  --description "NinjaOne RMM console (Vicinity tenant)" 2>&1 \
  | grep -oE '[0-9A-Z]{26}' | tail -1)

SITE=$(ffwebapps site install "$MANIFEST" \
  --document-url "https://vicinity.rmmservices.net/" \
  --name "Vicinity RMM" \
  --description "NinjaOne RMM console (Vicinity tenant)" \
  --profile "$PROFILE" 2>&1 | grep -oE '[0-9A-Z]{26}' | tail -1)

echo "profile=$PROFILE"
echo "site=$SITE"
```

**Record both ULIDs.** They are generated per install and differ between
machines; step 6 needs the site ULID.

`/img/favicon.png` is a real 512x512 PNG, found in NinjaOne's JS bundle as
`browserIcon`. It is the stock NinjaOne mark. A white-labelled tenant icon is
served base64 from an authenticated `/branding` endpoint, so to use yours, save
it from a logged-in tab and then:

```bash
ffwebapps site update "$SITE" --icon-url file:///path/to/icon.png --update-icons
```

The dedicated profile keeps the NinjaOne session isolated from your other web
apps. Drop `--profile` to share the default profile instead.

## 5. Install the pop-out script

```bash
cp examples/ninjaone-terminal-popout.js \
   ~/.local/share/ffwebapps/profiles/"$PROFILE"/ffwebapps.js
```

`ffwebapps.js` in a profile directory is injected into that app's pages in a
content-principal sandbox at `DOMContentLoaded`, so it is not subject to the
page's CSP. It is read once at startup — see the gotcha about restarting below.

## 6. Borderless pop-out window (KDE)

Optional. Without it the pop-out works, but keeps a KDE titlebar above
NinjaOne's own.

System Settings -> Window Management -> **Window Rules** -> New:

| Field | Value |
|---|---|
| Window class | `ffwebapps-<your SITE ULID>` — **Exact Match** |
| Window title | `NinjaOne terminal` — **Substring Match** |
| No titlebar and frame | **Force -> Yes** |

The class here has no `.webapp-<uuid>` suffix. A `window.open` popup carries the
bare `MOZ_APP_REMOTINGNAME`, unlike the app window, which is what lets this rule
target the pop-out without also stripping the main window.

## 7. Free Super+T (KDE)

KWin binds `Meta+T` to "Edit Tiles" by default, and on Wayland the compositor
consumes the key before Firefox sees it. Either clear it in System Settings ->
Shortcuts -> KWin -> Edit Tiles, or skip this and use **Ctrl+Alt+P**, which the
script also accepts.

`Ctrl+Alt+T` is not available — that is KDE's Konsole shortcut.

## 8. Using it

```bash
ffwebapps site launch "$SITE"
```

Or launch **Vicinity RMM** from your application menu. Sign in once; the profile
is separate from your normal browser, so no session carries over.

Open a device, then PowerShell or CMD. The terminal's titlebar gains a **pop-out
button** next to NinjaOne's own controls.

| Action | Result |
|---|---|
| Pop-out button, or `Super+T` | Move the terminal to its own window, or send it back |
| `X` while popped out | Returns the terminal to the app window |
| Drag the terminal's titlebar | Moves the real window |
| `Meta`+drag | Moves the window from anywhere on it |

Copy, keyboard and download all keep working while popped out.

`X` returns the terminal rather than closing it because NinjaOne renders its
"Exit terminal?" confirmation into a portal in the *app* window — so the prompt
would otherwise appear behind, asking about a terminal you are looking at
elsewhere. Click `X` again once it is home to end the session for real.

---

## Gotchas

**Editing `ffwebapps.js` requires a full quit.** Right-click the tray icon ->
Quit. The window's X only hides to tray; the process keeps running and the file
is read once at startup, so closing and reopening will not pick up an edit.

**Do not reuse another machine's ULIDs.** Profile and site ULIDs are generated
per install.

**SSO popups lose their address bar.** The chromeless-popup CSS applies to every
`window.open` popup in every ffwebapps app, which removes the visual origin check
on auth windows. The rule is self-contained and commented at the bottom of
`userchrome/profile/chrome/userChrome.css` if you would rather not have it.

---

## How the pop-out works

Useful if you want to adapt this to another site. The full script is
`examples/ninjaone-terminal-popout.js`.

The terminal is xterm.js in the main document — no iframe — fed by a WebSocket
that lives in the page's own JS realm. So `adoptNode` into a popup keeps the
session and scrollback: the socket never notices. NinjaOne also fixes the grid at
construction (`cols` from `window.innerWidth`: 110 below 1280px, else 120; 30
rows) and ships no fit addon, so the terminal never reflows and nothing is lost
by moving it.

Four things are not obvious, and each one produced a distinct failure:

**React's event delegation.** React 17+ does not bind `onClick` to elements. It
binds one listener set to the root container, and to each portal container, then
walks its fiber tree from the event target. Move a subtree out from under that
element and every handler inside goes inert — while listeners bound directly to
the moved nodes, like xterm's key handling, survive. The terminal kept working
while its buttons died. The script therefore moves the element React listens on,
discovered at runtime by looking for React's `_reactListening<hash>` marker,
because that container is generated per-modal and has no stable id. It refuses to
move anything containing the app root.

**xterm's visibility check.** xterm suspends *all* rendering when its screen
element reports as not intersecting the viewport. That check is an
`IntersectionObserver` created in the opener's realm, and a node in another
document can never intersect it — so the renderer latches paused and keystrokes
never appear. The script stubs that one observer with a permanent hit, scoped to
xterm's own elements so the app's lazy-loading keeps its real observers.

**Observing the document is a latency trap.** xterm's DOM renderer mutates on
every rendered frame, so a `MutationObserver` on `documentElement` with
`subtree: true` fires dozens of times a second. Detecting the terminal that way
added seconds of input lag — to the embedded terminal as much as the popped-out
one. The script polls twice a second instead.

**Wayland forbids client positioning.** `window.moveTo()` is a no-op and a
content node can never start a window move, so a page element cannot be a drag
handle. The only region that can is browser chrome with
`-moz-window-dragging: drag`, which goes through `xdg_toplevel.move`. The
userChrome rules take `#navigator-toolbox` out of flow and float it, invisible,
over the page's own titlebar — so dragging that titlebar moves the real window.
Resizing a window *is* permitted, which is how the pop-out trims itself to the
terminal's exact height.
