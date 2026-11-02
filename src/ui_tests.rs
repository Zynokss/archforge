//! Explicit, opt-in native UI verification against the real local system.
use super::*;
use std::time::Instant;

fn walk(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = vec![widget.clone()];
    let mut child = widget.first_child();
    while let Some(w) = child {
        result.extend(walk(&w));
        child = w.next_sibling();
    }
    result
}

fn expand_details(root: &gtk::Widget) {
    walk(root)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::Expander>().ok())
        .unwrap()
        .set_expanded(true);
}

fn named_button(root: &gtk::Widget, name: &str) -> gtk::Button {
    walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| {
            b.label().is_some_and(|s| s == name)
                || walk(b.upcast_ref()).iter().any(|w| {
                    w.downcast_ref::<gtk::Label>()
                        .is_some_and(|l| l.text() == name)
                })
        })
        .unwrap_or_else(|| panic!("Missing button: {name}"))
}

fn wait_for(mut condition: impl FnMut() -> bool) {
    let context = glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        while context.pending() {
            context.iteration(false);
        }
        if condition() {
            return;
        }
        assert!(Instant::now() < deadline, "Timed out waiting for UI state");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn has_label(root: &gtk::Widget, text: &str) -> bool {
    walk(root).iter().any(|w| {
        w.downcast_ref::<gtk::Label>()
            .is_some_and(|l| l.text() == text)
    })
}

fn clipboard_text() -> String {
    glib::MainContext::default()
        .block_on(
            gtk::gdk::Display::default()
                .unwrap()
                .clipboard()
                .read_text_future(),
        )
        .unwrap()
        .unwrap()
        .to_string()
}

fn dialog(title: &str) -> gtk::Window {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().is_some_and(|s| s == title))
        .unwrap_or_else(|| panic!("Missing dialog: {title}"))
}

fn capture(name: &str) {
    capture_size(name, "1640x950+0+0");
}

fn capture_size(name: &str, geometry: &str) {
    std::fs::create_dir_all("screenshots").unwrap();
    // Allow the compositor to render the frame before capturing.
    let start = Instant::now();
    wait_for(|| start.elapsed() > Duration::from_millis(300));
    assert!(
        std::process::Command::new("import")
            .args([
                "-window",
                "root",
                "-crop",
                geometry,
                "+repage",
                &format!("screenshots/{name}.png")
            ])
            .status()
            .unwrap()
            .success()
    );
}

