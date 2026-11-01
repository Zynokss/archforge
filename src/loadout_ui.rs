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
    fn effective(&self) -> Profile {
        let mut p = self.profile.borrow().clone();
        p.name = self.profile_name.text().to_string();
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
        let existing: BTreeSet<_> = apps.iter().map(|a| a.name.clone()).collect();
        for (name, source) in &self.profile.borrow().packages {
            if !existing.contains(name) {
                apps.push(App {
                    name: name.clone(),
                    title: name.clone(),
                    category: "Imported / custom".into(),
                    description: "From this loadout; verify availability on the target system."
                        .into(),
                    icon: "package-x-generic".into(),
                    source: *source,
                });
            }
        }
        for app in &mut apps {
            if let Some(source) = self.profile.borrow().packages.get(&app.name) {
                app.source = *source;
            }
        }
        apps.sort_by_key(|a| a.title.to_lowercase());
        apps
    }
    fn render_packages(self: &Rc<Self>) {
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
        let query = self.query.borrow().to_lowercase();
        let terms: Vec<_> = query.split_whitespace().collect();
        let filtered: Vec<_> = self
            .apps()
            .into_iter()
            .filter(|a| {
                (self.category.get() == 0
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
            let card = row(12);
            card.add_css_class("loadout-app");
            let theme = gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap());
            let icon: gtk::Widget = if app.icon != "package-x-generic" && theme.has_icon(&app.icon)
            {
                let icon = gtk::Image::from_icon_name(&app.icon);
                icon.set_pixel_size(28);
                icon.upcast()
            } else {
                let icon = crate::tool_icons::icon(crate::tool_icons::Glyph::Package, 26);
                icon.add_css_class("loadout-package-icon");
                icon.upcast()
            };
            card.append(&icon);
            let text = column(4);
            text.set_hexpand(true);
            let title = label(&app.title, "row-title");
            title.set_wrap(true);
            text.append(&title);
            let desc = label(&format!("{} · {}", app.name, app.description), "tiny");
            desc.set_ellipsize(gtk::pango::EllipsizeMode::End);
            desc.set_max_width_chars(60);
            desc.set_tooltip_text(Some(&app.description));
            text.append(&desc);
            card.append(&text);
            card.append(&label(app.source.label(), "badge"));
            let selected = gtk::CheckButton::new();
            selected.set_active(self.profile.borrow().packages.contains_key(&app.name));
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
                        profile.packages.remove(&app.name);
                    }
                    drop(profile);
                    v.render_summary();
                }
            });
            card.append(&selected);
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
    fn render_summary(&self) {
        let p = self.effective();
        let repo = p.packages.values().filter(|s| **s == Source::Repo).count();
        let aur = p.packages.values().filter(|s| **s == Source::Aur).count();
        let foreign = p.packages.len() - repo - aur;
        self.count
            .set_text(&format!("{} selected packages", p.packages.len()));
        while let Some(child) = self.summary.first_child() {
            self.summary.remove(&child);
        }
        for (name, value) in [
            ("Repository packages", repo.to_string()),
            ("AUR packages", aur.to_string()),
            ("Foreign / local builds", foreign.to_string()),
            ("Selected dotfiles", p.files.len().to_string()),
        ] {
            let item = row(10);
            item.add_css_class("loadout-summary-row");
            let name = label(name, "muted");
            name.set_hexpand(true);
            item.append(&name);
            item.append(&label(&value, "row-title"));
            self.summary.append(&item);
        }
        self.foreign.set_visible(foreign > 0);
        self.foreign_note.set_visible(foreign > 0);
        match p.command(self.foreign.is_active()) {
            Ok(command) => {
                self.command.buffer().set_text(&command);
                self.install.set_sensitive(true);
                self.export.set_sensitive(true);
            }
            Err(error) => {
                self.command.buffer().set_text(&error);
                self.install.set_sensitive(false);
                self.export.set_sensitive(false);
            }
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
        let profile = self.effective();
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
    page.append(&profile_bar);
    let body = row(14);
    body.add_css_class("loadout-columns");
    body.set_vexpand(true);
    let categories = column(3);
    categories.add_css_class("loadout-panel");
    categories.set_width_request(180);
    categories.set_valign(gtk::Align::Start);
    categories.append(&label("Categories", "section-title"));
    let mut category_buttons = Vec::new();
    for category in loadouts::CATEGORIES {
        let b = button(category, false);
        b.add_css_class("loadout-category");
        categories.append(&b);
        category_buttons.push(b);
    }
    body.append(&categories);
    let center = column(10);
    center.set_hexpand(true);
    center.add_css_class("loadout-panel");
    let library_tab = gtk::Stack::new();
    library_tab.set_vhomogeneous(false);
    library_tab.set_hhomogeneous(false);
    library_tab.set_vexpand(true);
    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&library_tab));
    center.append(&switcher);
    let packages = column(8);
    let filters = row(8);
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
    packages.append(&filters);
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
    packages.append(&custom);
    library_tab.add_titled(&packages, Some("packages"), "Packages");
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
    library_tab.add_titled(&styling, Some("dotfiles"), "Dotfiles & desktops");
    center.append(&library_tab);
    body.append(&center);
    let side = column(8);
    side.add_css_class("loadout-panel");
    side.set_width_request(265);
    side.set_hexpand(false);
    side.set_valign(gtk::Align::Start);
    let selection_heading = row(10);
    let selection_title = label("Your selection", "section-title");
    selection_title.set_hexpand(true);
    selection_heading.append(&selection_title);
    let clear = gtk::Button::from_icon_name("edit-clear-symbolic");
    clear.set_tooltip_text(Some("Clear packages"));
    clear.update_property(&[gtk::accessible::Property::Label("Clear packages")]);
    clear.add_css_class("loadout-clear");
    selection_heading.append(&clear);
    side.append(&selection_heading);
    let count = label("", "muted");
    side.append(&count);
    let summary = column(2);
    side.append(&summary);
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
    let install = button("Install selected", true);
    side.append(&install);
    let export = button("Export install script", false);
    export.set_tooltip_text(Some("Package-only script. Run it with bash as your regular user; dotfiles are applied separately."));
    side.append(&export);
    let apply = button("Review dotfiles", false);
    side.append(&apply);
    let note = wrapped(
        "Install commands prompt in your terminal. Dotfiles are reviewed and applied separately with backups.",
        "tiny",
    );
    note.set_max_width_chars(30);
    side.append(&note);
    body.append(&side);
    page.append(&body);
    let command_panel = column(5);
    command_panel.add_css_class("loadout-command");
    let command_heading = row(10);
    let l = label("Generated install command", "row-title");
    l.set_hexpand(true);
    command_heading.append(&l);
    let copy = button("Copy command", false);
    command_heading.append(&copy);
    command_panel.append(&command_heading);
    let command = text_view();
    let command_scroll = gtk::ScrolledWindow::builder()
        .child(&command)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(54)
        .max_content_height(90)
        .propagate_natural_height(true)
        .build();
    command_panel.append(&command_scroll);
    page.append(&command_panel);
    let status = wrapped(
        "Profiles cover packages and checked dotfiles. Personal data and system setup need their own backup; keep your loadout on another drive.",
        "tiny",
    );
    page.append(&status);
    let view = Rc::new(View {
        parent: parent.downgrade(),
        profile: RefCell::new(Profile::default()),
        enabled: RefCell::new(BTreeSet::new()),
        category: Cell::new(1),
        query: RefCell::new(String::new()),
        source: Cell::new(0),
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
        library_tab: library_tab.clone(),
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
                if (v.category.get() == 0 || loadouts::CATEGORIES[v.category.get()] == app.category)
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
            && let Ok(command) = v.effective().command(v.foreign.is_active())
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
    let keep = view.clone();
    page.connect_destroy(move |_| {
        let _ = &keep;
    });
    (page, view)
}
pub(crate) fn page(parent: &gtk::ApplicationWindow) -> gtk::Box {
    create(parent).0
}
