//! Small shared UI helpers + the app's "glass" appearance (matching the Poxicle
//! configurator: forced-dark, a translucent dark window with transparent chrome,
//! accent-tinted tabs, and thin-bordered glass buttons/entries).

use std::sync::Once;

/// Glass stylesheet, adapted from the Poxicle configurator (preset/tune-specific
/// rules dropped). The window background carries a single translucent tint that
/// the compositor composites; everything stacked on it is transparent so the
/// tint shows through uniformly.
const APP_CSS: &str = "\
@define-color accent_bg_color #3584e4;\
@define-color accent_color #3584e4;\
@define-color accent_fg_color #ffffff;\
window { background-color: rgba(20,20,26,0.92); color: rgba(255,255,255,0.97); }\
headerbar, .toolbar { background: transparent; background-image: none; box-shadow: none; border: none; }\
box, grid, stack, viewstack, list, row, scrolledwindow, viewport, .view, toolbarview { background: transparent; background-image: none; }\
label { text-shadow: 0 1px 2px rgba(0,0,0,0.55); }\
separator { background-color: rgba(255,255,255,0.10); }\
viewswitcher button { background: transparent; box-shadow: none; color: rgba(255,255,255,0.82); }\
viewswitcher button:hover { background-color: rgba(255,255,255,0.06); }\
viewswitcher button:checked { color: #ffffff; background-color: color-mix(in srgb, @accent_bg_color 24%, transparent); border: 1px solid color-mix(in srgb, @accent_bg_color 70%, transparent); }\
viewswitcher button:backdrop { color: rgba(255,255,255,0.80); }\
viewswitcher button:checked:backdrop { color: #ffffff; background-color: color-mix(in srgb, @accent_bg_color 28%, transparent); border-color: color-mix(in srgb, @accent_bg_color 62%, transparent); }\
button:not(.titlebutton):not(.close):not(.minimize):not(.maximize) { background: transparent; background-image: none; box-shadow: none; border: 1px solid rgba(255,255,255,0.15); border-radius: 6px; }\
button:not(.titlebutton):not(.close):not(.minimize):not(.maximize):hover { border-color: rgba(255,255,255,0.30); background-color: rgba(255,255,255,0.05); }\
button.flat:not(.titlebutton):not(.close):not(.minimize):not(.maximize) { border: none; background: transparent; }\
button.flat:not(.titlebutton):not(.close):not(.minimize):not(.maximize):hover { background-color: rgba(255,255,255,0.08); }\
button.suggested-action { border-color: color-mix(in srgb, @accent_bg_color 75%, transparent); color: @accent_color; }\
button.destructive-action { border-color: color-mix(in srgb, #e01b24 70%, transparent); color: #ff7b7b; }\
dropdown > button, entry, combobox button { background: transparent; border: 1px solid rgba(255,255,255,0.15); border-radius: 6px; color: rgba(255,255,255,0.97); }\
entry > text { color: rgba(255,255,255,0.97); caret-color: rgba(255,255,255,0.97); }\
entry > text > placeholder { color: rgba(255,255,255,0.40); }\
list, list > row { background: transparent; }\
list > row:selected { background-color: color-mix(in srgb, @accent_bg_color 20%, transparent); }\
list > row:selected, list > row:selected *, list > row:selected label, list > row:selected text { color: rgba(255,255,255,0.97); }\
/* AdwPreferencesGroup boxed lists read as clean rounded cards with separators \
   and breathing room (the glass transparency would otherwise flatten them into \
   borderless, condensed text). */\
list.boxed-list { background-color: rgba(255,255,255,0.045); border: 1px solid rgba(255,255,255,0.10); border-radius: 12px; }\
list.boxed-list > row { min-height: 46px; padding-top: 4px; padding-bottom: 4px; }\
list.boxed-list > row:not(:last-child) { border-bottom: 1px solid rgba(255,255,255,0.07); }\
check:checked { background-color: @accent_bg_color; background-image: none; border-color: @accent_bg_color; color: #ffffff; }\
/* switches: dark track when off, accent track when on, ALWAYS a white knob \
   (styling the knob with the accent made it invisible on the on-state track) */\
switch { background-color: rgba(255,255,255,0.18); border: 1px solid rgba(255,255,255,0.12); box-shadow: none; }\
switch:checked { background-color: @accent_bg_color; border-color: @accent_bg_color; }\
switch > slider { background-color: #ffffff; background-image: none; box-shadow: 0 1px 2px rgba(0,0,0,0.45); }\
switch:checked > slider { background-color: #ffffff; }\
popover > contents { background-color: rgb(34,34,42); border: 1px solid rgba(255,255,255,0.14); box-shadow: 0 6px 18px rgba(0,0,0,0.55); color: rgba(255,255,255,0.97); }\
popover > arrow { background-color: rgb(34,34,42); border: 1px solid rgba(255,255,255,0.14); }\
popover, popover label, popover row { color: rgba(255,255,255,0.97); }\
toast { color: rgba(255,255,255,0.97); }\
/* dialogs are an OPAQUE floating panel (not the translucent window glass), so \
   they read as a distinct sheet over a dimmed backdrop */\
.dialog-surface { background-color: rgb(36,36,44); }\
.dialog-surface headerbar { background-color: rgba(255,255,255,0.03); box-shadow: none; }\
/* multi-line text inputs read as a subtle inset field, not a bare void */\
.text-field { background-color: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.14); border-radius: 8px; }\
.text-field:focus-within { border-color: rgba(255,255,255,0.30); }\
textview, textview text { background: transparent; color: rgba(255,255,255,0.95); }\
scrollbar, scrollbar > trough { background: transparent; border: none; }\
scrollbar > range > trough > slider { background-color: rgba(255,255,255,0.25); }\
/* libadwaita dims section titles/subtitles/placeholders via --dim-opacity \
   (~0.55), which is unreadable on the dark glass — raise it. */\
:root, window { --dim-opacity: 0.85; }\
window:backdrop, headerbar:backdrop, .toolbar:backdrop { color: rgba(255,255,255,0.95); }\
";