#[test]
#[ignore = "Requires an isolated graphical session, pacman, and ImageMagick"]
fn native_ui_workflows() {
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.archforge.SmokeTest")
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    build(&app);
    let window = app.active_window().unwrap();
    let root = window.upcast_ref::<gtk::Widget>();
    wait_for(|| named_button(root, "Overview").is_sensitive());
    // Accumulate real chart samples, rather than generating fake telemetry.
    let start = Instant::now();
    wait_for(|| start.elapsed() > Duration::from_secs(4));
    capture("overview");
    for (title, heading, shot) in [
        ("Tools", "Find the right tool.", "tools"),
        ("Loadouts", "Build your Arch loadout.", "loadouts"),
        ("Wiki", "Your Arch field guide.", "wiki"),
        ("Gaming", "Make room for play.", "gaming"),
        ("Cleanup", "Less clutter. More space.", "cleanup"),
        ("Packages", "Packages", "packages"),
        ("Diagnostics", "Get to the root of it.", "diagnostics"),
    ] {
        named_button(root, title).emit_clicked();
        assert!(has_label(root, heading));
        let amber = gtk::CssProvider::new();
        if title == "Loadouts" {
            theme::apply(&amber, "amber");
            named_button(root, "Select matches").emit_clicked();
        }
        let tooltip_widgets: Vec<_> = if title == "Tools" || title == "Loadouts" {
            walk(root).into_iter().filter(|w| w.has_tooltip()).collect()
        } else {
            Vec::new()
        };
        for w in &tooltip_widgets {
            w.set_has_tooltip(false);
        }
        capture(shot);
        if title == "Tools" {
            let main_stack = walk(root)
                .into_iter()
                .find_map(|w| w.downcast::<gtk::Stack>().ok())
                .unwrap();
            let tool_scroll = main_stack
                .child_by_name("tools")
                .unwrap()
                .downcast::<gtk::ScrolledWindow>()
                .unwrap();
            assert!(
                tool_scroll.vadjustment().upper() <= tool_scroll.vadjustment().page_size() + 1.,
                "Grouped overview must fit the standard window"
            );
        }
        for w in &tooltip_widgets {
            w.set_has_tooltip(true);
        }
        if title == "Loadouts" {
            gtk::style_context_remove_provider_for_display(
                &gtk::gdk::Display::default().unwrap(),
                &amber,
            );
        }
    }
    let navigation_key = (0..window.observe_controllers().n_items())
        .filter_map(|i| window.observe_controllers().item(i))
        .filter_map(|o| o.downcast::<gtk::EventControllerKey>().ok())
        .find(|k| k.propagation_phase() == gtk::PropagationPhase::Capture)
        .unwrap();
    assert!(navigation_key.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::k,
            &0u32,
            &gtk::gdk::ModifierType::CONTROL_MASK
        ]
    ));
    let stack = walk(root)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::Stack>().ok())
        .unwrap();
    assert_eq!(stack.visible_child_name().as_deref(), Some("tools"));
    let page = stack.child_by_name("tools").unwrap();
    assert!(walk(&page).iter().any(|w| w.has_css_class("tool-search")));
    assert!(navigation_key.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::_3,
            &0u32,
            &gtk::gdk::ModifierType::CONTROL_MASK
        ]
    ));
    assert_eq!(stack.visible_child_name().as_deref(), Some("gaming"));
    named_button(root, "Gaming").emit_clicked();
    named_button(root, "Copy launch option").emit_clicked();
    assert_eq!(clipboard_text(), "gamemoderun mangohud %command%");
    named_button(root, "Packages").emit_clicked();
    let entry = walk(root)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::SearchEntry>().ok())
        .unwrap();
    wait_for(|| entry.is_mapped());
    let controllers = window.observe_controllers();
    let keyboard = (0..controllers.n_items())
        .find_map(|i| {
            controllers
                .item(i)
                .and_then(|o| o.downcast::<gtk::EventControllerKey>().ok())
        })
        .unwrap();
    let handled = keyboard.emit_by_name::<bool>(
        "key-pressed",
        &[&('f' as u32), &0_u32, &gtk::gdk::ModifierType::CONTROL_MASK],
    );
    assert!(handled, "Ctrl+F must be handled on the package screen");
    assert!(
        walk(entry.upcast_ref()).iter().any(|w| w.has_focus()),
        "Ctrl+F must focus the search field"
    );
    entry.set_text("forge-no-such-installed-package-982374");
    wait_for(|| has_label(root, "No packages match your search."));
    entry.set_text("pacman");
    wait_for(|| has_label(root, "pacman"));
    assert!(!has_label(root, "No packages match your search."));
    let sort = walk(root)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::DropDown>().ok())
        .unwrap();
    sort.set_selected(1);
    let names: Vec<_> = walk(root)
        .into_iter()
        .filter(|w| w.has_css_class("package-row"))
        .filter_map(|w| {
            walk(&w)
                .into_iter()
                .find(|w| w.has_css_class("row-title"))
                .and_then(|w| w.downcast::<gtk::Label>().ok())
                .map(|l| l.text().to_string())
        })
        .collect();
    assert!(names.windows(2).all(|w| w[0] >= w[1]));
    sort.set_selected(0);
    capture("package-search");

    named_button(root, "Diagnostics").emit_clicked();
    named_button(root, "Generate").emit_clicked();
    wait_for(|| has_label(root, "Completed"));
    let text = walk(root)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::TextView>().ok())
        .unwrap()
        .buffer();
    let contents = text.text(&text.start_iter(), &text.end_iter(), false);
    assert!(contents.contains("Arch Forge system report"));
    assert!(contents.contains("Installed packages:"));
    named_button(root, "Copy output").emit_clicked();
    assert!(clipboard_text().contains("Arch Forge system report"));
    assert!(has_label(root, "Memory in use"));
    assert!(has_label(root, "Failed system units"));
    assert!(text.tag_table().lookup("log-error").is_some());
    capture("diagnostic-output");
    // A slower earlier request must not overwrite the most recently selected
    // check in the shared diagnostic console.
    let report_button = named_button(root, "Generate");
    let vulkan_row = walk(root)
        .into_iter()
        .find(|w| w.has_css_class("tool-row") && has_label(w, "Vulkan devices"))
        .unwrap();
    let vulkan_button = named_button(&vulkan_row, "Inspect");
    report_button.emit_clicked();
    assert!(has_label(root, "Collecting findings…"));
    vulkan_button.emit_clicked();
    wait_for(|| report_button.is_sensitive() && vulkan_button.is_sensitive());
    assert!(
        has_label(root, "Vulkan devices inspected")
            || has_label(root, "No GPU details reported")
            || has_label(root, "Check needs attention")
    );
    let latest = text.text(&text.start_iter(), &text.end_iter(), false);
    assert!(
        latest.starts_with("$ /usr/bin/vulkaninfo --summary"),
        "An older diagnostic request overwrote the latest selection"
    );

    named_button(root, "Cleanup").emit_clicked();
    let cache_row = walk(root)
        .into_iter()
        .find(|w| w.has_css_class("tool-row") && has_label(w, "Clean older versions"))
        .unwrap();
    named_button(&cache_row, "Review & clean").emit_clicked();
    let confirmation = dialog("Clean older package versions");
    wait_for(|| {
        has_label(
            confirmation.upcast_ref(),
            "Your package cache is already tidy",
        ) || named_button(confirmation.upcast_ref(), "Authenticate & clean").is_sensitive()
    });
    if has_label(
        confirmation.upcast_ref(),
        "Your package cache is already tidy",
    ) {
        assert!(!named_button(confirmation.upcast_ref(), "Authenticate & clean").get_visible());
        assert!(named_button(confirmation.upcast_ref(), "Done").has_css_class("primary"));
    }
    expand_details(confirmation.upcast_ref());
    let text = walk(confirmation.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::TextView>().ok())
        .unwrap()
        .buffer();
    assert!(
        !text
            .text(&text.start_iter(), &text.end_iter(), false)
            .is_empty()
    );
    capture("cleanup-confirmation");
    confirmation.close();
    let app_window = window.downcast_ref::<gtk::ApplicationWindow>().unwrap();
    show_job(app_window, Job::JournalClean);
    let journal = dialog("Keep 7 days of archived logs");
    assert!(
        named_button(journal.upcast_ref(), "Authenticate & clean").has_css_class("destructive")
    );
    expand_details(journal.upcast_ref());
    let journal_buffer = walk(journal.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::TextView>().ok())
        .unwrap()
        .buffer();
    assert!(
        journal_buffer
            .text(
                &journal_buffer.start_iter(),
                &journal_buffer.end_iter(),
                false
            )
            .is_empty()
    );
    named_button(journal.upcast_ref(), "Copy command").emit_clicked();
    assert_eq!(clipboard_text(), Job::JournalClean.display_command());
    journal.close();
    show_setup(app_window, "Steam", "steam", true);
    let config = dialog("Configure Steam");
    assert!(has_label(config.upcast_ref(), "steam"));
    config.close();
    show_job(app_window, Job::CacheSize);
    let cache = dialog("Package cache usage");
    expand_details(cache.upcast_ref());
    wait_for(|| named_button(cache.upcast_ref(), "Copy output").is_sensitive());
    let buffer = walk(cache.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::TextView>().ok())
        .unwrap()
        .buffer();
    let contents = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
    if contents.contains("Permission denied") {
        assert!(contents.starts_with("PARTIAL MEASUREMENT"));
        assert!(
            walk(root)
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Label>())
                .any(|l| l.has_css_class("cache-size-summary") && l.text().contains("partial"))
        );
        assert!(named_button(cache.upcast_ref(), "Measure with authentication").is_visible());
        capture("cache-partial-size");
        named_button(cache.upcast_ref(), "Measure with authentication").emit_clicked();
    } else {
        cache.close();
        show_job(app_window, Job::CacheSizeAdmin);
    }
    let full_size = dialog("Full package cache usage");
    assert!(has_label(
        full_size.upcast_ref(),
        Job::CacheSizeAdmin.confirmation()
    ));
    assert!(
        named_button(full_size.upcast_ref(), "Authenticate & measure").has_css_class("primary")
    );
    expand_details(full_size.upcast_ref());
    let buffer = walk(full_size.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::TextView>().ok())
        .unwrap()
        .buffer();
    assert!(
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .is_empty()
    );
    capture("cache-authentication");
    full_size.close();
    named_button(root, "Gaming").emit_clicked();
    let shortcuts = keyboard.emit_by_name::<bool>(
        "key-pressed",
        &[&('f' as u32), &0_u32, &gtk::gdk::ModifierType::CONTROL_MASK],
    );
    assert!(
        !shortcuts,
        "Package search must not intercept Ctrl+F on other screens"
    );
    named_button(root, "Overview").emit_clicked();
    let menu = walk(root)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::MenuButton>().ok())
        .unwrap();
    menu.popup();
    let palettes = menu.popover().unwrap();
    capture("appearance");
    for name in ["Arch Night", "Sand", "Sage"] {
        named_button(palettes.upcast_ref(), name).emit_clicked();
        assert!(named_button(palettes.upcast_ref(), name).has_css_class("chosen"));
    }
    let before = window.color();
    named_button(palettes.upcast_ref(), "Lavender").emit_clicked();
    wait_for(|| window.color() != before);
    assert!(named_button(palettes.upcast_ref(), "Lavender").has_css_class("chosen"));
    menu.popdown();
    capture("lavender");
    menu.popup();
    named_button(palettes.upcast_ref(), "Desktop colors").emit_clicked();
    assert!(named_button(palettes.upcast_ref(), "Desktop colors").has_css_class("chosen"));
    menu.popdown();
    capture("desktop-colors");
    menu.popup();
    named_button(palettes.upcast_ref(), "Crimson").emit_clicked();
    menu.popdown();
    // Leave room for client-side decorations inside a 960px tiled allocation.
    named_button(root, "Overview").emit_clicked();
    window.set_default_size(1586, 992);
    let resized = Instant::now();
    wait_for(|| resized.elapsed() > Duration::from_millis(500));
    eprintln!(
        "Overview verification width {}, minimum {}",
        window.width(),
        window.measure(gtk::Orientation::Horizontal, -1).0
    );
    capture_size("overview-refreshed", "1586x992+0+0");
    let capture_button = walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| {
            b.has_css_class("overview-shortcut")
                && b.label().as_deref() == Some("Capture this system")
        })
        .unwrap();
    capture_button.emit_clicked();
    let capture_window = dialog("Capture this system");
    capture_window.close();
    named_button(root, "Settings").emit_clicked();
    assert!(has_label(root, "About Arch Forge"));
    let switches: Vec<_> = walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Switch>().ok())
        .collect();
    assert_eq!(switches.len(), 2);
    let initial = switches[0].is_active();
    switches[0].set_active(!initial);
    assert_eq!(crate::preferences::automatic_updates(), !initial);
    switches[0].set_active(initial);
    let initial = switches[1].is_active();
    switches[1].set_active(!initial);
    assert_eq!(crate::preferences::reduced_motion(), !initial);
    switches[1].set_active(initial);
    capture_size("settings", "1586x992+0+0");
    named_button(root, "Loadouts").emit_clicked();
    window.set_default_size(920, 820);
    wait_for(|| window.has_css_class("compact"));
    assert!(
        walk(root)
            .iter()
            .filter(|w| w.has_css_class("loadout-category-picker"))
            .all(|w| w.is_visible())
    );
    assert!(
        walk(root)
            .iter()
            .filter(|w| w.has_css_class("loadout-category-scroll"))
            .all(|w| !w.is_visible())
    );

    assert!(
        window.measure(gtk::Orientation::Horizontal, -1).0 <= 950,
        "Compact content must fit a 950px tile"
    );
    capture_size("compact", "1000x860+0+0");
    loadout_ui::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    app_icons::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    app_locations::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    tools::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    dialogs::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    cleanups::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    terminal::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    updater::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    wiki::verify_ui(window.downcast_ref::<gtk::ApplicationWindow>().unwrap());
    window.close();
}

