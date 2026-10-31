mod jobs;
mod screens;
mod system;
mod terminal;
mod theme;
mod updater;
use screens::{cleanup, diagnostics, gaming, overview, packages};

use gtk::{glib, prelude::*};
use jobs::Job;
use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Duration};
use system::{Inventory, Metrics, Sampler};

const APP_ID: &str = "io.github.zynokss.ArchForge";

fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    if !class.is_empty() {
        l.add_css_class(class);
    }
    l
}
fn wrapped(text: &str, class: &str) -> gtk::Label {
    let l = label(text, class);
    l.set_wrap(true);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.set_max_width_chars(65);
    l
}
fn column(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}
fn row(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}
fn panel(class: &str) -> gtk::Box {
    let p = column(0);
    p.add_css_class(class);
    p
}
fn button(text: &str, primary: bool) -> gtk::Button {
    let b = gtk::Button::with_label(text);
    if primary {
        b.add_css_class("primary");
    }
    b.set_valign(gtk::Align::Center);
    b.set_hexpand(false);
    b
}

fn symbol(glyph: &str, fallback: &str) -> gtk::Widget {
    let probe = gtk::Label::new(None);
    if probe
        .pango_context()
        .list_families()
        .iter()
        .any(|f| f.name() == "Material Symbols Rounded")
    {
        let icon = label(glyph, "material-symbol");
        icon.set_accessible_role(gtk::AccessibleRole::Presentation);
        icon.upcast()
    } else {
        gtk::Image::from_icon_name(fallback).upcast()
    }
}

fn arch_logo(color: &str, size: i32) -> gtk::Image {
    let svg = include_str!("../data/arch-mark.svg").replace("#ff6467", color);
    let loader = gtk::gdk_pixbuf::PixbufLoader::with_type("svg").expect("SVG loader");
    loader.set_size(size, size);
    loader.write(svg.as_bytes()).expect("Valid bundled logo");
    loader.close().expect("Complete logo");
    let pixbuf = loader.pixbuf().expect("Decoded logo");
    let texture = gtk::gdk::MemoryTexture::new(
        pixbuf.width(),
        pixbuf.height(),
        gtk::gdk::MemoryFormat::R8g8b8a8,
        &pixbuf.read_pixel_bytes(),
        pixbuf.rowstride() as usize,
    );
    let image = gtk::Image::from_paintable(Some(&texture));
    image.set_pixel_size(size);
    image
}

fn adapt_layout(window: &gtk::ApplicationWindow) {
    let compact = window.width() > 0 && window.width() < 1280;
    if compact {
        window.add_css_class("compact");
    } else {
        window.remove_css_class("compact");
    }
    fn visit(w: &gtk::Widget, compact: bool) {
        if let Some(b) = w.downcast_ref::<gtk::Box>()
            && [
                "split-layout",
                "metric-grid",
                "page-header",
                "command-toolbar",
                "footer",
            ]
            .iter()
            .any(|class| b.has_css_class(class))
        {
            b.set_orientation(if compact {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            });
            if b.has_css_class("split-layout")
                && let Some(aside) = b.last_child()
            {
                aside.set_width_request(if compact { -1 } else { 330 });
            }
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            visit(&c, compact);
            child = c.next_sibling();
        }
    }
    visit(window.upcast_ref(), compact);
}

