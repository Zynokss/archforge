//! Offline quick reference, with original summaries and links to official documentation.
use crate::wiki_data::{ENTRIES, Entry, TOPICS};
use crate::{button, column, label, row, wrapped};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};
fn saved_path() -> std::path::PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| glib::home_dir().join(".config"))
        .join("archforge/wiki-saved.json")
}
fn saved() -> BTreeSet<String> {
    std::fs::read(saved_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn save(items: &BTreeSet<String>) -> Result<(), String> {
    use std::io::Write;
    let path = saved_path();
    let parent = path.parent().ok_or("Invalid preferences path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(items).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn key(entry: &Entry) -> String {
    format!("{}:{}", entry.topic, entry.title)
}

struct View {
    parent: glib::WeakRef<gtk::ApplicationWindow>,
    selected: Cell<usize>,
    mode: Cell<u8>,
    query: RefCell<String>,
    saved: RefCell<BTreeSet<String>>,
    cards: gtk::Box,
    title: gtk::Label,
    intro: gtk::Label,
    tip: gtk::Label,
    source: gtk::LinkButton,
    contents: gtk::Box,
    scroll: gtk::ScrolledWindow,
    topics: Vec<gtk::Button>,
    message: gtk::Label,
}
impl View {
    fn render(self: &Rc<Self>) {
        while let Some(child) = self.cards.first_child() {
            self.cards.remove(&child);
        }
        while let Some(child) = self.contents.first_child() {
            self.contents.remove(&child);
        }
        let topic = &TOPICS[self.selected.get()];
        let query = self.query.borrow().to_lowercase();
        let saved = self.saved.borrow();
        self.title.set_text(match self.mode.get() {
            1 => "Command cheat sheet",
            2 => "Saved commands",
            _ => topic.title,
        });
        self.intro.set_text(if query.is_empty() {
            topic.intro
        } else {
            "Results across all topics. Replace placeholders before use."
        });
        self.tip.set_text(topic.tip);
        self.source.set_uri(topic.url);
        self.source
            .set_label(&format!("ArchWiki: {} ↗", topic.title));
        for (index, button) in self.topics.iter().enumerate() {
            if index == self.selected.get() {
                button.add_css_class("chosen");
            } else {
                button.remove_css_class("chosen");
            }
        }
        let mut count = 0;
        for entry in ENTRIES {
            if self.mode.get() == 2 && !saved.contains(&key(entry)) {
                continue;
            }
            if self.mode.get() == 0 && query.is_empty() && entry.topic != self.selected.get() {
                continue;
            }
            let text = format!(
                "{} {} {} {}",
                entry.title, entry.description, entry.command, TOPICS[entry.topic].title
            )
            .to_lowercase();
            if !query.split_whitespace().all(|term| text.contains(term)) {
                continue;
            }
            count += 1;
            let card = column(7);
            card.add_css_class("wiki-command-card");
            let top = row(8);
            let title = label(entry.title, "row-title");
            title.set_hexpand(true);
            title.set_wrap(true);
            top.append(&title);
            let effect = label(
                entry.effect,
                if entry.effect == "Read-only" {
                    "wiki-read-badge"
                } else {
                    "wiki-change-badge"
                },
            );
            top.append(&effect);
            card.append(&top);
            if self.mode.get() != 1 {
                card.append(&wrapped(entry.description, "muted"));
            }
            let command_row = row(8);
            command_row.add_css_class("wiki-code-row");
            let code = wrapped("", "wiki-code");
            code.set_selectable(true);
            code.set_hexpand(true);
            code.set_markup(&command_markup(entry.command));
            command_row.append(&code);
            let copy = gtk::Button::from_icon_name("edit-copy-symbolic");
            copy.set_tooltip_text(Some("Copy command"));
            copy.update_property(&[gtk::accessible::Property::Label("Copy command")]);
            copy.connect_clicked(move |button| {
                crate::copy_text(entry.command);
                button.set_tooltip_text(Some("Copied"));
            });
            command_row.append(&copy);
            card.append(&command_row);
            let actions = row(8);
            let reference =
                gtk::LinkButton::with_label(TOPICS[entry.topic].url, "Official guide ↗");
            actions.append(&reference);
            let star = gtk::ToggleButton::with_label(if saved.contains(&key(entry)) {
                "★ Saved"
            } else {
                "☆ Save"
            });
            star.set_active(saved.contains(&key(entry)));
            let weak = Rc::downgrade(self);
            star.connect_toggled(move |button| {
                if let Some(view) = weak.upgrade() {
                    let mut items = view.saved.borrow_mut();
                    if button.is_active() {
                        items.insert(key(entry));
                    } else {
                        items.remove(&key(entry));
                    }
                    let persisted = save(&items);
                    drop(items);
                    match persisted {
                        Ok(()) => {
                            view.message.set_text("Saved commands are stored locally.");
                            if view.mode.get() == 2 {
                                let weak = Rc::downgrade(&view);
                                glib::idle_add_local_once(move || {
                                    if let Some(view) = weak.upgrade() {
                                        view.render();
                                    }
                                });
                            }
                            button.set_label(if button.is_active() {
                                "★ Saved"
                            } else {
                                "☆ Save"
                            });
                        }
                        Err(e) => view.message.set_text(&format!("Could not save: {e}")),
                    };
                }
            });
            actions.append(&star);
            if entry.placeholder {
                actions.append(&label("Replace UPPERCASE placeholders · copy only", "tiny"));
            } else if entry.effect != "Read-only" || entry.command.starts_with("sudo ") {
                let review = button("Review command", false);
                let owner = self.parent.clone();
                review.connect_clicked(move |_| {
                    if let Some(parent) = owner.upgrade() {
                        crate::embedded_terminal::show(
                            parent.upcast_ref(),
                            entry.title,
                            entry.description,
                            entry.command,
                        );
                    }
                });
                actions.append(&review);
            }
            if !entry.placeholder
                && entry.effect == "Read-only"
                && !entry.command.starts_with("sudo ")
            {
                let inspect = button("Inspect", false);
                let owner = self.parent.clone();
                inspect.connect_clicked(move |_| {
                    if let Some(parent) = owner.upgrade() {
                        crate::tools::inspect_cleanup(
                            &parent,
                            entry.title,
                            entry.description,
                            "help-browser-symbolic",
                            entry.command,
                        );
                    }
                });
                actions.append(&inspect);
            }
            card.append(&actions);
            self.cards.append(&card);
            if count <= 6 {
                let jump = button(entry.title, false);
                jump.add_css_class("wiki-toc-link");
                jump.set_tooltip_text(Some(entry.title));
                if let Some(text) = jump.child().and_then(|w| w.downcast::<gtk::Label>().ok()) {
                    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    text.set_max_width_chars(26);
                    text.set_xalign(0.0);
                }
                let target = card.downgrade();
                let container = self.cards.downgrade();
                let scroll = self.scroll.clone();
                jump.connect_clicked(move |_| {
                    if let (Some(target), Some(container)) = (target.upgrade(), container.upgrade())
                        && let Some(rect) = target.compute_bounds(&container)
                    {
                        scroll.vadjustment().set_value(rect.y() as f64);
                    }
                });
                self.contents.append(&jump);
            }
        }
        if count == 0 {
            self.cards.append(&wrapped(
                if self.mode.get() == 2 {
                    "No saved commands match. Save commands while browsing to collect them here."
                } else {
                    "No commands match. Try a package name, task, command, or another topic."
                },
                "muted",
            ));
        }
        self.message.set_text(&format!("{count} commands · quick reference · consult the linked documentation before unfamiliar changes."));
        self.scroll.vadjustment().set_value(0.0);
    }
}
fn command_markup(command: &str) -> String {
    command
        .split_inclusive(char::is_whitespace)
        .map(|word| {
            let token = word.trim();
            let color = if token == "sudo" {
                "#cba6f7"
            } else if token.starts_with('-') {
                "#ffbf69"
            } else if token.chars().any(|c| c.is_ascii_uppercase()) {
                "#89b4fa"
            } else {
                "#a6e3a1"
            };
            format!(
                "<span foreground=\"{color}\">{}</span>",
                glib::markup_escape_text(word)
            )
        })
        .collect()
}
pub(crate) fn page(parent: &gtk::ApplicationWindow) -> gtk::Box {
    let page = column(12);
    page.add_css_class("content");
    page.add_css_class("wiki-page");
    page.set_vexpand(true);
    let banner_content = column(0);
    banner_content.append(&label("Workspace / Wiki", "eyebrow"));
    let header = row(12);
    let text = column(4);
    text.set_hexpand(true);
    text.append(&label("Your Arch field guide.", "page-title"));
    text.append(&label(
        "Find a command. Understand it. Get things done.",
        "subtitle",
    ));
    header.append(&text);
    let official = gtk::LinkButton::with_label("https://wiki.archlinux.org/", "Open ArchWiki ↗");
    official.set_valign(gtk::Align::Center);
    header.append(&official);
    banner_content.append(&header);
    page.append(&crate::theme::banner(&banner_content));
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search commands, tasks, errors, or topics…")
        .hexpand(true)
        .build();
    search.update_property(&[gtk::accessible::Property::Label("Search Wiki")]);
    page.append(&search);
    let tabs = row(4);
    tabs.add_css_class("wiki-tabs");
    let mut modes = Vec::new();
    for name in ["Browse", "Command cheat sheet", "Saved"] {
        let button = gtk::ToggleButton::with_label(name);
        if let Some(first) = modes.first() {
            button.set_group(Some(first));
        }
        tabs.append(&button);
        modes.push(button);
    }
    modes[0].set_active(true);
    page.append(&tabs);
    let topic_names: Vec<_> = TOPICS.iter().map(|t| t.title).collect();
    let picker = gtk::DropDown::from_strings(&topic_names);
    picker.set_selected(1);
    picker.add_css_class("wiki-topic-picker");
    picker.set_visible(false);
    page.append(&picker);
    let body = row(12);
    body.add_css_class("wiki-columns");
    body.set_vexpand(true);
    let topics = column(4);
    topics.add_css_class("wiki-panel");
    topics.set_width_request(180);
    topics.append(&label("Topics", "section-title"));
    let mut buttons = Vec::new();
    for topic in TOPICS {
        let b = gtk::Button::new();
        b.add_css_class("wiki-topic");
        let line = row(9);
        let icon = gtk::Image::from_icon_name(topic.icon);
        icon.set_pixel_size(18);
        line.append(&icon);
        line.append(&label(topic.title, ""));
        b.set_child(Some(&line));
        topics.append(&b);
        buttons.push(b);
    }
    let topics_scroll = gtk::ScrolledWindow::builder()
        .child(&topics)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(180)
        .max_content_width(180)
        .propagate_natural_width(true)
        .build();
    topics_scroll.add_css_class("wiki-topic-list");
    body.append(&topics_scroll);
    let center = column(8);
    center.add_css_class("wiki-panel");
    center.set_hexpand(true);
    let title = label("", "wiki-topic-title");
    center.append(&title);
    let intro = wrapped("", "subtitle");
    center.append(&intro);
    let cards = column(8);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&cards)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .min_content_height(260)
        .build();
    center.append(&scroll);
    let message = wrapped("", "tiny");
    center.append(&message);
    body.append(&center);
    let aside = column(12);
    aside.set_width_request(245);
    aside.set_hexpand(false);
    aside.set_valign(gtk::Align::Start);
    let toc = column(5);
    toc.add_css_class("wiki-panel");
    toc.append(&label("On this page", "section-title"));
    let contents = column(2);
    toc.append(&contents);
    aside.append(&toc);
    let knowledge = column(8);
    knowledge.add_css_class("wiki-panel");
    knowledge.append(&label("Good to know", "section-title"));
    let tip = wrapped("", "wiki-tip");
    tip.set_max_width_chars(30);
    knowledge.append(&tip);
    aside.append(&knowledge);
    let sources = column(8);
    sources.add_css_class("wiki-panel");
    sources.append(&label("Official reference", "section-title"));
    let source = gtk::LinkButton::with_label("https://wiki.archlinux.org/", "ArchWiki");
    source.set_halign(gtk::Align::Start);
    sources.append(&source);
    let source_note = wrapped(
        "Offline summaries; upstream documentation can change. Check the current guide before modifying your system.",
        "tiny",
    );
    source_note.set_max_width_chars(30);
    sources.append(&source_note);
    for (name, url) in [
        (
            "System maintenance",
            "https://wiki.archlinux.org/title/System_maintenance",
        ),
        (
            "Backup a package list",
            "https://wiki.archlinux.org/title/Pacman/Tips_and_tricks#List_of_installed_packages",
        ),
        (
            "Understand the AUR",
            "https://wiki.archlinux.org/title/Arch_User_Repository",
        ),
    ] {
        let link = gtk::LinkButton::with_label(url, name);
        link.set_halign(gtk::Align::Start);
        sources.append(&link);
    }
    aside.append(&sources);
    let aside_scroll = gtk::ScrolledWindow::builder()
        .child(&aside)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(245)
        .max_content_width(245)
        .propagate_natural_width(true)
        .build();
    aside_scroll.set_hexpand(false);
    body.append(&aside_scroll);
    page.append(&body);
    let view = Rc::new(View {
        parent: parent.downgrade(),
        selected: Cell::new(1),
        mode: Cell::new(0),
        query: RefCell::new(String::new()),
        saved: RefCell::new(saved()),
        cards,
        title,
        intro,
        tip,
        source,
        contents,
        scroll,
        topics: buttons,
        message,
    });
    let weak = Rc::downgrade(&view);
    picker.connect_selected_notify(move |picker| {
        if let Some(view) = weak.upgrade() {
            view.selected
                .set((picker.selected() as usize).min(TOPICS.len() - 1));
            view.render();
        }
    });
    for (i, button) in view.topics.iter().enumerate() {
        let weak = Rc::downgrade(&view);
        let search = search.clone();
        let browse = modes[0].clone();
        let picker = picker.clone();
        button.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.selected.set(i);
                picker.set_selected(i as u32);
                view.mode.set(0);
                view.query.borrow_mut().clear();
                search.set_text("");
                browse.set_active(true);
                view.render();
            }
        });
    }
    for (i, button) in modes.iter().enumerate() {
        let weak = Rc::downgrade(&view);
        button.connect_toggled(move |button| {
            if button.is_active()
                && let Some(view) = weak.upgrade()
            {
                view.mode.set(i as u8);
                view.render();
            }
        });
    }
    let weak = Rc::downgrade(&view);
    search.connect_search_changed(move |search| {
        if let Some(view) = weak.upgrade() {
            *view.query.borrow_mut() = search.text().to_string();
            view.render();
        }
    });
    view.render();
    page.connect_destroy(move |_| {
        let _ = &view;
    });
    page
}

