use crate::{
    button, column, copy_text,
    dialogs::{Surface, Tone},
    jobs::Job,
    label, row, wrapped,
};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::mpsc,
    time::Duration,
};

#[derive(Clone, Copy)]
enum Action {
    Read(&'static str, &'static [&'static str], &'static str),
    Terminal(&'static str),
    Cache,
    AppLocations,
    Owner,
    Job(Job),
}
#[derive(Clone, Copy)]
struct Tool {
    name: &'static str,
    category: &'static str,
    description: &'static str,
    keywords: &'static str,
    icon: &'static str,
    action: Action,
}
const TOOLS: &[Tool] = &[
    Tool {
        name: "App install locations",
        category: "Storage",
        description: "Install Flatpak apps on another drive, or inspect the system storage used by pacman packages.",
        keywords: "move packages install installation drive disk destination full root flatpak location",
        icon: "drive-removable-media-symbolic",
        action: Action::AppLocations,
    },
    Tool {
        name: "Package cache location",
        category: "Storage",
        description: "Choose where pacman downloads package archives. Keep existing archives and back up the configuration.",
        keywords: "install installation folder destination disk drive cachedir pacman change move download external relocate",
        icon: "folder-symbolic",
        action: Action::Cache,
    },
    Tool {
        name: "Full system upgrade",
        category: "Packages",
        description: "Refresh repositories and upgrade installed packages together. Review pacman's transaction before accepting.",
        keywords: "update upgrade sync pacman syu",
        icon: "software-update-available-symbolic",
        action: Action::Terminal("sudo pacman -Syu"),
    },
    Tool {
        name: "Package file check",
        category: "Packages",
        description: "Check installed packages for missing files. This inspects files; it does not reinstall anything.",
        keywords: "repair integrity missing broken verify pacman",
        icon: "document-properties-symbolic",
        action: Action::Read("/usr/bin/pacman", &["-Qk"], "Packages checked"),
    },
    Tool {
        name: "Explicit package list",
        category: "Packages",
        description: "Copy or save the names of explicitly installed packages for a future reinstall.",
        keywords: "backup export restore reinstall list pacman",
        icon: "document-save-symbolic",
        action: Action::Read("/usr/bin/pacman", &["-Qqe"], "Explicit packages"),
    },
    Tool {
        name: "Find a file's package",
        category: "Packages",
        description: "Enter a file path to find the installed package that owns it.",
        keywords: "owner ownership conflict exists filesystem pacman qo",
        icon: "system-search-symbolic",
        action: Action::Owner,
    },
    Tool {
        name: "Recent package transactions",
        category: "Packages",
        description: "Read the last 100 lines of pacman's transaction log.",
        keywords: "history installed removed upgraded pacman log",
        icon: "document-open-recent-symbolic",
        action: Action::Read(
            "/usr/bin/tail",
            &["-n", "100", "/var/log/pacman.log"],
            "Recent log lines",
        ),
    },
    Tool {
        name: "Drives and capacity",
        category: "Storage",
        description: "List block devices, sizes, models, and connection types without changing partitions.",
        keywords: "disk ssd hdd nvme usb space hardware",
        icon: "drive-harddisk-symbolic",
        action: Action::Read(
            "/usr/bin/lsblk",
            &[
                "--nodeps",
                "--exclude",
                "7",
                "--noheadings",
                "--output",
                "NAME,SIZE,MODEL,TRAN",
            ],
            "Block devices",
        ),
    },
    Tool {
        name: "Mounted filesystems",
        category: "Storage",
        description: "See mount paths, backing devices, filesystem types, and mount options.",
        keywords: "disk drive partition mount fstab btrfs ext4",
        icon: "drive-removable-media-symbolic",
        action: Action::Read(
            "/usr/bin/findmnt",
            &[
                "--real",
                "--noheadings",
                "--output",
                "TARGET,SOURCE,FSTYPE,OPTIONS",
            ],
            "Mount entries",
        ),
    },
    Tool {
        name: "Filesystem free space",
        category: "Storage",
        description: "Inspect used and available space on mounted filesystems.",
        keywords: "disk storage full usage free capacity",
        icon: "drive-harddisk-symbolic",
        action: Action::Read("/usr/bin/df", &["-hT"], "Filesystem rows"),
    },
    Tool {
        name: "Enabled services",
        category: "System",
        description: "List service files enabled to start automatically. This does not disable any services.",
        keywords: "startup autostart daemon systemd boot",
        icon: "preferences-system-symbolic",
        action: Action::Read(
            "/usr/bin/systemctl",
            &[
                "list-unit-files",
                "--type=service",
                "--state=enabled",
                "--no-legend",
                "--no-pager",
            ],
            "Enabled services",
        ),
    },
    Tool {
        name: "Scheduled timers",
        category: "System",
        description: "Inspect timers and their next scheduled runs, including cleanup and maintenance tasks.",
        keywords: "schedule systemd cron timer maintenance trim",
        icon: "appointment-soon-symbolic",
        action: Action::Read(
            "/usr/bin/systemctl",
            &["list-timers", "--all", "--no-legend", "--no-pager"],
            "Timer entries",
        ),
    },
    Tool {
        name: "Current boot errors",
        category: "System",
        description: "Read up to 100 journal entries at error priority or higher. Access depends on your account's journal permissions.",
        keywords: "logs crash kernel service journal errors troubleshoot",
        icon: "dialog-warning-symbolic",
        action: Action::Read(
            "/usr/bin/journalctl",
            &["-b", "-p", "err", "--no-pager", "-n", "100"],
            "Current boot",
        ),
    },
    Tool {
        name: "Boot critical chain",
        category: "System",
        description: "See the timing chain for boot targets. Activation time alone does not explain every boot delay.",
        keywords: "slow boot startup systemd performance",
        icon: "document-open-recent-symbolic",
        action: Action::Read(
            "/usr/bin/systemd-analyze",
            &["critical-chain", "--no-pager"],
            "Boot timing",
        ),
    },
    Tool {
        name: "SSD trim schedule",
        category: "System",
        description: "Inspect the SSD trim timer and its schedule without running trim or changing a service.",
        keywords: "fstrim ssd discard storage maintenance",
        icon: "drive-harddisk-symbolic",
        action: Action::Read(
            "/usr/bin/systemctl",
            &[
                "show",
                "fstrim.timer",
                "--property=LoadState,ActiveState,UnitFileState,NextElapseUSecRealtime",
            ],
            "SSD maintenance",
        ),
    },
    Tool {
        name: "Network addresses",
        category: "Network",
        description: "Inspect interface states and local IP addresses. Review addresses before sharing the output.",
        keywords: "internet wifi ethernet ip connection network",
        icon: "network-wired-symbolic",
        action: Action::Read("/usr/bin/ip", &["-brief", "address"], "Network interfaces"),
    },
    Tool {
        name: "DNS resolver status",
        category: "Network",
        description: "Inspect DNS servers and routing domains when systemd-resolved is available.",
        keywords: "internet dns nameserver resolve resolvectl",
        icon: "network-server-symbolic",
        action: Action::Read(
            "/usr/bin/resolvectl",
            &["status", "--no-pager"],
            "DNS configuration",
        ),
    },
    Tool {
        name: "Power profiles",
        category: "System",
        description: "List available power profiles if power-profiles-daemon is installed. This does not change your active profile.",
        keywords: "battery laptop performance balanced power saving",
        icon: "battery-good-symbolic",
        action: Action::Read("/usr/bin/powerprofilesctl", &["list"], "Power management"),
    },
    Tool {
        name: "Review old package archives",
        category: "Storage",
        description: "Preview removable package archives while keeping the latest two versions.",
        keywords: "cleanup cache space paccache versions",
        icon: "user-trash-symbolic",
        action: Action::Job(Job::CachePreview),
    },
    Tool {
        name: "Inspect failed services",
        category: "System",
        description: "Find system units that failed in the current boot.",
        keywords: "errors diagnose systemd repair",
        icon: "dialog-warning-symbolic",
        action: Action::Job(Job::Failed),
    },
    Tool {
        name: "Inspect Vulkan support",
        category: "Gaming",
        description: "Inspect graphics devices and Vulkan drivers for your gaming setup.",
        keywords: "gpu graphics vulkan driver gaming",
        icon: "video-display-symbolic",
        action: Action::Job(Job::Vulkan),
    },
];

pub(crate) fn launch_terminal(
    window: &gtk::Window,
    command: String,
    status: crate::dialogs::Status,
    open: &gtk::Button,
) {
    open.set_sensitive(false);
    status.set(
        Tone::Busy,
        "Opening embedded terminal…",
        "Review the action and command in Arch Forge before authenticating.",
        "Waiting for terminal",
    );
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::terminal::open(&command));
    });
    let weak = window.downgrade();
    let open = open.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("Terminal worker stopped. You can still copy the command.".into()),
        };
        if let Some(window) = weak.upgrade() {
            match result {
                Ok(()) => window.close(),
                Err(e) => {
                    status.set(
                        Tone::Error,
                        "Could not open a terminal",
                        &e,
                        "Copy command is available",
                    );
                    open.set_sensitive(true);
                }
            }
        }
        glib::ControlFlow::Break
    });
}
pub(crate) fn terminal_review(
    parent: &gtk::ApplicationWindow,
    name: &str,
    description: &str,
    command: &str,
) {
    crate::embedded_terminal::show(parent.upcast_ref(), name, description, command);
}