fn appearance_menu(provider: &gtk::CssProvider, chosen: Rc<RefCell<String>>) -> gtk::MenuButton {
    let menu = gtk::MenuButton::new();
    menu.add_css_class("appearance-button");
    let title = row(12);
    title.append(&symbol("\u{e3ae}", "applications-graphics-symbolic"));
    title.append(&label("Appearance", ""));
    menu.set_child(Some(&title));
    let popover = gtk::Popover::new();
    popover.set_position(gtk::PositionType::Right);
    let body = column(6);
    body.append(&label("Color palette", "section-title"));
    let description = wrapped(
        &theme::Palette::load(&chosen.borrow()).1,
        "palette-description",
    );
    description.set_max_width_chars(30);
    let choices = Rc::new(RefCell::new(Vec::<(String, gtk::Button)>::new()));
    for (id, name) in theme::OPTIONS {
        let b = gtk::Button::new();
        b.add_css_class("palette-choice");
        if *chosen.borrow() == id {
            b.add_css_class("chosen");
        }
        let content = row(12);
        let swatch = label("", "swatch");
        swatch.add_css_class(&format!("{id}-swatch"));
        content.append(&swatch);
        content.append(&label(name, ""));
        b.set_child(Some(&content));
        choices.borrow_mut().push((id.into(), b.clone()));
        let provider = provider.clone();
        let chosen = chosen.clone();
        let description = description.clone();
        let choices = choices.clone();
        b.connect_clicked(move |_| {
            *chosen.borrow_mut() = id.into();
            let mut message = theme::apply(&provider, id);
            if let Err(error) = theme::save(id) {
                message = format!("Applied, but could not save preference: {error}");
            }
            description.set_text(&message);
            for (other_id, other) in choices.borrow().iter() {
                if other_id == id {
                    other.add_css_class("chosen");
                } else {
                    other.remove_css_class("chosen");
                }
            }
        });
        body.append(&b);
    }
    body.append(&description);
    popover.set_child(Some(&body));
    menu.set_popover(Some(&popover));
    // Re-read only when the desktop's generated palette actually changes.
    let provider = provider.clone();
    let weak = menu.downgrade();
    let mut previous = String::new();
    glib::timeout_add_local(Duration::from_secs(5), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        if chosen.borrow().as_str() == "desktop" {
            let signature = theme::desktop_signature();
            if signature != previous {
                previous = signature;
                description.set_text(&theme::apply(&provider, "desktop"));
            }
        }
        glib::ControlFlow::Continue
    });
    menu
}

fn copy_text(text: &str) {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text(text);
    }
}

fn update_cache_summary(
    parent: &gtk::ApplicationWindow,
    job: Job,
    result: &Result<String, String>,
) {
    if !matches!(job, Job::CacheSize | Job::CacheSizeAdmin | Job::CacheClean) {
        return;
    }
    let text = if matches!(job, Job::CacheClean) {
        if result.is_ok() {
            "Cleaned. Measure again for current usage.".into()
        } else {
            return;
        }
    } else {
        let output = result
            .as_ref()
            .map(String::as_str)
            .unwrap_or_else(|e| e.as_str());
        let size = output.lines().find_map(|line| {
            let mut words = line.split_whitespace();
            let size = words.next()?;
            (words.next()? == "/var/cache/pacman/pkg").then_some(size)
        });
        match (result.is_ok(), size) {
            (true, Some(size)) => format!("{size} total"),
            (false, Some(size)) => format!("{size} partial · authentication needed"),
            _ => "Measurement unavailable. See command output.".into(),
        }
    };
    fn visit(w: &gtk::Widget, text: &str) {
        if w.has_css_class("cache-size-summary")
            && let Some(label) = w.downcast_ref::<gtk::Label>()
        {
            label.set_text(text);
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            visit(&c, text);
            child = c.next_sibling();
        }
    }
    visit(parent.upcast_ref(), &text);
}

