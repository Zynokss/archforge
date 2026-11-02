use super::*;
fn walk(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut all = vec![root.clone()];
    let mut child = root.first_child();
    while let Some(c) = child {
        all.extend(walk(&c));
        child = c.next_sibling();
    }
    all
}
fn click(root: &gtk::Widget, name: &str) {
    walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some(name))
        .unwrap()
        .emit_clicked();
}
pub(super) fn ui(parent: &gtk::ApplicationWindow) {
    let provider = gtk::CssProvider::new();
    crate::theme::apply(&provider, "amber");
    let (page, view) = create(parent);
    let window = gtk::Window::builder()
        .title("Loadouts preview")
        .default_width(1400)
        .default_height(940)
        .build();
    let scroll = gtk::ScrolledWindow::builder().child(&page).build();
    window.set_child(Some(&scroll));
    window.present();
    let start = std::time::Instant::now();
    while view.scanning.get() {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(10));
    }
    *view.packages.borrow_mut() = crate::loadout_state::Snapshot {
        ready: true,
        ..Default::default()
    };
    // AUR/source-built variants must use the installed name for status and removal.
    view.packages
        .borrow_mut()
        .installed
        .insert("vesktop".into(), "1.6.8-1".into());
    let vesktop = view
        .apps()
        .into_iter()
        .find(|a| a.title == "Vesktop")
        .unwrap();
    assert_eq!(vesktop.name, "vesktop");
    assert_eq!(
        view.packages.borrow().describe(&vesktop.name).0,
        "Installed"
    );
    view.profile
        .borrow_mut()
        .packages
        .insert("vesktop-bin".into(), Source::Aur);
    assert!(view.effective().packages.contains_key("vesktop"));
    assert!(!view.install_profile().packages.contains_key("vesktop"));
    assert_eq!(
        crate::loadout_state::remove_command(&[vesktop.name]).unwrap(),
        "sudo pacman -R -- vesktop"
    );

    // General suffix matching must feed status, filters, selection and source review.
    view.packages
        .borrow_mut()
        .installed
        .insert("firefox-bin".into(), "1.0".into());
    view.packages
        .borrow_mut()
        .installed
        .insert("obs-studio-git".into(), "1.0".into());
    for (requested, actual, category) in [
        ("firefox", "firefox-bin", "Browsers"),
        ("obs-studio", "obs-studio-git", "Media"),
    ] {
        let app = view.apps().into_iter().find(|a| a.name == actual).unwrap();
        assert_eq!(app.category, category);
        assert_eq!(app.source, Source::Foreign);
        view.status_filter.set(1);
        assert!(view.matches_status(actual));
        view.status_filter.set(2);
        assert!(!view.matches_status(actual));
        view.status_filter.set(0);
        view.profile
            .borrow_mut()
            .packages
            .insert(requested.into(), Source::Repo);
        assert_eq!(
            view.effective().packages.get(actual),
            Some(&Source::Foreign)
        );
        assert!(!view.install_profile().packages.contains_key(actual));
        view.render_summary();
        // The summary displays the resolved installed name but clears the original selection.
        let summary = view.summary.upcast_ref::<gtk::Widget>();
        let target = walk(summary)
            .into_iter()
            .find(|w| {
                w.has_css_class("loadout-selected-item")
                    && walk(w)
                        .iter()
                        .filter_map(|w| w.downcast_ref::<gtk::Label>())
                        .any(|l| l.text() == app.title)
            })
            .unwrap();
        let removal = walk(&target)
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.has_css_class("loadout-clear"))
            .unwrap();
        removal.emit_clicked();
        assert!(!view.profile.borrow().packages.contains_key(requested));
    }
    view.profile.borrow_mut().packages.clear();
    view.packages.borrow_mut().installed.clear();
    view.render_packages();
    view.render_summary();
    let root = page.upcast_ref::<gtk::Widget>();
    assert!(!view.install.is_sensitive());
    let mut p = Profile {
        name: "Fresh Arch desktop".into(),
        ..Profile::default()
    };
    for name in ["firefox", "git", "kitty", "zsh", "fastfetch"] {
        p.packages.insert(name.into(), Source::Repo);
    }
    p.packages
        .insert("visual-studio-code-bin".into(), Source::Aur);
    p.packages
        .insert("local-test-build".into(), Source::Foreign);
    view.replace(p);
    assert!(view.install.is_sensitive());
    assert!(
        !view
            .effective()
            .command(false)
            .unwrap()
            .contains("local-test-build")
    );
    view.foreign.set_active(true);
    assert!(
        view.effective()
            .command(true)
            .unwrap()
            .contains("local-test-build")
    );
    view.foreign.set_active(false);
    let entry = walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::SearchEntry>().ok())
        .find(|w| w.placeholder_text().as_deref() == Some("Search apps and package names…"))
        .unwrap();
    entry.set_text("firefox");
    let start = std::time::Instant::now();
    while view.query.borrow().as_str() != "firefox" {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        assert!(start.elapsed() < Duration::from_secs(10));
    }
    assert_eq!(view.category.get(), 0);
    assert!(
        walk(view.library.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Label>().ok())
            .any(|l| l.text() == "Firefox")
    );
    assert!(
        walk(view.library.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::CheckButton>().ok())
            .count()
            >= 1
    );
    entry.set_text("");
    *view.query.borrow_mut() = String::new();
    view.category.set(1);
    view.render_packages();
    click(root, "Select matches");
    assert!(view.profile.borrow().packages.contains_key("rsync"));
    click(view.ricing.upcast_ref(), "Use starter dotfiles");
    assert!(
        view.profile
            .borrow()
            .files
            .contains_key(".config/hypr/hyprland.lua")
    );
    assert!(view.apply.is_sensitive());
    let file_count = view.effective().files.len();
    let first = walk(view.files.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::CheckButton>().ok())
        .unwrap();
    first.set_active(false);
    assert_eq!(view.effective().files.len(), file_count - 1);
    first.set_active(true);
    let encoded = view.effective().encode().unwrap();
    let decoded = Profile::decode(&encoded).unwrap();
    view.replace(decoded);
    assert_eq!(view.effective().files.len(), file_count);
    let tmp = tempfile::tempdir().unwrap();
    write_file(&tmp.path().join("profile.json"), &encoded).unwrap();
    assert_eq!(
        Profile::read(&tmp.path().join("profile.json"))
            .unwrap()
            .packages,
        view.effective().packages
    );
    let sort = walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::DropDown>().ok())
        .find(|d| {
            d.model()
                .and_then(|m| m.downcast::<gtk::StringList>().ok())
                .and_then(|m| m.string(0))
                .as_deref()
                == Some("Recommended")
        })
        .unwrap();
    sort.set_selected(2);
    assert_eq!(view.sort.get(), 2);
    let names: Vec<_> = view
        .apps()
        .into_iter()
        .map(|a| a.title.to_lowercase())
        .collect();
    assert!(names.windows(2).all(|p| p[0] <= p[1]));
    sort.set_selected(3);
    let names: Vec<_> = view
        .apps()
        .into_iter()
        .map(|a| a.title.to_lowercase())
        .collect();
    assert!(names.windows(2).all(|p| p[0] >= p[1]));
    sort.set_selected(0);
    let ai = loadouts::CATEGORIES
        .iter()
        .position(|c| *c == "AI & ML")
        .unwrap();
    view.category_picker.set_selected(ai as u32);
    assert_eq!(view.category.get(), ai);
    assert!(view.results.text().starts_with("32 matches"));
    view.library_tab.set_visible_child_name("packages");
    crate::dialogs::snapshot(&window, "loadouts-ai-catalog");
    view.category.set(1);
    view.render_packages();
    // Existing selections remain portable, while local installation skips installed names.
    {
        let mut state = view.packages.borrow_mut();
        state.updates_checked = true;
        state.installed.insert("git".into(), "2.49-1".into());
        state.installed.insert("firefox".into(), "140-1".into());
        state
            .updates
            .insert("git".into(), ("2.49-1".into(), "2.50-1".into()));
        state.cached.insert("kitty".into());
    }
    view.render_packages();
    view.render_summary();
    assert!(!view.install_profile().packages.contains_key("git"));
    assert!(view.effective().packages.contains_key("git"));
    assert!(view.install_profile().packages.contains_key("kitty"));
    assert!(view.effective().script(false).unwrap().contains("git"));
    assert!(view.uninstall.is_sensitive());
    view.skip.set_active(false);
    assert!(view.install_profile().packages.contains_key("git"));
    view.skip.set_active(true);
    view.status_filter.set(4);
    view.category.set(0);
    view.render_packages();
    assert!(view.results.text().starts_with("1 matches"));
    view.status_filter.set(3);
    view.render_packages();
    assert!(view.results.text().starts_with("1 matches"));
    view.status_filter.set(0);
    let saved = view.profile.borrow().clone();
    let mut only_installed = Profile::default();
    only_installed.packages.insert("git".into(), Source::Repo);
    view.replace(only_installed);
    assert!(!view.install.is_sensitive());
    assert!(view.export.is_sensitive());
    assert!(
        view.command
            .buffer()
            .text(
                &view.command.buffer().start_iter(),
                &view.command.buffer().end_iter(),
                false
            )
            .contains("All selected packages are installed")
    );
    view.uninstall.emit_clicked();
    let review = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some("Uninstall packages"))
        .unwrap();
    assert!(
        walk(review.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Label>().ok())
            .any(|l| l.text().contains("confirmation"))
    );
    review.close();
    // Exercise real read-only dependency checks without opening a terminal.
    for name in ["7zip", "archforge"] {
        let names = vec![name.to_owned()];
        if let Ok(plan) = crate::loadout_state::preview_removal(&names) {
            let before = crate::system::run("/usr/bin/pacman", &["-Q", name], 5).unwrap();
            removal_review(
                parent,
                names.clone(),
                crate::loadout_state::remove_command(&names).unwrap(),
            );
            let dialog = gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::Window>().ok())
                .find(|w| w.is_visible() && w.title().as_deref() == Some("Uninstall packages"))
                .unwrap();
            let start = std::time::Instant::now();
            loop {
                while glib::MainContext::default().pending() {
                    glib::MainContext::default().iteration(false);
                }
                let text = if plan.blockers.is_empty() {
                    "Ready to review removal"
                } else {
                    "Other apps still need these packages"
                };
                if walk(dialog.upcast_ref())
                    .iter()
                    .filter_map(|w| w.downcast_ref::<gtk::Label>())
                    .any(|l| l.text() == text)
                {
                    break;
                }
                assert!(start.elapsed() < Duration::from_secs(20));
                std::thread::sleep(Duration::from_millis(10));
            }
            let open = walk(dialog.upcast_ref())
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::Button>().ok())
                .find(|b| b.label().as_deref() == Some("Open embedded terminal"))
                .unwrap();
            assert_eq!(open.is_visible(), plan.blockers.is_empty());
            for blocker in &plan.blockers {
                assert!(
                    walk(dialog.upcast_ref())
                        .iter()
                        .filter_map(|w| w.downcast_ref::<gtk::Label>())
                        .any(|l| l.text() == blocker.required_by)
                );
            }
            if !plan.blockers.is_empty() {
                crate::dialogs::snapshot(&dialog, "uninstall-dependency-blocked");
            }
            dialog.close();
            assert_eq!(
                before,
                crate::system::run("/usr/bin/pacman", &["-Q", name], 5).unwrap(),
                "Preview must not remove a package"
            );
        }
    }
    view.packages.borrow_mut().ready = false;
    view.render_summary();
    assert!(!view.install.is_sensitive());
    assert!(!view.uninstall.is_sensitive());
    assert!(!view.update.is_sensitive());
    assert_eq!(view.packages.borrow().describe("git").0, "Not scanned");
    view.packages.borrow_mut().ready = true;
    view.replace(saved);
    crate::dialogs::snapshot(&window, "loadouts-desktop-kits-amber");
    view.library_tab.set_visible_child_name("packages");
    view.category.set(0);
    view.render_packages();
    crate::dialogs::snapshot(&window, "loadouts-packages-amber");
    // Column slots must line up across short/long names and every status.
    let header = walk(root)
        .into_iter()
        .find(|w| w.has_css_class("loadout-table-header"))
        .unwrap();
    fn children(w: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut result = Vec::new();
        let mut child = w.first_child();
        while let Some(c) = child {
            result.push(c.clone());
            child = c.next_sibling();
        }
        result
    }
    let heading_cells = children(&header);
    for card in walk(view.library.upcast_ref())
        .into_iter()
        .filter(|w| w.has_css_class("loadout-app"))
    {
        let cells = children(&card);
        for i in [0, 2, 3, 4] {
            let a = cells[i].compute_bounds(&window).unwrap();
            let b = heading_cells[i].compute_bounds(&window).unwrap();
            assert!(
                (a.x() - b.x()).abs() <= 1.0,
                "Column {i} must align with its header"
            );
        }
    }
    assert!(
        page.measure(gtk::Orientation::Horizontal, -1).0 <= 1200,
        "Loadouts must fit a smaller content area without hiding the selection panel"
    );
    view.category.set(1);
    {
        let mut state = view.packages.borrow_mut();
        state
            .installed
            .insert("bash-completion".into(), "2.16-1".into());
        state.installed.insert("git".into(), "2.49-1".into());
        state.installed.insert("curl".into(), "8.13-1".into());
        state
            .updates
            .insert("curl".into(), ("8.13-1".into(), "8.14-1".into()));
        state.cached.insert("xdg-desktop-portal".into());
    }
    view.render_packages();
    view.render_summary();
    crate::dialogs::snapshot(&window, "loadouts-aligned-essentials");

    click(root, "Capture this system");
    let capture = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some("Capture this system"))
        .unwrap();
    assert!(
        walk(capture.upcast_ref())
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::CheckButton>())
            .all(|c| !c.is_active())
    );
    click(capture.upcast_ref(), "Preview snapshot");
    let start = std::time::Instant::now();
    loop {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        let ready = walk(capture.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.label().as_deref() == Some("Save profile…"))
            .unwrap()
            .is_sensitive();
        if ready {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(20));
    }
    capture.close();
    window.close();
    if let Some(stack) = walk(parent.upcast_ref())
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Stack>().ok())
        .find(|s| s.child_by_name("loadouts").is_some())
    {
        let before = stack.child_by_name("loadouts").unwrap();
        crate::refresh_pages(
            parent,
            &stack,
            &crate::system::Inventory::load(),
            Rc::new(RefCell::new(crate::system::Sampler::default().sample())),
        );
        crate::adapt_layout(parent);
        assert_eq!(
            before,
            stack.child_by_name("loadouts").unwrap(),
            "Refreshing packages must not discard an unsaved loadout"
        );
    }
    if std::path::Path::new("/usr/bin/sway").is_file() {
        let p = crate::loadout_styles::style(Some(5), true);
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("sway.conf");
        std::fs::write(&config, &p.files[".config/sway/config"]).unwrap();
        let result = std::process::Command::new("/usr/bin/sway")
            .args(["--validate", "--config"])
            .arg(&config)
            .env("WLR_BACKENDS", "headless")
            .env("WLR_RENDERER", "pixman")
            .env("XDG_RUNTIME_DIR", temp.path())
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
            .env_remove("SWAYSOCK")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    gtk::style_context_remove_provider_for_display(
        &gtk::gdk::Display::default().unwrap(),
        &provider,
    );
}

#[test]
#[ignore = "Isolated layout probe"]
fn isolated_loadout_layout() {
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.archforge.LoadoutLayout")
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let parent = gtk::ApplicationWindow::builder()
        .application(&app)
        .default_width(1640)
        .default_height(950)
        .title("Loadout layout parent")
        .build();
    parent.present();
    ui(&parent);
    parent.close();
}