#[test]
#[ignore = "Isolated read-only README capture"]
fn readme_overview_screenshot() {
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.archforge.ReadmeCapture")
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    build(&app);
    let window = app.active_window().unwrap();
    window.set_default_size(1586, 992);
    wait_for(|| named_button(window.upcast_ref(), "Overview").is_sensitive());
    let start = Instant::now();
    wait_for(|| start.elapsed() > Duration::from_secs(5));
    capture_size("readme-overview", "1586x992+0+0");
    window.close();
}

#[test]
#[ignore = "Isolated package workbench and theme verification"]
fn packages_workbench() {
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.archforge.PackagesCapture")
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    build(&app);
    let window = app.active_window().unwrap();
    let root = window.upcast_ref::<gtk::Widget>();
    wait_for(|| named_button(root, "Packages").is_sensitive());
    named_button(root, "Packages").emit_clicked();
    let provider = gtk::CssProvider::new();
    theme::apply(&provider, "arch-night");
    wait_for(|| !has_label(root, "Reading the local package database…"));
    capture("packages-arch-night");
    let hover = named_button(root, "Check for updates");
    hover.set_state_flags(gtk::StateFlags::PRELIGHT, false);
    capture("packages-hover-arch-night");
    hover.unset_state_flags(gtk::StateFlags::PRELIGHT);
    let page = walk(root)
        .into_iter()
        .find(|w| w.has_css_class("packages-page") && w.is_mapped())
        .unwrap();
    let search = walk(&page)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::SearchEntry>().ok())
        .unwrap();
    search.set_text("forge-no-such-installed-package-982374");
    wait_for(|| has_label(&page, "No packages match your search."));
    search.set_text("pacman");
    wait_for(|| has_label(&page, "pacman"));
    let first_check = walk(&page)
        .into_iter()
        .find(|w| w.has_css_class("package-row"))
        .unwrap()
        .first_child()
        .unwrap()
        .downcast::<gtk::CheckButton>()
        .unwrap();
    first_check.set_active(true);
    assert!(named_button(&page, "Remove selected").is_sensitive());
    assert!(named_button(&page, "Reinstall selected").is_sensitive());
    named_button(&page, "×").emit_clicked();
    assert!(!named_button(&page, "Remove selected").is_sensitive());
    // Pagination must expose packages beyond the old 80-result limit.
    search.set_text("");
    wait_for(|| {
        walk(&page)
            .iter()
            .filter(|w| w.has_css_class("package-row"))
            .count()
            == 50
    });
    named_button(&page, "›").emit_clicked();
    assert!(
        walk(&page)
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::Label>())
            .any(|l| l.has_css_class("package-page-number") && l.text().starts_with("2 /"))
    );
    named_button(&page, "‹").emit_clicked();
    assert!(
        walk(&page)
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::Label>())
            .any(|l| l.has_css_class("package-page-number") && l.text().starts_with("1 /"))
    );
    // Browse contains curated packages as well as everything installed.
    let browse = walk(&page)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::ToggleButton>().ok())
        .find(|b| b.label().as_deref() == Some("Browse"))
        .unwrap();
    browse.set_active(true);
    wait_for(|| {
        has_label(
            &page,
            "Browse the curated catalog and installed packages. Categories apply to catalog entries.",
        )
    });
    capture("packages-browse-arch-night");
    window.set_default_size(1000, 860);
    wait_for(|| window.has_css_class("compact"));
    window.set_default_size(1000, 860);
    let start = Instant::now();
    wait_for(|| start.elapsed() > Duration::from_millis(600));
    eprintln!(
        "Compact packages: allocated {}, minimum {}",
        window.width(),
        window.measure(gtk::Orientation::Horizontal, -1).0
    );
    capture_size("packages-compact-arch-night", "1020x900+0+0");
    assert!(
        window.measure(gtk::Orientation::Horizontal, -1).0 <= 1000,
        "Package page must fit a compact window"
    );
    window.close();
}

