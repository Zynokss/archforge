use super::*;
use std::cell::Cell;

fn heading(
    parent: &gtk::ApplicationWindow,
    title: &str,
    subtitle: &str,
    context: &str,
) -> gtk::Box {
    let p = column(18);
    p.add_css_class("content");
    p.set_vexpand(true);
    let top = row(20);
    top.add_css_class("page-header");
    top.set_valign(gtk::Align::Start);
    let text = column(0);
    text.set_hexpand(true);
    text.append(&label(&format!("Workspace  /  {context}"), "eyebrow"));
    text.append(&label(title, "page-title"));
    text.append(&wrapped(subtitle, "subtitle"));
    top.append(&text);
    let report = icon_job(
        parent,
        "Generate system report",
        "\u{e873}",
        Job::Report,
        true,
    );
    report.set_valign(gtk::Align::Center);
    top.append(&report);
    p.append(&top);
    p
}

fn title(text: &str, glyph: &str) -> gtk::Box {
    let r = row(12);
    r.add_css_class("section-heading");
    if !glyph.is_empty() {
        r.append(&symbol(glyph, "applications-system-symbolic"));
    }
    let l = label(text, "section-title");
    l.set_hexpand(true);
    r.append(&l);
    r
}

fn icon_job(
    parent: &gtk::ApplicationWindow,
    text: &str,
    glyph: &str,
    job: Job,
    primary: bool,
) -> gtk::Button {
    let b = job_button(parent, text, job, primary);
    let r = row(10);
    r.append(&symbol(glyph, "utilities-terminal-symbolic"));
    let l = label(text, "");
    l.set_hexpand(true);
    r.append(&l);
    b.set_child(Some(&r));
    b
}

fn split(main: &gtk::Box, aside: &gtk::Box) -> gtk::Box {
    let r = row(18);
    r.add_css_class("split-layout");
    main.set_hexpand(true);
    aside.set_width_request(330);
    aside.set_hexpand(false);
    r.set_vexpand(true);
    r.append(main);
    r.append(aside);
    r
}

fn note(glyph: &str, text: &str) -> gtk::Box {
    let r = row(12);
    r.add_css_class("notice");
    r.append(&symbol(glyph, "dialog-information-symbolic"));
    r.append(&wrapped(text, "muted"));
    r
}

fn footer(parts: &[(&str, &str, &str)]) -> gtk::Box {
    let r = row(0);
    r.add_css_class("footer");
    for (i, (glyph, name, value)) in parts.iter().enumerate() {
        if i > 0 {
            let sep = gtk::Separator::new(gtk::Orientation::Vertical);
            sep.set_margin_top(5);
            sep.set_margin_bottom(5);
            r.append(&sep);
        }
        let item = row(12);
        item.add_css_class("footer-item");
        item.set_hexpand(true);
        item.append(&symbol(glyph, "dialog-information-symbolic"));
        let text = column(3);
        text.append(&label(name, "tiny"));
        let value = label(value, "footer-value");
        value.set_ellipsize(gtk::pango::EllipsizeMode::End);
        value.set_max_width_chars(27);
        text.append(&value);
        item.append(&text);
        r.append(&item);
    }
    r
}

fn tool_row(
    parent: &gtk::ApplicationWindow,
    name: &str,
    description: &str,
    glyph: &str,
    actions: &[(&str, Job)],
) -> gtk::Box {
    let r = row(14);
    r.add_css_class("tool-row");
    let icon = symbol(glyph, "applications-system-symbolic");
    icon.add_css_class("tool-icon");
    r.append(&icon);
    let text = column(4);
    text.set_hexpand(true);
    text.append(&label(name, "row-title"));
    text.append(&wrapped(description, "row-sub"));
    r.append(&text);
    for (name, job) in actions {
        r.append(&job_button(parent, name, *job, job.destructive()));
    }
    r
}

fn info_row(name: &str, value: &str, glyph: &str) -> gtk::Box {
    let r = row(14);
    r.add_css_class("info-row");
    r.append(&symbol(glyph, "computer-symbolic"));
    let key = label(name, "muted");
    key.set_width_chars(11);
    r.append(&key);
    let value = wrapped(value, "row-title");
    value.set_selectable(true);
    value.set_hexpand(true);
    r.append(&value);
    r
}

fn guide_row(number: &str, name: &str, text: &str) -> gtk::Box {
    let r = row(12);
    r.set_margin_top(12);
    let n = label(number, "step-number");
    n.set_xalign(0.5);
    n.set_valign(gtk::Align::Start);
    r.append(&n);
    let c = column(3);
    c.append(&label(name, "row-title"));
    c.append(&wrapped(text, "row-sub"));
    r.append(&c);
    r
}

fn read_only_card(name: &str, description: &str, glyph: &str) -> gtk::Box {
    let r = row(12);
    r.add_css_class("tool-row");
    r.append(&symbol(glyph, "dialog-information-symbolic"));
    let c = column(4);
    c.append(&label(name, "row-title"));
    c.append(&wrapped(description, "row-sub"));
    r.append(&c);
    r
}

