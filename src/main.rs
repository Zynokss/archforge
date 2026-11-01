mod app_icons;
mod app_locations;
mod app_storage;
mod cleanups;
mod diagnostic_results;
mod dialogs;
mod jobs;
mod loadout_state;
mod loadout_styles;
mod loadout_ui;
mod loadouts;
mod package_cache;
mod ricing;
mod ricing_document;
mod ricing_ui;
mod screens;
mod system;
mod terminal;
mod theme;
mod tool_icons;
mod tools;
mod updater;
use screens::{cleanup, diagnostics, gaming, overview, packages};

use gtk::{glib, prelude::*};
use jobs::Job;
use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Duration};
use system::{Inventory, Metrics, Sampler};

static ACTIVE_JOBS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Operation;
impl Operation {
    fn new() -> Self {
        ACTIVE_JOBS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        ACTIVE_JOBS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

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
        if w.has_css_class("loadout-category-picker") {
            w.set_visible(compact);
        }
        if w.has_css_class("loadout-category-scroll") {
            w.set_visible(!compact);
        }

        if let Some(b) = w.downcast_ref::<gtk::Box>()
            && [
                "split-layout",
                "metric-grid",
                "page-header",
                "command-toolbar",
                "catalog-columns",
                "loadout-columns",
                "ricing-discover-layout",
                "ricing-detail-layout",
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
            if b.has_css_class("loadout-columns") {
                let mut child = b.first_child();
                let mut index = 0;
                while let Some(c) = child {
                    c.set_width_request(if compact {
                        -1
                    } else {
                        match index {
                            0 => 160,
                            2 => 236,
                            _ => -1,
                        }
                    });
                    index += 1;
                    child = c.next_sibling();
                }
            }
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
        let size = package_cache::measured_size(output);
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
    dialogs::show_job(parent, job);
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
    let (d, surface) = dialogs::Surface::new(
        parent,
        &title,
        if installed {
            "Set up your gaming tools for the way you play."
        } else {
            "Review the package and command before installing."
        },
        "\u{ea28}",
        "applications-games-symbolic",
    );
    let body = surface.content.clone();
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
    let launch_option = cmd.contains("%command%");
    surface.status.set(
        dialogs::Tone::Ready,
        if !installed {
            "Review before installing"
        } else if launch_option {
            "Ready for your Steam settings"
        } else {
            "Your tool is ready to launch"
        },
        description,
        "No command has run",
    );
    surface.stats(
        ("Package", package, "package-x-generic-symbolic"),
        (
            "Action",
            if !installed {
                "Install"
            } else if launch_option {
                "Steam launch option"
            } else {
                "Launch"
            },
            "applications-games-symbolic",
        ),
    );
    let card = column(10);
    card.add_css_class("dialog-command-card");
    card.append(&label(
        if launch_option {
            "Steam launch option"
        } else {
            "Terminal command"
        },
        "dialog-stat-caption",
    ));
    let l = wrapped(&cmd, "mono");
    l.set_selectable(true);
    card.append(&l);
    let command_tools = row(10);
    command_tools.add_css_class("dialog-command-tools");
    card.append(&command_tools);
    body.append(&card);
    if package == "steam" && !installed {
        body.append(&wrapped("Steam requires the multilib repository and suitable 32-bit GPU drivers. Check the ArchWiki Steam installation guide before installing.", "notice"));
    }
    let status = surface.status.clone();
    let buttons = surface.actions.clone();
    surface.close_button(&d, "Close", false);
    if !launch_option {
        let open = button("Open terminal", true);
        let proposed = cmd.clone();
        let status = status.clone();
        let setup = d.downgrade();
        open.connect_clicked(move |b| {
            b.set_sensitive(false);
            status.set(
                dialogs::Tone::Busy,
                "Opening your terminal…",
                "The command will wait for you to review it and press Enter.",
                "Starting terminal",
            );
            let (tx, rx) = mpsc::channel();
            let proposed = proposed.clone();
            std::thread::spawn(move || {
                let _ = tx.send(terminal::open(&proposed));
            });
            let button = b.downgrade();
            let status = status.clone();
            let setup = setup.clone();
            glib::timeout_add_local(Duration::from_millis(100), move || {
                let result = match rx.try_recv() {
                    Ok(result) => result,
                    Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                    Err(_) => {
                        Err("The terminal launcher stopped. You can still copy the command.".into())
                    }
                };
                if result.is_ok() {
                    if let Some(setup) = setup.upgrade() {
                        setup.close();
                    }
                    return glib::ControlFlow::Break;
                }
                if let Some(button) = button.upgrade() {
                    button.set_sensitive(true);
                }
                if let Err(error) = &result {
                    status.set(
                        dialogs::Tone::Error,
                        "Could not open your terminal",
                        error,
                        "Copy command is available",
                    );
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
    command_tools.append(&copy);
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
    command_tools.append(&wiki);
    d.present();
}

fn refresh_pages(
    window: &gtk::ApplicationWindow,
    stack: &gtk::Stack,
    inv: &Inventory,
    metrics: Rc<RefCell<Metrics>>,
) {
    fn search(widget: &gtk::Widget) -> Option<gtk::SearchEntry> {
        if let Some(e) = widget.downcast_ref::<gtk::SearchEntry>() {
            return Some(e.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            if let Some(e) = search(&w) {
                return Some(e);
            }
            child = w.next_sibling();
        }
        None
    }
    fn sort(widget: &gtk::Widget) -> Option<gtk::DropDown> {
        if let Some(e) = widget.downcast_ref::<gtk::DropDown>() {
            return Some(e.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            if let Some(e) = sort(&w) {
                return Some(e);
            }
            child = w.next_sibling();
        }
        None
    }
    let order = stack
        .child_by_name("packages")
        .and_then(|w| sort(&w))
        .map(|e| e.selected());
    let selected = stack
        .visible_child_name()
        .map(|s| s.to_string())
        .filter(|s| s != "loading")
        .unwrap_or_else(|| "overview".into());
    let query = stack
        .child_by_name("packages")
        .and_then(|w| search(&w))
        .map(|e| e.text());
    let tool_selection = stack
        .child_by_name("tools")
        .map(|page| tools::selection(&page));
    for (name, page) in [
        ("overview", overview(window, inv, metrics)),
        ("gaming", gaming(window, inv)),
        ("cleanup", cleanup(window)),
        ("packages", packages(window, inv)),
        ("diagnostics", diagnostics(window, inv)),
        ("tools", tools::page(window)),
    ] {
        let offset = stack
            .child_by_name(name)
            .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok())
            .map(|w| w.vadjustment().value())
            .unwrap_or(0.0);
        if let Some(old) = stack.child_by_name(name) {
            stack.remove(&old);
        }
        if let Some(error) = &inv.error {
            page.append(&wrapped(
                &format!("Package database could not be read: {error}"),
                "notice",
            ));
        }
        if name == "packages"
            && let Some(query) = &query
            && let Some(e) = search(page.upcast_ref())
        {
            e.set_text(query);
        }
        if name == "packages"
            && let Some(order) = order
            && let Some(e) = sort(page.upcast_ref())
        {
            e.set_selected(order);
        }
        if name == "tools"
            && let Some(state) = &tool_selection
        {
            tools::restore(page.upcast_ref(), state);
        }
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&page)
            .build();
        stack.add_named(&scroll, Some(name));
        let weak = scroll.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(scroll) = weak.upgrade() {
                scroll.vadjustment().set_value(offset);
            }
        });
    }
    if stack.child_by_name("loadouts").is_none() {
        let page = loadout_ui::page(window, stack);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&page)
            .build();
        stack.add_named(&scroll, Some("loadouts"));
    }
    stack.set_visible_child_name(&selected);
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
    brand.set_margin_bottom(20);
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_hhomogeneous(false);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(160);
    let navs = Rc::new(RefCell::new(Vec::<gtk::Button>::new()));
    for (name, title, glyph, icon) in [
        ("overview", "Overview", "\u{e871}", "view-grid-symbolic"),
        ("tools", "Tools", "\u{e8b8}", "applications-system-symbolic"),
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
        ("loadouts", "Loadouts", "\u{e161}", "document-save-symbolic"),
        (
            "ricing",
            "Ricing",
            "\u{e3ae}",
            "applications-graphics-symbolic",
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
    let find = button("Find a tool   Ctrl+K", false);
    find.add_css_class("find-tool");
    sidebar.insert_child_after(&find, Some(&brand));
    let search_navs = navs.clone();
    let search_stack = stack.clone();
    find.connect_clicked(move |_| {
        if search_stack.child_by_name("tools").is_some() {
            search_navs.borrow()[1].emit_clicked();
            if let Some(page) = search_stack.child_by_name("tools") {
                tools::focus_search(&page);
            }
        }
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let jump_navs = navs.clone();
    let jump_stack = stack.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        if !state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }
        if key == gtk::gdk::Key::k || key == gtk::gdk::Key::K {
            if jump_stack.child_by_name("tools").is_some() {
                jump_navs.borrow()[1].emit_clicked();
                if let Some(page) = jump_stack.child_by_name("tools") {
                    tools::focus_search(&page);
                }
            }
            return glib::Propagation::Stop;
        }
        if let Some(digit) = key.to_unicode().and_then(|c| c.to_digit(10))
            && (1..=8).contains(&digit)
        {
            let nav = jump_navs.borrow()[digit as usize - 1].clone();
            if nav.is_sensitive() {
                nav.emit_clicked();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
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
    let w = window.downgrade();
    let mut fingerprint = None;
    let mut pending = None;
    glib::timeout_add_local(Duration::from_millis(500), move || {
        let Some(w) = w.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let stamp = std::fs::metadata("/var/lib/pacman/local")
            .and_then(|m| m.modified())
            .ok();
        if pending.is_none()
            && (fingerprint.is_none() || fingerprint != Some(stamp))
            && !std::path::Path::new("/var/lib/pacman/db.lck").exists()
        {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(Inventory::load());
            });
            pending = Some(rx);
            fingerprint = Some(stamp);
        }
        let Some(rx) = pending.as_ref() else {
            return glib::ControlFlow::Continue;
        };
        let inv = match rx.try_recv() {
            Ok(inv) => inv,
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => {
                pending = None;
                fingerprint = None;
                return glib::ControlFlow::Continue;
            }
        };
        pending = None;
        refresh_pages(&w, &stack, &inv, metrics.clone());
        for b in navs.borrow().iter() {
            b.set_sensitive(true);
        }
        if !navs.borrow().iter().any(|b| b.has_css_class("active")) {
            navs.borrow()[0].add_css_class("active");
        }
        adapt_layout(&w);
        glib::ControlFlow::Continue
    });
    window.present();
}

fn main() -> glib::ExitCode {
    glib::set_application_name("Arch Forge");
    if !system::is_arch(&system::read("/etc/os-release")) {
        eprintln!("Arch Forge supports Arch Linux only (ID=arch).");
        return glib::ExitCode::FAILURE;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|a| a == "--set-package-cache" || a == "--reset-package-cache")
    {
        let destination = match args.as_slice() {
            [flag, path] if flag == "--set-package-cache" => Some(path.as_str()),
            [flag] if flag == "--reset-package-cache" => None,
            _ => {
                eprintln!(
                    "Usage: archforge --set-package-cache /absolute/path | --reset-package-cache"
                );
                return glib::ExitCode::FAILURE;
            }
        };
        return match package_cache::apply(destination) {
            Ok(message) => {
                println!("{message}");
                glib::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                glib::ExitCode::FAILURE
            }
        };
    }
    if args
        .first()
        .is_some_and(|a| a == "--add-app-drive" || a == "--check-app-drive")
    {
        let result = match args.as_slice() {
            [flag, id, mount] if flag == "--add-app-drive" => app_storage::register(id, mount),
            [flag, id] if flag == "--check-app-drive" => app_storage::check(id),
            _ => Err(
                "Usage: archforge --add-app-drive name /mount/path | --check-app-drive name".into(),
            ),
        };
        return match result {
            Ok(message) => {
                println!("{message}");
                glib::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                glib::ExitCode::FAILURE
            }
        };
    }
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("Run Arch Forge as your regular desktop user, never as root.");
        return glib::ExitCode::FAILURE;
    }
    let app = gtk::Application::builder()
        .application_id(if std::env::var_os("ARCH_FORGE_PREVIEW").is_some() {
            "io.github.archforge.Preview"
        } else {
            APP_ID
        })
        .build();
    app.connect_activate(build);
    app.run()
}
