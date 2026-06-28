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
check:checked, switch:checked, switch:checked > slider { background-color: @accent_bg_color; background-image: none; border-color: @accent_bg_color; color: #ffffff; }\
popover > contents { background-color: rgb(34,34,42); border: 1px solid rgba(255,255,255,0.14); box-shadow: 0 6px 18px rgba(0,0,0,0.55); color: rgba(255,255,255,0.97); }\
popover > arrow { background-color: rgb(34,34,42); border: 1px solid rgba(255,255,255,0.14); }\
popover, popover label, popover row { color: rgba(255,255,255,0.97); }\
toast { color: rgba(255,255,255,0.97); }\
scrollbar, scrollbar > trough { background: transparent; border: none; }\
scrollbar > range > trough > slider { background-color: rgba(255,255,255,0.25); }\
label.dim-label, .dim-label { opacity: 1; color: rgba(255,255,255,0.72); }\
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
        .child(&body)
        .build();
    (scroll, body)
}
