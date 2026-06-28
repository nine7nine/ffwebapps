//! Small shared UI helpers.

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