fn overview_jump(parent: &gtk::ApplicationWindow, destination: &str, capture: bool) {
    fn walk(w: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut out = vec![w.clone()];
        let mut c = w.first_child();
        while let Some(child) = c {
            out.extend(walk(&child));
            c = child.next_sibling();
        }
        out
    }
    for w in walk(parent.upcast_ref()) {
        if let Some(button) = w.downcast_ref::<gtk::Button>()
            && button.has_css_class("nav")
            && walk(w.upcast_ref()).iter().any(|c| {
                c.downcast_ref::<gtk::Label>()
                    .is_some_and(|l| l.text() == destination)
            })
        {
            button.emit_clicked();
            break;
        }
    }
    if capture {
        let loadouts = walk(parent.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Stack>().ok())
            .find_map(|stack| stack.child_by_name("loadouts"));
        if let Some(loadouts) = loadouts {
            for w in walk(&loadouts) {
                if let Some(button) = w.downcast_ref::<gtk::Button>()
                    && button.label().as_deref() == Some("Capture this system")
                {
                    button.emit_clicked();
                    break;
                }
            }
        }
    }
}
fn overview_link(
    parent: &gtk::ApplicationWindow,
    text: &str,
    destination: &'static str,
    primary: bool,
    capture: bool,
) -> gtk::Button {
    let b = button(text, primary);
    b.add_css_class("overview-shortcut");
    let owner = parent.downgrade();
    b.connect_clicked(move |_| {
        if let Some(parent) = owner.upgrade() {
            overview_jump(&parent, destination, capture);
        }
    });
    b
}
fn overview_gpu(gpu: &str) -> String {
    for prefix in ["Radeon", "GeForce", "Intel"] {
        if let Some(start) = gpu.find(prefix) {
            return gpu[start..]
                .split(['/', ']', '('])
                .next()
                .unwrap_or(gpu)
                .trim()
                .to_string();
        }
    }
    gpu.to_string()
}
pub(super) fn overview(
    parent: &gtk::ApplicationWindow,
    inv: &Inventory,
    metrics: Rc<RefCell<Metrics>>,
) -> gtk::Box {
    let p = heading(
        parent,
        "Your system, in focus.",
        "Performance, maintenance, and your next restore point.",
        "Overview",
    );
    p.add_css_class("overview-page");
    p.set_spacing(12);
    let refresh = button("↻", false);
    refresh.set_tooltip_text(Some("Refresh hardware and service status"));
    let owner = parent.downgrade();
    let readings = metrics.clone();
    refresh.connect_clicked(move |b| {
        b.set_sensitive(false);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(Inventory::load());
        });
        let owner = owner.clone();
        let readings = readings.clone();
        let refresh = b.downgrade();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let Some(parent) = owner.upgrade() else {
                return glib::ControlFlow::Break;
            };
            match rx.try_recv() {
                Ok(inventory) => {
                    fn stack(w: &gtk::Widget) -> Option<gtk::Stack> {
                        if let Some(s) = w.downcast_ref::<gtk::Stack>()
                            && s.child_by_name("overview").is_some()
                        {
                            return Some(s.clone());
                        }
                        let mut child = w.first_child();
                        while let Some(c) = child {
                            if let Some(found) = stack(&c) {
                                return Some(found);
                            }
                            child = c.next_sibling();
                        }
                        None
                    }
                    if let Some(stack) = stack(parent.upcast_ref()) {
                        crate::refresh_pages(&parent, &stack, &inventory, readings.clone());
                    }
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    if let Some(b) = refresh.upgrade() {
                        b.set_sensitive(true);
                        b.set_tooltip_text(Some("Refresh failed; click to retry"));
                    }
                    glib::ControlFlow::Break
                }
            }
        });
    });
    if let Some(top) = p.first_child().and_then(|w| w.downcast::<gtk::Box>().ok()) {
        top.append(&refresh);
    }

    let strip = row(22);
    strip.add_css_class("overview-strip");
    strip.append(&arch_logo("#48b7e8", 30));
    strip.append(&label("Arch Linux", "section-title"));
    strip.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    strip.append(&label(
        &format!(
            "{} · {}",
            inv.desktop,
            std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "Unknown session".into())
        ),
        "row-title",
    ));
    strip.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    let uptime = label("Uptime · …", "muted");
    uptime.set_hexpand(true);
    strip.append(&uptime);
    strip.append(&label("● Live", "scan-installed"));
    p.append(&strip);
    let cards = row(14);
    cards.add_css_class("metric-grid");
    cards.set_homogeneous(true);
    let mut values = Vec::new();
    let mut details = Vec::new();
    let mut bars = Vec::new();
    for (name, icon) in [
        ("CPU", "computer-symbolic"),
        ("Memory", "media-flash-symbolic"),
        ("Graphics", "video-display-symbolic"),
        ("Root storage", "drive-harddisk-symbolic"),
    ] {
        let card = column(12);
        card.add_css_class("overview-metric");
        card.set_hexpand(true);
        let top = row(10);
        top.append(&gtk::Image::from_icon_name(icon));
        top.append(&label(name, "row-title"));
        card.append(&top);
        let value = wrapped("…", "overview-metric-value");
        value.set_ellipsize(gtk::pango::EllipsizeMode::End);
        value.set_lines(2);
        card.append(&value);
        let detail = wrapped("…", "muted");
        detail.set_max_width_chars(30);
        card.append(&detail);
        let bar = gtk::ProgressBar::new();
        bar.set_visible(name != "Graphics");
        card.append(&bar);
        values.push(value);
        details.push(detail);
        bars.push(bar);
        cards.append(&card);
    }
    let gpu_name = overview_gpu(&inv.gpu);
    values[2].set_text(&gpu_name);
    values[2].set_tooltip_text(Some(&inv.gpu));
    values[2].add_css_class("overview-gpu-name");
    details[2].set_text("GPU utilization telemetry unavailable");
    p.append(&cards);
    let left = column(16);
    let right = column(16);
    let hardware = panel("panel");
    hardware.append(&title("System details", "\u{e873}"));
    for (name, value, glyph) in [
        ("Processor", inv.cpu.as_str(), "\u{e322}"),
        ("Graphics", gpu_name.as_str(), "\u{e30a}"),
        ("Kernel", inv.kernel.as_str(), "\u{eb8e}"),
        ("Desktop", inv.desktop.as_str(), "\u{e30c}"),
    ] {
        hardware.append(&info_row(name, value, glyph));
    }
    hardware.append(&info_row(
        "Session",
        &std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "Unknown".into()),
        "\u{e30c}",
    ));
    left.append(&hardware);
    let recovery = panel("panel");
    recovery.append(&title("Recovery readiness", "\u{e889}"));
    recovery.append(&wrapped(
        "Keep a package list and selected configs ready for your next installation.",
        "muted",
    ));
    recovery.append(&label("Capture and export a profile in Loadouts", "tiny"));
    let actions = row(12);
    actions.append(&overview_link(
        parent,
        "Capture this system",
        "Loadouts",
        true,
        true,
    ));
    actions.append(&overview_link(
        parent,
        "Open Loadouts",
        "Loadouts",
        false,
        false,
    ));
    recovery.append(&actions);
    recovery.append(&wrapped(
        "Store your exported profile on another drive. A loadout is not a full system backup.",
        "tiny",
    ));
    left.append(&recovery);
    let attention = panel("panel");
    attention.append(&title("Needs attention", "\u{e002}"));
    let service_label = inv.failed_services.map_or_else(
        || "Service status unavailable".into(),
        |n| {
            if n == 0 {
                "No failed services".into()
            } else {
                format!("{n} failed service(s) · Inspect")
            }
        },
    );
    attention.append(&icon_job(
        parent,
        &service_label,
        "\u{e8b9}",
        Job::Failed,
        false,
    ));
    attention.append(&icon_job(
        parent,
        "Check repository updates",
        "\u{e863}",
        Job::Updates,
        false,
    ));
    attention.append(&label(
        "Package updates are checked when you choose Check.",
        "tiny",
    ));
    right.append(&attention);
    let quick = panel("panel");
    quick.append(&title("Quick actions", "\u{e3e7}"));
    for (text, destination) in [
        ("Browse packages  ›", "Packages"),
        ("Review cleanup  ›", "Cleanup"),
        ("Run diagnostics  ›", "Diagnostics"),
        ("Explore ricing  ›", "Ricing"),
    ] {
        quick.append(&overview_link(parent, text, destination, false, false));
    }
    right.append(&quick);
    let columns = split(&left, &right);
    columns.add_css_class("overview-columns");
    columns.set_vexpand(false);
    right.set_width_request(540);
    p.append(&columns);
    let live_footer = row(16);
    live_footer.add_css_class("overview-live-footer");
    let live_text = label("Metrics refresh automatically", "tiny");
    live_text.set_hexpand(true);
    live_footer.append(&live_text);
    live_footer.append(&label(
        "Hardware and service details refresh on demand",
        "tiny",
    ));
    p.append(&live_footer);
    let update = move || {
        let m = metrics.borrow();
        values[0].set_text(
            &m.cpu
                .map_or_else(|| "Measuring…".into(), |v| format!("{v:.0}%")),
        );
        details[0].set_text(&m.temperature.map_or_else(
            || "CPU temperature unavailable".into(),
            |v| format!("{v:.0} °C"),
        ));
        bars[0].set_fraction(m.cpu.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0);
        values[1].set_text(&format!(
            "{:.1} GiB",
            m.used_memory as f64 / 1_073_741_824.0
        ));
        details[1].set_text(&format!(
            "of {} · {:.0}%",
            system::bytes(m.total_memory),
            m.memory
        ));
        bars[1].set_fraction((m.memory / 100.0).clamp(0.0, 1.0));
        let ratio = if m.total_disk > 0 {
            m.used_disk as f64 / m.total_disk as f64
        } else {
            0.0
        };
        values[3].set_text(&format!("{:.0}%", ratio * 100.0));
        details[3].set_text(&format!(
            "{} / {}\n{} free",
            system::bytes(m.used_disk),
            system::bytes(m.total_disk),
            system::bytes(m.total_disk.saturating_sub(m.used_disk))
        ));
        bars[3].set_fraction(ratio.clamp(0.0, 1.0));
        uptime.set_text(&format!(
            "Uptime · {}h {}m",
            m.uptime / 3600,
            m.uptime % 3600 / 60
        ));
    };
    update();
    let weak = p.downgrade();
    glib::timeout_add_local(Duration::from_secs(2), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        update();
        glib::ControlFlow::Continue
    });
    p
}

