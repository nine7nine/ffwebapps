# Screenshots

Drop PNG screenshots here using the filenames below and they will appear in the
generated docs automatically (add the matching `<figure class="screenshot">`
block to the page's `.md` source). Capture at a comfortable window size; they are
displayed at full container width with a rounded border.

| File | Page | Suggested shot |
| --- | --- | --- |
| `app-window.png` | architecture / runtime | A web app running as a chromeless window (dark titlebar, no tabs/urlbar) next to its tray icon |
| `tray-menu.png` | tray | The system-tray icon with its menu open (Show/Hide, Mute, DND, Start on login, Quit) and an unread badge |
| `gtk-apps.png` | gtk-gui | The GTK GUI's Apps tab — apps grouped by profile with running dots |
| `gtk-editor.png` | gtk-gui | The per-app editor dialog with the Live-control group and the Performance section |
| `gtk-injection.png` | gtk-gui | The GtkSourceView CSS/JS injection editor with syntax highlighting |
| `chromeless-titlebar.png` | runtime | A close-up of the black titlebar showing the kept site-identity/permission lock icon |
| `link-routing.png` | link-routing | An out-of-scope link opening as a tab in the user's real browser |

To add one to a page, insert a block like this into the relevant `.md` and
re-run `./md2html.sh <page>.md`:

```html
<figure class="screenshot">
  <img src="img/app-window.png" alt="An ffwebapps web app window">
  <figcaption>A website running as a native, chromeless Firefox Web App window.</figcaption>
</figure>
```

Filenames are referenced from the `.md` sources; if you rename a file, update the
matching `<img src="img/...">` in that page and re-run `./md2html.sh <page>.md`.