fn show_job(parent: &gtk::ApplicationWindow, job: Job) {
    let dialog = gtk::Window::builder()
        .title(job.title())
        .transient_for(parent)
        .modal(true)
        .default_width(780)
        .default_height(520)
        .build();
    let body = column(16);
    body.set_margin_top(24);
    body.set_margin_bottom(24);
    body.set_margin_start(24);
    body.set_margin_end(24);
    body.append(&label(job.title(), "section-title"));
    let command = wrapped(&job.display_command(), "mono");
    command.set_selectable(true);
    body.append(&command);
    let status = wrapped(
        if job.privileged() {
            job.confirmation()
        } else {
            "Running diagnostic…"
        },
        "notice",
    );
    body.append(&status);
    let output = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(14)
        .right_margin(14)
        .top_margin(14)
        .bottom_margin(14)
        .build();
    output.add_css_class("console");
    let scroll = gtk::ScrolledWindow::builder()
        .child(&output)
        .vexpand(true)
        .build();
    body.append(&scroll);
    let controls = row(10);
    let copy = button("Copy output", false);
    copy.set_sensitive(false);
    controls.append(&copy);
    let copy_cmd = button("Copy command", false);
    copy_cmd.connect_clicked(move |_| copy_text(&job.display_command()));
    controls.append(&copy_cmd);
    let retry = button("Measure with authentication", true);
    let retry_parent = parent.downgrade();
    let previous_dialog = dialog.downgrade();
    retry.connect_clicked(move |_| {
        if let Some(previous) = previous_dialog.upgrade() {
            previous.close();
        }
        if let Some(parent) = retry_parent.upgrade() {
            show_job(&parent, Job::CacheSizeAdmin);
        }
    });
    retry.set_visible(false);
    controls.append(&retry);
    let spacer = row(0);
    spacer.set_hexpand(true);
    controls.append(&spacer);
    let execute = button("Authenticate & run", false);
    execute.add_css_class(if job.destructive() {
        "destructive"
    } else {
        "primary"
    });
    if job.privileged() {
        controls.append(&execute);
    }
    let close = button("Close", false);
    controls.append(&close);
    body.append(&controls);
    dialog.set_child(Some(&body));
    let d = dialog.downgrade();
    close.connect_clicked(move |_| {
        if let Some(d) = d.upgrade() {
            d.close();
        }
    });
    let view = output.clone();
    copy.connect_clicked(move |_| {
        let buffer = view.buffer();
        copy_text(&buffer.text(&buffer.start_iter(), &buffer.end_iter(), false));
    });
    let summary_parent = parent.downgrade();
    let start: Rc<dyn Fn()> = Rc::new(move || {
        status.set_text("Running… You can close this window; the operation will continue.");
        output.buffer().set_text("");
        copy.set_sensitive(false);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(job.execute());
        });
        let status = status.clone();
        let output = output.clone();
        let copy = copy.clone();
        let retry = retry.clone();
        let summary_parent = summary_parent.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
            Ok(result) => {
                if let Some(parent) = summary_parent.upgrade() {
                    update_cache_summary(&parent, job, &result);
                }
                let (message, data) = match result {
                    Ok(data) => (
                        "Completed",
                        if data.trim().is_empty() {
                            "Command completed without output.".into()
                        } else {
                            data
                        },
                    ),
                    Err(error) if job.cache_permission_error(&error) => {
                        retry.set_visible(true);
                        (
                            "Incomplete size: some pacman folders are protected. Authenticate to measure the full cache.",
                            format!(
                                "PARTIAL MEASUREMENT: the size below excludes unreadable folders.\n\n{error}"
                            ),
                        )
                    }
                    Err(error) => ("Could not complete. See the details below.", error),
                };
                status.set_text(message);
                output.buffer().set_text(&data);
                copy.set_sensitive(true);
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => {
                status.set_text("Worker disconnected.");
                glib::ControlFlow::Break
            }
        });
    });
    if job.privileged() {
        execute.connect_clicked(move |b| {
            b.set_sensitive(false);
            start();
        });
    } else {
        start();
    }
    dialog.present();
}

fn job_button(parent: &gtk::ApplicationWindow, text: &str, job: Job, primary: bool) -> gtk::Button {
    let b = button(text, primary);
    let parent = parent.downgrade();
    b.connect_clicked(move |_| {
        if let Some(parent) = parent.upgrade() {
            show_job(&parent, job);
        }
    });
    b
}