pub(super) fn gaming(parent: &gtk::ApplicationWindow, inv: &Inventory) -> gtk::Box {
    let p = heading(
        parent,
        "Make room for play.",
        "Inspect your gaming stack and get the right tools in place.",
        "Gaming",
    );
    let launch = panel("panel");
    let top = title("One launch option. Your tools, together.", "");
    let missing: Vec<_> = [("gamemode", "GameMode"), ("mangohud", "MangoHud")]
        .into_iter()
        .filter(|(pkg, _)| !inv.installed(pkg) && !inv.unmanaged_tool(pkg))
        .map(|(_, name)| name)
        .collect();
    if !missing.is_empty() {
        let warning = wrapped(
            &format!(
                "Install {} before using this command.",
                missing.join(" and ")
            ),
            "warning",
        );
        top.append(&warning);
    }
    launch.append(&top);
    let command = row(12);
    command.add_css_class("command-toolbar");
    let code = label("gamemoderun mangohud %command%", "command");
    code.set_selectable(true);
    code.set_hexpand(true);
    command.append(&code);
    let copy = button("Copy launch option", true);
    copy.connect_clicked(|b| {
        copy_text("gamemoderun mangohud %command%");
        b.set_label("Copied");
        let b = b.clone();
        glib::timeout_add_local_once(Duration::from_secs(2), move || {
            b.set_label("Copy launch option")
        });
    });
    command.append(&copy);
    launch.append(&command);
    p.append(&launch);
    let stack = panel("panel");
    stack.add_css_class("gaming-stack");
    let top = title("Your gaming stack", "");
    let installed = [
        "steam",
        "gamemode",
        "mangohud",
        "vulkan-tools",
        "gamescope",
        "lutris",
    ]
    .iter()
    .filter(|pkg| inv.installed(pkg) || inv.unmanaged_tool(pkg))
    .count();
    top.append(&label(&format!("{installed} of 6 detected"), "badge"));
    stack.append(&top);
    for (name, pkg, glyph, description) in [
        (
            "Steam",
            "steam",
            "\u{ea28}",
            "Game library and Proton compatibility tools",
        ),
        (
            "GameMode",
            "gamemode",
            "\u{e9e4}",
            "Performance optimization requests while you play",
        ),
        (
            "MangoHud",
            "mangohud",
            "\u{e26b}",
            "In-game hardware performance overlay",
        ),
        (
            "Vulkan tools",
            "vulkan-tools",
            "\u{e322}",
            "Inspect graphics drivers and Vulkan support",
        ),
        (
            "Gamescope",
            "gamescope",
            "\u{e30c}",
            "Game scaling and display control",
        ),
        (
            "Lutris",
            "lutris",
            "\u{ea28}",
            "Manage games across multiple runners",
        ),
    ] {
        let r = row(14);
        r.add_css_class("tool-row");
        r.append(&symbol(glyph, "applications-games-symbolic"));
        let text = column(3);
        text.set_hexpand(true);
        text.append(&label(name, "row-title"));
        text.append(&wrapped(description, "row-sub"));
        r.append(&text);
        let unmanaged = inv.unmanaged_tool(pkg);
        let has = inv.installed(pkg) || unmanaged;
        let state = label(
            if inv.error.is_some() {
                "Unknown"
            } else if unmanaged {
                "Executable found"
            } else if has {
                "Installed"
            } else {
                "Not installed"
            },
            if has { "installed" } else { "muted" },
        );
        r.append(&state);
        let b = button(
            if has { "Configure" } else { "Install" },
            !has && pkg == "gamemode",
        );
        let parent = parent.downgrade();
        b.connect_clicked(move |_| {
            if let Some(parent) = parent.upgrade() {
                show_setup(&parent, name, pkg, has);
            }
        });
        r.append(&b);
        stack.append(&r);
    }
    let aside = column(16);
    let checks = panel("panel");
    checks.append(&title("Quick checks", ""));
    checks.append(&tool_row(
        parent,
        "Inspect Vulkan support",
        "Check Vulkan devices and driver support.",
        "\u{e30c}",
        &[("Inspect", Job::Vulkan)],
    ));
    checks.append(&tool_row(
        parent,
        "Test GameMode",
        "Verify that GameMode works on your system.",
        "\u{e9e4}",
        &[("Test", Job::GameMode)],
    ));
    aside.append(&checks);
    let guide = panel("panel");
    guide.append(&title("Steam launch options guide", "\u{e873}"));
    guide.append(&guide_row(
        "1",
        "Open game properties",
        "In your Steam library, right-click a game and select Properties.",
    ));
    guide.append(&guide_row(
        "2",
        "Select Launch Options",
        "Find the Launch Options field in the General tab.",
    ));
    guide.append(&guide_row(
        "3",
        "Paste your command",
        "Paste the launch option above and close the window.",
    ));
    aside.append(&guide);
    p.append(&split(&stack, &aside));
    p.append(&footer(&[
        ("\u{ea28}", "Gaming environment", "Tools for Linux gaming"),
        (
            "\u{e8f1}",
            "Detected tools",
            &format!("{installed} of 6 detected"),
        ),
        (
            "\u{e1a1}",
            "Available tools",
            &format!("{} not installed", 6 - installed),
        ),
    ]));
    p
}