/// Force the dark variant and install the glass stylesheet (idempotent).
pub fn apply_app_style() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

        if let Some(display) = gtk::gdk::Display::default() {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(APP_CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

thread_local! {
    /// A second provider (USER priority, above the static glass sheet) that only
    /// redefines the accent colours + window tint. Every static rule that
    /// references `@accent_bg_color`/`@accent_color` re-resolves for free.
    static APPEARANCE_PROVIDER: gtk::CssProvider = {
        let provider = gtk::CssProvider::new();
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
        provider
    };
}

/// Apply the live appearance (opacity / glass tint / accent). Re-callable.
pub fn apply_appearance(appearance: &crate::core::Appearance) {
    let (r, g, b) = parse_hex(&appearance.glass);
    let alpha = f64::from(appearance.opacity.min(100)) / 100.0;
    let accent = &appearance.accent;
    let css = format!(
        "@define-color accent_bg_color {accent};\
         @define-color accent_color {accent};\
         @define-color accent_fg_color #ffffff;\
         window {{ background-color: rgba({r},{g},{b},{alpha:.3}); }}"
    );
    APPEARANCE_PROVIDER.with(|provider| provider.load_from_data(&css));
}

fn parse_hex(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        )
    {
        return (r, g, b);
    }
    (20, 20, 26)
}

/// A full-width, vertically-scrolling content area. Used instead of
/// `AdwPreferencesPage` so there's no clamp and no large left/right padding —
/// append `AdwPreferencesGroup`s (or any widgets) to the returned box.
pub fn content() -> (gtk::ScrolledWindow, gtk::Box) {
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(true)
        .vexpand(true)
        // Report the child's natural height so short dialogs size to their
        // content instead of collapsing to a tiny scrolling sliver. In the main
        // window vexpand still fills the height, and taller content scrolls.
        .propagate_natural_height(true)
        .child(&body)
        .build();
    (scroll, body)
}
