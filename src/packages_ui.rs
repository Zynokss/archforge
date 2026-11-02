//! Package workbench. Database inspection stays off GTK's main thread.
use super::*;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    sync::mpsc,
};

#[derive(Clone, Default)]
struct Detail {
    description: String,
    size: u64,
}
#[derive(Default)]
struct Database {
    details: BTreeMap<String, Detail>,
    foreign: Option<BTreeSet<String>>,
    updates: Option<BTreeSet<String>>,
}
fn database() -> Database {
    let mut db = Database::default();
    if let Ok(entries) = std::fs::read_dir("/var/lib/pacman/local") {
        for entry in entries.flatten() {
            if let Ok(text) = std::fs::read_to_string(entry.path().join("desc")) {
                let field = |key: &str| {
                    text.split_once(&format!("%{key}%\n"))
                        .and_then(|(_, s)| s.lines().next())
                        .unwrap_or("")
                        .to_string()
                };
                let name = field("NAME");
                if !name.is_empty() {
                    db.details.insert(
                        name,
                        Detail {
                            description: field("DESC"),
                            size: field("SIZE").parse().unwrap_or(0),
                        },
                    );
                }
            }
        }
    }
    fn names(args: &[&str]) -> Option<BTreeSet<String>> {
        let output = std::process::Command::new("/usr/bin/pacman")
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .ok()?;
        if output.status.success()
            || (output.status.code() == Some(1)
                && output.stdout.is_empty()
                && output.stderr.is_empty())
        {
            Some(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(str::to_owned)
                    .collect(),
            )
        } else {
            None
        }
    }
    db.foreign = names(&["-Qmq"]);
    db.updates = names(&["-Quq"]);
    db
}
// Inherit catalog metadata for reviewed alternatives without changing the
// installed package identity or inferring its installation source.
fn catalog_entry<'a>(catalog: &'a [loadouts::App], name: &str) -> Option<&'a loadouts::App> {
    loadout_styles::catalog_entry(catalog, name)
}
fn category_matches(catalog: &[loadouts::App], name: &str, selected: u32) -> bool {
    if selected == 0 {
        return true;
    }
    let app = catalog_entry(catalog, name);
    if selected as usize == loadouts::CATEGORIES.len() {
        return app.is_none();
    }
    loadouts::CATEGORIES
        .get(selected as usize)
        .is_some_and(|category| app.is_some_and(|a| a.category == *category))
}