pub(super) fn cleanup(parent: &gtk::ApplicationWindow) -> gtk::Box {
    let p = heading(
        parent,
        "Less clutter. More space.",
        "Understand what is taking up room before removing anything.",
        "Cleanup",
    );
    p.append(&note(
        "\u{e88e}",
        "Cleanup runs only when you choose. Review candidates before removing them.",
    ));
    p.append(&crate::cleanup_tools::panel(parent));
    let main = column(16);
    let cache = panel("panel");
    let top = title("Package cache", "\u{e2c7}");
    top.append(&label("Keeps latest 2 versions", "badge"));
    cache.append(&top);
    cache.append(&label(
        "Directories configured in pacman.conf",
        "path-label",
    ));
    cache.append(&wrapped(
        "Older package versions can be useful for downgrades.",
        "subtitle",
    ));
    let size = read_only_card(
        "Cache size",
        "Measure to see disk usage. Protected folders may require authentication.",
        "\u{e1db}",
    );
    let measurement = label("Not measured", "cache-size-summary");
    measurement.set_wrap(true);
    measurement.set_max_width_chars(24);
    measurement.add_css_class("row-title");
    size.append(&measurement);
    cache.append(&size);
    cache.append(&tool_row(
        parent,
        "Measure cache",
        "Check disk usage of downloaded packages.",
        "\u{e922}",
        &[("Measure", Job::CacheSize)],
    ));
    cache.append(&tool_row(
        parent,
        "Review older versions",
        "Preview cleanup candidates with paccache.",
        "\u{e873}",
        &[("Preview", Job::CachePreview)],
    ));
    cache.append(&tool_row(
        parent,
        "Clean older versions",
        "Remove older cached packages, keeping 2 versions.",
        "\u{e872}",
        &[("Review & clean", Job::CacheClean)],
    ));
    cache.append(&tool_row(
        parent,
        "Uninstalled package archives",
        "Remove every cached version of packages that are no longer installed.",
        "\u{e872}",
        &[
            ("Preview", Job::UninstalledPreview),
            ("Review & clean", Job::UninstalledClean),
        ],
    ));
    main.append(&cache);
    let logs = panel("panel");
    logs.append(&title("Logs & unused dependencies", "\u{e8ef}"));
    logs.append(&tool_row(
        parent,
        "System journal",
        "Measure stored logs or remove archived files older than 7 days.",
        "\u{e873}",
        &[
            ("Measure", Job::JournalSize),
            ("Review & clean", Job::JournalClean),
        ],
    ));
    logs.append(&tool_row(
        parent,
        "Orphan packages",
        "Inspect dependencies no longer required by installed packages.",
        "\u{e1a1}",
        &[("Inspect", Job::Orphans)],
    ));
    fn cleanup_row(
        parent: &gtk::ApplicationWindow,
        action: cleanups::Action,
        description: &str,
    ) -> gtk::Box {
        let r = row(12);
        r.add_css_class("tool-row");
        let text = column(3);
        text.set_hexpand(true);
        text.append(&label(action.title(), "row-title"));
        text.append(&wrapped(description, "row-sub"));
        r.append(&text);
        let b = button("Review", false);
        let parent = parent.downgrade();
        b.connect_clicked(move |_| {
            if let Some(parent) = parent.upgrade() {
                cleanups::show(&parent, action);
            }
        });
        r.append(&b);
        r
    }
    logs.append(&cleanup_row(
        parent,
        cleanups::Action::Orphans,
        "Preview orphan dependencies, then review pacman's removal transaction in a terminal.",
    ));
    main.append(&logs);
    let personal = panel("panel");
    personal.append(&title("Personal caches & Trash", "\u{e2c7}"));
    for (action, description) in [
        (
            cleanups::Action::Trash,
            "Permanently empty your home Trash, including its metadata.",
        ),
        (
            cleanups::Action::YayCache,
            "Remove reviewed contents of your default yay build cache.",
        ),
        (
            cleanups::Action::PipCache,
            "Remove downloaded Python package cache files.",
        ),
        (
            cleanups::Action::Thumbnails,
            "Clear thumbnails; apps regenerate them when needed.",
        ),
        (
            cleanups::Action::Aur,
            "Let yay review AUR cache cleanup as your regular user.",
        ),
    ] {
        personal.append(&cleanup_row(parent, action, description));
    }
    main.append(&personal);
    let aside = column(16);
    let policy = panel("panel");
    policy.append(&title("Cleanup policy", "\u{e8b8}"));
    policy.append(&info_row("Packages", "2 latest versions", "\u{e53b}"));
    policy.append(&info_row("Journals", "7 days", "\u{e873}"));
    policy.append(&wrapped(
        "Only archived journal files older than 7 days are eligible for removal.",
        "subtitle",
    ));
    policy.append(&read_only_card(
        "You stay in control",
        "Every cleanup starts with a review.",
        "\u{e8e8}",
    ));
    aside.append(&policy);
    let before = panel("panel");
    before.append(&title("Before you clean", "\u{e873}"));
    before.append(&tool_row(
        parent,
        "Preview candidates",
        "See which cached package versions would be removed.",
        "\u{e8b6}",
        &[("Preview", Job::CachePreview)],
    ));
    before.append(&tool_row(
        parent,
        "Review orphan dependencies",
        "Check unused packages before removal.",
        "\u{e1a1}",
        &[("Inspect", Job::Orphans)],
    ));
    aside.append(&before);
    p.append(&split(&main, &aside));
    p.append(&footer(&[
        ("\u{e869}", "Manual cleanup", "You choose what to remove"),
        ("\u{e53b}", "Package retention", "2 latest versions"),
        (
            "\u{e873}",
            "Journal retention",
            "7 days, archived files only",
        ),
    ]));
    p
}

