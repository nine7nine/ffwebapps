// ffwebapps per-app injection — NinjaOne / Vicinity
//
// Pops the NinjaOne PowerShell/CMD terminal out of the page into its own
// window, and puts it back on demand.
//
// The terminal is xterm.js in the main document (no iframe), fed by a WebSocket
// that lives in this realm, so moving the node with adoptNode keeps the live
// session and scrollback. NinjaOne fixes the grid at construction (cols from
// window.innerWidth: <1280 -> 110, else 120; rows: 30) with no fit addon, so it
// never reflows and nothing is lost by the move.
//
// THE CATCH: React 17+ does not bind onClick to buttons. It binds one listener
// set to the root container, and to each portal container, then walks its fiber
// tree from the event target. Move a subtree out from under that element and
// every React handler inside goes inert — while xterm's own key handling, bound
// directly to its nodes, survives. That is why the terminal kept working but
// the titlebar buttons died.
//
// So we move the element React listens on, not the modal inside it. That is
// discovered at runtime via React's `_reactListening<hash>` marker, with a guard
// against grabbing the whole app. If no safe element is found we still pop out,
// but the buttons are inert, so the close button gets a native handler meaning
// "put it back".
//
// Trigger: the ⧉ button on the terminal titlebar, or Super+T / Ctrl+Alt+P.