#[cfg(test)]
pub(crate) fn verify_ui(parent: &gtk::ApplicationWindow) {
    tests::verify(parent);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guide_commands_are_valid_and_identifiers_are_unique() {
        let mut keys = BTreeSet::new();
        for entry in ENTRIES {
            assert!(entry.topic < TOPICS.len());
            assert!(keys.insert(key(entry)));
            assert!(
                TOPICS[entry.topic]
                    .url
                    .starts_with("https://wiki.archlinux.org/")
            );
            assert!(!entry.command.contains("pacman -Sy "));
            assert!(
                std::process::Command::new("/usr/bin/bash")
                    .args(["-n", "-c", entry.command])
                    .status()
                    .unwrap()
                    .success(),
                "{}",
                entry.title
            );
        }
    }
    pub(super) fn verify(parent: &gtk::ApplicationWindow) {
        fn walk(w: &gtk::Widget) -> Vec<gtk::Widget> {
            let mut out = vec![w.clone()];
            let mut c = w.first_child();
            while let Some(child) = c {
                out.extend(walk(&child));
                c = child.next_sibling();
            }
            out
        }
        let window = gtk::Window::builder()
            .title("Wiki verification")
            .transient_for(parent)
            .default_width(1300)
            .default_height(900)
            .build();
        let content = page(parent);
        window.set_child(Some(&content));
        window.present();
        let search = walk(content.upcast_ref())
            .into_iter()
            .find_map(|w| w.downcast::<gtk::SearchEntry>().ok())
            .unwrap();
        search.set_text("journalctl vacuum");
        search.emit_by_name::<()>("search-changed", &[]);
        assert!(
            walk(content.upcast_ref())
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Label>())
                .any(|l| l.text() == "Keep seven days of archived logs")
        );
        let saved = walk(content.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::ToggleButton>().ok())
            .find(|b| b.label().as_deref() == Some("☆ Save"))
            .unwrap();
        saved.set_active(true);
        assert!(super::saved().iter().any(|k| k.contains("Keep seven days")));
        let review = walk(content.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.label().as_deref() == Some("Review command"))
            .unwrap();
        review.emit_clicked();
        if let Some(review) = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.title().as_deref() == Some("Keep seven days of archived logs"))
        {
            crate::dialogs::snapshot(&review, "wiki-review-command");
            review.close();
        } else {
            panic!("Review dialog missing");
        }
        search.set_text("no-such-guide-entry-12345");
        search.emit_by_name::<()>("search-changed", &[]);
        assert!(
            walk(content.upcast_ref())
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Label>())
                .any(|l| l.text().starts_with("No commands match"))
        );
        search.set_text("");
        search.emit_by_name::<()>("search-changed", &[]);
        crate::dialogs::snapshot(&window, "wiki-browse");
        window.close();
    }
}