pub(super) fn packages(parent: &gtk::ApplicationWindow, inv: &Inventory) -> gtk::Box {
    let p = heading(
        parent,
        "Know what is installed.",
        "Search your local package database. Package versions come directly from pacman.",
        "Packages",
    );
    let actions = row(12);
    actions.append(&icon_job(
        parent,
        "Check updates",
        "\u{e863}",
        Job::Updates,
        true,
    ));
    actions.append(&icon_job(
        parent,
        "Inspect foreign packages",
        "\u{e1a1}",
        Job::Foreign,
        false,
    ));
    p.append(&actions);
    let table = panel("panel");
    table.set_vexpand(true);
    let toolbar = row(14);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search installed packages…")
        .hexpand(true)
        .build();
    toolbar.append(&search);
    let sort = gtk::DropDown::from_strings(&["Name A–Z", "Name Z–A"]);
    sort.set_valign(gtk::Align::Center);
    toolbar.append(&sort);
    table.append(&toolbar);
    let count = label("Loading local database…", "muted");
    count.set_margin_top(12);
    count.set_margin_bottom(14);
    table.append(&count);
    let columns = row(16);
    columns.add_css_class("table-columns");
    let name = label("PACKAGE", "tiny");
    name.set_hexpand(true);
    columns.append(&name);
    let version = label("INSTALLED VERSION", "tiny");
    version.set_width_chars(28);
    columns.append(&version);
    table.append(&columns);
    let list = column(0);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .min_content_height(310)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    table.append(&scroll);
    table.append(&note(
        "\u{e88e}",
        "Search to narrow down the list. Results are limited to 80 packages.",
    ));
    p.append(&table);
    let packages = Rc::new(inv.packages.clone());
    let query = Rc::new(RefCell::new(String::new()));
    let descending = Rc::new(RefCell::new(false));
    let draw: Rc<dyn Fn()> = Rc::new({
        let query = query.clone();
        let descending = descending.clone();
        move || {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let query = query.borrow();
            let mut matching: Vec<_> = packages
                .iter()
                .filter(|pkg| pkg.name.contains(query.as_str()))
                .collect();
            matching.sort_by(|a, b| a.name.cmp(&b.name));
            if *descending.borrow() {
                matching.reverse();
            }
            count.set_text(&format!(
                "{} matching packages · Showing first {} results",
                matching.len(),
                matching.len().min(80)
            ));
            if matching.is_empty() {
                list.append(&label("No packages match your search.", "muted"));
            }
            for pkg in matching.iter().take(80) {
                let r = row(12);
                r.add_css_class("package-row");
                r.append(&symbol("\u{e1a1}", "package-x-generic-symbolic"));
                let name = label(&pkg.name, "row-title");
                name.set_hexpand(true);
                name.set_ellipsize(gtk::pango::EllipsizeMode::End);
                name.set_tooltip_text(Some(&pkg.name));
                name.set_selectable(true);
                r.append(&name);
                let version = label(&pkg.version, "mono");
                version.set_width_chars(28);
                version.set_max_width_chars(28);
                version.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                version.set_tooltip_text(Some(&pkg.version));
                version.set_selectable(true);
                r.append(&version);
                list.append(&r);
            }
        }
    });
    draw();
    let render = draw.clone();
    search.connect_search_changed(move |entry| {
        *query.borrow_mut() = entry.text().to_lowercase();
        render();
    });
    sort.connect_selected_notify(move |s| {
        *descending.borrow_mut() = s.selected() == 1;
        draw();
    });
    let controllers = parent.observe_controllers();
    let old: Vec<_> = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .filter_map(|w| w.downcast::<gtk::EventController>().ok())
        .filter(|c| c.name().as_deref() == Some("archforge-package-search"))
        .collect();
    for controller in old {
        parent.remove_controller(&controller);
    }
    let controller = gtk::EventControllerKey::new();
    controller.set_name(Some("archforge-package-search"));
    let entry = search.downgrade();
    let page = p.downgrade();
    controller.connect_key_pressed(move |_, key, _, mods| {
        if key == gtk::gdk::Key::f
            && mods.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            && page.upgrade().is_some_and(|p| p.is_mapped())
        {
            if let Some(entry) = entry.upgrade() {
                entry.grab_focus();
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    parent.add_controller(controller);
    p.append(&footer(&[
        (
            "\u{e8f1}",
            "Local package database",
            &format!("{} installed packages", inv.packages.len()),
        ),
        ("\u{e1db}", "Source", "pacman"),
    ]));
    p
}

#[derive(Clone)]
struct DiagnosticFindings {
    panel: gtk::Box,
    badge: gtk::Label,
    title: gtk::Label,
    detail: gtk::Label,
    facts: gtk::Box,
}
impl DiagnosticFindings {
    fn clear(&self, busy: bool) {
        while let Some(child) = self.facts.first_child() {
            self.facts.remove(&child);
        }
        self.tone(if busy { "busy" } else { "ready" });
        self.badge.set_text(if busy {
            "READING SYSTEM"
        } else {
            "READY TO INSPECT"
        });
        self.title.set_text(if busy {
            "Collecting findings…"
        } else {
            "Your key findings"
        });
        self.detail.set_text(if busy {
            "Waiting for this check to finish. Previous results have been cleared."
        } else {
            "Run a check to see its main results here, alongside the full command output."
        });
    }
    fn tone(&self, tone: &str) {
        for name in ["busy", "ready", "success", "warning", "error"] {
            self.panel.remove_css_class(name);
        }
        self.panel.add_css_class(tone);
    }
    fn show(&self, job: Job, result: &Result<String, String>) {
        let findings = crate::diagnostic_results::summarize(job, result);
        while let Some(child) = self.facts.first_child() {
            self.facts.remove(&child);
        }
        self.tone(findings.tone);
        self.badge.set_text(match findings.tone {
            "error" => "COMMAND ERROR",
            "warning" => "REVIEW FINDINGS",
            _ => "CHECK COMPLETE",
        });
        self.title.set_text(findings.title);
        self.detail.set_text(findings.detail);
        for finding in findings.facts {
            let card = row(12);
            card.add_css_class("finding-card");
            let icon = gtk::Image::from_icon_name(finding.icon);
            icon.set_pixel_size(22);
            card.append(&icon);
            let text = column(5);
            text.set_hexpand(true);
            text.append(&label(finding.caption, "tiny"));
            let value = wrapped(&finding.value, "finding-value");
            value.set_max_width_chars(26);
            value.set_selectable(true);
            if finding.caption.starts_with("Failed")
                && finding.value.parse::<usize>().is_ok_and(|count| count > 0)
            {
                value.add_css_class("finding-attention");
            }
            text.append(&value);
            card.append(&text);
            self.facts.append(&card);
        }
    }
}

fn diagnostic_action(
    parent: &gtk::ApplicationWindow,
    name: &str,
    description: &str,
    glyph: &str,
    button_text: &str,
    job: Job,
    output: (
        &gtk::TextView,
        &gtk::Label,
        &gtk::Button,
        &Rc<Cell<u64>>,
        &DiagnosticFindings,
    ),
) -> gtk::Box {
    let (view, status, copy, generation, findings) = output;
    let findings = findings.clone();
    let r = row(14);
    r.add_css_class("tool-row");
    let icon = symbol(glyph, "utilities-terminal-symbolic");
    icon.add_css_class("tool-icon");
    r.append(&icon);
    let c = column(3);
    c.set_hexpand(true);
    c.append(&label(name, "row-title"));
    c.append(&wrapped(description, "row-sub"));
    r.append(&c);
    let b = button(button_text, false);
    let view = view.clone();
    let status = status.clone();
    let copy = copy.clone();
    let generation = generation.clone();
    let busy_parent = parent.downgrade();
    b.connect_clicked(move |b| {
        let request = generation.get().wrapping_add(1);
        generation.set(request);
        findings.clear(true);
        status.set_text(&format!("Running {}…", job.title()));
        view.buffer()
            .set_text(&format!("$ {}\n\nRunning…", job.display_command()));
        copy.set_sensitive(false);
        b.set_sensitive(false);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(job.execute());
        });
        let view = view.clone();
        let status = status.clone();
        let copy = copy.clone();
        let button = b.clone();
        let parent = busy_parent.clone();
        let generation = generation.clone();
        let findings = findings.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            if parent.upgrade().is_none() {
                return glib::ControlFlow::Break;
            }
            match rx.try_recv() {
                Ok(result) => {
                    button.set_sensitive(true);
                    if generation.get() != request {
                        return glib::ControlFlow::Break;
                    }
                    findings.show(job, &result);
                    let (message, text) = match result {
                        Ok(text) => ("Completed", text),
                        Err(e) => ("Could not complete", e),
                    };
                    status.set_text(message);
                    view.buffer()
                        .set_text(&format!("$ {}\n\n{}", job.display_command(), text));
                    copy.set_sensitive(true);
                    button.set_sensitive(true);
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    if generation.get() == request {
                        status.set_text("Worker disconnected");
                        let error = "The diagnostic worker stopped unexpectedly. Please run the check again.".to_string();
                        findings.show(job, &Err(error.clone()));
                        view.buffer().set_text(&error);
                        copy.set_sensitive(true);
                    }
                    button.set_sensitive(true);
                    glib::ControlFlow::Break
                }
            }
        });
    });
    r.append(&b);
    r
}

