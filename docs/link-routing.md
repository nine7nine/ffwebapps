# ffwebapps — Link Routing & Scope

This page documents how an ffwebapps window keeps the app's own pages in-window while sending out-of-scope links to your real browser: the two-layer interception, the in-app allow-list, and the auth/SSO carve-outs that keep logins working.

## Table of Contents

1. [The problem](#1-the-problem)
2. [The in-app allow-list](#2-the-in-app-allow-list)
3. [Default domains: scope, auth, Microsoft](#3-default-domains-scope-auth-microsoft)
4. [Two layers of interception](#4-two-layers-of-interception)
5. [Layer 1: the content router](#5-layer-1-the-content-router)
6. [Layer 2: the request backstop](#6-layer-2-the-request-backstop)
7. [Opening the default browser correctly](#7-opening-the-default-browser-correctly)
8. [Why window.open is left alone](#8-why-windowopen-is-left-alone)

---

## 1. The problem

A native app window should behave like a native app: the application's own pages open inside it, and a link to *anywhere else* opens in your normal browser — not in a stray, chromeless app window with no address bar. Electron apps get this with `shell.openExternal` plus a `setWindowOpenHandler` that denies. ffwebapps has to reconstruct the same behaviour inside a real Firefox.

The difficulty is telling "in scope" from "out of scope" precisely, and doing it at the right moment. Route too aggressively and an SSO redirect or an in-app sub-page bounces out to the browser mid-flow; route too late and Firefox has already opened a blank popup window before the link is cancelled. ffwebapps solves this with a per-app **allow-list** consulted by **two cooperating interceptors** living in `_autoconfig.cfg`.

## 2. The in-app allow-list

Two prefs in the profile's `user.js` drive everything (written by `taskbartabs::write_profile_prefs`):

| Pref | Meaning |
| --- | --- |
| `ffwebapps.externalLinks.enabled` | Master switch; from `SiteConfig::external_links` (`unwrap_or(true)`) |
| `ffwebapps.allowedDomains` | Comma-separated wildcard host patterns kept *in-window* |

A host is "in scope" if it matches any pattern in `allowedDomains`; anything else is "out of scope" and routed to the browser. Matching is wildcard-based: a pattern like `*.cloud.microsoft` becomes an anchored regex (`* → .*`), so `teams.cloud.microsoft` matches but `evil.com` does not. The same matcher is implemented twice — once in the chrome script and once, slightly simplified, in the content frame script — so both layers agree on what is in scope.

## 3. Default domains: scope, auth, Microsoft

When a user hasn't set an explicit `allowed_domains` list, ffwebapps derives a sensible default from the site's scope (`default_allowed_domains`, `taskbartabs.rs:148-173`):

1. **The scope host** itself (e.g. `teams.microsoft.com`).
2. **Its parent-domain wildcard** (e.g. `*.microsoft.com` and the bare `microsoft.com`).
3. **A shared auth/SSO bundle** — `AUTH_DOMAINS`.
4. For Microsoft apps, **a Microsoft 365 service bundle** — `MICROSOFT_DOMAINS`.

The auth carve-out is the subtle, important part. Logins routinely bounce through identity providers on *different* domains than the app, and if those bounced mid-flow to the external browser the sign-in would break. So a fixed set of identity domains is always kept in-window:

```text
AUTH_DOMAINS:  login.microsoftonline.com, login.microsoft.com, login.live.com,
               login.windows.net, *.msftauth.net, *.msauth.net, *.b2clogin.com,
               accounts.google.com, *.okta.com, *.auth0.com,
               *.duosecurity.com, *.onelogin.com
```

A site is treated as "Microsoft" (and gets the broader `*.office.com`, `*.sharepoint.com`, `*.teams.microsoft.com`, … bundle) when its host ends with `microsoft`, contains `.microsoft.`, or contains `office`/`teams`. This is why Teams and Outlook web apps keep their whole multi-domain experience in-window while a link to an external blog still opens in the browser.

## 4. Two layers of interception

ffwebapps intercepts out-of-scope navigation at two points, with different responsibilities. The content router is the primary, native-app-style handler; the request observer is a backstop for anything the content layer can't catch.

<div class="diagram-container">
<svg width="100%" viewBox="0 0 900 420" xmlns="http://www.w3.org/2000/svg">
  <style>
    .bg     { fill: #1a1b26; }
    .l1     { fill: #1a2a1a; stroke: #9ece6a; stroke-width: 1.5; }
    .l2     { fill: #2a2438; stroke: #e0af68; stroke-width: 1.5; }
    .out    { fill: #1a2235; stroke: #7aa2f7; stroke-width: 1.5; }
    .box    { fill: #24283b; stroke: #3b4261; stroke-width: 1; }
    .lbl    { fill: #c0caf5; font-size: 11px; font-family: 'JetBrains Mono', monospace; }
    .lbl-sm { fill: #c0caf5; font-size: 10px; font-family: 'JetBrains Mono', monospace; }
    .lbl-mut{ fill: #8c92b3; font-size: 9px;  font-family: 'JetBrains Mono', monospace; }
    .lbl-grn{ fill: #9ece6a; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-yel{ fill: #e0af68; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .lbl-blu{ fill: #7aa2f7; font-size: 11px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
    .ln     { stroke: #7dcfff; stroke-width: 1.5; fill: none; }
    .ln-g   { stroke: #9ece6a; stroke-width: 1.5; fill: none; }
    .title  { fill: #7aa2f7; font-size: 14px; font-weight: bold; font-family: 'JetBrains Mono', monospace; }
  </style>
  <rect x="0" y="0" width="900" height="420" class="bg"/>
  <text x="450" y="26" text-anchor="middle" class="title">two-layer out-of-scope routing</text>

  <rect x="330" y="44" width="240" height="40" class="box"/>
  <text x="450" y="62" text-anchor="middle" class="lbl-sm">a navigation begins</text>
  <text x="450" y="76" text-anchor="middle" class="lbl-mut">click / target=_blank / window.open / redirect</text>

  <!-- layer 1 -->
  <rect x="60" y="120" width="360" height="120" class="l1"/>
  <text x="240" y="142" text-anchor="middle" class="lbl-grn">Layer 1 — content frame script  (PRIMARY)</text>
  <text x="240" y="164" text-anchor="middle" class="lbl-mut">click capture on &lt;a&gt; with target=_blank /</text>
  <text x="240" y="178" text-anchor="middle" class="lbl-mut">_new / middle-click / ctrl / meta</text>
  <text x="240" y="198" text-anchor="middle" class="lbl-mut">out of scope? preventDefault + stopPropagation</text>
  <text x="240" y="212" text-anchor="middle" class="lbl-mut">→ sendAsyncMessage("ffwebapps:open-external")</text>
  <text x="240" y="230" text-anchor="middle" class="lbl-mut">Firefox never opens a window at all</text>

  <!-- layer 2 -->
  <rect x="480" y="120" width="360" height="120" class="l2"/>
  <text x="660" y="142" text-anchor="middle" class="lbl-yel">Layer 2 — http-on-modify-request  (BACKSTOP)</text>
  <text x="660" y="164" text-anchor="middle" class="lbl-mut">observes top-level GET main-document loads</text>
  <text x="660" y="178" text-anchor="middle" class="lbl-mut">catches window.open / SSO / SafeLinks</text>
  <text x="660" y="198" text-anchor="middle" class="lbl-mut">out of scope? channel.cancel(NS_BINDING_ABORTED)</text>
  <text x="660" y="212" text-anchor="middle" class="lbl-mut">+ dedupe rapid repeats (600ms)</text>
  <text x="660" y="230" text-anchor="middle" class="lbl-mut">+ close any blank popup it spawned</text>

  <line x1="420" y1="64" x2="240" y2="120" class="ln-g"/>
  <line x1="480" y1="64" x2="660" y2="120" class="ln"/>

  <!-- opener -->
  <rect x="280" y="290" width="340" height="100" class="out"/>
  <text x="450" y="312" text-anchor="middle" class="lbl-blu">ffwaOpenExternal(spec)  — shared</text>
  <text x="450" y="334" text-anchor="middle" class="lbl-mut">env -u &lt;Firefox context vars&gt; xdg-open &lt;url&gt;</text>
  <text x="450" y="350" text-anchor="middle" class="lbl-mut">strips only this app's Firefox identity;</text>
  <text x="450" y="364" text-anchor="middle" class="lbl-mut">keeps display vars → opens as a TAB in</text>
  <text x="450" y="378" text-anchor="middle" class="lbl-mut">the user's running browser</text>

  <line x1="240" y1="240" x2="380" y2="290" class="ln-g"/>
  <line x1="660" y1="240" x2="520" y2="290" class="ln"/>
</svg>
</div>

## 5. Layer 1: the content router

The primary handler runs **in content**, the same place an Electron app would intercept (`_autoconfig.cfg:217-297`). A frame script, loaded into every content process, captures `click` events and walks up to the nearest `<a>`. If the link would open a new window or tab — `target="_blank"`/`"_new"`, a middle-click, or a ctrl/meta-click — and the URL is out of scope, it calls `preventDefault()` + `stopPropagation()` and posts an async message to the chrome process, which opens it externally.

The win here is that the link is stopped *before Firefox creates anything*. There is no popup window to flash and close, no `about:blank` flicker — the out-of-scope link simply never produces an app window, exactly like a native app's `openExternal`. This is why it is the primary path and the request observer is only a backstop.

## 6. Layer 2: the request backstop

Some out-of-scope navigations don't originate from a clickable `<a>` the content script can see — `window.open` calls, SSO interstitials, Microsoft SafeLinks redirects. For those, a chrome-side observer on `http-on-modify-request` (`_autoconfig.cfg:145-210`) acts as a net. It only fires for top-level, main-document, `GET` loads whose host is out of scope, and then:

1. Hands the URL to the shared external opener.
2. `channel.cancel(NS_BINDING_ABORTED)` to stop the in-app load.
3. **De-duplicates** rapid repeats of the same URL within 600 ms — a single click can produce more than one top-level request, and we don't want two browser tabs.
4. If the app spawned an extra window just for this link (i.e. not the main taskbar-tab window), moves it off-screen and closes it, so the user never sees a blank popup.

The observer deliberately acts on *any* window in the app's runtime, not only the main taskbar-tab window. The runtime is exclusive to one web app, so the extra popup windows it spawns for a link are themselves in scope for routing — and requiring a taskbar-tab window was exactly the bug that let out-of-scope links leak into stray app windows.

## 7. Opening the default browser correctly

Handing a URL to the OS browser sounds trivial but isn't, inside Firefox. Two naive approaches both fail:

- `nsIExternalProtocolService.loadURI` for an `http(s)` URL opens the link in *this very runtime*, not the user's default browser.
- Spawning `xdg-open` with the inherited environment launches the browser inside *this app's* Firefox context, so it can't remote into the user's session and opens a fresh window instead of a tab.

The shared `ffwaOpenExternal` (`_autoconfig.cfg:90-139`) threads the needle. It runs `xdg-open` through `/usr/bin/env -u …`, stripping **only the app's unique Firefox context** — `MOZ_APP_REMOTINGNAME`, `XRE_PROFILE_PATH`, the crash-reporter restart args, and so on — so the launched browser starts with its own identity and remotes the link in as a **tab** in the user's already-running browser. It deliberately *keeps* the session/display variables (`MOZ_ENABLE_WAYLAND`, etc.): stripping those put the spawned browser on a different display backend, which again forced a new window. If `xdg-open` is unavailable it falls back to the external-protocol service.

This same opener is exported on `_ffwaShared` and reused by the tray's "Open page in browser" command, so there is one correct implementation, not two.

## 8. Why window.open is left alone

A tempting "complete" solution would override `window.open` in the content script to catch programmatic navigations directly. ffwebapps deliberately does **not** (`_autoconfig.cfg:279-282`). Tampering with that global can make Microsoft's authentication library (MSAL) abort sign-in — the auth flow checks and uses `window.open` itself. So `window.open`-based out-of-scope navigations are left to the Layer 2 request backstop, which routes them just as well without breaking auth.

This restraint is the through-line of the whole subsystem: keep logins working, route only what is genuinely out of scope, and never break a page's own navigation to do it.