fn bytes(size: u64) -> String {
    if size >= 1_073_741_824 {
        format!("{:.1} GB", size as f64 / 1_073_741_824.)
    } else if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.)
    } else {
        format!("{} KB", size / 1024)
    }
}
fn cell(text: &str, class: &str, width: i32) -> gtk::Label {
    let l = label(text, "");
    for c in class.split_whitespace() {
        l.add_css_class(c);
    }
    l.set_valign(gtk::Align::Center);
    l.set_width_request(width);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    l.set_max_width_chars(24);
    l.set_tooltip_text(Some(text));
    l
}
fn review(parent: &gtk::ApplicationWindow, names: Vec<String>, action: u32) {
    let names: Vec<_> = names
        .into_iter()
        .filter(|s| loadouts::valid_package(s))
        .collect();
    if names.is_empty() {
        return;
    }
    let quoted = names
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(" ");
    if action == 1 {
        loadout_ui::removal_review(parent, names, format!("sudo pacman -R -- {quoted}"));
    } else {
        let catalog = loadout_styles::catalog();
        let mut profile = loadouts::Profile {
            name: "Package selection".into(),
            ..Default::default()
        };
        for n in &names {
            profile.packages.insert(
                n.clone(),
                catalog
                    .iter()
                    .find(|a| &a.name == n)
                    .map(|a| a.source)
                    .unwrap_or(loadouts::Source::Repo),
            );
        }
        let Ok(mut command) = profile.command(false) else {
            return;
        };
        if action == 2 {
            command = command.replace(" --needed", "");
        }
        tools::terminal_review(
            parent,
            "Review package installation",
            "Review the package names and pacman's transaction before confirming. This includes a full system upgrade.",
            &command,
        );
    }
}
pub(crate) fn build(parent: &gtk::ApplicationWindow, inv: &Inventory) -> gtk::Box {
    let p = screens::heading(
        parent,
        "Packages",
        "Browse, search, and manage your Arch Linux packages.",
        "Packages",
    );
    p.add_css_class("packages-page");
    let tabs = row(10);
    tabs.add_css_class("package-tabs");
    let mode = Rc::new(Cell::new(0u32));
    let mut tab_buttons = Vec::new();
    for (i, text) in ["Installed", "Browse", "Updates", "Foreign"]
        .iter()
        .enumerate()
    {
        let b = gtk::ToggleButton::with_label(text);
        if let Some(first) = tab_buttons.first() {
            b.set_group(Some(first));
        }
        b.set_active(i == 0);
        tabs.append(&b);
        tab_buttons.push(b);
    }
    p.append(&tabs);
    let main = column(16);
    main.set_hexpand(true);
    let stats = row(12);
    stats.add_css_class("package-stats");
    let mut values = Vec::new();
    for (glyph, caption, value) in [
        (
            "\u{e1a1}",
            "Installed packages",
            inv.packages.len().to_string(),
        ),
        ("\u{e863}", "Updates (local DB)", "…".into()),
        ("\u{e53b}", "Foreign packages", "…".into()),
        ("\u{e1db}", "Installed size", "…".into()),
    ] {
        let card = row(12);
        card.add_css_class("package-stat");
        card.set_hexpand(true);
        card.append(&symbol(glyph, "package-x-generic-symbolic"));
        let text = column(4);
        let v = cell(&value, "package-stat-value", -1);
        text.append(&v);
        let caption = cell(caption, "tiny", -1);
        caption.set_max_width_chars(18);
        text.append(&caption);
        card.append(&text);
        stats.append(&card);
        values.push(v);
    }
    main.append(&stats);
    let table = column(12);
    table.add_css_class("panel");
    table.add_css_class("package-table");
    table.set_vexpand(true);
    let toolbar = row(10);
    toolbar.add_css_class("package-toolbar");
    toolbar.add_css_class("command-toolbar");
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search packages…")
        .hexpand(true)
        .build();
    toolbar.append(&search);
    let sort = gtk::DropDown::from_strings(&["Name A–Z", "Name Z–A", "Largest first"]);
    let filters = row(10);
    filters.append(&sort);
    let source =
        gtk::DropDown::from_strings(&["All sources", "Repository", "AUR catalog", "Foreign"]);
    filters.append(&source);
    let mut category_choices = loadouts::CATEGORIES.to_vec();
    category_choices.push("Uncategorized");
    let categories = gtk::DropDown::from_strings(&category_choices);
    filters.append(&categories);
    toolbar.append(&filters);
    table.append(&toolbar);
    let hint = label("Reading the local package database…", "tiny");
    hint.set_hexpand(true);
    hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let filter_status = row(8);
    filter_status.append(&hint);
    let reset = button("Reset filters", false);
    filter_status.append(&reset);
    table.append(&filter_status);
    let header = row(12);
    header.add_css_class("package-table-header");
    let select_all = gtk::CheckButton::new();
    select_all.set_tooltip_text(Some("Select this page"));
    header.append(&select_all);
    let name = label("Package ↕", "package-name-heading");
    name.set_hexpand(true);
    header.append(&name);
    for (text, width) in [("Version", 145), ("Source", 94), ("Size", 72)] {
        header.append(&cell(text, "tiny", width));
    }
    let action_space = label("", "");
    action_space.set_width_request(28);
    header.append(&action_space);
    table.append(&header);
    let list = column(0);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .min_content_height(340)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    table.append(&scroll);
    let foot = row(10);
    let count = label("", "muted");
    count.set_hexpand(true);
    foot.append(&count);
    let prev = button("‹", false);
    prev.set_tooltip_text(Some("Previous page"));
    let page_label = label("1 / 1", "package-page-number");
    let next = button("›", false);
    next.set_tooltip_text(Some("Next page"));
    foot.append(&prev);
    foot.append(&page_label);
    foot.append(&next);
    let per_page = gtk::DropDown::from_strings(&["25 rows", "50 rows", "100 rows"]);
    per_page.set_selected(1);
    foot.append(&per_page);
    table.append(&foot);
    main.append(&table);
    let aside = column(18);
    aside.add_css_class("package-aside");
    aside.set_width_request(280);
    aside.set_hexpand(false);
    let quick = column(9);
    quick.add_css_class("panel");
    quick.append(&label("ϟ  Quick actions", "section-title"));
    let check = job_button(parent, "Check for updates", Job::Updates, true);
    quick.append(&check);
    let selected_label = label("Select packages to manage", "tiny");
    selected_label.set_hexpand(true);
    let selection_status = row(6);
    selection_status.append(&selected_label);
    let clear = button("×", false);
    clear.set_tooltip_text(Some("Clear package selection"));
    selection_status.append(&clear);
    quick.append(&selection_status);
    let install = button("＋  Install selected", false);
    let remove = button("Remove selected", false);
    let reinstall = button("Reinstall selected", false);
    for b in [&install, &remove, &reinstall] {
        b.set_hexpand(true);
        b.set_sensitive(false);
        quick.append(b);
    }
    aside.append(&quick);
    let info = column(12);
    info.add_css_class("panel");
    info.append(&label("Local package database", "section-title"));
    for (key, value) in [
        ("Database", "/var/lib/pacman/local".to_string()),
        ("Packages", inv.packages.len().to_string()),
        ("Source", "pacman".into()),
    ] {
        let r = row(8);
        let k = label(key, "muted");
        k.set_hexpand(true);
        r.append(&k);
        r.append(&label(&value, "tiny"));
        info.append(&r);
    }
    info.append(&job_button(
        parent,
        "Inspect foreign packages",
        Job::Foreign,
        false,
    ));
    aside.append(&info);
    let tips = column(10);
    tips.add_css_class("panel");
    tips.append(&label("Tips", "section-title"));
    let tip = wrapped(
        "Filter by source, category or name. Select packages across pages to review a bulk action. Foreign packages may be local builds, not AUR packages.",
        "muted",
    );
    tip.set_max_width_chars(30);
    tips.append(&tip);
    aside.append(&tips);
    let layout = row(18);
    layout.add_css_class("split-layout");
    layout.set_vexpand(true);
    layout.append(&main);
    layout.append(&aside);
    p.append(&layout);
    let db = Rc::new(RefCell::new(Database::default()));
    let ready = Rc::new(Cell::new(false));
    let selected = Rc::new(RefCell::new(BTreeSet::<String>::new()));
    let page = Rc::new(Cell::new(0usize));
    let visible = Rc::new(RefCell::new(Vec::<String>::new()));
    let syncing = Rc::new(Cell::new(false));
    let catalog = Rc::new(loadout_styles::catalog());
    let installed = Rc::new(
        inv.packages
            .iter()
            .map(|p| (p.name.clone(), p.version.clone()))
            .collect::<BTreeMap<_, _>>(),
    );
    let update_selection: Rc<dyn Fn()> = Rc::new({
        let selected = selected.clone();
        let installed = installed.clone();
        let install = install.clone();
        let remove = remove.clone();
        let reinstall = reinstall.clone();
        let db = db.clone();
        let visible = visible.clone();
        let select_all = select_all.downgrade();
        let syncing = syncing.clone();
        move || {
            let s = selected.borrow();
            if let Some(check) = select_all.upgrade() {
                let visible = visible.borrow();
                let count = visible.iter().filter(|n| s.contains(*n)).count();
                syncing.set(true);
                check.set_inconsistent(count > 0 && count < visible.len());
                check.set_active(!visible.is_empty() && count == visible.len());
                syncing.set(false);
            }
            selected_label.set_text(&format!("{} packages selected", s.len()));
            let all_installed = !s.is_empty() && s.iter().all(|n| installed.contains_key(n));
            remove.set_sensitive(all_installed);
            let repository_selection = db
                .borrow()
                .foreign
                .as_ref()
                .is_some_and(|f| s.iter().all(|n| !f.contains(n)));
            reinstall.set_sensitive(all_installed && repository_selection);
            install.set_sensitive(!s.is_empty() && repository_selection);
            install.set_tooltip_text(Some("Foreign packages need an explicit source decision in Loadouts before installation."));
        }
    });
    let inventory_error = inv.error.clone();
    let draw: Rc<dyn Fn()> = Rc::new({
        let db = db.clone();
        let ready = ready.clone();
        let selected = selected.clone();
        let mode = mode.clone();
        let page = page.clone();
        let visible = visible.clone();
        let catalog = catalog.clone();
        let installed = installed.clone();
        let update_selection = update_selection.clone();
        let search = search.downgrade();
        let sort = sort.downgrade();
        let source = source.downgrade();
        let categories = categories.downgrade();
        let per_page = per_page.downgrade();
        let parent = parent.downgrade();
        let prev = prev.downgrade();
        let next = next.downgrade();
        move || {
            let (Some(prev), Some(next)) = (prev.upgrade(), next.upgrade()) else {
                return;
            };
            let (Some(search), Some(sort), Some(source), Some(categories), Some(per_page)) = (
                search.upgrade(),
                sort.upgrade(),
                source.upgrade(),
                categories.upgrade(),
                per_page.upgrade(),
            ) else {
                return;
            };
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            let db = db.borrow();
            let query = search.text().to_lowercase();
            let mut names: BTreeSet<String> = installed.keys().cloned().collect();
            if mode.get() == 1 {
                names.extend(
                    catalog
                        .iter()
                        .map(|a| package_variants::installed_name(&installed, &a.name).to_owned()),
                );
            }
            let mut matching: Vec<_> = names
                .into_iter()
                .filter(|n| {
                    let app = catalog_entry(&catalog, n);
                    let detail = db.details.get(n);
                    let foreign = db.foreign.as_ref().is_some_and(|f| f.contains(n));
                    let description = detail
                        .map(|d| d.description.as_str())
                        .or_else(|| app.map(|a| a.description.as_str()))
                        .unwrap_or("");
                    (query.is_empty()
                        || n.contains(&query)
                        || description.to_lowercase().contains(&query))
                        && (mode.get() != 2 || db.updates.as_ref().is_some_and(|u| u.contains(n)))
                        && (mode.get() != 3 || foreign)
                        && match source.selected() {
                            1 => {
                                if installed.contains_key(n) {
                                    db.foreign.is_some() && !foreign
                                } else {
                                    app.is_some_and(|a| a.source == loadouts::Source::Repo)
                                }
                            }
                            2 => app.is_some_and(|a| a.source == loadouts::Source::Aur),
                            3 => foreign,
                            _ => true,
                        }
                        && category_matches(&catalog, n, categories.selected())
                })
                .collect();
            matching.sort_by(|a, b| match sort.selected() {
                1 => b.cmp(a),
                2 => db
                    .details
                    .get(b)
                    .map(|d| d.size)
                    .unwrap_or(0)
                    .cmp(&db.details.get(a).map(|d| d.size).unwrap_or(0))
                    .then(a.cmp(b)),
                _ => a.cmp(b),
            });
            let limit = [25, 50, 100][per_page.selected() as usize];
            let pages = matching.len().div_ceil(limit).max(1);
            page.set(page.get().min(pages - 1));
            let start = page.get() * limit;
            let end = (start + limit).min(matching.len());
            count.set_text(&format!(
                "Showing {}–{} of {} packages",
                if matching.is_empty() { 0 } else { start + 1 },
                end,
                matching.len()
            ));
            page_label.set_text(&format!("{} / {}", page.get() + 1, pages));
            prev.set_sensitive(page.get() > 0);
            next.set_sensitive(page.get() + 1 < pages);
            hint.set_text(if !ready.get() { "Reading the local package database…" } else if mode.get()==1 { "Browse the curated catalog and installed packages. Categories apply to catalog entries." } else if mode.get()==2 { "Updates from existing sync databases. Use Check for updates for a fresh read-only check." } else { "Installed packages · Ctrl+F to search · categories apply to catalog entries" });
            if let Some(error) = &inventory_error {
                hint.set_text(&format!("Could not read installed packages: {error}"));
            }
            *visible.borrow_mut() = matching[start..end].to_vec();
            if matching.is_empty() {
                list.append(&wrapped(
                    if ready.get() {
                        "No packages match your search."
                    } else {
                        "Loading package details…"
                    },
                    "muted",
                ));
                if ready.get() {
                    let category = if categories.selected() as usize == loadouts::CATEGORIES.len() {
                        "Uncategorized"
                    } else {
                        loadouts::CATEGORIES
                            .get(categories.selected() as usize)
                            .copied()
                            .unwrap_or("All packages")
                    };
                    list.append(&wrapped(&format!("No matches in {category} with the current source and search filters. Reset filters to show all packages. Categories cover catalog apps and known variants; other packages appear under Uncategorized."),"tiny"));
                }
            }
            for name in &matching[start..end] {
                let r = row(12);
                r.add_css_class("package-row");
                let check = gtk::CheckButton::new();
                check.set_active(selected.borrow().contains(name));
                check.set_tooltip_text(Some(&format!("Select {name}")));
                let selection = selected.clone();
                let n = name.clone();
                let update = update_selection.clone();
                check.connect_toggled(move |b| {
                    if b.is_active() {
                        selection.borrow_mut().insert(n.clone());
                    } else {
                        selection.borrow_mut().remove(&n);
                    }
                    update();
                });
                r.append(&check);
                let app = catalog_entry(&catalog, name);
                if let Some(app) = app {
                    let icon = app_icons::icon(app);
                    icon.set_size_request(26, 26);
                    r.append(&icon);
                } else {
                    r.append(&symbol("\u{e1a1}", "package-x-generic-symbolic"));
                }
                let text = column(3);
                text.set_hexpand(true);
                let title = cell(name, "row-title", -1);
                text.append(&title);
                let detail = db.details.get(name);
                let description = detail
                    .map(|d| d.description.as_str())
                    .or_else(|| app.map(|a| a.description.as_str()))
                    .unwrap_or("Installed package");
                let sub = cell(description, "row-sub", -1);
                sub.set_max_width_chars(48);
                text.append(&sub);
                r.append(&text);
                r.append(&cell(
                    installed
                        .get(name)
                        .map(String::as_str)
                        .unwrap_or("Not installed"),
                    "package-version mono",
                    145,
                ));
                let foreign = db.foreign.as_ref().is_some_and(|f| f.contains(name));
                let origin = if installed.contains_key(name) {
                    if foreign {
                        "Foreign"
                    } else if db.foreign.is_some() {
                        "Repository"
                    } else {
                        "Unknown"
                    }
                } else if app.is_some_and(|a| a.source == loadouts::Source::Aur) {
                    "AUR"
                } else {
                    "Repository"
                };
                let badge = cell(origin, "package-source", 94);
                r.append(&badge);
                r.append(&cell(
                    &detail.map(|d| bytes(d.size)).unwrap_or_else(|| "—".into()),
                    "tiny",
                    72,
                ));
                let menu = gtk::MenuButton::new();
                menu.set_icon_name("view-more-symbolic");
                menu.add_css_class("package-more");
                let pop = gtk::Popover::new();
                let body = column(8);
                let info = wrapped(
                    &format!("{name}\n\n{description}\n\nSource: {origin}"),
                    "muted",
                );
                body.append(&info);
                for (caption, action) in [
                    ("Install / update", 0),
                    ("Remove package", 1),
                    ("Reinstall package", 2),
                ] {
                    if (action != 0 && !installed.contains_key(name))
                        || (action != 1
                            && installed.contains_key(name)
                            && (foreign || db.foreign.is_none()))
                    {
                        continue;
                    }
                    let b = button(caption, false);
                    let parent = parent.clone();
                    let n = name.clone();
                    b.connect_clicked(move |_| {
                        if let Some(p) = parent.upgrade() {
                            review(&p, vec![n.clone()], action);
                        }
                    });
                    body.append(&b);
                }
                pop.set_child(Some(&body));
                menu.set_popover(Some(&pop));
                r.append(&menu);
                list.append(&r);
            }
            update_selection();
        }
    });
    for (i, b) in tab_buttons.iter().enumerate() {
        let mode = mode.clone();
        let page = page.clone();
        let draw = draw.clone();
        b.connect_toggled(move |b| {
            if b.is_active() {
                mode.set(i as u32);
                page.set(0);
                draw();
            }
        });
    }
    {
        let draw = draw.clone();
        let page = page.clone();
        search.connect_search_changed(move |_| {
            page.set(0);
            draw();
        });
    }
    {
        let search = search.downgrade();
        let source = source.downgrade();
        let categories = categories.downgrade();
        let page = page.clone();
        let draw = draw.clone();
        reset.connect_clicked(move |_| {
            if let (Some(search), Some(source), Some(categories)) =
                (search.upgrade(), source.upgrade(), categories.upgrade())
            {
                search.set_text("");
                source.set_selected(0);
                categories.set_selected(0);
                page.set(0);
                draw();
            }
        });
    }
    for d in [&sort, &source, &categories, &per_page] {
        let draw = draw.clone();
        let page = page.clone();
        d.connect_selected_notify(move |_| {
            page.set(0);
            draw();
        });
    }
    // Pagination shares the same filtered result set as the table.
    for (b, delta) in [(prev, -1isize), (next, 1)] {
        let page = page.clone();
        let draw = draw.clone();
        b.connect_clicked(move |_| {
            page.set(page.get().saturating_add_signed(delta));
            draw();
        });
    }
    {
        let visible = visible.clone();
        let selected = selected.clone();
        let draw = draw.clone();
        let syncing = syncing.clone();
        select_all.connect_toggled(move |b| {
            if syncing.get() {
                return;
            }
            for n in visible.borrow().iter() {
                if b.is_active() {
                    selected.borrow_mut().insert(n.clone());
                } else {
                    selected.borrow_mut().remove(n);
                }
            }
            draw();
        });
    }
    {
        let selected = selected.clone();
        let draw = draw.clone();
        clear.connect_clicked(move |_| {
            selected.borrow_mut().clear();
            draw();
        });
    }
    for (b, action) in [(install, 0), (remove, 1), (reinstall, 2)] {
        let parent = parent.downgrade();
        let selected = selected.clone();
        b.connect_clicked(move |_| {
            if let Some(p) = parent.upgrade() {
                review(&p, selected.borrow().iter().cloned().collect(), action);
            }
        });
    }
    let old: Vec<_> = (0..parent.observe_controllers().n_items())
        .filter_map(|i| parent.observe_controllers().item(i))
        .filter_map(|o| o.downcast::<gtk::EventController>().ok())
        .filter(|c| c.name().as_deref() == Some("archforge-package-search"))
        .collect();
    for c in old {
        parent.remove_controller(&c);
    }
    let controller = gtk::EventControllerKey::new();
    controller.set_name(Some("archforge-package-search"));
    let entry = search.downgrade();
    let weak = p.downgrade();
    controller.connect_key_pressed(move |_, key, _, mods| {
        if key == gtk::gdk::Key::f
            && mods.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            && weak.upgrade().is_some_and(|p| p.is_mapped())
        {
            if let Some(e) = entry.upgrade() {
                e.grab_focus();
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    parent.add_controller(controller);
    draw();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(database());
    });
    let weak = p.downgrade();
    glib::timeout_add_local(Duration::from_millis(80), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        match rx.try_recv() {
            Ok(data) => {
                values[1].set_text(
                    &data
                        .updates
                        .as_ref()
                        .map(|s| s.len().to_string())
                        .unwrap_or_else(|| "Unknown".into()),
                );
                values[2].set_text(
                    &data
                        .foreign
                        .as_ref()
                        .map(|s| s.len().to_string())
                        .unwrap_or_else(|| "Unknown".into()),
                );
                values[3].set_text(&if data.details.is_empty() {
                    "Unknown".into()
                } else {
                    bytes(data.details.values().map(|d| d.size).sum())
                });
                *db.borrow_mut() = data;
                ready.set(true);
                draw();
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reviewed_variants_keep_catalog_categories_and_metadata() {
        let catalog = loadout_styles::catalog();
        for expected in &catalog {
            let category = loadouts::CATEGORIES
                .iter()
                .position(|c| *c == expected.category)
                .unwrap() as u32;
            for suffix in ["-bin", "-git", "-svn", "-hg"] {
                let name = format!("{}{suffix}", package_variants::base_name(&expected.name));
                if catalog
                    .iter()
                    .any(|a| a.name == name && a.category != expected.category)
                {
                    continue;
                }
                if let Some(app) = catalog_entry(&catalog, &name) {
                    assert_eq!(app.category, expected.category, "{name}");
                    assert!(category_matches(&catalog, &name, category));
                }
            }
        }
    }
    #[test]
    fn uncatalogued_packages_remain_accessible_without_guessing_categories() {
        let catalog = loadout_styles::catalog();
        let name = "unlisted-local-package";
        assert!(category_matches(&catalog, name, 0));
        assert!(category_matches(
            &catalog,
            name,
            loadouts::CATEGORIES.len() as u32
        ));
        assert!(!category_matches(
            &catalog,
            name,
            loadouts::CATEGORIES
                .iter()
                .position(|c| *c == "Communication")
                .unwrap() as u32
        ));
        assert!(catalog_entry(&catalog, "vesktop-debug").is_none());
    }
}