fn read_tool(
    parent: &gtk::ApplicationWindow,
    tool: Tool,
    program: &'static str,
    args: Vec<String>,
    metric: &'static str,
) {
    let (window, surface) =
        Surface::new(parent, tool.name, tool.description, "\u{eb8e}", tool.icon);
    crate::dialogs::size_output(&window, parent);
    surface.status.set(
        Tone::Busy,
        "Reading your system…",
        "This check does not modify your configuration.",
        "Read-only check",
    );
    let stats = surface.stats(
        (metric, "—", tool.icon),
        ("Access", "Your account", "avatar-default-symbolic"),
    );
    let command = format!(
        "{} {}",
        program,
        args.iter()
            .map(|s| crate::package_cache::quote(s))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let details = surface.details(
        "Command output",
        "Full log · scroll here to review",
        Some(&command),
    );
    details
        .output
        .buffer()
        .set_text("Waiting for the command to finish…");
    let copy = button("Copy output", false);
    copy.set_sensitive(false);
    details.tools.append(&copy);
    let buffer = details.output.buffer();
    copy.connect_clicked(move |_| {
        copy_text(&buffer.text(&buffer.start_iter(), &buffer.end_iter(), false))
    });
    let save = button("Save output…", false);
    save.set_sensitive(false);
    details.tools.append(&save);
    let weak = window.downgrade();
    let buffer = details.output.buffer();
    let save_status = surface.status.clone();
    save.connect_clicked(move |_| {
        let Some(window)=weak.upgrade() else{return};
        let text=buffer.text(&buffer.start_iter(),&buffer.end_iter(),false).to_string();let status=save_status.clone();
        glib::spawn_future_local(async move {
            let chooser=gtk::FileDialog::builder().title("Save tool output").initial_name("archforge-output.txt").build();
            if let Ok(file)=chooser.save_future(Some(&window)).await {
                let Some(path)=file.path() else{status.set(Tone::Error,"Choose a local file","Remote locations are not supported for this export.","Output remains available");return};
                let (tx,rx)=mpsc::channel();std::thread::spawn(move ||{let _=tx.send(std::fs::write(path,text));});
                glib::timeout_add_local(Duration::from_millis(100),move ||{match rx.try_recv(){Ok(Ok(()))=>status.set(Tone::Success,"Output saved","The exported file contains the displayed output. Review it before sharing.","Saved to file"),Ok(Err(e))=>status.set(Tone::Error,"Could not save output",&e.to_string(),"Copy output is available"),Err(mpsc::TryRecvError::Empty)=>return glib::ControlFlow::Continue,Err(_)=>status.set(Tone::Error,"Could not save output","Export worker stopped.","Copy output is available")};glib::ControlFlow::Break});
            }
        });
    });
    let copy_command = button("Copy command", false);
    copy_command.connect_clicked(move |_| copy_text(&command));
    details.tools.append(&copy_command);
    surface.close_button(&window, "Done", true);
    let (tx, rx) = mpsc::channel();
    let operation = crate::Operation::new();
    std::thread::spawn(move || {
        let _guard = operation;
        let refs: Vec<_> = args.iter().map(String::as_str).collect();
        let _ = tx.send(crate::system::run(program, &refs, 120));
    });
    let weak = window.downgrade();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("The command worker stopped unexpectedly.".into()),
        };
        let text = match &result {
            Ok(t) | Err(t) => t,
        };
        details.output.buffer().set_text(if text.trim().is_empty() {
            "Command completed without output."
        } else {
            text
        });
        copy.set_sensitive(true);
        save.set_sensitive(true);
        if result.is_ok() {
            let count = text.lines().filter(|l| !l.trim().is_empty()).count();
            if matches!(
                metric,
                "Explicit packages"
                    | "Enabled services"
                    | "Timer entries"
                    | "Network interfaces"
                    | "Block devices"
                    | "Mount entries"
                    | "Recent log lines"
            ) {
                stats[0].set_text(&count.to_string());
            } else if metric == "Filesystem rows" {
                stats[0].set_text(&count.saturating_sub(1).to_string());
            } else if metric == "Packages checked" {
                stats[0].set_text(
                    &text
                        .lines()
                        .filter(|l| l.contains("total files"))
                        .count()
                        .to_string(),
                );
            } else {
                stats[0].set_text("See output");
            }
            surface.status.set(
                Tone::Success,
                "Check completed",
                tool.description,
                "Read-only · completed",
            );
        } else {
            stats[0].set_text("Not available");
            surface.status.set(Tone::Error,"Check needs attention","The command returned an error. The output explains missing tools, permissions, or unavailable services.","No changes made");
        }
        glib::ControlFlow::Break
    });
    window.present();
}
fn owner_lookup(parent: &gtk::ApplicationWindow, tool: Tool) {
    let (window, surface) =
        Surface::new(parent, tool.name, tool.description, "\u{e8b6}", tool.icon);
    surface.status.set(
        Tone::Ready,
        "Which file do you want to inspect?",
        "Use an absolute path. Pacman can identify only files owned by installed packages.",
        "Read-only lookup",
    );
    let entry = gtk::Entry::builder()
        .placeholder_text("/usr/bin/gamemoderun")
        .hexpand(true)
        .build();
    entry.update_property(&[gtk::accessible::Property::Label("File path")]);
    surface.content.append(&entry);
    let error = wrapped("", "warning");
    surface.content.append(&error);
    surface.close_button(&window, "Cancel", false);
    let inspect = button("Find owner", true);
    surface.actions.append(&inspect);
    let parent = parent.downgrade();
    let weak = window.downgrade();
    inspect.connect_clicked(move |_| {
        let path = entry.text().to_string();
        if !std::path::Path::new(&path).is_absolute() || path.contains(['\n', '\r', '\0']) {
            error.set_text("Enter a valid absolute file path.");
            return;
        }
        if let (Some(parent), Some(window)) = (parent.upgrade(), weak.upgrade()) {
            read_tool(
                &parent,
                tool,
                "/usr/bin/pacman",
                vec!["-Qo".into(), "--".into(), path],
                "File ownership",
            );
            window.close();
        }
    });
    window.present();
}
fn cache_location(parent: &gtk::ApplicationWindow) {
    let (window, surface) = Surface::new(
        parent,
        "Package cache location",
        "Choose where pacman stores downloaded archives.",
        "\u{e2c7}",
        "folder-symbolic",
    );
    surface.status.set(Tone::Ready,"Choose a download cache","Installed system files stay in their package-defined locations. Use a protected directory on a mounted Linux filesystem. Existing archives remain in place.","Configuration review only");
    let stats = surface.stats(
        ("Current cache", "Reading…", "folder-symbolic"),
        (
            "Existing archives",
            "Kept in place",
            "package-x-generic-symbolic",
        ),
    );
    surface
        .content
        .append(&label("New download directory", "row-title"));
    stats[0].add_css_class("cache-directory-value");
    stats[0].set_max_width_chars(22);
    let entry = gtk::Entry::builder()
        .text("/var/cache/pacman/pkg")
        .width_chars(20)
        .build();
    entry.update_property(&[gtk::accessible::Property::Label("Package cache directory")]);
    surface.content.append(&entry);
    let choose = button("Choose folder…", false);
    surface.content.append(&choose);
    let weak = window.downgrade();
    let selected = entry.clone();
    choose.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            let entry = selected.clone();
            glib::spawn_future_local(async move {
                let chooser = gtk::FileDialog::builder()
                    .title("Select a package cache folder")
                    .build();
                if let Ok(folder) = chooser.select_folder_future(Some(&window)).await
                    && let Some(path) = folder.path()
                {
                    entry.set_text(&path.to_string_lossy());
                }
            });
        }
    });
    let details = surface.details(
        "Configuration preview",
        "Creates a backup · preserves existing cache settings as fallbacks",
        None,
    );
    let update = Rc::new(move |value: &str, buffer: &gtk::TextBuffer| {
        match crate::package_cache::validate_path(value){Ok(_)=>buffer.set_text(&format!("sudo /usr/bin/archforge --set-package-cache {}\n\nAdds the first CacheDir in [options].\nPreserves existing cache directories as fallbacks.\nChecks the proposed configuration with pacman-conf.\nBacks up /etc/pacman.conf before saving.\nCreates the cache folder with mode 0755 if needed; its parent must already exist.\nDoes not move or delete existing archives.\n\nParent directories must be root-owned, traversable, and not writable by other accounts. The helper refuses symlinks and runs only with administrator privileges.",crate::package_cache::quote(value))),Err(e)=>buffer.set_text(&e)}
    });
    update(&entry.text(), &details.output.buffer());
    let u = update.clone();
    let buffer = details.output.buffer();
    let edit_status = surface.status.clone();
    entry.connect_changed(move |entry| {
        u(&entry.text(), &buffer);
        if crate::package_cache::validate_path(&entry.text()).is_ok() && edit_status.title.text()=="Check the directory path" {
            edit_status.set(Tone::Ready,"Choose a download cache","Installed files keep their package-defined locations. Existing archives remain in place; the new folder is used for future downloads.","Configuration review only");
        }
    });
    let proposed = entry.clone();
    let copy = button("Copy command", false);
    copy.connect_clicked(move |_| {
        if crate::package_cache::validate_path(&proposed.text()).is_ok() {
            copy_text(&format!(
                "sudo /usr/bin/archforge --set-package-cache {}",
                crate::package_cache::quote(&proposed.text())
            ));
        }
    });
    details.tools.append(&copy);
    let reset = button("Restore previous settings…", false);
    let weak_parent = parent.downgrade();
    reset.connect_clicked(move |_|{if let Some(parent)=weak_parent.upgrade(){terminal_review(&parent,"Restore cache settings","Removes only Arch Forge's CacheDir entry. Existing pacman cache settings resume, or pacman's default applies if none remain. A configuration backup is created; archives are kept.","sudo /usr/bin/archforge --reset-package-cache");}});
    details.tools.append(&reset);
    surface.close_button(&window, "Cancel", false);
    let open = button("Open embedded terminal", true);
    surface.actions.append(&open);
    let weak = window.downgrade();
    let status = surface.status.clone();
    let proposed = entry.clone();
    open.connect_clicked(
        move |b| match crate::package_cache::validate_path(&proposed.text()) {
            Ok(_) => {
                if let Some(window) = weak.upgrade() {
                    launch_terminal(
                        &window,
                        format!(
                            "sudo /usr/bin/archforge --set-package-cache {}",
                            crate::package_cache::quote(&proposed.text())
                        ),
                        status.clone(),
                        b,
                    );
                }
            }
            Err(e) => status.set(
                Tone::Warning,
                "Check the directory path",
                &e,
                "Nothing has changed",
            ),
        },
    );
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::package_cache::directories());
    });
    let weak = window.downgrade();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        match rx.try_recv() {
            Ok(Ok(dirs)) => stats[0].set_text(&dirs.join("\n")),
            Ok(Err(e)) => {
                stats[0].set_text("Unavailable");
                surface.status.set(
                    Tone::Warning,
                    "Could not read cache settings",
                    &e,
                    "Review pacman.conf first",
                );
            }
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => stats[0].set_text("Unavailable"),
        };
        glib::ControlFlow::Break
    });
    window.present();
}
fn activate(parent: &gtk::ApplicationWindow, tool: Tool) {
    match tool.action {
        Action::Read(p, a, m) => read_tool(
            parent,
            tool,
            p,
            a.iter().map(|s| s.to_string()).collect(),
            m,
        ),
        Action::Terminal(command) => terminal_review(parent, tool.name, tool.description, command),
        Action::Cache => cache_location(parent),
        Action::AppLocations => crate::app_locations::show(parent),
        Action::Owner => owner_lookup(parent, tool),
        Action::Job(job) => crate::show_job(parent, job),
    }
}
const CATEGORIES: &[&str] = &[
    "All tools",
    "Packages",
    "Storage",
    "System",
    "Network",
    "Gaming",
];
#[derive(Clone, Default)]
pub(crate) struct Selection {
    query: String,
    category: usize,
    sort: u32,
    list: bool,
}
struct Catalog {
    parent: glib::WeakRef<gtk::ApplicationWindow>,
    search: glib::WeakRef<gtk::SearchEntry>,
    body: gtk::Box,
    count: gtk::Label,
    query: RefCell<String>,
    category: Cell<usize>,
    sort: Cell<u32>,
    list: Cell<bool>,
    tabs: Vec<gtk::Button>,
    views: [gtk::Button; 2],
}
fn category_icon(category: &str) -> crate::tool_icons::Glyph {
    use crate::tool_icons::Glyph;
    match category {
        "Packages" => Glyph::Package,
        "Storage" => Glyph::Drive,
        "System" => Glyph::Services,
        "Network" => Glyph::Network,
        _ => Glyph::Gpu,
    }
}
fn action_label(tool: Tool) -> &'static str {
    match tool.action {
        Action::Cache | Action::AppLocations => "Configure",
        Action::Owner => "Look up",
        Action::Terminal(_) => "Review",
        _ => "Inspect",
    }
}
fn tool_card(parent: &glib::WeakRef<gtk::ApplicationWindow>, tool: Tool, list: bool) -> gtk::Box {
    let card = row(12);
    card.add_css_class("catalog-tool");
    card.set_tooltip_text(Some(tool.description));
    let badge = row(0);
    badge.add_css_class("catalog-tool-icon");
    badge.set_valign(gtk::Align::Center);
    badge.append(&crate::tool_icons::icon(
        crate::tool_icons::for_tool(tool.name).expect("Catalog icon"),
        26,
    ));
    card.append(&badge);
    let text = column(5);
    text.set_hexpand(true);
    let name = label(tool.name, "row-title");
    name.set_wrap(true);
    name.set_max_width_chars(35);
    text.append(&name);
    let description = label(tool.description, "row-sub");
    description.set_ellipsize(gtk::pango::EllipsizeMode::End);
    description.set_max_width_chars(55);
    text.append(&description);
    if list {
        text.append(&label(tool.category, "catalog-category-label"));
    }
    card.append(&text);
    let action = button(action_label(tool), false);
    action.add_css_class("catalog-action");
    action.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} {}",
        action_label(tool),
        tool.name
    ))]);
    let content = row(10);
    content.append(&label(action_label(tool), ""));
    content.append(&gtk::Image::from_icon_name("go-next-symbolic"));
    action.set_child(Some(&content));
    let parent = parent.clone();
    action.connect_clicked(move |_| {
        if let Some(parent) = parent.upgrade() {
            activate(&parent, tool);
        }
    });
    card.append(&action);
    card
}
fn matching(query: &str, category: usize, sort: u32) -> Vec<Tool> {
    let query = query.to_lowercase();
    let terms: Vec<_> = query.split_whitespace().collect();
    let mut tools: Vec<_> = TOOLS
        .iter()
        .copied()
        .filter(|tool| {
            let haystack = format!(
                "{} {} {} {}",
                tool.name, tool.category, tool.description, tool.keywords
            )
            .to_lowercase();
            (category == 0 || CATEGORIES.get(category) == Some(&tool.category))
                && terms.iter().all(|word| haystack.contains(word))
        })
        .collect();
    if sort > 0 {
        tools.sort_by_key(|t| t.name.to_lowercase());
        if sort == 2 {
            tools.reverse();
        }
    }
    tools
}
impl Catalog {
    fn render(self: &Rc<Self>) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        for (i, tab) in self.tabs.iter().enumerate() {
            if i == self.category.get() {
                tab.add_css_class("chosen");
            } else {
                tab.remove_css_class("chosen");
            }
            tab.update_state(&[gtk::accessible::State::Selected(Some(
                i == self.category.get(),
            ))]);
        }
        for (i, b) in self.views.iter().enumerate() {
            if (i == 1) == self.list.get() {
                b.add_css_class("chosen");
            } else {
                b.remove_css_class("chosen");
            }
            b.update_state(&[gtk::accessible::State::Pressed(
                if (i == 1) == self.list.get() {
                    gtk::AccessibleTristate::True
                } else {
                    gtk::AccessibleTristate::False
                },
            )]);
        }
        let query = self.query.borrow();
        let tools = matching(&query, self.category.get(), self.sort.get());
        self.count.set_text(&format!(
            "{} {}",
            tools.len(),
            if tools.len() == 1 { "tool" } else { "tools" }
        ));
        if tools.is_empty() {
            let empty = column(10);
            empty.add_css_class("catalog-empty");
            empty.append(&label("No matching tools", "section-title"));
            empty.append(&wrapped(
                "Try a shorter search, or choose All tools to search every category.",
                "muted",
            ));
            let clear = button("Clear search and filters", false);
            let weak = Rc::downgrade(self);
            clear.connect_clicked(move |_| {
                if let Some(c) = weak.upgrade() {
                    c.category.set(0);
                    c.query.borrow_mut().clear();
                    if let Some(search) = c.search.upgrade() {
                        search.set_text("");
                    }
                    c.render();
                }
            });
            empty.append(&clear);
            self.body.append(&empty);
            return;
        }
        if self.list.get() {
            let list = column(6);
            for tool in tools {
                list.append(&tool_card(&self.parent, tool, true));
            }
            self.body.append(&list);
            return;
        }
        let columns = row(16);
        columns.add_css_class("catalog-columns");
        columns.set_homogeneous(true);
        columns.set_valign(gtk::Align::Start);
        if self
            .parent
            .upgrade()
            .is_some_and(|p| p.width() > 0 && p.width() < 1280)
        {
            columns.set_orientation(gtk::Orientation::Vertical);
        }
        let left = column(14);
        let right = column(14);
        left.set_hexpand(true);
        right.set_hexpand(true);
        left.set_valign(gtk::Align::Start);
        right.set_valign(gtk::Align::Start);
        let overview = self.category.get() == 0 && query.trim().is_empty();
        let mut group_index = 0;
        for (index, category) in CATEGORIES.iter().enumerate().skip(1) {
            let grouped: Vec<_> = tools
                .iter()
                .copied()
                .filter(|t| t.category == *category)
                .collect();
            if grouped.is_empty() {
                continue;
            }
            let panel = column(8);
            panel.add_css_class("catalog-panel");
            let heading = row(12);
            heading.add_css_class("catalog-group-heading");
            let badge = row(0);
            badge.add_css_class("catalog-group-icon");
            badge.append(&crate::tool_icons::icon(category_icon(category), 25));
            heading.append(&badge);
            let name = label(category, "catalog-group-title");
            name.set_hexpand(true);
            heading.append(&name);
            heading.append(&label(
                &format!(
                    "{} {}",
                    grouped.len(),
                    if grouped.len() == 1 { "tool" } else { "tools" }
                ),
                "muted",
            ));
            panel.append(&heading);
            let limit = if !overview {
                usize::MAX
            } else if *category == "System" {
                1
            } else {
                2
            };
            for tool in grouped.iter().take(limit) {
                panel.append(&tool_card(&self.parent, *tool, false));
            }
            if grouped.len() > limit {
                let more = button(
                    &format!(
                        "View all {} tools",
                        category.to_lowercase().trim_end_matches('s')
                    ),
                    false,
                );
                more.add_css_class("catalog-more");
                let weak: Weak<Self> = Rc::downgrade(self);
                more.connect_clicked(move |_| {
                    if let Some(c) = weak.upgrade() {
                        c.category.set(index);
                        c.render();
                        c.scroll_top();
                    }
                });
                panel.append(&more);
            }
            if self.category.get() != 0 {
                self.body.append(&panel);
                return;
            }
            if group_index % 2 == 0 {
                left.append(&panel);
            } else {
                right.append(&panel);
            }
            group_index += 1;
        }
        columns.append(&left);
        if right.first_child().is_some() {
            columns.append(&right);
        }
        self.body.append(&columns);
    }
    fn scroll_top(&self) {
        let mut ancestor = self.body.parent();
        while let Some(p) = ancestor {
            if let Some(scroll) = p.downcast_ref::<gtk::ScrolledWindow>() {
                scroll.vadjustment().set_value(0.);
                break;
            }
            ancestor = p.parent();
        }
    }
}
pub(crate) fn page(parent: &gtk::ApplicationWindow) -> gtk::Box {
    let page = column(12);
    page.add_css_class("content");
    page.add_css_class("tools-page");
    let header = column(4);
    header.append(&label("Workspace / Tools", "eyebrow"));
    header.append(&label("Find the right tool.", "page-title"));
    header.append(&wrapped(
        "Packages, storage, system services, and network checks.",
        "subtitle",
    ));
    page.append(&crate::theme::banner(&header));
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search tools… e.g. cache, boot, missing files")
        .hexpand(true)
        .build();
    search.add_css_class("tool-search");
    search.set_search_delay(0);
    search.update_property(&[gtk::accessible::Property::Label("Search tools")]);
    page.append(&search);
    let controls = row(12);
    controls.add_css_class("command-toolbar");
    controls.add_css_class("catalog-controls");
    let tabs = row(0);
    let mut buttons = Vec::new();
    for category in CATEGORIES {
        let tab = button(category, false);
        tab.add_css_class("catalog-tab");
        tab.set_accessible_role(gtk::AccessibleRole::Tab);
        tabs.append(&tab);
        buttons.push(tab);
    }
    let tabs_scroll = gtk::ScrolledWindow::builder()
        .child(&tabs)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(true)
        .propagate_natural_height(true)
        .min_content_height(42)
        .build();
    controls.append(&tabs_scroll);
    let options = row(8);
    let count = label("", "muted");
    options.append(&count);
    let sort = gtk::DropDown::from_strings(&["Recommended", "Name A–Z", "Name Z–A"]);
    sort.add_css_class("catalog-sort");
    sort.update_property(&[gtk::accessible::Property::Label("Sort tools")]);
    options.append(&sort);
    let grouped = gtk::Button::from_icon_name("view-grid-symbolic");
    grouped.add_css_class("catalog-view");
    grouped.set_tooltip_text(Some("Grouped view"));
    grouped.update_property(&[gtk::accessible::Property::Label("Grouped view")]);
    let list = gtk::Button::from_icon_name("view-list-symbolic");
    list.add_css_class("catalog-view");
    list.add_css_class("catalog-list");
    list.set_tooltip_text(Some("Compact list"));
    list.update_property(&[gtk::accessible::Property::Label("Compact list")]);
    options.append(&grouped);
    options.append(&list);
    controls.append(&options);
    page.append(&controls);
    let body = column(0);
    page.append(&body);
    let catalog = Rc::new(Catalog {
        parent: parent.downgrade(),
        search: search.downgrade(),
        body,
        count,
        query: RefCell::new(String::new()),
        category: Cell::new(0),
        sort: Cell::new(0),
        list: Cell::new(false),
        tabs: buttons,
        views: [grouped.clone(), list.clone()],
    });
    for (i, tab) in catalog.tabs.iter().enumerate() {
        let weak = Rc::downgrade(&catalog);
        tab.connect_clicked(move |_| {
            if let Some(c) = weak.upgrade() {
                c.category.set(i);
                c.render();
                c.scroll_top();
            }
        });
    }
    let weak = Rc::downgrade(&catalog);
    search.connect_search_changed(move |s| {
        if let Some(c) = weak.upgrade() {
            *c.query.borrow_mut() = s.text().to_string();
            c.render();
            c.scroll_top();
        }
    });
    let weak = Rc::downgrade(&catalog);
    sort.connect_selected_notify(move |s| {
        if let Some(c) = weak.upgrade() {
            c.sort.set(s.selected());
            c.render();
        }
    });
    for (i, b) in [grouped, list].into_iter().enumerate() {
        let weak = Rc::downgrade(&catalog);
        b.connect_clicked(move |_| {
            if let Some(c) = weak.upgrade() {
                c.list.set(i == 1);
                c.render();
                c.scroll_top();
            }
        });
    }
    catalog.render();
    let keep = catalog.clone();
    page.connect_destroy(move |_| {
        let _ = &keep;
    });
    page
}
fn walk(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut widgets = vec![root.clone()];
    let mut child = root.first_child();
    while let Some(c) = child {
        widgets.extend(walk(&c));
        child = c.next_sibling();
    }
    widgets
}
pub(crate) fn selection(root: &gtk::Widget) -> Selection {
    let widgets = walk(root);
    Selection {
        query: widgets
            .iter()
            .find_map(|w| w.downcast_ref::<gtk::SearchEntry>())
            .map(|e| e.text().to_string())
            .unwrap_or_default(),
        category: widgets
            .iter()
            .filter(|w| w.has_css_class("catalog-tab"))
            .position(|w| w.has_css_class("chosen"))
            .unwrap_or(0),
        sort: widgets
            .iter()
            .find(|w| w.has_css_class("catalog-sort"))
            .and_then(|w| w.downcast_ref::<gtk::DropDown>())
            .map(|d| d.selected())
            .unwrap_or(0),
        list: widgets
            .iter()
            .any(|w| w.has_css_class("catalog-list") && w.has_css_class("chosen")),
    }
}
pub(crate) fn restore(root: &gtk::Widget, state: &Selection) {
    let widgets = walk(root);
    if let Some(entry) = widgets
        .iter()
        .find_map(|w| w.downcast_ref::<gtk::SearchEntry>())
    {
        entry.set_text(&state.query);
    }
    if let Some(dropdown) = widgets
        .iter()
        .find(|w| w.has_css_class("catalog-sort"))
        .and_then(|w| w.downcast_ref::<gtk::DropDown>())
    {
        dropdown.set_selected(state.sort);
    }
    if let Some(tab) = widgets
        .iter()
        .filter(|w| w.has_css_class("catalog-tab"))
        .nth(state.category)
        .and_then(|w| w.downcast_ref::<gtk::Button>())
    {
        tab.emit_clicked();
    }
    if let Some(button) = widgets
        .iter()
        .find(|w| {
            w.has_css_class(if state.list {
                "catalog-list"
            } else {
                "catalog-view"
            })
        })
        .and_then(|w| w.downcast_ref::<gtk::Button>())
    {
        button.emit_clicked();
    }
}
pub(crate) fn focus_search(root: &gtk::Widget) {
    if root.has_css_class("tool-search") {
        root.grab_focus();
        return;
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        focus_search(&c);
        child = c.next_sibling();
    }
}

pub(crate) fn inspect_mounts(parent: &gtk::ApplicationWindow) {
    activate(
        parent,
        *TOOLS
            .iter()
            .find(|t| t.name == "Mounted filesystems")
            .expect("Mount inspection tool"),
    );
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) fn verify_ui(parent: &gtk::ApplicationWindow) {
    tests::ui(parent);
}

pub(crate) fn inspect_cleanup(
    parent: &gtk::ApplicationWindow,
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    command: &'static str,
) {
    read_tool(
        parent,
        Tool {
            name,
            category: "Storage",
            description,
            keywords: "cleanup",
            icon,
            action: Action::Read("/usr/bin/bash", &[], "Inspection"),
        },
        "/usr/bin/bash",
        vec![
            "--noprofile".into(),
            "--norc".into(),
            "-c".into(),
            command.into(),
        ],
        "Inspection",
    );
}
