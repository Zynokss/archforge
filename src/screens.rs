use super::*;
use std::cell::Cell;
use std::collections::VecDeque;

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

#[derive(Clone)]
struct MetricCard {
    root: gtk::Box,
    value: gtk::Label,
    detail: gtk::Label,
    chart: gtk::DrawingArea,
    gauge: gtk::DrawingArea,
    history: Rc<RefCell<VecDeque<f64>>>,
    current: Rc<RefCell<Option<f64>>>,
}
impl MetricCard {
    fn new(name: &str, glyph: &str, axis: &[&str; 3]) -> Self {
        let root = panel("metric");
        root.set_hexpand(true);
        root.set_vexpand(false);
        root.set_height_request(260);
        root.append(&label(name, "metric-label"));
        let reading = row(18);
        reading.set_margin_top(16);
        let current = Rc::new(RefCell::new(None::<f64>));
        let gauge = gtk::DrawingArea::builder()
            .width_request(74)
            .height_request(74)
            .build();
        gauge.add_css_class("gauge");
        let level = current.clone();
        let cpu = name == "Processor";
        gauge.set_draw_func(move |area, cr, w, h| {
            let (x, y) = (w as f64 / 2.0, h as f64 / 2.0);
            let radius = (x.min(y) - 5.0).max(1.0);
            let c = area.color();
            cr.set_line_width(if cpu { 7.0 } else { 2.0 });
            cr.set_source_rgba(0.7, 0.75, 0.8, 0.10);
            cr.arc(x, y, radius, 0.0, std::f64::consts::TAU);
            let _ = cr.stroke();
            if cpu && let Some(percent) = *level.borrow() {
                cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, 1.0);
                cr.set_line_cap(gtk::cairo::LineCap::Round);
                cr.arc(
                    x,
                    y,
                    radius,
                    -std::f64::consts::FRAC_PI_2,
                    -std::f64::consts::FRAC_PI_2
                        + percent.clamp(0.0, 100.0) / 100.0 * std::f64::consts::TAU,
                );
                let _ = cr.stroke();
            }
        });
        let icon = symbol(glyph, "utilities-system-monitor-symbolic");
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        icon.add_css_class("metric-icon");
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&gauge));
        overlay.add_overlay(&icon);
        reading.append(&overlay);
        let values = column(4);
        values.set_valign(gtk::Align::Center);
        let value = label("…", "metric-value");
        let detail = label("Waiting for a sample", "metric-detail");
        values.append(&value);
        values.append(&detail);
        reading.append(&values);
        root.append(&reading);
        let plot = row(8);
        plot.set_margin_top(14);
        let ticks = column(0);
        ticks.add_css_class("axis");
        ticks.set_width_request(30);
        for (i, text) in axis.iter().enumerate() {
            let l = label(text, "axis-label");
            if i == 1 {
                l.set_vexpand(true);
                l.set_valign(gtk::Align::Center);
            }
            ticks.append(&l);
        }
        plot.append(&ticks);
        let chart = gtk::DrawingArea::builder()
            .height_request(70)
            .hexpand(true)
            .build();
        chart.add_css_class("chart");
        let history = Rc::new(RefCell::new(VecDeque::<f64>::new()));
        let samples = history.clone();
        chart.set_draw_func(move |area, cr, width, height| {
            let (w, h) = (width as f64, height as f64);
            let c = area.color();
            cr.set_source_rgba(0.7, 0.75, 0.8, 0.12);
            cr.set_line_width(1.0);
            for y in [1.0, h / 2.0, h - 1.0] {
                cr.move_to(0.0, y);
                cr.line_to(w, y);
            }
            let _ = cr.stroke();
            let data = samples.borrow();
            if data.len() < 2 {
                return;
            }
            let points: Vec<_> = data
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    (
                        i as f64 / (data.len() - 1) as f64 * w,
                        h - 2.0 - v.clamp(0.0, 100.0) / 100.0 * (h - 4.0),
                    )
                })
                .collect();
            cr.move_to(0.0, h);
            for (x, y) in &points {
                cr.line_to(*x, *y);
            }
            cr.line_to(w, h);
            cr.close_path();
            let gradient = gtk::cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
            gradient.add_color_stop_rgba(
                0.0,
                c.red() as f64,
                c.green() as f64,
                c.blue() as f64,
                0.27,
            );
            gradient.add_color_stop_rgba(
                1.0,
                c.red() as f64,
                c.green() as f64,
                c.blue() as f64,
                0.02,
            );
            let _ = cr.set_source(&gradient);
            let _ = cr.fill();
            cr.move_to(points[0].0, points[0].1);
            for (x, y) in &points[1..] {
                cr.line_to(*x, *y);
            }
            cr.set_source_rgb(c.red() as f64, c.green() as f64, c.blue() as f64);
            cr.set_line_width(2.0);
            let _ = cr.stroke();
        });
        plot.append(&chart);
        root.append(&plot);
        Self {
            root,
            value,
            detail,
            chart,
            gauge,
            history,
            current,
        }
    }
    fn update(&self, value: &str, detail: &str, percent: Option<f64>) {
        self.value.set_text(value);
        self.detail.set_text(detail);
        *self.current.borrow_mut() = percent;
        if let Some(percent) = percent {
            let mut h = self.history.borrow_mut();
            if h.len() == 60 {
                h.pop_front();
            }
            h.push_back(percent);
            self.chart.set_tooltip_text(Some(&format!(
                "{} actual samples over {} seconds. Samples refresh every 2 seconds.",
                h.len(),
                h.len().saturating_sub(1) * 2
            )));
        }
        self.chart.queue_draw();
        self.gauge.queue_draw();
    }
}