(function () {
  "use strict";

  var CFG = {
    // Stamps a marker into document.title so behaviour can be checked from
    // outside the browser (it shows up in the KWin window caption).
    diag: false,

    // Pop out automatically as soon as a terminal appears. Off by default: the
    // popup would open without user activation and the popup blocker eats it.
    auto: false,

    windowName: "ninja-terminal",
    // Super+T is the real binding (KWin's "Edit Tiles" grab on Meta+T has been
    // cleared). Ctrl+Alt+P is a fallback; Ctrl+Alt+T is KDE's Konsole shortcut
    // and never reaches Firefox.
    isHotkey: function (e) {
      var t = e.key === "t" || e.key === "T";
      var p = e.key === "p" || e.key === "P";
      if (e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey && t) return true;
      return e.ctrlKey && e.altKey && !e.shiftKey && !e.metaKey && p;
    },
  };

  // The injection runs on every page in this profile, including the about:blank
  // pop-out we create. Inside that window there is nothing to observe; the
  // opener wires its own hotkey handler into it explicitly.
  if (window.name === CFG.windowName) return;

  var state = null;
  var stamp = " [ffwa]";
  var ioHits = 0;

  // xterm suspends ALL rendering when its screen element is reported as not
  // intersecting the viewport, via an IntersectionObserver created in THIS
  // window. Once the terminal is adopted into the pop-out it lives in another
  // document, which this window's observer can never report as intersecting, so
  // the renderer latches paused and only repaints on a forced full refresh —
  // keystrokes never appear. Stub out that one observer with a permanent hit.
  // Scoped to xterm's own elements, so the app's lazy-loading and virtualised
  // lists keep their real observers. Installed now, before any terminal exists.
  (function () {
    var Native = window.IntersectionObserver;
    if (!Native) return;
    function isXterm(el) {
      try {
        if (el && el.className && String(el.className).indexOf("xterm") !== -1) return true;
        return !!(el && el.closest && el.closest(".xterm"));
      } catch (e) {
        return false;
      }
    }
    function Patched(cb, opts) {
      var real = new Native(cb, opts);
      var self = this;
      this.observe = function (target) {
        if (isXterm(target)) {
          ioHits++;
          setTimeout(function () {
            try {
              cb([{ target: target, isIntersecting: true, intersectionRatio: 1, time: 0 }], self);
            } catch (e) {}
          }, 0);
          return;
        }
        return real.observe(target);
      };
      this.unobserve = function (t) {
        try {
          return real.unobserve(t);
        } catch (e) {}
      };
      this.disconnect = function () {
        try {
          return real.disconnect();
        } catch (e) {}
      };
      this.takeRecords = function () {
        try {
          return real.takeRecords();
        } catch (e) {
          return [];
        }
      };
    }
    window.IntersectionObserver = Patched;
  })();

  function log() {
    try {
      console.log.apply(console, ["[ffwa-term]"].concat([].slice.call(arguments)));
    } catch (e) {}
  }

  // Nearest ancestor React delegates events to. React tags every such element
  // with an own property named `_reactListening<hash>`.
  function reactListenerOf(node) {
    var el = node;
    while (el) {
      var names = Object.getOwnPropertyNames(el);
      for (var i = 0; i < names.length; i++) {
        if (names[i].indexOf("_reactListening") === 0) return el;
      }
      el = el.parentElement;
    }
    return null;
  }

  // `inner` is the visible modal (used for measuring and for resetting the drag
  // transform). `container` is what we actually move: the element React listens
  // on when that is safe, otherwise `inner` itself.
  function findParts(xterm) {
    var inner = xterm;
    while (inner.parentElement) {
      var p = inner.parentElement;
      if (p === document.body) break;
      if (p.id && /portal|modal/i.test(p.id)) break;
      inner = p;
    }

    var listener = reactListenerOf(xterm);
    var appRoot = document.getElementById("root");
    var container = inner;
    var kept = false;

    // Refuse to move the app itself: React's root container holds the whole UI,
    // so if that is where it listens, moving is not an option.
    if (
      listener &&
      listener !== document.body &&
      listener !== document.documentElement &&
      listener !== appRoot &&
      !(appRoot && listener.contains(appRoot)) &&
      listener.contains(inner)
    ) {
      container = listener;
      kept = true;
    }

    return { container: container, inner: inner, reactKept: kept, listener: listener };
  }

  // Emotion inserts rules via CSSOM in production, so cloning the <style> tags
  // copies nothing — serialise cssRules instead, and fall back to re-linking any
  // sheet we are not allowed to read.
  function copyStyles(dst) {
    Array.prototype.forEach.call(document.styleSheets, function (sheet) {
      try {
        var css = Array.prototype.map
          .call(sheet.cssRules, function (r) {
            return r.cssText;
          })
          .join("\n");
        var style = dst.createElement("style");
        style.textContent = css;
        dst.head.appendChild(style);
      } catch (e) {
        if (sheet.href) {
          var link = dst.createElement("link");
          link.rel = "stylesheet";
          link.href = sheet.href;
          dst.head.appendChild(link);
        }
      }
    });
  }

  // Rightmost control on the modal's titlebar row — NinjaOne's close button.
  function findCloseButton(root) {
    var r = root.getBoundingClientRect();
    var best = null;
    var bestX = -Infinity;
    var nodes = root.querySelectorAll(
      "button:not([data-ffwa-popout]), [role='button']:not([data-ffwa-popout])"
    );
    for (var i = 0; i < nodes.length; i++) {
      var b = nodes[i].getBoundingClientRect();
      if (!b.height || b.top - r.top > 60) continue;
      if (b.left > bestX) {
        bestX = b.left;
        best = nodes[i];
      }
    }
    return best;
  }

  function popOut() {
    if (state) {
      try {
        state.popup.focus();
      } catch (e) {}
      return;
    }

    var xterm = document.querySelector(".xterm");
    if (!xterm) return log("no terminal open");

    var parts = findParts(xterm);
    var container = parts.container;
    var inner = parts.inner;

    // Measure the modal, never the container: a portal container is often a
    // full-viewport overlay and would size the window to the whole screen.
    var rect = inner.getBoundingClientRect();
    var w = Math.ceil(rect.width);
    var h = Math.ceil(rect.height);

    var popup = window.open("", CFG.windowName, "popup=yes,width=" + w + ",height=" + h);
    if (!popup) return log("popup blocked — needs a user gesture");

    popup.document.write("<!doctype html><meta charset=utf-8><title>NinjaOne terminal</title>");
    popup.document.close();
    copyStyles(popup.document);
    popup.document.body.style.cssText = "margin:0;background:#000;overflow:hidden";

    var parent = container.parentNode;
    state = {
      container: container,
      inner: inner,
      reactKept: parts.reactKept,
      parent: parent,
      next: container.nextSibling,
      popup: popup,
      chain: null,
      origRemove: Object.prototype.hasOwnProperty.call(parent, "removeChild")
        ? parent.removeChild
        : null,
    };

    // If React still owns this node and unmounts it while it lives in the popup,
    // it calls parent.removeChild() on a node that is no longer its child and
    // throws NotFoundError. Absorb that on this one parent.
    parent.removeChild = function (child) {
      if (state && child === state.container && child.parentNode !== parent) {
        try {
          popup.close();
        } catch (e) {}
        return child;
      }
      return Node.prototype.removeChild.call(parent, child);
    };

    popup.document.body.appendChild(popup.document.adoptNode(container));

    // Strip the overlay/backdrop positioning the container carries as a portal
    // root, and the drag transform the modal picked up inside the page.
    // Every element from the moved container down to the modal carries portal
    // layout — fixed insets, flex centring, backdrops — and any one of them left
    // alone offsets the modal inside a void. Neutralise the whole chain, keeping
    // the modal itself visually intact (it owns the titlebar and the terminal).
    var chain = [];
    for (var el = inner; el; el = el.parentElement) {
      chain.push({ el: el, css: el.style.cssText });
      if (el === container) break;
    }
    state.chain = chain;

    var WRAPPER =
      "position:static !important;inset:auto !important;transform:none !important;" +
      "margin:0 !important;padding:0 !important;width:100% !important;height:auto !important;" +
      "max-width:none !important;max-height:none !important;display:block !important;" +
      "background:transparent !important;overflow:visible !important;z-index:auto !important;";
    var MODAL =
      "position:static !important;inset:auto !important;transform:none !important;" +
      "margin:0 !important;width:100% !important;height:auto !important;" +
      "max-width:none !important;max-height:none !important;";

    chain.forEach(function (entry) {
      entry.el.style.cssText = entry.el === inner ? MODAL : WRAPPER;
    });

    // Trim the window to the height the modal actually renders at HERE. It was
    // opened at the height measured in the page, which can differ once the
    // wrappers are flattened — the difference shows up as dead black space under
    // the terminal. Wayland forbids a client positioning its own window, but
    // resizing one is allowed. Two frames so layout and fonts have settled, then
    // once more late in case the terminal reflows after the socket attaches.
    var fit = function () {
      try {
        var want = Math.ceil(inner.getBoundingClientRect().height);
        var max = (popup.screen && popup.screen.availHeight) || want;
        var delta = Math.min(want, max) - popup.innerHeight;
        if (delta) popup.resizeBy(0, delta);
      } catch (e) {}
    };
    popup.requestAnimationFrame(function () {
      popup.requestAnimationFrame(fit);
    });
    popup.setTimeout(fit, 300);

    // Re-purpose the close button as "put it back" — even when React is live in
    // here and its real handler would fire. NinjaOne renders the "Exit terminal?"
    // confirmation into a portal in the OPENER's document, so the prompt appears
    // in the app window behind this one, asking about a terminal you are looking
    // at somewhere else. Returning it home first puts the prompt and the terminal
    // back in the same window, where the second click closes it for real.
    var closeBtn = findCloseButton(inner);
    if (closeBtn) {
      state.closeBtn = closeBtn;
      state.closeTitle = closeBtn.getAttribute("title");
      state.onClose = function (ev) {
        ev.preventDefault();
        ev.stopPropagation();
        restore();
      };
      closeBtn.addEventListener("click", state.onClose, true);
      closeBtn.setAttribute("title", "Return the terminal to the app window");
    }

    // If the terminal is torn down while popped out (its own Exit flow, or the
    // socket dropping), bring the empty container home and close the window.
    // Debounced, and deliberately trivial: this observes the terminal's own
    // container, which xterm mutates every frame. A subtree query per mutation
    // here is the same latency trap as above.
    var watchTimer = null;
    state.watch = new MutationObserver(function () {
      if (watchTimer) return;
      watchTimer = setTimeout(function () {
        watchTimer = null;
        if (state && !state.container.querySelector(".xterm")) restore();
      }, 300);
    });
    state.watch.observe(container, { childList: true, subtree: true });

    // xterm batches every repaint through requestAnimationFrame on the window it
    // was constructed in — this one. Once the pop-out covers the app window the
    // compositor stops sending it frame callbacks and Firefox throttles its rAF
    // to a crawl, so the terminal renders in bursts even though keystrokes and
    // WebSocket output are flowing normally. Point rAF at the window that is
    // actually on screen for as long as the pop-out is open.
    var origRAF = window.requestAnimationFrame;
    var origCAF = window.cancelAnimationFrame;
    state.origRAF = origRAF;
    state.origCAF = origCAF;
    window.requestAnimationFrame = function (cb) {
      try {
        if (!popup.closed) return popup.requestAnimationFrame(cb);
      } catch (e) {}
      return origRAF.call(window, cb);
    };
    window.cancelAnimationFrame = function (handle) {
      try {
        if (!popup.closed) return popup.cancelAnimationFrame(handle);
      } catch (e) {}
      return origCAF.call(window, handle);
    };

    popup.addEventListener("keydown", onKey, true);
    popup.addEventListener("pagehide", restore);
    log("popped out, react=" + (parts.reactKept ? "kept" : "lost"));
  }

  function restore() {
    if (!state) return;
    var s = state;
    state = null;

    try {
      if (s.watch) s.watch.disconnect();
    } catch (e) {}

    try {
      if (s.origRAF) window.requestAnimationFrame = s.origRAF;
      if (s.origCAF) window.cancelAnimationFrame = s.origCAF;
    } catch (e) {}

    // Back in the page React works again, so our capture-phase handler must go
    // or it would swallow the real close.
    try {
      if (s.closeBtn && s.onClose) {
        s.closeBtn.removeEventListener("click", s.onClose, true);
        if (s.closeTitle === null) s.closeBtn.removeAttribute("title");
        else s.closeBtn.setAttribute("title", s.closeTitle);
      }
    } catch (e) {}

    try {
      if (s.origRemove) s.parent.removeChild = s.origRemove;
      else delete s.parent.removeChild;
    } catch (e) {}

    try {
      document.adoptNode(s.container);
      if (s.next && s.next.parentNode === s.parent) s.parent.insertBefore(s.container, s.next);
      else s.parent.appendChild(s.container);
      if (s.chain) {
        s.chain.forEach(function (entry) {
          entry.el.style.cssText = entry.css;
        });
      }
    } catch (e) {
      log("restore failed", e);
    }

    try {
      if (!s.popup.closed) s.popup.close();
    } catch (e) {}
    log("restored");
  }

  function onKey(e) {
    if (!CFG.isHotkey(e)) return;
    e.preventDefault();
    e.stopPropagation();
    if (state) restore();
    else popOut();
  }

  // Match NinjaOne's own titlebar controls exactly: clone one of them so we
  // inherit its classes, sizing and hover treatment, swap in our own icon, and
  // sit IN the row rather than floating over it at a guessed offset.
  // cloneNode does not copy event listeners, so the clone arrives inert and
  // React never knew about it — it removes its own children by reference.
  //
  // The icon is inline SVG rather than a glyph: it scales with their icons, and
  // it keeps this file pure ASCII.
  function addButton(inner) {
    if (inner.querySelector("[data-ffwa-popout]")) return;
    var close = findCloseButton(inner);
    if (!close || !close.parentElement) return;

    var b = close.cloneNode(true);
    b.setAttribute("data-ffwa-popout", "1");
    b.setAttribute("title", "Pop out into its own window (Super+T)");
    b.removeAttribute("id");
    b.removeAttribute("aria-describedby");
    b.innerHTML =
      '<svg width="16" height="16" viewBox="0 0 16 16" fill="none" ' +
      'stroke="currentColor" stroke-width="1.4" stroke-linecap="round" ' +
      'stroke-linejoin="round" aria-hidden="true">' +
      '<path d="M9.5 2.5H14V7"/><path d="M14 2.5 8.5 8"/>' +
      '<path d="M12 9.5v3A1.5 1.5 0 0 1 10.5 14h-7A1.5 1.5 0 0 1 2 12.5v-7A1.5 1.5 0 0 1 3.5 4h3"/>' +
      "</svg>";

    var swallow = function (ev) {
      ev.preventDefault();
      ev.stopPropagation();
    };
    b.addEventListener("mousedown", swallow, true); // don't start a window drag
    b.addEventListener(
      "click",
      function (ev) {
        swallow(ev);
        if (state) restore();
        else popOut();
      },
      true
    );

    close.parentElement.insertBefore(b, close.parentElement.firstChild);
  }

  window.addEventListener("keydown", onKey, true);
  window.addEventListener("pagehide", function () {
    if (state) {
      try {
        state.popup.close();
      } catch (e) {}
    }
  });

  // Detection polls; it does NOT observe the document.
  //
  // A MutationObserver on documentElement with subtree:true fires on every DOM
  // change the page makes — and xterm's DOM renderer rewrites its rows on every
  // rendered frame. The callback was running dozens of times a second, each time
  // walking ancestors with Object.getOwnPropertyNames and doing subtree queries.
  // That added seconds of input latency on its own, to the embedded terminal as
  // much as the popped-out one. Twice a second is ample to notice one open.
  var lastXterm = null;
  setInterval(function () {
    if (state) return;
    var x = document.querySelector(".xterm");
    if (!x) {
      lastXterm = null;
      return;
    }
    if (x === lastXterm) return;
    lastXterm = x;
    var parts = findParts(x);
    addButton(parts.inner);
    if (CFG.diag) {
      stamp = " [ffwa react=" + (parts.reactKept ? "kept" : "lost") + " io=" + ioHits + "]";
    }
    if (CFG.auto) popOut();
  }, 500);

  if (CFG.diag) {
    // The SPA rewrites <title> on route changes, so re-stamp it.
    setInterval(function () {
      try {
        if (document.title.indexOf(stamp) === -1) {
          document.title = document.title.split(" [ffwa")[0] + stamp;
        }
      } catch (e) {}
    }, 1000);
  }

  log("ready — Super+T or the titlebar button pops the terminal out");
})();