fn show_setup(parent: &gtk::ApplicationWindow, name: &str, package: &str, installed: bool) {
    let title = format!("{} {name}", if installed { "Configure" } else { "Install" });
    let d = gtk::Window::builder()
        .title(&title)
        .transient_for(parent)
        .modal(true)
        .default_width(600)
        .build();
    let body = column(18);
    for setter in [
        gtk::Widget::set_margin_top,
        gtk::Widget::set_margin_bottom,
        gtk::Widget::set_margin_start,
        gtk::Widget::set_margin_end,
    ] {
        setter(body.upcast_ref(), 24);
    }
    body.append(&label(&title, "section-title"));
    let (description, cmd) = if installed {
        match package {
            "steam" => (
                "Open Steam to manage games and per-game compatibility settings. The ArchWiki guide covers Proton and driver setup.",
                "steam".into(),
            ),
            "gamemode" => (
                "For Steam games, paste this into Properties → Launch Options. Use Test GameMode in the Gaming screen to verify your setup.",
                "gamemoderun %command%".into(),
            ),
            "mangohud" => (
                "For Steam games, paste this into Properties → Launch Options. MangoHud's optional configuration file is ~/.config/MangoHud/MangoHud.conf; see the guide for settings.",
                "mangohud %command%".into(),
            ),
            "gamescope" => (
                "This Steam launch option runs a game in a fullscreen Gamescope session. Review the guide for supported rendering and display options.",
                "gamescope -f -- %command%".into(),
            ),
            "vulkan-tools" => (
                "Inspect your Vulkan implementation and detected graphics devices with this terminal command, or use Inspect Vulkan support in the Gaming screen.",
                "vulkaninfo --summary".into(),
            ),
            _ => (
                "Open Lutris to manage game runners and per-game settings. See the ArchWiki guide for configuration details.",
                "lutris".into(),
            ),
        }
    } else {
        (
            "Open a terminal with this command ready to review. Press Enter to perform a full system upgrade and install the package, then review pacman's transaction before accepting.",
            format!("sudo pacman -Syu --needed {package}"),
        )
    };
    body.append(&wrapped(description, "subtitle"));
    let l = wrapped(&cmd, "mono");
    l.set_selectable(true);
    body.append(&l);
    if package == "steam" {
        body.append(&wrapped("Steam requires the multilib repository and suitable 32-bit GPU drivers. Check the ArchWiki Steam installation guide before installing.", "notice"));
    }
    let status = wrapped("", "muted");
    status.set_visible(false);
    body.append(&status);
    let buttons = row(12);
    let launch_option = cmd.contains("%command%");
    if !launch_option {
        let open = button("Open terminal", true);
        let proposed = cmd.clone();
        let status = status.clone();
        open.connect_clicked(move |b| {
            b.set_sensitive(false);
            status.set_visible(true);
            status.set_text("Opening terminal…");
            let (tx, rx) = mpsc::channel();
            let proposed = proposed.clone();
            std::thread::spawn(move || {
                let _ = tx.send(terminal::open(&proposed));
            });
            let button = b.downgrade();
            let status = status.downgrade();
            glib::timeout_add_local(Duration::from_millis(100), move || {
                let result = match rx.try_recv() {
                    Ok(result) => result,
                    Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                    Err(_) => {
                        Err("The terminal launcher stopped. You can still copy the command.".into())
                    }
                };
                if let Some(button) = button.upgrade() {
                    button.set_sensitive(true);
                }
                if let Some(status) = status.upgrade() {
                    status.set_text(match &result {
                        Ok(()) => "Terminal opened. Review the command and press Enter to run it.",
                        Err(error) => error,
                    });
                }
                glib::ControlFlow::Break
            });
        });
        buttons.append(&open);
    }
    let copy = button(
        if launch_option {
            "Copy launch option"
        } else {
            "Copy command"
        },
        launch_option,
    );
    copy.connect_clicked(move |b| {
        copy_text(&cmd);
        b.set_label("Copied");
    });
    buttons.append(&copy);
    let wiki = gtk::LinkButton::with_label(
        &format!(
            "https://wiki.archlinux.org/title/{}",
            if package == "steam" {
                "Steam"
            } else if package == "gamemode" {
                "GameMode"
            } else if package == "mangohud" {
                "MangoHud"
            } else if package == "gamescope" {
                "Gamescope"
            } else if package == "lutris" {
                "Lutris"
            } else {
                "Vulkan"
            }
        ),
        "ArchWiki guide",
    );
    buttons.append(&wiki);
    let close = button("Close", false);
    let dc = d.downgrade();
    close.connect_clicked(move |_| {
        if let Some(dc) = dc.upgrade() {
            dc.close();
        }
    });
    buttons.append(&close);
    body.append(&buttons);
    d.set_child(Some(&body));
    d.present();
}

