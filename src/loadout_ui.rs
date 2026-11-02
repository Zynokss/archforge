use crate::{
    button, column,
    dialogs::{Surface, Tone},
    label,
    loadouts::{self, App, Profile, Source},
    row, wrapped,
};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
    sync::mpsc,
    time::Duration,
};
struct View {
    parent: glib::WeakRef<gtk::ApplicationWindow>,
    profile: RefCell<Profile>,
    enabled: RefCell<BTreeSet<String>>,
    category: Cell<usize>,
    query: RefCell<String>,
    source: Cell<u32>,
    sort: Cell<u32>,
    category_picker: gtk::DropDown,
    category_scroll: gtk::ScrolledWindow,
    revealed_category: Cell<usize>,
    page: Cell<usize>,
    library: gtk::Box,
    files: gtk::Box,
    summary: gtk::Box,
    count: gtk::Label,
    profile_name: gtk::Entry,
    command: gtk::TextView,
    status: gtk::Label,
    foreign: gtk::CheckButton,
    foreign_note: gtk::Label,
    results: gtk::Label,
    previous: gtk::Button,
    next: gtk::Button,
    install: gtk::Button,
    export: gtk::Button,
    apply: gtk::Button,
    category_buttons: Vec<gtk::Button>,
    library_tab: gtk::Stack,
    ricing: gtk::Box,
    packages: RefCell<crate::loadout_state::Snapshot>,
    scanning: Cell<bool>,
    status_filter: Cell<u32>,
    scan_status: gtk::Label,
    scan_indicators: gtk::Box,
    columns: [gtk::SizeGroup; 4],
    command_hint: gtk::Label,
    refresh: gtk::Button,
    skip: gtk::CheckButton,
    uninstall: gtk::Button,
    update: gtk::Button,
}
fn text_view() -> gtk::TextView {
    let view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(12)
        .right_margin(12)
        .top_margin(10)
        .bottom_margin(10)
        .build();
    view.add_css_class("console");
    crate::dialogs::style_output(&view.buffer());
    for (name, color) in [
        ("command-program", "#cba6f7"),
        ("command-flag", "#ffbf69"),
        ("command-package", "#89dceb"),
    ] {
        view.buffer()
            .tag_table()
            .add(&gtk::TextTag::builder().name(name).foreground(color).build());
    }
    view.buffer().connect_changed(|buffer| {
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        if !["sudo ", "pacman ", "yay ", "paru "]
            .iter()
            .any(|prefix| text.starts_with(prefix))
        {
            return;
        }
        let mut offset = 0;
        for segment in text.split_inclusive(char::is_whitespace) {
            let token = segment.trim();
            if !token.is_empty() {
                let tag = if ["sudo", "pacman", "yay", "paru"].contains(&token) {
                    "command-program"
                } else if token.starts_with('-') {
                    "command-flag"
                } else {
                    "command-package"
                };
                buffer.apply_tag_by_name(
                    tag,
                    &buffer.iter_at_offset(offset),
                    &buffer.iter_at_offset(offset + token.chars().count() as i32),
                );
            }
            offset += segment.chars().count() as i32;
        }
    });

    view
}
fn write_file(path: &std::path::Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("Choose a local file")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn save_text(parent: &gtk::Window, name: &str, text: String, status: &gtk::Label) {
    let parent = parent.clone();
    let name = name.to_owned();
    let status = status.clone();
    glib::spawn_future_local(async move {
        let chooser = gtk::FileDialog::builder()
            .title("Save loadout file")
            .initial_name(name.to_owned())
            .build();
        if let Ok(file) = chooser.save_future(Some(&parent)).await {
            let Some(path) = file.path() else {
                status.set_text("Choose a local file or mounted drive.");
                return;
            };
            let (tx, rx) = mpsc::channel();
            let operation = crate::Operation::new();
            std::thread::spawn(move || {
                let _operation = operation;
                let result = write_file(&path, &text).map(|_| {
                    if path.extension().is_some_and(|s| s == "sh") {
                        format!(
                            "Saved {}. Run with bash as your regular user.",
                            path.display()
                        )
                    } else {
                        format!("Saved {}", path.display())
                    }
                });
                let _ = tx.send(result);
            });
            glib::timeout_add_local(Duration::from_millis(100), move || {
                match rx.try_recv() {
                    Ok(Ok(message)) => status.set_text(&message),
                    Ok(Err(error)) => status.set_text(&format!("Could not save: {error}")),
                    Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                    Err(_) => status.set_text("The save worker stopped."),
                };
                glib::ControlFlow::Break
            });
        }
    });
}
impl View {
    fn scan_summary(&self, apps: &[App]) {
        while let Some(child) = self.scan_indicators.first_child() {
            self.scan_indicators.remove(&child);
        }
        let state = self.packages.borrow();
        self.scan_status.set_text(if self.scanning.get() {
            "◌ Scanning…"
        } else if state.ready {
            "● System scanned"
        } else {
            "● Scan unavailable"
        });
        self.scan_status.remove_css_class("scan-ready");
        if state.ready && !self.scanning.get() {
            self.scan_status.add_css_class("scan-ready");
        }
        let installed = apps
            .iter()
            .filter(|a| state.installed.contains_key(&a.name))
            .count();
        let updates = apps
            .iter()
            .filter(|a| state.updates.contains_key(&a.name))
            .count();
        let values = if state.ready {
            vec![
                (format!("● {installed} installed"), "scan-installed"),
                (
                    format!("● {} missing", apps.len() - installed),
                    "scan-missing",
                ),
                (
                    if state.updates_checked {
                        format!("● {updates} updates")
                    } else {
                        "◷ Not checked".into()
                    },
                    "scan-updates",
                ),
            ]
        } else {
            vec![("Installation status unknown".into(), "muted")]
        };
        for (text, class) in values {
            let l = label(&text, class);
            l.add_css_class("scan-factor");
            l.set_tooltip_text(Some(if class == "scan-updates" {
                &state.update_note
            } else {
                "Package counts in the current filtered list"
            }));
            self.scan_indicators.append(&l);
        }
    }
    fn install_profile(&self) -> Profile {
        let mut p = self.effective();
        if self.skip.is_active() {
            p.packages
                .retain(|name, _| !self.packages.borrow().installed.contains_key(name));
        }
        p
    }
    fn refresh_packages(self: &Rc<Self>, updates: bool) {
        if self.scanning.replace(true) {
            return;
        }
        self.refresh.set_sensitive(false);
        if updates || !self.packages.borrow().ready {
            self.scan_status.set_text(if updates {
                "Checking repository updates…"
            } else {
                "Scanning installed packages & cache…"
            });
        }
        let weak = Rc::downgrade(self);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::loadout_state::scan(updates));
        });
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let Some(v) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let result = match rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Package scan stopped. Try again.".into()),
            };
            v.scanning.set(false);
            v.refresh.set_sensitive(true);
            match result {
                Ok(mut state) => {
                    if !updates {
                        state.updates = v.packages.borrow().updates.clone();
                        state.updates_checked = v.packages.borrow().updates_checked;
                        state.update_note = v.packages.borrow().update_note.clone();
                        state
                            .updates
                            .retain(|name, (old, _)| state.installed.get(name) == Some(old));
                    }
                    let changed = *v.packages.borrow() != state;
                    if changed {
                        *v.packages.borrow_mut() = state;
                    }
                    let state = v.packages.borrow();
                    v.scan_status.set_tooltip_text(Some(&format!(
                        "{} installed system-wide. {}\n{}",
                        state.installed.len(),
                        state.update_note,
                        state.cache_note
                    )));
                    if !changed && !updates {
                        return glib::ControlFlow::Break;
                    }
                }
                Err(e) => {
                    // An unreadable database must never be presented as an empty installation.
                    v.packages.borrow_mut().ready = false;
                    v.scan_status.set_text(&format!("Scan unavailable: {e}"));
                    v.scan_status.set_tooltip_text(Some(&v.scan_status.text()));
                }
            }
            v.render_packages();
            v.render_summary();
            glib::ControlFlow::Break
        });
    }
    fn matches_status(&self, name: &str) -> bool {
        let state = self.packages.borrow();
        match self.status_filter.get() {
            1 => state.ready && state.installed.contains_key(name),
            2 => state.ready && !state.installed.contains_key(name),
            3 => state.ready && state.cached.contains(name) && !state.installed.contains_key(name),
            4 => state.ready && state.updates.contains_key(name),
            _ => true,
        }
    }
    fn remove_review(&self, names: Vec<String>) {
        if let Some(parent) = self.parent.upgrade() {
            match crate::loadout_state::remove_command(&names) {
                Ok(command) => removal_review(&parent, names, command),
                Err(error) => self.status.set_text(&error),
            }
        }
    }
    fn effective(&self) -> Profile {
        let mut p = self.profile.borrow().clone();
        p.name = self.profile_name.text().to_string();
        let state = self.packages.borrow();
        p.packages = p
            .packages
            .into_iter()
            .map(|(name, source)| (state.installed_variant(&name).to_owned(), source))
            .collect();
        let enabled = self.enabled.borrow();
        p.files.retain(|path, _| enabled.contains(path));
        p.executable.retain(|path| enabled.contains(path));
        p.modes.retain(|path, _| enabled.contains(path));
        p
    }
    fn replace(self: &Rc<Self>, profile: Profile) {
        self.profile_name.set_text(&profile.name);
        *self.enabled.borrow_mut() = profile.files.keys().cloned().collect();
        *self.profile.borrow_mut() = profile;
        self.page.set(0);
        self.render_packages();
        self.render_files();
        self.render_summary();
    }
    fn apps(&self) -> Vec<App> {
        let mut apps = crate::loadout_styles::catalog();
        for app in &mut apps {
            let state = self.packages.borrow();
            let actual = state.installed_variant(&app.name);
            if actual != app.name {
                app.description = format!("{} Installed package: {actual}.", app.description);
                app.name = actual.to_owned();
            }
        }
        let existing: BTreeSet<_> = apps.iter().map(|a| a.name.clone()).collect();
        for (name, source) in &self.profile.borrow().packages {
            if !existing.contains(name) {
                apps.push(App {
                    name: name.clone(),
                    title: name.clone(),
                    category: "Imported / custom".into(),
                    description: "From this loadout; verify availability on the target system."
                        .into(),
                    source: *source,
                    priority: 2,
                    popularity: None,
                });
            }
        }
        for app in &mut apps {
            if let Some(source) = self.profile.borrow().packages.get(&app.name) {
                app.source = *source;
            }
        }
        crate::loadout_styles::sort_catalog(
            &mut apps,
            self.sort.get(),
            &self.packages.borrow().installed,
        );
        apps
    }
    fn render_packages(self: &Rc<Self>) {
        if self.category_picker.selected() != self.category.get() as u32 {
            self.category_picker
                .set_selected(self.category.get() as u32);
        }
        while let Some(child) = self.library.first_child() {
            self.library.remove(&child);
        }
        for (i, b) in self.category_buttons.iter().enumerate() {
            if i == self.category.get() {
                b.add_css_class("chosen");
            } else {
                b.remove_css_class("chosen");
            }
        }
        if self.revealed_category.replace(self.category.get()) != self.category.get() {
            let chosen = self.category_buttons[self.category.get()].downgrade();
            let adjustment = self.category_scroll.vadjustment();
            glib::timeout_add_local_once(Duration::from_millis(50), move || {
                if let Some(button) = chosen.upgrade()
                    && button.has_css_class("chosen")
                    && let Some(parent) = button.parent()
                    && let Some(bounds) = button.compute_bounds(&parent)
                {
                    let top = bounds.y() as f64;
                    let bottom = top + bounds.height() as f64;
                    if top < adjustment.value() {
                        adjustment.set_value(top);
                    } else if bottom > adjustment.value() + adjustment.page_size() {
                        adjustment.set_value((bottom - adjustment.page_size()).max(0.));
                    }
                }
            });
        }
        let query = self.query.borrow().to_lowercase();
        let terms: Vec<_> = query.split_whitespace().collect();
        let filtered: Vec<_> = self
            .apps()
            .into_iter()
            .filter(|a| {
                self.matches_status(&a.name)
                    && (self.category.get() == 0
                        || loadouts::CATEGORIES[self.category.get()] == a.category)
                    && (self.source.get() == 0
                        || self.source.get()
                            == match a.source {
                                Source::Repo => 1,
                                Source::Aur => 2,
                                Source::Foreign => 3,
                            })
                    && terms.iter().all(|q| {
                        format!("{} {} {}", a.name, a.title, a.description)
                            .to_lowercase()
                            .contains(q)
                    })
            })
            .collect();
        self.scan_summary(&filtered);
        let pages = filtered.len().div_ceil(40).max(1);
        if self.page.get() >= pages {
            self.page.set(pages - 1);
        }
        self.previous.set_sensitive(self.page.get() > 0);
        self.next.set_sensitive(self.page.get() + 1 < pages);
        self.results.set_text(&format!(
            "{} matches · page {} of {pages}",
            filtered.len(),
            self.page.get() + 1
        ));
        if filtered.is_empty() {
            self.library.append(&wrapped("No packages match. Try All packages, another source filter, or add an exact package name below.","muted"));
        }
        for app in filtered.into_iter().skip(self.page.get() * 40).take(40) {
            let card = row(10);
            card.add_css_class("loadout-app");
            let icon = crate::app_icons::icon(&app);
            let package = row(10);
            package.set_hexpand(true);
            icon.set_size_request(28, 28);
            package.append(&icon);
            let text = column(3);
            text.set_hexpand(true);
            let title = label(&app.title, "row-title");
            title.set_ellipsize(gtk::pango::EllipsizeMode::End);
            title.set_tooltip_text(Some(&format!(
                "{}\n{}",
                app.title,
                crate::loadout_styles::ranking_note(&app)
            )));
            text.append(&title);
            let desc = label(&format!("{} · {}", app.name, app.description), "tiny");
            desc.set_ellipsize(gtk::pango::EllipsizeMode::End);
            desc.set_max_width_chars(48);
            desc.set_tooltip_text(Some(&app.description));
            text.append(&desc);
            package.append(&text);
            card.append(&package);
            let state = self.packages.borrow();
            let (status, class, detail) = state.describe(&app.name);
            let status_cell = column(2);
            let badge = label(&format!("● {status}"), class);
            badge.add_css_class("loadout-status-badge");
            badge.set_halign(gtk::Align::Start);
            badge.set_tooltip_text(Some(&detail));
            status_cell.append(&badge);
            if let Some((old, new)) = state.updates.get(&app.name) {
                let version = label(&format!("{old} → {new}"), "loadout-version");
                version.set_ellipsize(gtk::pango::EllipsizeMode::End);
                version.set_max_width_chars(22);
                version.set_tooltip_text(Some(&detail));
                status_cell.append(&version);
            }
            card.append(&table_cell(&status_cell, &self.columns[0], 150));
            let uri = match app.source {
                Source::Repo => format!("https://archlinux.org/packages/?name={}", app.name),
                Source::Aur => format!("https://aur.archlinux.org/packages/{}", app.name),
                Source::Foreign => String::new(),
            };
            let source: gtk::Widget = if uri.is_empty() {
                label(app.source.label(), "loadout-source-badge").upcast()
            } else {
                let link = gtk::LinkButton::with_label(&uri, app.source.label());
                link.add_css_class("loadout-source-badge");
                link.set_tooltip_text(Some(if app.source == Source::Aur { "Community recipe · review the current PKGBUILD before installing. Click to open its AUR page." } else { "Official Arch package · click for package details." }));
                link.upcast()
            };
            source.set_halign(gtk::Align::Start);
            card.append(&table_cell(&source, &self.columns[1], 76));
            let installed = state.ready && state.installed.contains_key(&app.name);
            let action = button(if installed { "Uninstall" } else { "Install" }, !installed);
            action.add_css_class("loadout-package-action");
            if installed {
                action.add_css_class("loadout-uninstall");
            }
            action.set_sensitive(state.ready && (installed || app.source != Source::Foreign));
            action.set_tooltip_text(Some(if installed {
                "Review removal in the embedded terminal"
            } else {
                "Review installation; cached archives may be reused by pacman"
            }));
            drop(state);
            let name = app.name.clone();
            let source = app.source;
            let weak = Rc::downgrade(self);
            action.connect_clicked(move |_| {
                if let Some(v) = weak.upgrade() {
                    if installed { v.remove_review(vec![name.clone()]); }
                    else if let Some(parent) = v.parent.upgrade() {
                        let mut p = Profile::default();
                        p.packages.insert(name.clone(), source);
                        if let Ok(command) = p.command(false) {
                            crate::tools::terminal_review(&parent, "Install package", "Review the package transaction before accepting. Repository installs include a full system upgrade. Pacman reuses suitable cached archives and verifies packages before installation.", &command);
                        }
                    }
                }
            });
            card.append(&table_cell(&action, &self.columns[2], 94));
            let selected = gtk::CheckButton::new();
            selected.set_active(self.effective().packages.contains_key(&app.name));
            selected.update_property(&[gtk::accessible::Property::Label(&format!(
                "Select {}",
                app.title
            ))]);
            let weak = Rc::downgrade(self);
            selected.connect_toggled(move |check| {
                if let Some(v) = weak.upgrade() {
                    let mut profile = v.profile.borrow_mut();
                    if check.is_active() {
                        profile.packages.insert(app.name.clone(), app.source);
                    } else {
                        let state = v.packages.borrow();
                        profile
                            .packages
                            .retain(|name, _| state.installed_variant(name) != app.name);
                    }
                    drop(profile);
                    v.render_summary();
                }
            });
            selected.set_halign(gtk::Align::Center);
            if selected.is_active() {
                card.add_css_class("selected");
            }
            let selected_row = card.downgrade();
            selected.connect_toggled(move |check| {
                if let Some(row) = selected_row.upgrade() {
                    if check.is_active() {
                        row.add_css_class("selected");
                    } else {
                        row.remove_css_class("selected");
                    }
                }
            });
            card.prepend(&table_cell(&selected, &self.columns[3], 24));
            self.library.append(&card);
        }
    }
    fn render_files(self: &Rc<Self>) {
        while let Some(child) = self.files.first_child() {
            self.files.remove(&child);
        }
        let p = self.profile.borrow();
        if p.files.is_empty() {
            self.files.append(&wrapped("No dotfiles selected. Add a starter below, import a saved profile, or capture selected configuration folders from this system.","muted"));
        }
        for (path, text) in &p.files {
            let row = row(10);
            row.add_css_class("loadout-file");
            let check = gtk::CheckButton::with_label(path);
            check.set_hexpand(true);
            if let Some(label) = check.child().and_then(|w| w.downcast::<gtk::Label>().ok()) {
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_max_width_chars(40);
                label.set_tooltip_text(Some(path));
            }
            check.set_active(self.enabled.borrow().contains(path));
            let path = path.clone();
            let weak = Rc::downgrade(self);
            check.connect_toggled(move |c| {
                if let Some(v) = weak.upgrade() {
                    if c.is_active() {
                        v.enabled.borrow_mut().insert(path.clone());
                    } else {
                        v.enabled.borrow_mut().remove(&path);
                    }
                    v.render_summary();
                }
            });
            row.append(&check);
            row.append(&label(&format!("{} B", text.len()), "tiny"));
            self.files.append(&row);
        }
    }
    fn render_summary(self: &Rc<Self>) {
        let p = self.effective();
        let repo = p.packages.values().filter(|s| **s == Source::Repo).count();
        let aur = p.packages.values().filter(|s| **s == Source::Aur).count();
        let foreign = p.packages.len() - repo - aur;
        self.count
            .set_text(&format!("{} selected", p.packages.len()));
        while let Some(child) = self.summary.first_child() {
            self.summary.remove(&child);
        }
        self.count.set_tooltip_text(Some(&format!(
            "{repo} repository · {aur} AUR · {foreign} foreign/local · {} dotfiles",
            p.files.len()
        )));
        self.foreign.set_visible(foreign > 0);
        self.foreign_note.set_visible(foreign > 0);
        let state = self.packages.borrow();
        let installed = p
            .packages
            .keys()
            .filter(|n| state.installed.contains_key(*n))
            .count();
        let updates = p
            .packages
            .keys()
            .filter(|n| state.updates.contains_key(*n))
            .count();
        let catalog = crate::loadout_styles::catalog();
        for (heading, installed_group) in [("Will install", false), ("Already installed", true)] {
            let names: Vec<_> = p
                .packages
                .keys()
                .filter(|n| state.installed.contains_key(*n) == installed_group)
                .collect();
            if names.is_empty() {
                continue;
            }
            self.summary.append(&label(
                &format!(
                    "{} ({})",
                    if !state.ready {
                        "Awaiting scan"
                    } else {
                        heading
                    },
                    names.len()
                ),
                "loadout-group-heading",
            ));
            for name in names.iter().take(20) {
                let line = row(7);
                line.add_css_class("loadout-selected-item");
                if let Some(app) = catalog.iter().find(|app| app.name == ***name) {
                    let icon = crate::app_icons::icon(app);
                    icon.set_size_request(20, 20);
                    line.append(&icon);
                    let title = label(&app.title, "");
                    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    title.set_hexpand(true);
                    line.append(&title);
                } else {
                    let title = label(name, "");
                    title.set_hexpand(true);
                    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    line.append(&title);
                }
                if installed_group && self.skip.is_active() {
                    line.append(&label("Skipped", "scan-installed"));
                }
                let remove = gtk::Button::from_icon_name("window-close-symbolic");
                remove.add_css_class("loadout-clear");
                remove.set_tooltip_text(Some("Remove from selection"));
                let name = (*name).clone();
                let weak = Rc::downgrade(self);
                remove.connect_clicked(move |_| {
                    if let Some(view) = weak.upgrade() {
                        view.profile.borrow_mut().packages.remove(&name);
                        view.render_packages();
                        view.render_summary();
                    }
                });
                line.append(&remove);
                self.summary.append(&line);
            }
            if names.len() > 20 {
                self.summary
                    .append(&label(&format!("+ {} more", names.len() - 20), "tiny"));
            }
        }
        if p.packages.is_empty() {
            self.summary.append(&wrapped(
                "Select packages from the library to build your loadout.",
                "muted",
            ));
        }
        if updates > 0 {
            self.summary.append(&label(
                &format!("{updates} selected package updates available"),
                "scan-updates",
            ));
        }
        if !p.files.is_empty() {
            self.summary.append(&label(
                &format!("{} selected dotfiles", p.files.len()),
                "tiny",
            ));
        }
        self.uninstall
            .set_label(&format!("Uninstall selected ({installed})"));
        self.uninstall.set_sensitive(state.ready && installed > 0);
        self.update.set_sensitive(state.ready);
        self.update.set_label(&if state.updates_checked {
            format!("Update system ({})", state.updates.len())
        } else {
            "Update system…".into()
        });
        self.update.set_tooltip_text(Some("Full repository upgrade. Includes packages outside this selection; AUR updates are handled separately."));
        self.command_hint.set_text(if self.skip.is_active() {
            "Missing packages only · installed packages skipped"
        } else {
            "Full selection · pacman skips current versions"
        });
        self.install.set_label(&format!(
            "Review & install {} ({})",
            if self.skip.is_active() {
                "missing"
            } else {
                "selected"
            },
            if self.skip.is_active() {
                p.packages.len() - installed
            } else {
                p.packages.len()
            }
        ));
        let ready = state.ready;
        drop(state);
        match self.install_profile().command(self.foreign.is_active()) {
            Ok(command) => {
                self.command.buffer().set_text(&command);
                self.install.set_sensitive(ready);
            }
            Err(error) => {
                self.command.buffer().set_text(&error);
                self.install.set_sensitive(false);
            }
        }
        self.export
            .set_sensitive(p.command(self.foreign.is_active()).is_ok());
        if !ready {
            self.command
                .buffer()
                .set_text("Waiting for a successful package scan. Use Check for updates to retry.");
        } else if self.skip.is_active() && installed > 0 && installed == p.packages.len() {
            self.command.buffer().set_text("All selected packages are installed. Use Update system to upgrade, or save this selection for another installation.");
        }
        self.apply.set_sensitive(!p.files.is_empty());
    }
    fn import(self: &Rc<Self>) {
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let chooser = gtk::FileDialog::builder()
                .title("Import an Arch Forge loadout")
                .build();
            if let Ok(file) = chooser.open_future(Some(&parent)).await {
                let Some(path) = file.path() else { return };
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(Profile::read(&path));
                });
                glib::timeout_add_local(Duration::from_millis(100), move || {
                    let result = match rx.try_recv() {
                        Ok(r) => r,
                        Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                        Err(_) => Err("Import worker stopped".into()),
                    };
                    if let Some(v) = weak.upgrade() {
                        match result {
                            Ok(profile) => {
                                v.replace(profile);
                                v.status.set_text(
                                    "Profile imported. Nothing has been installed or applied.",
                                );
                            }
                            Err(error) => v.status.set_text(&format!("Could not import: {error}")),
                        }
                    }
                    glib::ControlFlow::Break
                });
            }
        });
    }
    fn install_review(self: &Rc<Self>) {
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let profile = self.install_profile();
        let foreign = self.foreign.is_active();
        let command = match profile.command(foreign) {
            Ok(c) => c,
            Err(e) => {
                self.status.set_text(&e);
                return;
            }
        };
        let recorded = if profile.repositories.is_empty() {
            "No repository list was recorded.".into()
        } else {
            format!(
                "Repositories recorded in this profile: {}.",
                profile.repositories.join(", ")
            )
        };
        crate::tools::terminal_review(
            &parent,
            "Install selected packages",
            &format!(
                "Repository packages use a full system upgrade. AUR packages use paru or yay as your user. Custom repositories and multilib must already be configured where needed. {recorded} Review the transaction and AUR builds. Dotfiles are applied separately."
            ),
            &command,
        );
    }
}
fn removal_review(parent: &gtk::ApplicationWindow, names: Vec<String>, command: String) {
    let (window, surface) = Surface::new(
        parent,
        "Uninstall packages",
        "Check dependencies before reviewing removal. Pacman asks for confirmation in the embedded terminal.",
        "\u{e872}",
        "edit-delete-symbolic",
    );
    surface.status.set(
        Tone::Busy,
        "Checking package dependencies…",
        "This preview does not remove packages or request administrator access.",
        "No changes made",
    );
    let summary = row(16);
    summary.add_css_class("removal-summary");
    let stats = [
        label(&names.len().to_string(), "row-title"),
        label("Checking…", "row-title"),
    ];
    for (caption, value) in ["Selected packages", "Blocking apps"]
        .into_iter()
        .zip(stats.iter())
    {
        let cell = column(5);
        cell.set_hexpand(true);
        cell.append(&label(caption, "tiny"));
        cell.append(value);
        summary.append(&cell);
    }
    surface.content.append(&summary);
    let blockers = column(6);
    let blocker_scroll = gtk::ScrolledWindow::builder()
        .child(&blockers)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(70)
        .max_content_height(180)
        .propagate_natural_height(true)
        .build();
    blocker_scroll.set_visible(false);
    surface.content.append(&blocker_scroll);
    let args = crate::loadout_state::removal_preview_args(&names).expect("Validated package names");
    let details = surface.details(
        "Removal preview",
        "Read-only dependency check · full output",
        Some(&format!("/usr/bin/pacman {}", args.join(" "))),
    );
    details
        .output
        .buffer()
        .set_text("Checking the exact selected packages against pacman's installed database…");
    let close = surface.close_button(&window, "Cancel", false);
    let open = button("Open embedded terminal", true);
    open.set_visible(false);
    surface.actions.append(&open);
    let allowed = Rc::new(Cell::new(false));
    let gate = allowed.clone();
    let weak = window.downgrade();
    let status = surface.status.clone();
    let proposed = command.clone();
    open.connect_clicked(move |b| {
        if gate.get()
            && let Some(w) = weak.upgrade()
        {
            crate::tools::launch_terminal(&w, proposed.clone(), status.clone(), b);
        }
    });
    let copy = button("Copy command", false);
    copy.set_visible(false);
    details.tools.append(&copy);
    let proposed = command.clone();
    copy.connect_clicked(move |_| crate::copy_text(&proposed));
    let weak = window.downgrade();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::loadout_state::preview_removal(&names));
    });
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let Some(w) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if !w.is_visible() {
            return glib::ControlFlow::Break;
        }
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("The dependency-check worker stopped. Please retry.".into()),
        };
        match result {
            Ok(plan) if plan.blockers.is_empty() => {
                stats[1].set_text("None");
                surface.status.set(Tone::Ready, "Ready to review removal", "Open the terminal to review the command and choose Authenticate & run. Pacman checks dependencies again and asks for confirmation.", "Nothing removed yet");
                details.title.set_text("Removal preview & command");
                details
                    .subtitle
                    .set_text("Preview passed · terminal confirmation required");
                details.output.buffer().set_text(&format!("$ {command}\n\nSelected packages confirmed by pacman:\n{}\nPacman will check dependencies again when you run this command.", plan.output));
                copy.set_visible(true);
                open.set_visible(true);
                allowed.set(true);
            }
            Ok(plan) => {
                let dependents: BTreeSet<_> = plan
                    .blockers
                    .iter()
                    .map(|b| b.required_by.as_str())
                    .collect();
                stats[1].set_text(&dependents.len().to_string());
                surface.status.set(Tone::Warning, "Other apps still need these packages", "These apps need the selected packages. Keep them installed, or explicitly select the apps too before reviewing removal.", "Removal blocked · nothing changed");
                blocker_scroll.set_visible(true);
                blockers.append(&label("Required by", "row-title"));
                for blocker in plan.blockers.iter().take(8) {
                    let card = column(2);
                    card.add_css_class("removal-blocker");
                    card.append(&label(&blocker.required_by, "row-title"));
                    card.append(&wrapped(
                        &format!("Requires {}", blocker.dependency),
                        "tiny",
                    ));
                    blockers.append(&card);
                }
                if plan.blockers.len() > 8 {
                    blockers.append(&wrapped(
                        "More dependencies are listed in the output.",
                        "tiny",
                    ));
                }
                details.output.buffer().set_text(&plan.output);
                close.set_label("Keep packages");
            }
            Err(error) => {
                stats[1].set_text("Unknown");
                surface.status.set(Tone::Error, "Could not verify removal", "Nothing was removed. Review the output, resolve the package database or selection issue, then retry.", "No changes made");
                details.output.buffer().set_text(&error);
                close.set_label("Done");
            }
        }
        glib::ControlFlow::Break
    });
    window.present();
}
fn apply_review(parent: &gtk::ApplicationWindow, profile: Profile) {
    let (window, surface) = Surface::new(
        parent,
        "Review dotfiles",
        "Preview selected configuration files before replacing anything.",
        "\u{e873}",
        "document-properties-symbolic",
    );
    let stats = surface.stats(
        (
            "Selected files",
            &profile.files.len().to_string(),
            "text-x-generic-symbolic",
        ),
        ("Changes", "Reading…", "document-edit-symbolic"),
    );
    let details = surface.details(
        "File changes",
        "Existing files are backed up before writing. Symlinked paths are refused.",
        None,
    );
    surface.content.append(&wrapped("Configurations can contain startup commands and private values. Your running desktop may notice file changes automatically. Review imported profiles before applying.","muted"));
    surface.close_button(&window, "Cancel", false);
    let apply = button("Back up & apply", true);
    apply.set_sensitive(false);
    surface.actions.append(&apply);
    let plan = Rc::new(RefCell::new(None));
    let slot = plan.clone();
    let weak = window.downgrade();
    let status = surface.status.clone();
    let buffer = details.output.buffer();
    let commit = apply.clone();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = loadouts::home().and_then(|home| loadouts::preview(&profile, &home));
        let _ = tx.send(result);
    });
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        match rx.try_recv() {
            Ok(Ok(p)) => {
                stats[1].set_text(&p.changes.len().to_string());
                let mut text = String::new();
                for change in &p.changes {
                    text.push_str(&format!(
                        "# {} · {} · permissions {:04o}\n",
                        change.path,
                        if change.previous.is_some() {
                            "replace"
                        } else {
                            "new file"
                        },
                        change.mode
                    ));
                    if let Some(old) = &change.previous {
                        text.push_str("--- Current file ---\n");
                        text.push_str(old);
                        text.push('\n');
                    }
                    text.push_str("+++ Selected file +++\n");
                    text.push_str(&change.text);
                    text.push_str("\n\n");
                }
                if p.changes.is_empty() {
                    text = "All selected files already match. No changes needed.".into();
                }
                buffer.set_text(&text);
                status.set(Tone::Ready,"Review the exact file changes","Only checked dotfiles are included. A backup of replaced files is written before applying.","Nothing has changed");
                commit.set_sensitive(!p.changes.is_empty());
                *slot.borrow_mut() = Some(p);
            }
            Ok(Err(e)) => {
                buffer.set_text(&e);
                status.set(
                    Tone::Error,
                    "Could not prepare these files",
                    &e,
                    "Nothing has changed",
                );
            }
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => status.set(
                Tone::Error,
                "Preview worker stopped",
                "Please try again.",
                "Nothing has changed",
            ),
        };
        glib::ControlFlow::Break
    });
    let status = surface.status.clone();
    let buffer = details.output.buffer();
    apply.connect_clicked(move|b|{let Some(plan)=plan.borrow_mut().take()else{return};b.set_sensitive(false);status.set(Tone::Busy,"Backing up and applying…","Keep the backup to recover replaced configuration files.","Writing reviewed files");let (tx,rx)=mpsc::channel();std::thread::spawn(move||{let result=loadouts::state_home().and_then(|state|loadouts::apply(plan,&state));let _=tx.send(result);});let status=status.clone();let buffer=buffer.clone();glib::timeout_add_local(Duration::from_millis(100),move||{match rx.try_recv(){Ok(Ok(message))=>{buffer.set_text(&message);status.set(Tone::Success,"Dotfiles applied","The output contains the backup location. No desktop reload command was sent.","Completed");},Ok(Err(error))=>{buffer.set_text(&error);status.set(Tone::Error,"Dotfile application stopped","Review the output for completed files and the backup path.","Review output");},Err(mpsc::TryRecvError::Empty)=>return glib::ControlFlow::Continue,Err(_)=>status.set(Tone::Error,"The apply worker stopped","Review your configuration before retrying.","Review output")};glib::ControlFlow::Break});});
    window.present();
}
fn capture_dialog(parent: &gtk::ApplicationWindow, loadout: std::rc::Weak<View>) {
    let (window, surface) = Surface::new(
        parent,
        "Capture this system",
        "Save explicitly installed packages and opted-in text configuration files.",
        "\u{e161}",
        "document-save-symbolic",
    );
    surface.status.set(Tone::Ready,"Choose what to include","Packages are captured as names, not a disk image. Keep the exported file outside this installation.","No files saved");
    let stats = surface.stats(
        ("Packages", "—", "package-x-generic-symbolic"),
        ("Dotfiles", "—", "text-x-generic-symbolic"),
    );
    surface.content.append(&wrapped("Dotfiles can contain private values. Symlinks, binary files, secret-named files, and cache folders are omitted. Review the snapshot before saving or sharing.","muted"));
    let details = surface.details(
        "Snapshot preview",
        "Foreign packages remain unverified; they are not silently assumed to be AUR packages.",
        None,
    );
    details.form.set_visible(true);
    let roots = column(4);
    let mut checks = Vec::new();
    for (name, _, _) in loadouts::CONFIGS {
        let check = gtk::CheckButton::with_label(name);
        roots.append(&check);
        checks.push(check);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .child(&roots)
        .min_content_height(85)
        .max_content_height(110)
        .propagate_natural_height(true)
        .build();
    details.form.append(&wrapped(
        "Optional dotfiles: select only the folders you want to save.",
        "tiny",
    ));
    details.form.append(&scroll);
    let name = gtk::Entry::builder()
        .text("My Arch loadout")
        .max_length(100)
        .build();
    details.form.append(&name);
    let capture = button("Preview snapshot", false);
    details.tools.append(&capture);
    let use_selection = button("Use selection", false);
    use_selection.set_sensitive(false);
    details.tools.append(&use_selection);
    surface.close_button(&window, "Close", false);
    let save = button("Save profile…", true);
    save.set_sensitive(false);
    surface.actions.append(&save);
    let ready = Rc::new(RefCell::new(None::<Profile>));
    for check in &checks {
        let ready = ready.clone();
        let save = save.clone();
        let use_selection = use_selection.clone();
        check.connect_toggled(move |_| {
            ready.borrow_mut().take();
            save.set_sensitive(false);
            use_selection.set_sensitive(false);
        });
    }
    let ready2 = ready.clone();
    let save2 = save.clone();
    let use2 = use_selection.clone();
    name.connect_changed(move |_| {
        ready2.borrow_mut().take();
        save2.set_sensitive(false);
        use2.set_sensitive(false);
    });
    let weak = window.downgrade();
    let state = surface.status.clone();
    let buffer = details.output.buffer();
    let ready2 = ready.clone();
    let name2 = name.clone();
    let save2 = save.clone();
    let use2 = use_selection.clone();
    capture.connect_clicked(move|b|{let ids:Vec<_>=checks.iter().enumerate().filter_map(|(i,c)|c.is_active().then_some(i)).collect();let profile_name=name2.text().to_string();b.set_sensitive(false);save2.set_sensitive(false);use2.set_sensitive(false);for c in &checks{c.set_sensitive(false);}name2.set_sensitive(false);state.set(Tone::Busy,"Capturing your selection…","Reading package names and only the opted-in configuration folders.","Read-only snapshot");let (tx,rx)=mpsc::channel();std::thread::spawn(move||{let result=(||{let mut p=loadouts::capture_packages()?;p.name=profile_name;let skipped=if ids.is_empty(){Vec::new()}else{loadouts::capture_files(&mut p,&loadouts::home()?,&ids)?};p.validate()?;Ok::<_,String>((p,skipped))})();let _=tx.send(result);});let controls=(b.clone(),save2.clone(),use2.clone(),name2.clone(),checks.clone());let status=state.clone();let buffer=buffer.clone();let ready=ready2.clone();let stats=stats.values.clone();let weak=weak.clone();glib::timeout_add_local(Duration::from_millis(100),move||{if weak.upgrade().is_none(){return glib::ControlFlow::Break;}let result=match rx.try_recv(){Ok(r)=>r,Err(mpsc::TryRecvError::Empty)=>return glib::ControlFlow::Continue,Err(_)=>Err("Snapshot worker stopped".into())};controls.0.set_sensitive(true);controls.3.set_sensitive(true);for c in &controls.4{c.set_sensitive(true);}match result{Ok((p,skipped))=>{stats[0].set_text(&p.packages.len().to_string());stats[1].set_text(&p.files.len().to_string());buffer.set_text(&format!("{}\n\nOmitted files:\n{}",p.encode().unwrap_or_default(),skipped.join("\n")));status.set(Tone::Ready,"Your snapshot is ready to review","Save the profile to another drive or your own backup storage. Nothing has been uploaded.","Review private values");*ready.borrow_mut()=Some(p);controls.1.set_sensitive(true);controls.2.set_sensitive(true);},Err(e)=>{buffer.set_text(&e);status.set(Tone::Error,"Could not capture this selection",&e,"No file saved");}}glib::ControlFlow::Break});});
    let weak = window.downgrade();
    let ready2 = ready.clone();
    let note = label("", "tiny");
    surface.content.append(&note);
    save.connect_clicked(move |_| {
        if let (Some(window), Some(p)) = (weak.upgrade(), ready2.borrow().as_ref()) {
            match p.encode() {
                Ok(text) => save_text(&window, "archforge-loadout.json", text, &note),
                Err(e) => note.set_text(&e),
            }
        }
    });
    use_selection.connect_clicked(move |_| {
        if let (Some(v), Some(profile)) = (loadout.upgrade(), ready.borrow().as_ref()) {
            v.replace(profile.clone());
            v.status.set_text(
                "Captured selection loaded. Packages and dotfiles have not been applied.",
            );
        }
    });
    window.present();
}

fn style_preview(amber: bool) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(92);
    area.set_hexpand(true);
    area.set_draw_func(move |_, c, w, h| {
        let w = w as f64;
        let h = h as f64;
        c.set_source_rgb(0.078, 0.09, 0.10);
        let _ = c.paint();
        c.set_source_rgb(0.14, 0.16, 0.18);
        c.rectangle(8., 8., w - 16., 15.);
        let _ = c.fill();
        let accent = if amber {
            (1., 0.70, 0.33)
        } else {
            (1., 0.39, 0.40)
        };
        c.set_source_rgb(accent.0, accent.1, accent.2);
        c.rectangle(14., 13., 35., 4.);
        let _ = c.fill();
        for (x, y, width, height) in [
            (12., 31., w * 0.54 - 18., h - 43.),
            (w * 0.54, 31., w * 0.46 - 12., h - 43.),
        ] {
            c.set_source_rgb(0.12, 0.14, 0.16);
            c.rectangle(x, y, width, height);
            let _ = c.fill();
            c.set_source_rgb(accent.0, accent.1, accent.2);
            c.rectangle(x, y, width, 2.);
            let _ = c.fill();
            for i in 0..3 {
                c.set_source_rgba(0.68, 0.72, 0.75, 0.3);
                c.rectangle(
                    x + 9.,
                    y + 12. + i as f64 * 10.,
                    width * (0.6 - i as f64 * 0.13),
                    3.,
                );
                let _ = c.fill();
            }
        }
    });
    area
}
fn table_cell(child: &impl IsA<gtk::Widget>, group: &gtk::SizeGroup, width: i32) -> gtk::Box {
    let cell = column(0);
    cell.set_width_request(width);
    cell.set_valign(gtk::Align::Center);
    cell.add_css_class("loadout-table-cell");
    group.add_widget(&cell);
    cell.append(child);
    cell
}
fn create(parent: &gtk::ApplicationWindow) -> (gtk::Box, Rc<View>) {
    let page = column(14);
    page.add_css_class("content");
    page.add_css_class("loadouts-page");
    page.set_vexpand(true);
    let top = row(16);
    top.add_css_class("page-header");
    let heading = column(3);
    heading.set_hexpand(true);
    heading.append(&label("Workspace / Loadouts", "eyebrow"));
    heading.append(&label("Build your Arch loadout.", "page-title"));
    heading.append(&wrapped(
        "Pick your packages, save your configuration, and bring a setup to your next installation.",
        "subtitle",
    ));
    top.append(&heading);
    let import = button("Import profile", false);
    let capture = button("Capture this system", true);
    top.append(&import);
    top.append(&capture);
    page.append(&top);
    let workspace_tabs = gtk::Stack::new();
    workspace_tabs.set_vhomogeneous(false);
    workspace_tabs.set_vexpand(true);
    let workspace_switcher = gtk::StackSwitcher::new();
    workspace_switcher.set_stack(Some(&workspace_tabs));
    workspace_switcher.set_visible(false);
    page.append(&workspace_switcher);
    let packages_workspace = column(14);
    packages_workspace.set_vexpand(true);
    let profile_bar = row(12);
    profile_bar.add_css_class("loadout-profile");
    profile_bar.append(&crate::tool_icons::icon(
        crate::tool_icons::Glyph::Package,
        26,
    ));
    let profile_name = gtk::Entry::builder()
        .text("My Arch loadout")
        .max_length(100)
        .hexpand(true)
        .build();
    profile_name.update_property(&[gtk::accessible::Property::Label("Loadout profile name")]);
    profile_bar.append(&profile_name);
    let save = button("Save selection", false);
    profile_bar.append(&save);
    packages_workspace.append(&profile_bar);
    let body = row(14);
    body.add_css_class("loadout-columns");
    body.set_vexpand(true);
    let categories = column(3);
    categories.add_css_class("loadout-panel");
    categories.set_width_request(160);
    categories.set_valign(gtk::Align::Start);
    categories.append(&label("Categories", "section-title"));
    let category_picker = gtk::DropDown::from_strings(loadouts::CATEGORIES);
    category_picker.add_css_class("loadout-category-picker");
    category_picker.update_property(&[gtk::accessible::Property::Label("Choose app category")]);
    category_picker.set_visible(false);
    categories.append(&category_picker);
    let category_list = column(3);
    let mut category_buttons = Vec::new();
    for category in loadouts::CATEGORIES {
        let b = button(category, false);
        b.add_css_class("loadout-category");
        let content = row(9);
        let icon_name = match *category {
            "Essentials" => "package-x-generic-symbolic",
            "Browsers" => "web-browser-symbolic",
            "Development" => "utilities-terminal-symbolic",
            "Gaming" => "applications-games-symbolic",
            "Media" => "video-display-symbolic",
            "Communication" => "user-available-symbolic",
            "System utilities" => "applications-system-symbolic",
            "Desktop & styling" => "preferences-desktop-theme-symbolic",
            _ => "view-grid-symbolic",
        };
        let icon = gtk::Image::from_icon_name(icon_name);
        icon.set_pixel_size(18);
        content.append(&icon);
        content.append(&label(category, ""));
        b.set_child(Some(&content));
        category_list.append(&b);
        category_buttons.push(b);
    }
    let category_scroll = gtk::ScrolledWindow::builder()
        .child(&category_list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(220)
        .max_content_height(440)
        .propagate_natural_height(true)
        .build();
    category_scroll.add_css_class("loadout-category-scroll");
    categories.append(&category_scroll);
    body.append(&categories);
    let center = column(10);
    center.set_hexpand(true);
    center.add_css_class("loadout-panel");
    let package_mode = row(6);
    package_mode.add_css_class("loadout-mode-tabs");
    let build_mode = gtk::ToggleButton::with_label("Build loadout");
    build_mode.set_active(true);
    let manage_mode = gtk::ToggleButton::with_label("Manage installed");
    manage_mode.set_tooltip_text(Some("Show installed packages in this library. Use Packages for the complete system package database."));
    manage_mode.set_group(Some(&build_mode));
    package_mode.append(&build_mode);
    package_mode.append(&manage_mode);
    center.append(&package_mode);
    let packages = column(8);
    let scan_bar = row(8);
    scan_bar.add_css_class("loadout-scan-toolbar");
    let scan_strip = row(12);
    scan_strip.add_css_class("loadout-scan-strip");
    scan_strip.set_hexpand(true);
    let scan_status = label("◌ Scanning…", "loadout-scan-status");
    scan_status.set_hexpand(false);
    scan_status.set_wrap(false);
    scan_status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    scan_status.set_max_width_chars(18);
    let refresh = button("Check for updates", false);
    refresh.add_css_class("loadout-refresh");
    let refresh_contents = row(7);
    let refresh_icon = gtk::Image::from_icon_name("view-refresh-symbolic");
    refresh_icon.set_pixel_size(16);
    refresh_contents.append(&refresh_icon);
    refresh_contents.append(&label("Check for updates", ""));
    refresh.set_child(Some(&refresh_contents));
    refresh.update_property(&[gtk::accessible::Property::Label("Check for updates")]);
    refresh.set_tooltip_text(Some(
        "Refresh repository update information using checkupdates. AUR packages are not checked.",
    ));
    scan_strip.append(&scan_status);
    let scan_indicators = row(12);
    scan_strip.append(&scan_indicators);
    scan_bar.append(&scan_strip);
    scan_bar.append(&refresh);
    packages.append(&scan_bar);
    let filters = row(8);
    filters.add_css_class("loadout-filters");
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search apps and package names…")
        .hexpand(true)
        .build();
    search.set_search_delay(0);
    search.update_property(&[gtk::accessible::Property::Label("Search loadout packages")]);
    filters.append(&search);
    let sources = gtk::DropDown::from_strings(&["All sources", "Repository", "AUR", "Foreign"]);
    sources.update_property(&[gtk::accessible::Property::Label("Filter package source")]);
    filters.append(&sources);
    let statuses = gtk::DropDown::from_strings(&[
        "All statuses",
        "Installed",
        "Not installed",
        "Cached",
        "Update available",
    ]);
    statuses.update_property(&[gtk::accessible::Property::Label(
        "Filter installation status",
    )]);
    filters.append(&statuses);
    let status_picker = statuses.clone();
    manage_mode.connect_toggled(move |mode| {
        if mode.is_active() {
            status_picker.set_selected(1);
        }
    });
    let status_picker = statuses.clone();
    build_mode.connect_toggled(move |mode| {
        if mode.is_active() {
            status_picker.set_selected(0);
        }
    });
    let sort = gtk::DropDown::from_strings(&[
        "Recommended",
        "Popular first",
        "Name A–Z",
        "Name Z–A",
        "Installed first",
    ]);
    sort.update_property(&[gtk::accessible::Property::Label("Sort apps")]);
    sort.set_tooltip_text(Some("Recommended: curated common choices, then October 2026 pkgstats usage. Popular first: voluntary survey percentages, not a safety rating. Installed first uses this system's package database."));
    filters.append(&sort);
    packages.append(&filters);
    let columns = std::array::from_fn(|_| gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal));
    let header = row(10);
    header.add_css_class("loadout-table-header");
    let name_heading = label("Package", "");
    name_heading.set_hexpand(true);
    name_heading.set_margin_start(38);
    let selection_heading = label("", "");
    header.append(&table_cell(&selection_heading, &columns[3], 24));
    header.append(&name_heading);
    for (i, (title, width)) in [("Status", 150), ("Source", 76), ("Actions", 94), ("✓", 24)]
        .into_iter()
        .enumerate()
    {
        let l = label(title, "");
        if i == 3 {
            l.set_xalign(0.5);
        }
        if i != 3 {
            header.append(&table_cell(&l, &columns[i], width));
        }
    }
    packages.append(&header);
    let library = column(0);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&library)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(190)
        .vexpand(true)
        .build();
    packages.append(&scroll);
    let pager = row(8);
    let results = label("", "tiny");
    results.set_hexpand(true);
    pager.append(&results);
    let select_matches = button("Select matches", false);
    pager.append(&select_matches);
    let previous = button("Previous", false);
    let next = button("Next", false);
    pager.append(&previous);
    pager.append(&next);
    packages.append(&pager);
    let custom = row(8);
    let custom_name = gtk::Entry::builder()
        .placeholder_text("Add an exact package name")
        .hexpand(true)
        .build();
    custom_name.update_property(&[gtk::accessible::Property::Label("Custom package name")]);
    custom.append(&custom_name);
    let custom_source = gtk::DropDown::from_strings(&["Repository", "AUR", "Foreign"]);
    custom_source.update_property(&[gtk::accessible::Property::Label("Custom package source")]);
    custom.append(&custom_source);
    let add = button("Add", false);
    custom.append(&add);
    let custom_more = gtk::Expander::builder()
        .label("Add a custom package")
        .child(&custom)
        .build();
    custom_more.add_css_class("loadout-custom-package");
    packages.append(&custom_more);
    center.append(&packages);
    let styling = column(10);
    styling.append(&label("Desktop kits & starter dotfiles", "section-title"));
    let kit_row = row(8);
    let names: Vec<_> = crate::loadout_styles::KITS.iter().map(|k| k.name).collect();
    let kit = gtk::DropDown::from_strings(&names);
    kit.set_hexpand(true);
    kit.update_property(&[gtk::accessible::Property::Label("Desktop kit")]);
    let colors = gtk::DropDown::from_strings(&["Amber Night", "Crimson Night"]);
    colors.update_property(&[gtk::accessible::Property::Label("Starter colors")]);
    kit_row.append(&kit);
    kit_row.append(&colors);
    styling.append(&kit_row);
    let preview_box = column(4);
    preview_box.append(&style_preview(true));
    styling.append(&preview_box);
    let description = wrapped(crate::loadout_styles::KITS[0].description, "muted");
    styling.append(&description);
    let kit_buttons = row(8);
    let add_kit = button("Add desktop packages", false);
    let add_files = button("Use starter dotfiles", false);
    kit_buttons.append(&add_kit);
    kit_buttons.append(&add_files);
    styling.append(&kit_buttons);
    styling.append(&wrapped("Hyprland and Sway include compositor and bar starters; Plasma includes color schemes. Other kits add desktop packages and shared app colors. Display managers and services are not enabled automatically.","tiny"));
    let files = column(2);
    let file_scroll = gtk::ScrolledWindow::builder()
        .child(&files)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(100)
        .max_content_height(200)
        .propagate_natural_height(true)
        .vexpand(true)
        .build();
    styling.append(&label("Checked dotfiles", "row-title"));
    styling.append(&file_scroll);
    let ricing_page = crate::ricing_ui::page(parent, styling.upcast());
    body.append(&center);
    let side = column(8);
    side.add_css_class("loadout-panel");
    side.add_css_class("loadout-selection-panel");
    side.set_width_request(236);
    side.set_hexpand(false);
    side.set_valign(gtk::Align::Start);
    let selection_heading = row(10);
    let selection_title = label("Your loadout", "section-title");
    selection_title.set_hexpand(true);
    selection_heading.append(&selection_title);
    let clear = gtk::Button::from_icon_name("edit-clear-symbolic");
    clear.set_tooltip_text(Some("Clear packages"));
    clear.update_property(&[gtk::accessible::Property::Label("Clear packages")]);
    clear.add_css_class("loadout-clear");
    selection_heading.append(&clear);
    side.append(&selection_heading);
    let count = label("", "muted");
    count.add_css_class("loadout-selected-count");
    side.append(&count);
    let summary = column(2);
    let summary_scroll = gtk::ScrolledWindow::builder()
        .child(&summary)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(50)
        .max_content_height(220)
        .propagate_natural_height(true)
        .build();
    side.append(&summary_scroll);
    let foreign = gtk::CheckButton::new();
    let foreign_label = wrapped("Try selected foreign packages through AUR", "tiny");
    foreign_label.set_max_width_chars(28);
    foreign.set_child(Some(&foreign_label));
    side.append(&foreign);
    let foreign_note = wrapped(
        "Foreign packages may be local builds, not AUR packages. Only enable this after checking their origin.",
        "tiny",
    );
    foreign_note.set_max_width_chars(30);
    side.append(&foreign_note);
    let skip = gtk::CheckButton::with_label("Skip installed packages");
    skip.set_active(true);
    side.append(&skip);
    let cache_note = wrapped("Reuse cached downloads when available.", "tiny");
    cache_note.set_max_width_chars(30);
    side.append(&cache_note);
    let install = button("Install missing", true);
    side.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    side.append(&label("Installation plan", "row-title"));
    side.append(&label("1   Update repository packages", "tiny"));
    side.append(&label("2   Install your selected apps", "tiny"));
    side.append(&install);
    let update = button("Update system", false);
    let installed_actions = column(6);
    installed_actions.append(&update);
    let uninstall = button("Uninstall selected (0)", false);
    uninstall.add_css_class("destructive-action");
    installed_actions.append(&uninstall);
    let export = button("Export install script", false);
    export.set_tooltip_text(Some("Package-only script. Run it with bash as your regular user; dotfiles are applied separately."));
    let profile_actions = column(8);
    side.append(&export);
    let installed_more = gtk::Expander::builder()
        .label("Installed package actions")
        .child(&installed_actions)
        .build();
    let installed_mode = installed_more.clone();
    manage_mode.connect_toggled(move |mode| installed_mode.set_expanded(mode.is_active()));
    side.append(&installed_more);
    let apply = button("Review dotfiles", false);
    profile_actions.append(&apply);
    let note = wrapped(
        "Install commands prompt in the embedded terminal. Dotfiles are reviewed and applied separately with backups.",
        "tiny",
    );
    note.set_max_width_chars(30);
    profile_actions.append(&note);
    let profile_more = gtk::Expander::builder()
        .label("Profile & dotfile actions")
        .child(&profile_actions)
        .build();
    profile_more.add_css_class("loadout-profile-actions");
    side.append(&profile_more);
    let side_scroll = gtk::ScrolledWindow::builder()
        .child(&side)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_width(236)
        .max_content_width(236)
        .propagate_natural_width(true)
        .min_content_height(300)
        .max_content_height(650)
        .propagate_natural_height(true)
        .valign(gtk::Align::Start)
        .hexpand(false)
        .build();
    body.append(&side_scroll);
    packages_workspace.append(&body);
    let command_panel = column(5);
    command_panel.add_css_class("loadout-command");
    let command_heading = row(10);
    let l = label("Command preview", "row-title");
    command_heading.append(&l);
    let command_hint = label("", "tiny");
    command_hint.set_hexpand(true);
    command_hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
    command_heading.append(&command_hint);
    let copy = button("Copy command", false);
    command_heading.append(&copy);
    command_panel.append(&command_heading);
    let command = text_view();
    let command_scroll = gtk::ScrolledWindow::builder()
        .child(&command)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(40)
        .max_content_height(54)
        .propagate_natural_height(true)
        .build();
    command_panel.append(&command_scroll);
    packages_workspace.append(&command_panel);
    let status = wrapped(
        "Profiles cover packages and checked dotfiles. Personal data and system setup need their own backup; keep your loadout on another drive.",
        "tiny",
    );
    packages_workspace.append(&status);
    workspace_tabs.add_titled(&packages_workspace, Some("packages"), "Packages");

    page.append(&workspace_tabs);
    let view = Rc::new(View {
        parent: parent.downgrade(),
        profile: RefCell::new(Profile::default()),
        enabled: RefCell::new(BTreeSet::new()),
        category: Cell::new(1),
        query: RefCell::new(String::new()),
        source: Cell::new(0),
        sort: Cell::new(0),
        category_picker: category_picker.clone(),
        category_scroll,
        revealed_category: Cell::new(usize::MAX),
        page: Cell::new(0),
        library,
        files,
        summary,
        count,
        profile_name: profile_name.clone(),
        command,
        status,
        foreign: foreign.clone(),
        foreign_note: foreign_note.clone(),
        results,
        previous: previous.clone(),
        next: next.clone(),
        install: install.clone(),
        export: export.clone(),
        apply: apply.clone(),
        category_buttons,
        library_tab: workspace_tabs.clone(),
        ricing: ricing_page,
        packages: RefCell::new(crate::loadout_state::Snapshot::default()),
        scanning: Cell::new(false),
        status_filter: Cell::new(0),
        scan_status,
        scan_indicators,
        columns,
        command_hint,
        refresh: refresh.clone(),
        skip: skip.clone(),
        uninstall: uninstall.clone(),
        update: update.clone(),
    });
    let weak = Rc::downgrade(&view);
    sort.connect_selected_notify(move |s| {
        if let Some(v) = weak.upgrade() {
            v.sort.set(s.selected());
            v.page.set(0);
            v.render_packages();
        }
    });
    let weak = Rc::downgrade(&view);
    category_picker.connect_selected_notify(move |s| {
        if let Some(v) = weak.upgrade()
            && v.category.get() != s.selected() as usize
        {
            v.category.set(s.selected() as usize);
            v.page.set(0);
            v.render_packages();
        }
    });
    for (i, b) in view.category_buttons.iter().enumerate() {
        let weak = Rc::downgrade(&view);
        b.connect_clicked(move |_| {
            if let Some(v) = weak.upgrade() {
                v.category.set(i);
                v.page.set(0);
                v.library_tab.set_visible_child_name("packages");
                v.render_packages();
            }
        });
    }
    let weak = Rc::downgrade(&view);
    refresh.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            v.refresh_packages(true);
        }
    });
    let weak = Rc::downgrade(&view);
    skip.connect_toggled(move |_| {
        if let Some(v) = weak.upgrade() {
            v.render_summary();
        }
    });
    let weak = Rc::downgrade(&view);
    statuses.connect_selected_notify(move |s| {
        if let Some(v) = weak.upgrade() {
            v.status_filter.set(s.selected());
            v.page.set(0);
            v.render_packages();
        }
    });
    let weak = Rc::downgrade(&view);
    uninstall.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            let names = v
                .effective()
                .packages
                .keys()
                .filter(|name| v.packages.borrow().installed.contains_key(*name))
                .cloned()
                .collect();
            v.remove_review(names);
        }
    });
    let weak = Rc::downgrade(&view);
    update.connect_clicked(move |_| { if let Some(v) = weak.upgrade() && let Some(parent) = v.parent.upgrade() {
        crate::tools::terminal_review(&parent, "Update system", "This performs a full repository upgrade, including packages outside this loadout. Review pacman's transaction before confirming. For AUR packages, use your helper separately.", "sudo pacman -Syu");
    } });
    let weak = Rc::downgrade(&view);
    clear.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            v.profile.borrow_mut().packages.clear();
            v.render_packages();
            v.render_summary();
        }
    });
    let weak = Rc::downgrade(&view);
    select_matches.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            let query = v.query.borrow().to_lowercase();
            let terms: Vec<_> = query.split_whitespace().collect();
            for app in v.apps() {
                if v.matches_status(&app.name)
                    && (v.category.get() == 0
                        || loadouts::CATEGORIES[v.category.get()] == app.category)
                    && (v.source.get() == 0
                        || v.source.get()
                            == match app.source {
                                Source::Repo => 1,
                                Source::Aur => 2,
                                Source::Foreign => 3,
                            })
                    && terms.iter().all(|q| {
                        format!("{} {} {}", app.name, app.title, app.description)
                            .to_lowercase()
                            .contains(q)
                    })
                {
                    v.profile.borrow_mut().packages.insert(app.name, app.source);
                }
            }
            v.render_packages();
            v.render_summary();
        }
    });
    let weak = Rc::downgrade(&view);
    search.connect_search_changed(move |s| {
        if let Some(v) = weak.upgrade() {
            *v.query.borrow_mut() = s.text().to_string();
            if !s.text().is_empty() {
                v.category.set(0);
            }
            v.page.set(0);
            v.render_packages();
        }
    });
    let weak = Rc::downgrade(&view);
    sources.connect_selected_notify(move |s| {
        if let Some(v) = weak.upgrade() {
            v.source.set(s.selected());
            v.page.set(0);
            v.render_packages();
        }
    });
    for (b, delta) in [(previous, -1i32), (next, 1)] {
        let weak = Rc::downgrade(&view);
        b.connect_clicked(move |_| {
            if let Some(v) = weak.upgrade() {
                v.page.set((v.page.get() as i32 + delta).max(0) as usize);
                v.render_packages();
            }
        });
    }
    let weak = Rc::downgrade(&view);
    add.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            let name = custom_name.text().trim().to_string();
            if !loadouts::valid_package(&name) {
                v.status
                    .set_text("Enter a valid package name, not a command or a URL.");
                return;
            }
            let source = match custom_source.selected() {
                1 => Source::Aur,
                2 => Source::Foreign,
                _ => Source::Repo,
            };
            v.profile.borrow_mut().packages.insert(name, source);
            custom_name.set_text("");
            v.render_packages();
            v.render_summary();
        }
    });
    let weak = Rc::downgrade(&view);
    profile_name.connect_changed(move |_| {
        if let Some(v) = weak.upgrade() {
            v.render_summary();
        }
    });
    let weak = Rc::downgrade(&view);
    foreign.connect_toggled(move |_| {
        if let Some(v) = weak.upgrade() {
            v.render_summary();
        }
    });
    let weak = Rc::downgrade(&view);
    import.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            v.import();
        }
    });
    let weak = Rc::downgrade(&view);
    capture.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade()
            && let Some(parent) = v.parent.upgrade()
        {
            capture_dialog(&parent, Rc::downgrade(&v));
        }
    });
    let weak = Rc::downgrade(&view);
    save.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade()
            && let Some(parent) = v.parent.upgrade()
        {
            match v.effective().encode() {
                Ok(text) => save_text(
                    parent.upcast_ref(),
                    "archforge-loadout.json",
                    text,
                    &v.status,
                ),
                Err(error) => v.status.set_text(&error),
            }
        }
    });
    let weak = Rc::downgrade(&view);
    install.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            v.install_review();
        }
    });
    let weak = Rc::downgrade(&view);
    export.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade()
            && let Some(parent) = v.parent.upgrade()
        {
            match v.effective().script(v.foreign.is_active()) {
                Ok(text) => save_text(parent.upcast_ref(), "archforge-install.sh", text, &v.status),
                Err(e) => v.status.set_text(&e),
            }
        }
    });
    let weak = Rc::downgrade(&view);
    copy.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade()
            && let Ok(command) = v.install_profile().command(v.foreign.is_active())
        {
            crate::copy_text(&command);
            v.status
                .set_text("Install command copied. Dotfiles are applied separately.");
        }
    });
    let weak = Rc::downgrade(&view);
    apply.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade()
            && let Some(parent) = v.parent.upgrade()
        {
            apply_review(&parent, v.effective());
        }
    });
    let weak = Rc::downgrade(&view);
    let kit2 = kit.clone();
    add_kit.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade()
            && let Some(kit) = crate::loadout_styles::KITS.get(kit2.selected() as usize)
        {
            for name in kit.packages {
                v.profile
                    .borrow_mut()
                    .packages
                    .insert(name.to_string(), Source::Repo);
            }
            v.render_packages();
            v.render_summary();
            v.status
                .set_text("Desktop packages added. No session or service has been changed.");
        }
    });
    let weak = Rc::downgrade(&view);
    let kit2 = kit.clone();
    let colors2 = colors.clone();
    add_files.connect_clicked(move |_|{if let Some(v)=weak.upgrade(){let preset=crate::loadout_styles::style(Some(kit2.selected() as usize),colors2.selected()==0);v.profile.borrow_mut().packages.extend(preset.packages);v.enabled.borrow_mut().extend(preset.files.keys().cloned());v.profile.borrow_mut().files.extend(preset.files);v.render_packages();v.render_files();v.render_summary();v.status.set_text("Starter files added to the selection. Use Review dotfiles to inspect changes before applying.");}});
    let colors2 = colors.clone();
    let description2 = description.clone();
    kit.connect_selected_notify(move |k| {
        if let Some(kit) = crate::loadout_styles::KITS.get(k.selected() as usize) {
            description2.set_text(kit.description);
        }
        let _ = colors2.selected();
    });
    colors.connect_selected_notify(move |c| {
        while let Some(child) = preview_box.first_child() {
            preview_box.remove(&child);
        }
        preview_box.append(&style_preview(c.selected() == 0));
    });
    view.render_packages();
    view.render_files();
    view.render_summary();
    view.refresh_packages(false);
    let weak = Rc::downgrade(&view);
    glib::timeout_add_local(Duration::from_secs(15), move || {
        let Some(v) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if v.library.is_mapped() {
            v.refresh_packages(false);
        }
        glib::ControlFlow::Continue
    });
    let keep = view.clone();
    page.connect_destroy(move |_| {
        let _ = &keep;
    });
    (page, view)
}
pub(crate) fn page(parent: &gtk::ApplicationWindow, stack: &gtk::Stack) -> gtk::Box {
    let (page, view) = create(parent);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&view.ricing)
        .build();
    stack.add_named(&scroll, Some("ricing"));
    page
}