#[test]
#[ignore = "Isolated category, sorting and pagination regression check"]
fn package_category_filters() {
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.archforge.CategoryRegression")
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    let window = gtk::ApplicationWindow::builder()
        .application(&app)
        .default_width(1640)
        .default_height(950)
        .build();
    let provider = gtk::CssProvider::new();
    theme::apply(&provider, "arch-night");
    let mut inv = Inventory::default();
    for name in ["vesktop", "lutris-git", "adw-gtk-theme-git"]
        .into_iter()
        .map(str::to_owned)
        .chain((0..130).map(|i| format!("uncatalogued-fixture-{i:03}")))
    {
        inv.packages.push(system::Package {
            name,
            version: "1.0-1".into(),
        });
    }
    let page = packages_ui::build(&window, &inv);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&page)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    window.set_child(Some(&scroll));
    window.present();
    let root = page.upcast_ref::<gtk::Widget>();
    wait_for(|| !has_label(root, "Reading the local package database…"));
    let dropdowns: Vec<_> = walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::DropDown>().ok())
        .collect();
    let (sort, category, rows) = (&dropdowns[0], &dropdowns[2], &dropdowns[3]);
    rows.set_selected(2);
    named_button(root, "›").emit_clicked();
    assert!(has_label(root, "2 / 2"));
    for (category_name, expected) in [
        ("Communication", "vesktop"),
        ("Gaming", "lutris-git"),
        ("Desktop & styling", "adw-gtk-theme-git"),
    ] {
        category.set_selected(
            loadouts::CATEGORIES
                .iter()
                .position(|c| *c == category_name)
                .unwrap() as u32,
        );
        for limit in 0..3 {
            rows.set_selected(limit);
            for order in 0..3 {
                sort.set_selected(order);
                assert!(has_label(root, expected));
                assert!(has_label(root, "1 / 1"));
                assert_eq!(
                    walk(root)
                        .iter()
                        .filter(|w| w.has_css_class("package-row"))
                        .count(),
                    1
                );
            }
        }
    }
    category.set_selected(
        loadouts::CATEGORIES
            .iter()
            .position(|c| *c == "Communication")
            .unwrap() as u32,
    );
    capture("packages-communication-fixed");
    category.set_selected(loadouts::CATEGORIES.len() as u32);
    assert!(has_label(root, "uncatalogued-fixture-000"));
    assert!(!has_label(root, "vesktop"));
    category.set_selected(
        loadouts::CATEGORIES
            .iter()
            .position(|c| *c == "Office & study")
            .unwrap() as u32,
    );
    assert!(has_label(root, "No packages match your search."));
    named_button(root, "Reset filters").emit_clicked();
    assert!(!has_label(root, "No packages match your search."));
    assert_eq!(category.selected(), 0);
    assert!(
        walk(root)
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::Label>())
            .any(|l| l.has_css_class("package-page-number") && l.text().starts_with("1 /"))
    );
    window.close();
}