pub(super) fn overview(
    parent: &gtk::ApplicationWindow,
    inv: &Inventory,
    metrics: Rc<RefCell<Metrics>>,
) -> gtk::Box {
    let p = heading(
        parent,
        "Your system, in focus.",
        "Live telemetry and the essentials of your Arch installation.",
        "Overview",
    );
    let cards = row(16);
    cards.set_vexpand(false);
    cards.add_css_class("metric-grid");
    cards.set_homogeneous(true);
    let cpu = MetricCard::new("Processor", "\u{e322}", &["100%", "50%", "0%"]);
    let total_gib = metrics.borrow().total_memory as f64 / 1_073_741_824.0;
    let max = format!("{total_gib:.1}");
    let mid = format!("{:.1}", total_gib / 2.0);
    let memory = MetricCard::new("Memory", "\u{e1db}", &[&max, &mid, "0 GiB"]);
    let disk = MetricCard::new("Root storage", "\u{e161}", &["100%", "50%", "0%"]);
    for c in [&cpu, &memory, &disk] {
        cards.append(&c.root);
    }
    p.append(&cards);
    let hardware = panel("panel");
    hardware.add_css_class("system-overview");
    hardware.append(&title("System overview", ""));
    hardware.append(&info_row("Processor", &inv.cpu, "\u{e322}"));
    hardware.append(&info_row("Graphics", &inv.gpu, "\u{e30a}"));
    hardware.append(&info_row("Kernel", &inv.kernel, "\u{eb8e}"));
    hardware.append(&info_row("Session", &inv.desktop, "\u{e30c}"));
    let maintenance = panel("panel");
    maintenance.add_css_class("maintenance");
    maintenance.append(&title("Maintenance", "\u{e869}"));
    maintenance.append(&wrapped(
        "A little maintenance goes a long way. Inspect first, then choose what changes.",
        "subtitle",
    ));
    for (text, glyph, job) in [
        ("Check repository updates", "\u{e863}", Job::Updates),
        ("Inspect failed services", "\u{e8ef}", Job::Failed),
        ("Generate system report", "\u{e873}", Job::Report),
    ] {
        let b = icon_job(parent, text, glyph, job, matches!(job, Job::Updates));
        b.add_css_class("action-tile");
        maintenance.append(&b);
    }
    p.append(&split(&hardware, &maintenance));
    let uptime = label("…", "footer-value");
    let temperature = label("…", "footer-value");
    let status = row(0);
    status.add_css_class("footer");
    for (glyph, name, value) in [
        ("\u{e8b5}", "Uptime", uptime.clone()),
        ("\u{f076}", "CPU temperature", temperature.clone()),
        (
            "\u{e30c}",
            "Session",
            label(
                &std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "Unknown".into()),
                "footer-value",
            ),
        ),
    ] {
        let item = row(12);
        item.add_css_class("footer-item");
        item.set_hexpand(true);
        item.append(&symbol(glyph, "computer-symbolic"));
        let c = column(3);
        c.append(&label(name, "tiny"));
        c.append(&value);
        item.append(&c);
        status.append(&item);
    }
    let services = icon_job(
        parent,
        &inv.failed_services.map_or_else(
            || "Service status unavailable".into(),
            |n| {
                if n == 0 {
                    "No failed services".into()
                } else {
                    format!("{n} failed services")
                }
            },
        ),
        "\u{e8b9}",
        Job::Failed,
        false,
    );
    status.append(&services);
    p.append(&status);
    let update = move || {
        let m = metrics.borrow();
        cpu.update(
            &m.cpu.map_or_else(|| "…".into(), |v| format!("{v:.0}%")),
            "Total utilization",
            m.cpu,
        );
        memory.update(
            &format!("{:.1} GiB", m.used_memory as f64 / 1_073_741_824.0),
            &format!("{:.0}% used of {}", m.memory, system::bytes(m.total_memory)),
            Some(m.memory),
        );
        let pct = if m.total_disk > 0 {
            m.used_disk as f64 / m.total_disk as f64 * 100.0
        } else {
            0.0
        };
        disk.update(
            &format!("{pct:.0}%"),
            &format!(
                "{} of {}",
                system::bytes(m.used_disk),
                system::bytes(m.total_disk)
            ),
            Some(pct),
        );
        uptime.set_text(&format!("{}h {}m", m.uptime / 3600, m.uptime % 3600 / 60));
        temperature.set_text(
            &m.temperature
                .map_or_else(|| "Unavailable".into(), |v| format!("{v:.0} °C")),
        );
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
        .filter(|(pkg, _)| !inv.installed(pkg))
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
    .filter(|pkg| inv.installed(pkg))
    .count();
    top.append(&label(&format!("{installed} of 6 installed"), "badge"));
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
        let has = inv.installed(pkg);
        let state = label(
            if inv.error.is_some() {
                "Unknown"
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
            "Installed tools",
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
    let main = column(16);
    let cache = panel("panel");
    let top = title("Package cache", "\u{e2c7}");
    top.append(&label("Keeps latest 3 versions", "badge"));
    cache.append(&top);
    cache.append(&label("/var/cache/pacman/pkg", "path-label"));
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
        "Remove older cached packages, keeping 3 versions.",
        "\u{e872}",
        &[("Review & clean", Job::CacheClean)],
    ));
    main.append(&cache);
    let logs = panel("panel");
    logs.append(&title("Logs & unused dependencies", "\u{e8ef}"));
    logs.append(&tool_row(
        parent,
        "System journal",
        "Measure stored logs or remove archived files older than 14 days.",
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
    main.append(&logs);
    let aside = column(16);
    let policy = panel("panel");
    policy.append(&title("Cleanup policy", "\u{e8b8}"));
    policy.append(&info_row("Packages", "3 latest versions", "\u{e53b}"));
    policy.append(&info_row("Journals", "14 days", "\u{e873}"));
    policy.append(&wrapped(
        "Only archived journal files older than 14 days are eligible for removal.",
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
        ("\u{e53b}", "Package retention", "3 latest versions"),
        (
            "\u{e873}",
            "Journal retention",
            "14 days, archived files only",
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
    let controller = gtk::EventControllerKey::new();
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

fn diagnostic_action(
    parent: &gtk::ApplicationWindow,
    name: &str,
    description: &str,
    glyph: &str,
    button_text: &str,
    job: Job,
    output: (&gtk::TextView, &gtk::Label, &gtk::Button, &Rc<Cell<u64>>),
) -> gtk::Box {
    let (view, status, copy, generation) = output;
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
    let generation = Rc::new(Cell::new(0_u64));
    let main = column(16);
    let checks = panel("panel");
    checks.append(&title("Inspect your system", "\u{e8b9}"));
    for (name, desc, glyph, action, job) in [
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
    ] {
        checks.append(&diagnostic_action(
            parent,
            name,
            desc,
            glyph,
            action,
            job,
            (&view, &status, &copy, &generation),
        ));
    }
    main.append(&checks);
    let output = panel("panel");
    let top = title("Diagnostic output", "\u{eb8e}");
    top.append(&copy);
    output.append(&top);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&view)
        .min_content_height(160)
        .vexpand(true)
        .build();
    output.append(&scroll);
    output.append(&status);
    main.append(&output);
    let aside = column(16);
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
    aside.append(&install);
    let available = panel("panel");
    available.append(&title("Available checks", "\u{e8ef}"));
    available.append(&read_only_card(
        "Services",
        "Inspect units that failed in the current boot.",
        "\u{e8b8}",
    ));
    available.append(&read_only_card(
        "Boot timing",
        "See how long system services took to start.",
        "\u{e9e4}",
    ));
    available.append(&read_only_card(
        "Graphics drivers",
        "Inspect Vulkan implementations, drivers, and detected GPUs.",
        "\u{e322}",
    ));
    aside.append(&available);
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
