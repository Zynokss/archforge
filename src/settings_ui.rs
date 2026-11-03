use crate::{button, column, label, row, wrapped};
use gtk::prelude::*;
pub(crate) fn page(
    updates: &gtk::Button,
    appearance: &gtk::MenuButton,
    stack: &gtk::Stack,
) -> gtk::Box {
    let page = column(18);
    page.add_css_class("content");
    page.add_css_class("settings-page");
    let header = column(0);
    header.append(&label("Workspace / Settings", "eyebrow"));
    header.append(&label("Make Arch Forge yours.", "page-title"));
    header.append(&wrapped(
        "Appearance, updates, preferences, and information about the app.",
        "subtitle",
    ));
    page.append(&crate::theme::banner(&header));
    let controls = column(12);
    controls.add_css_class("panel");
    controls.append(&label("App preferences", "section-title"));
    for (key, title, description, initial) in [
        (
            "automatic_updates",
            "Check for app updates on startup",
            "Check GitHub for new releases when Arch Forge opens. Installation always requires your action. Applies on the next launch.",
            crate::preferences::automatic_updates(),
        ),
        (
            "reduced_motion",
            "Reduce motion",
            "Disable page transitions and GTK animations in Arch Forge.",
            crate::preferences::reduced_motion(),
        ),
    ] {
        let line = row(16);
        let text = column(4);
        text.set_hexpand(true);
        text.append(&label(title, "row-title"));
        text.append(&wrapped(description, "muted"));
        line.append(&text);
        let switch = gtk::Switch::new();
        switch.set_active(initial);
        switch.set_valign(gtk::Align::Center);
        line.append(&switch);
        let status = label("", "tiny");
        let feedback = status.clone();
        let stack = stack.clone();
        switch.connect_active_notify(
            move |s| match crate::preferences::save(key, s.is_active()) {
                Ok(()) => {
                    feedback.set_text("Saved");
                    if key == "reduced_motion" {
                        stack.set_transition_duration(if s.is_active() { 0 } else { 160 });
                        if let Some(settings) = gtk::Settings::default() {
                            settings.set_gtk_enable_animations(!s.is_active());
                        }
                    }
                }
                Err(e) => feedback.set_text(&format!("Could not save: {e}")),
            },
        );
        controls.append(&line);
        controls.append(&status);
    }
    let links = row(12);
    let theme = button("Appearance", false);
    let menu = appearance.clone();
    theme.connect_clicked(move |_| menu.popup());
    links.append(&theme);
    let update = button("App updates", false);
    let launcher = updates.clone();
    update.connect_clicked(move |_| launcher.emit_clicked());
    links.append(&update);
    controls.append(&links);
    page.append(&controls);
    let about = column(12);
    about.add_css_class("panel");
    about.append(&label("About Arch Forge", "section-title"));
    about.append(&label(
        &format!("Arch Forge {}", env!("CARGO_PKG_VERSION")),
        "row-title",
    ));
    about.append(&wrapped("Created and maintained by zynoksy. A native Arch Linux system workbench, built with Rust and GTK4.", "muted"));
    for (name, url) in [
        ("zynoksy on GitHub", "https://github.com/zynoksy"),
        ("Source code", "https://github.com/zynoksy/archforge"),
        (
            "Report an issue",
            "https://github.com/zynoksy/archforge/issues",
        ),
        (
            "License & artwork notices",
            "https://github.com/zynoksy/archforge#license",
        ),
    ] {
        let link = gtk::LinkButton::with_label(url, name);
        link.set_halign(gtk::Align::Start);
        about.append(&link);
    }
    about.append(&wrapped("Independent community software. Not affiliated with Arch Linux. App preferences are stored locally; update checks contact GitHub when enabled. Ricing previews contact their upstream image hosts when opened.", "tiny"));
    page.append(&about);
    page
}