pub(super) fn diagnostics(parent: &gtk::ApplicationWindow, inv: &Inventory) -> gtk::Box {
    let p = heading(
        parent,
        "Get to the root of it.",
        "Useful diagnostics with readable output you can copy and share.",
        "Diagnostics",
    );
    let view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(16)
        .right_margin(16)
        .top_margin(14)
        .bottom_margin(14)
        .build();
    view.add_css_class("console");
    view.add_css_class("diagnostic-console");
    crate::dialogs::style_output(&view.buffer());
    view.buffer().set_text(
        "$\n\nReady when you are\n\nRun an inspection above to view readable results here.",
    );
    let copy = button("Copy output", false);
    copy.set_sensitive(false);
    let v = view.clone();
    copy.connect_clicked(move |_| {
        let b = v.buffer();
        copy_text(&b.text(&b.start_iter(), &b.end_iter(), false));
    });
    let status = label("Ready when you are", "muted");
    let finding_panel = panel("panel");
    finding_panel.add_css_class("diagnostic-findings");
    let badge = label("", "finding-badge");
    let finding_title = wrapped("", "finding-title");
    let finding_detail = wrapped("", "muted");
    finding_detail.set_max_width_chars(36);
    let facts = column(9);
    finding_panel.append(&badge);
    finding_panel.append(&finding_title);
    finding_panel.append(&finding_detail);
    finding_panel.append(&facts);
    let findings = DiagnosticFindings {
        panel: finding_panel,
        badge,
        title: finding_title,
        detail: finding_detail,
        facts,
    };
    findings.clear(false);
    let generation = Rc::new(Cell::new(0_u64));
    let main = column(16);
    let checks = panel("panel");
    checks.append(&title("Inspect your system", "\u{e8b9}"));
    let check_grid = gtk::Grid::builder()
        .column_homogeneous(true)
        .column_spacing(8)
        .row_spacing(8)
        .build();
    for (index, (name, desc, glyph, action, job)) in [
        (
            "Failed services",
            "Inspect units that failed in the current boot.",
            "\u{e8b9}",
            "Inspect",
            Job::Failed,
        ),
        (
            "Boot performance",
            "See how long system services took to start.",
            "\u{e9e4}",
            "Analyze",
            Job::Boot,
        ),
        (
            "Vulkan devices",
            "Inspect Vulkan implementations, drivers, and detected GPUs.",
            "\u{e322}",
            "Inspect",
            Job::Vulkan,
        ),
        (
            "System report",
            "Collect hardware, memory, kernel, and failed service details.",
            "\u{e873}",
            "Generate",
            Job::Report,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let action = diagnostic_action(
            parent,
            name,
            desc,
            glyph,
            action,
            job,
            (&view, &status, &copy, &generation, &findings),
        );
        check_grid.attach(&action, (index % 2) as i32, (index / 2) as i32, 1, 1);
    }
    checks.append(&check_grid);
    main.append(&checks);
    let output = panel("panel");
    output.add_css_class("diagnostic-output");
    output.set_vexpand(true);
    let top = title("Command output", "\u{eb8e}");
    top.append(&copy);
    output.append(&top);
    output.append(&crate::command_output::pane(&view, None, true));
    output.append(&status);
    main.append(&output);
    let aside = column(16);
    aside.append(&findings.panel);
    let install = panel("panel");
    install.append(&title("Installation", "\u{e161}"));
    for (name, value) in [
        ("Hostname", inv.hostname.as_str()),
        ("Distribution", "Arch Linux · rolling release"),
        ("Desktop / compositor", inv.desktop.as_str()),
    ] {
        let r = column(5);
        r.add_css_class("tool-row");
        r.append(&label(name, "tiny"));
        r.append(&wrapped(value, "row-title"));
        install.append(&r);
    }
    install.append(&read_only_card(
        "Share useful details",
        "Generate a system report, review its contents, then copy its output when asking for help.",
        "\u{e873}",
    ));
    let installation = gtk::Expander::new(Some("Installation details"));
    installation.add_css_class("diagnostic-installation");
    installation.set_child(Some(&install));
    aside.append(&installation);
    p.append(&split(&main, &aside));
    p.append(&footer(&[
        ("\u{eb8e}", "Readable output", "Diagnostics in plain text"),
        ("\u{e14d}", "Copy and share", "Review output before sharing"),
        (
            "\u{e88e}",
            "System reports",
            "May contain identifying details",
        ),
    ]));
    p
}