fn build(app: &gtk::Application) {
    gtk::Window::set_default_icon_name("archforge");
    let provider = gtk::CssProvider::new();
    let chosen = Rc::new(RefCell::new(theme::selected()));
    theme::apply(&provider, &chosen.borrow());
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_interface_color_scheme(gtk::InterfaceColorScheme::Dark);
    }
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Arch Forge")
        .icon_name("archforge")
        .default_width(1640)
        .default_height(950)
        .build();
    // GTK CSS has no media queries. Adjust only when crossing the breakpoint;
    // polling avoids a continuous frame callback while the app is idle.
    let weak = window.downgrade();
    let mut compact = false;
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let Some(window) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let next = window.width() > 0 && window.width() < 1280;
        if next != compact {
            compact = next;
            adapt_layout(&window);
        }
        glib::ControlFlow::Continue
    });
    let header = gtk::HeaderBar::new();
    header.set_title_widget(Some(&label("", "muted")));
    window.set_titlebar(Some(&header));
    let root = row(0);
    let sidebar = column(0);
    sidebar.add_css_class("sidebar");
    sidebar.set_width_request(220);
    let brand = row(10);
    let icon = arch_logo("#ff6467", 50);
    icon.add_css_class("brand-icon");
    brand.append(&icon);
    let brand_text = column(0);
    brand_text.append(&label("Arch Forge", "brand"));
    brand_text.append(&label("System workbench", "brand-sub"));
    brand.append(&brand_text);
    sidebar.append(&brand);
    brand.set_margin_bottom(42);
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_hhomogeneous(false);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(160);
    let navs = Rc::new(RefCell::new(Vec::<gtk::Button>::new()));
    for (name, title, glyph, icon) in [
        ("overview", "Overview", "\u{e871}", "view-grid-symbolic"),
        (
            "gaming",
            "Gaming",
            "\u{ea28}",
            "applications-games-symbolic",
        ),
        ("cleanup", "Cleanup", "\u{f0ff}", "user-trash-symbolic"),
        (
            "packages",
            "Packages",
            "\u{e1a1}",
            "package-x-generic-symbolic",
        ),
        (
            "diagnostics",
            "Diagnostics",
            "\u{eaa2}",
            "utilities-terminal-symbolic",
        ),
    ] {
        let b = gtk::Button::new();
        b.add_css_class("nav");
        let r = row(0);
        r.append(&symbol(glyph, icon));
        r.append(&label(title, ""));
        b.set_child(Some(&r));
        b.set_sensitive(false);
        sidebar.append(&b);
        navs.borrow_mut().push(b.clone());
        let navs = navs.clone();
        let stack = stack.clone();
        b.connect_clicked(move |b| {
            for other in navs.borrow().iter() {
                other.remove_css_class("active");
            }
            b.add_css_class("active");
            stack.set_visible_child_name(name);
        });
    }
    let spacer = column(0);
    spacer.set_vexpand(true);
    sidebar.append(&spacer);
    sidebar.append(&updater::launcher(&window));
    sidebar.append(&appearance_menu(&provider, chosen));
    let foot = row(12);
    foot.add_css_class("sidebar-foot");
    foot.append(&arch_logo("#48b7e8", 30));
    let details = column(3);
    details.append(&label("Arch Linux", "row-title"));
    details.append(&label("Native · rolling release", "tiny"));
    foot.append(&details);
    sidebar.append(&foot);
    root.append(&sidebar);
    root.append(&stack);
    window.set_child(Some(&root));
    let loading = column(16);
    loading.add_css_class("content");
    loading.append(&label("Reading your system…", "page-title"));
    loading.append(&label("Loading hardware and installed packages.", "muted"));
    stack.add_named(&loading, Some("loading"));
    let metrics = Rc::new(RefCell::new(Metrics::default()));
    let mut sampler = Sampler::default();
    *metrics.borrow_mut() = sampler.sample();
    let mc = metrics.clone();
    let weak = window.downgrade();
    glib::timeout_add_local(Duration::from_secs(2), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        *mc.borrow_mut() = sampler.sample();
        glib::ControlFlow::Continue
    });
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(Inventory::load());
    });
    let w = window.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if let Ok(inv) = rx.try_recv() {
            let pages = [
                ("overview", overview(&w, &inv, metrics.clone())),
                ("gaming", gaming(&w, &inv)),
                ("cleanup", cleanup(&w)),
                ("packages", packages(&w, &inv)),
                ("diagnostics", diagnostics(&w, &inv)),
            ];
            for (name, page) in pages {
                if let Some(error) = &inv.error {
                    page.append(&wrapped(
                        &format!("Package database could not be read: {error}"),
                        "notice",
                    ));
                }
                let scroll = gtk::ScrolledWindow::builder()
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .child(&page)
                    .build();
                stack.add_named(&scroll, Some(name));
            }
            for b in navs.borrow().iter() {
                b.set_sensitive(true);
            }
            navs.borrow()[0].add_css_class("active");
            stack.set_visible_child_name("overview");
            adapt_layout(&w);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
    window.present();
}

fn main() -> glib::ExitCode {
    glib::set_application_name("Arch Forge");
    if !system::is_arch(&system::read("/etc/os-release")) {
        eprintln!("Arch Forge supports Arch Linux only (ID=arch).");
        return glib::ExitCode::FAILURE;
    }
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("Run Arch Forge as your regular desktop user, never as root.");
        return glib::ExitCode::FAILURE;
    }
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    app.run()
}