#[test]
#[ignore = "Isolated palette and banner visual verification"]
fn theme_gallery() {
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("io.github.archforge.ThemeGallery")
        .build();
    app.register(None::<&gtk::gio::Cancellable>).unwrap();
    build(&app);
    let window = app.active_window().unwrap();
    let root = window.upcast_ref::<gtk::Widget>();
    wait_for(|| named_button(root, "Overview").is_sensitive());
    let start = Instant::now();
    wait_for(|| start.elapsed() > Duration::from_secs(3));
    let menu = walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::MenuButton>().ok())
        .find(|m| m.has_css_class("appearance-button"))
        .unwrap();
    let palettes = menu.popover().unwrap();
    for (id, name) in theme::OPTIONS {
        menu.popup();
        named_button(palettes.upcast_ref(), name).emit_clicked();
        menu.popdown();
        assert!(named_button(palettes.upcast_ref(), name).has_css_class("chosen"));
        assert_eq!(
            std::fs::read_to_string(
                std::path::PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap())
                    .join("archforge/appearance")
            )
            .unwrap(),
            id
        );
        capture(&format!("theme-{id}"));
        let stack = walk(root)
            .into_iter()
            .find_map(|w| w.downcast::<gtk::Stack>().ok())
            .unwrap();
        let overview = stack
            .child_by_name("overview")
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        assert!(
            overview.vadjustment().upper() <= overview.vadjustment().page_size() + 1.,
            "Overview must fit without scrolling for {id}: {} / {}",
            overview.vadjustment().upper(),
            overview.vadjustment().page_size()
        );
    }
    menu.popup();
    named_button(palettes.upcast_ref(), "Crimson").emit_clicked();
    menu.popdown();
    let hover = named_button(root, "Generate system report");
    hover.set_state_flags(gtk::StateFlags::PRELIGHT, false);
    capture("theme-crimson-hover");
    hover.unset_state_flags(gtk::StateFlags::PRELIGHT);
    for name in [
        "Tools", "Cleanup", "Ricing", "Packages", "Loadouts", "Wiki", "Settings",
    ] {
        named_button(root, name).emit_clicked();
        assert!(
            walk(root)
                .iter()
                .any(|w| w.has_css_class("forge-banner") && w.is_mapped())
        );
        capture(&format!("theme-crimson-{}", name.to_lowercase()));
    }
    window.close();
}
