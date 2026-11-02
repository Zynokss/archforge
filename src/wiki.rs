//! Offline quick reference, with original summaries and links to official documentation.
use crate::{button, column, label, row, wrapped};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};
struct Topic {
    title: &'static str,
    intro: &'static str,
    tip: &'static str,
    url: &'static str,
    icon: &'static str,
}
const TOPICS: &[Topic] = &[
    Topic {
        title: "Getting started",
        intro: "Identify your system and learn where to find help.",
        tip: "Arch Forge is a quick reference. For installing Arch itself, follow the official Installation guide and General recommendations.",
        url: "https://wiki.archlinux.org/title/General_recommendations",
        icon: "help-browser-symbolic",
    },
    Topic {
        title: "Packages & updates",
        intro: "Everyday pacman commands, explained.",
        tip: "Arch uses rolling updates. Keep the system fully upgraded. Never refresh package databases and then upgrade only selected packages. AUR recipes require their own review.",
        url: "https://wiki.archlinux.org/title/Pacman",
        icon: "package-x-generic-symbolic",
    },
    Topic {
        title: "Files & permissions",
        intro: "Find files, inspect ownership, and understand access.",
        tip: "Avoid chmod 777 and broad recursive ownership changes. Check the exact file and its current permissions first.",
        url: "https://wiki.archlinux.org/title/File_permissions_and_attributes",
        icon: "text-x-generic-symbolic",
    },
    Topic {
        title: "Services & startup",
        intro: "Inspect services, logs, and boot performance.",
        tip: "System units and user units are separate. Restarting a service can interrupt applications or connectivity; enabling a unit changes future startup behavior.",
        url: "https://wiki.archlinux.org/title/Systemd",
        icon: "applications-system-symbolic",
    },
    Topic {
        title: "Networking",
        intro: "Inspect interfaces, routes, DNS, and connections.",
        tip: "Use the network manager already configured on your system. resolvectl requires systemd-resolved; nmcli requires NetworkManager. Avoid enabling competing network managers.",
        url: "https://wiki.archlinux.org/title/Network_configuration",
        icon: "network-wireless-symbolic",
    },
    Topic {
        title: "Disks & storage",
        intro: "Understand devices, filesystems, mounts, and free space.",
        tip: "Verify device names before modifying storage. Formatting, partitioning, and filesystem repair need device-specific instructions and a backup; this guide does not offer a generic format button.",
        url: "https://wiki.archlinux.org/title/File_systems",
        icon: "drive-harddisk-symbolic",
    },
    Topic {
        title: "Drivers & hardware",
        intro: "Identify hardware and inspect the active driver stack.",
        tip: "Choose graphics drivers for the actual GPU and kernel. Vulkan tools, mesa-utils, lm_sensors, and NVIDIA tools are optional; a missing command does not mean the hardware is broken.",
        url: "https://wiki.archlinux.org/title/General_recommendations#Graphical_user_interface",
        icon: "video-display-symbolic",
    },
    Topic {
        title: "Hyprland & Wayland",
        intro: "Inspect your compositor and graphical session.",
        tip: "hyprctl commands require a running Hyprland session. Back up configuration before changing it; reloading applies configuration to the running desktop.",
        url: "https://wiki.archlinux.org/title/Hyprland",
        icon: "preferences-desktop-display-symbolic",
    },
    Topic {
        title: "Shell & terminal",
        intro: "Find commands, inspect your shell, and use documentation.",
        tip: "Replace UPPERCASE placeholders before use. Quote paths with spaces. Read scripts before running them, and use sudo only when the operation needs it.",
        url: "https://wiki.archlinux.org/title/Command-line_shell",
        icon: "utilities-terminal-symbolic",
    },
    Topic {
        title: "Troubleshooting",
        intro: "Gather evidence before changing your system.",
        tip: "Read the actual error, check Arch news, and keep logs. Do not delete pacman's lock file while a package process is running. Review identifying information before sharing logs.",
        url: "https://wiki.archlinux.org/title/General_troubleshooting",
        icon: "dialog-warning-symbolic",
    },
    Topic {
        title: "Recovery & backups",
        intro: "Prepare package lists, snapshots, and a recovery plan.",
        tip: "A package list is not a system backup. Keep backups on another drive and verify a restore path. Timeshift can exclude personal files depending on its configuration.",
        url: "https://wiki.archlinux.org/title/System_backup",
        icon: "document-save-symbolic",
    },
];
struct Entry {
    topic: usize,
    title: &'static str,
    command: &'static str,
    description: &'static str,
    effect: &'static str,
    placeholder: bool,
}
macro_rules! e {
    ($t:expr,$title:expr,$cmd:expr,$desc:expr) => {
        Entry {
            topic: $t,
            title: $title,
            command: $cmd,
            description: $desc,
            effect: "Read-only",
            placeholder: false,
        }
    };
}
macro_rules! m {
    ($t:expr,$title:expr,$cmd:expr,$desc:expr,$effect:expr) => {
        Entry {
            topic: $t,
            title: $title,
            command: $cmd,
            description: $desc,
            effect: $effect,
            placeholder: false,
        }
    };
}
macro_rules! p {
    ($t:expr,$title:expr,$cmd:expr,$desc:expr,$effect:expr) => {
        Entry {
            topic: $t,
            title: $title,
            command: $cmd,
            description: $desc,
            effect: $effect,
            placeholder: true,
        }
    };
}
const ENTRIES: &[Entry] = &[
    e!(
        0,
        "Identify your distribution",
        "cat /etc/os-release",
        "Show the distribution identity and version metadata."
    ),
    e!(
        0,
        "Check the running kernel",
        "uname -r",
        "Print the kernel currently running; installing a new kernel does not change this until booted."
    ),
    e!(
        0,
        "Read pacman's manual",
        "man pacman",
        "Open the local manual. Use / to search and q to quit; man-db must be installed."
    ),
    e!(
        0,
        "Inspect time synchronization",
        "timedatectl status",
        "Check clock, timezone, and network time status."
    ),
    e!(
        0,
        "Inspect locale settings",
        "localectl status",
        "Show configured locale and keyboard settings."
    ),
    m!(
        1,
        "Update your system",
        "sudo pacman -Syu",
        "Refresh repository databases and upgrade installed packages together. Check Arch news before major updates.",
        "Changes system packages"
    ),
    p!(
        1,
        "Search for a package",
        "pacman -Ss KEYWORD",
        "Search names and descriptions in your existing repository databases. Replace KEYWORD.",
        "Read-only"
    ),
    p!(
        1,
        "Install a package",
        "sudo pacman -S --needed PACKAGE",
        "Install from the existing repository database. Replace PACKAGE; keep the system fully upgraded first. If databases need refreshing, use a full upgrade with installation.",
        "Changes system packages"
    ),
    p!(
        1,
        "Upgrade and install together",
        "sudo pacman -Syu --needed PACKAGE",
        "Refresh databases, upgrade the system, and install PACKAGE. --needed skips targets already current.",
        "Changes system packages"
    ),
    p!(
        1,
        "Inspect a repository package",
        "pacman -Si PACKAGE",
        "Read repository metadata and dependencies for PACKAGE.",
        "Read-only"
    ),
    p!(
        1,
        "Inspect an installed package",
        "pacman -Qi PACKAGE",
        "Read installed package metadata, including required-by information.",
        "Read-only"
    ),
    e!(
        1,
        "List explicit packages",
        "pacman -Qqe",
        "List explicitly installed package names; useful for planning a reinstall."
    ),
    e!(
        1,
        "List foreign packages",
        "pacman -Qm",
        "List packages absent from configured repositories. Foreign does not automatically mean AUR."
    ),
    e!(
        1,
        "Find orphan dependencies",
        "pacman -Qdtq",
        "List dependency-installed packages with no required-by relationship. Review candidates before removing them."
    ),
    p!(
        1,
        "Remove a package",
        "sudo pacman -R PACKAGE",
        "Remove PACKAGE after reviewing its dependencies and pacman's transaction. No recursive removal or automatic acceptance.",
        "Removes packages"
    ),
    p!(
        1,
        "Find a file's owner",
        "pacman -Qo /path/to/FILE",
        "Show which installed package owns a file. Replace the example path.",
        "Read-only"
    ),
    e!(
        1,
        "Check updates without syncing pacman's database",
        "checkupdates",
        "Requires pacman-contrib. Checks with a separate database; exit status 2 means no updates, not a failed upgrade."
    ),
    e!(
        1,
        "Preview older cached packages",
        "paccache -dv -k 2",
        "Requires pacman-contrib. Dry-run candidates, keeping two cached versions."
    ),
    e!(
        2,
        "List files and permissions",
        "ls -lah",
        "Show hidden entries, permissions, ownership, and readable sizes in the current directory."
    ),
    p!(
        2,
        "Inspect a file",
        "stat FILE",
        "Show ownership, mode, size, and timestamps. Replace FILE and quote paths containing spaces.",
        "Read-only"
    ),
    e!(
        2,
        "Measure home directory folders",
        "du -xh --max-depth=1 \"$HOME\"",
        "Estimate folder usage without crossing filesystem boundaries. Permission errors can make the result partial."
    ),
    e!(
        2,
        "Find large home files",
        "find \"$HOME\" -xdev -type f -size +1G -print",
        "List files over 1 GiB on the home filesystem; does not remove anything."
    ),
    p!(
        2,
        "Make your script executable",
        "chmod u+x SCRIPT",
        "Add execute permission for the owner of SCRIPT. Read the script first; permissions do not establish trust.",
        "Changes file permissions"
    ),
    p!(
        2,
        "Inspect access-control lists",
        "getfacl FILE",
        "Requires ACL tools. Show extended access rules for FILE.",
        "Read-only"
    ),
    e!(
        3,
        "Find failed system services",
        "systemctl --failed --no-pager",
        "List system units currently marked failed."
    ),
    e!(
        3,
        "Find failed user services",
        "systemctl --user --failed --no-pager",
        "Inspect the current account's user service manager."
    ),
    p!(
        3,
        "Inspect a service",
        "systemctl status UNIT --no-pager",
        "Replace UNIT, for example NetworkManager.service. Inactive units may return a nonzero status.",
        "Read-only"
    ),
    p!(
        3,
        "Read a service's current-boot logs",
        "journalctl -b -u UNIT --no-pager",
        "Replace UNIT. Some system journal entries require elevated permissions or journal-group access.",
        "Read-only"
    ),
    e!(
        3,
        "Inspect boot timing",
        "systemd-analyze blame",
        "Show unit activation times. These timings are not a complete critical-path analysis."
    ),
    e!(
        3,
        "Inspect scheduled tasks",
        "systemctl list-timers --all --no-pager",
        "Show enabled and inactive timer units."
    ),
    p!(
        3,
        "Restart a service",
        "sudo systemctl restart UNIT",
        "Replace UNIT. Restarting can interrupt applications, network access, or other services.",
        "Restarts a service"
    ),
    p!(
        3,
        "Enable a service at boot",
        "sudo systemctl enable UNIT",
        "Replace UNIT. Adds startup enablement; without --now it does not start the unit immediately.",
        "Changes startup configuration"
    ),
    e!(
        4,
        "Inspect network interfaces",
        "ip address show",
        "Show addresses and interface state. Addresses may identify your network when shared."
    ),
    e!(
        4,
        "Inspect routes",
        "ip route show",
        "Show the IPv4 routing table and default gateway."
    ),
    e!(
        4,
        "Inspect DNS resolver status",
        "resolvectl status",
        "For systems using systemd-resolved: show per-link DNS configuration."
    ),
    e!(
        4,
        "Inspect NetworkManager devices",
        "nmcli device status",
        "Requires NetworkManager. Show managed device state."
    ),
    e!(
        4,
        "List saved network connections",
        "nmcli connection show",
        "List NetworkManager connection profiles, without requesting their secrets."
    ),
    e!(
        4,
        "Test connectivity",
        "ping -c 4 archlinux.org",
        "Send four network probes. A failed ping can reflect DNS or ICMP filtering, not only a disconnected link."
    ),
    e!(
        4,
        "Inspect listening sockets",
        "ss -tuln",
        "List listening TCP and UDP sockets with numeric addresses."
    ),
    e!(
        4,
        "Inspect wireless blocks",
        "rfkill list",
        "Show software and hardware radio-block status."
    ),
    e!(
        5,
        "Inspect block devices and filesystems",
        "lsblk -f",
        "Show devices, filesystem types, UUIDs, and mountpoints."
    ),
    e!(
        5,
        "Inspect mounted filesystems",
        "findmnt",
        "Show the current mount tree and mount options."
    ),
    e!(
        5,
        "Check filesystem capacity",
        "df -hT",
        "Show used/free space and filesystem types."
    ),
    e!(
        5,
        "Check inode capacity",
        "df -i",
        "A filesystem can run out of inodes even when free space remains."
    ),
    e!(
        5,
        "Check automatic SSD trim",
        "systemctl status fstrim.timer --no-pager",
        "Inspect the trim timer before changing its schedule."
    ),
    m!(
        5,
        "Trim supported mounted filesystems",
        "sudo fstrim -av",
        "Discard unused blocks on supported mounted filesystems. This can take time and is not a substitute for deleting unwanted files.",
        "Changes storage allocation"
    ),
    e!(
        6,
        "Inspect devices and kernel drivers",
        "lspci -k",
        "Requires pciutils. Show PCI devices, active drivers, and candidate modules."
    ),
    e!(
        6,
        "Inspect CPU capabilities",
        "lscpu",
        "Show processor architecture, topology, and capabilities."
    ),
    e!(
        6,
        "Inspect loaded kernel modules",
        "lsmod",
        "List modules currently loaded in the running kernel."
    ),
    e!(
        6,
        "Inspect Vulkan support",
        "vulkaninfo --summary",
        "Requires vulkan-tools and suitable drivers. Show Vulkan implementation and device information."
    ),
    e!(
        6,
        "Inspect OpenGL rendering",
        "glxinfo -B",
        "Requires mesa-utils and a working X/XWayland connection."
    ),
    e!(
        6,
        "Inspect hardware sensors",
        "sensors",
        "Requires lm_sensors and hardware/kernel support. Missing sensors do not necessarily indicate a fault."
    ),
    e!(
        6,
        "Inspect NVIDIA status",
        "nvidia-smi",
        "Available with supported NVIDIA proprietary driver tools; not applicable to every GPU."
    ),
    e!(
        7,
        "Inspect Hyprland version",
        "hyprctl version",
        "Requires a running Hyprland session. Check version compatibility before applying dotfiles."
    ),
    e!(
        7,
        "Inspect Hyprland monitors",
        "hyprctl monitors",
        "List outputs and their active configuration."
    ),
    e!(
        7,
        "Inspect Hyprland windows",
        "hyprctl clients",
        "List managed windows. Titles may contain identifying information."
    ),
    e!(
        7,
        "Inspect session environment",
        "printenv XDG_CURRENT_DESKTOP XDG_SESSION_TYPE WAYLAND_DISPLAY",
        "Show only the named session variables, not the full environment."
    ),
    e!(
        7,
        "Inspect your login session",
        "loginctl session-status",
        "Show the current login session and its processes."
    ),
    m!(
        7,
        "Reload Hyprland configuration",
        "hyprctl reload",
        "Apply configuration to the running compositor. Back up files and check the current version's documentation first.",
        "Changes running desktop"
    ),
    e!(
        8,
        "Show the current directory",
        "pwd",
        "Print the current working directory."
    ),
    e!(
        8,
        "Identify the configured login shell",
        "printf '%s\\n' \"$SHELL\"",
        "Show the SHELL environment variable. The currently executing shell can differ."
    ),
    p!(
        8,
        "Find an executable",
        "command -v COMMAND",
        "Replace COMMAND. Shell builtins, functions, and external tools can resolve differently.",
        "Read-only"
    ),
    p!(
        8,
        "Explain command resolution",
        "type -a COMMAND",
        "In Bash, list matching builtins, functions, aliases, and executables for COMMAND.",
        "Read-only"
    ),
    p!(
        8,
        "Read a command's manual",
        "man COMMAND",
        "Replace COMMAND. Local manual pages work offline when installed.",
        "Read-only"
    ),
    p!(
        8,
        "Check Bash script syntax",
        "bash -n SCRIPT",
        "Parse SCRIPT without executing its commands. Syntax validity does not establish safety.",
        "Read-only"
    ),
    e!(
        9,
        "Read current-boot errors",
        "journalctl -b -p err --no-pager",
        "Review current-boot journal errors. Assess each message in context; access can be restricted."
    ),
    e!(
        9,
        "Read previous-boot logs",
        "journalctl -b -1 --no-pager",
        "Requires retained logs from the previous boot."
    ),
    e!(
        9,
        "Check installed package files",
        "pacman -Qk",
        "Check for missing files belonging to installed packages. Does not reinstall packages or verify all file contents."
    ),
    e!(
        9,
        "Inspect recent package transactions",
        "tail -n 100 /var/log/pacman.log",
        "Read the last 100 pacman log lines to relate failures to recent changes."
    ),
    e!(
        9,
        "List recorded crashes",
        "coredumpctl list --no-pager",
        "List retained crash records. Check identifying details before sharing."
    ),
    e!(
        9,
        "Measure journal storage",
        "journalctl --disk-usage",
        "Show current journal disk usage, subject to access permissions."
    ),
    m!(
        9,
        "Keep seven days of archived logs",
        "sudo journalctl --vacuum-time=7d",
        "Permanently remove archived journal files older than seven days. Active journal files are kept.",
        "Deletes archived logs"
    ),
    m!(
        10,
        "Export explicit package names",
        "pacman -Qqe > explicit-packages.txt",
        "Write a package-name list in the current directory. This overwrites a file with that name; choose your output path deliberately.",
        "Writes a file"
    ),
    e!(
        10,
        "Inspect Timeshift snapshots",
        "sudo timeshift --list",
        "Requires configured Timeshift. Read its snapshot scope; personal data may be excluded."
    ),
    m!(
        10,
        "Create a Timeshift snapshot",
        "sudo timeshift --create --comments 'Before maintenance' --tags O",
        "Create an on-demand snapshot using the existing configuration. Confirm the destination has space.",
        "Creates a snapshot"
    ),
    e!(
        10,
        "Inspect home filesystem identity",
        "findmnt --target \"$HOME\"",
        "Check which filesystem currently holds the home directory before choosing a backup destination."
    ),
];
fn saved_path() -> std::path::PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| glib::home_dir().join(".config"))
        .join("archforge/wiki-saved.json")
}
fn saved() -> BTreeSet<String> {
    std::fs::read(saved_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn save(items: &BTreeSet<String>) -> Result<(), String> {
    use std::io::Write;
    let path = saved_path();
    let parent = path.parent().ok_or("Invalid preferences path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(items).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn key(entry: &Entry) -> String {
    format!("{}:{}", entry.topic, entry.title)
}

struct View {
    parent: glib::WeakRef<gtk::ApplicationWindow>,
    selected: Cell<usize>,
    mode: Cell<u8>,
    query: RefCell<String>,
    saved: RefCell<BTreeSet<String>>,
    cards: gtk::Box,
    title: gtk::Label,
    intro: gtk::Label,
    tip: gtk::Label,
    source: gtk::LinkButton,
    contents: gtk::Box,
    scroll: gtk::ScrolledWindow,
    topics: Vec<gtk::Button>,
    message: gtk::Label,
}
impl View {
    fn render(self: &Rc<Self>) {
        while let Some(child) = self.cards.first_child() {
            self.cards.remove(&child);
        }
        while let Some(child) = self.contents.first_child() {
            self.contents.remove(&child);
        }
        let topic = &TOPICS[self.selected.get()];
        let query = self.query.borrow().to_lowercase();
        let saved = self.saved.borrow();
        self.title.set_text(match self.mode.get() {
            1 => "Command cheat sheet",
            2 => "Saved commands",
            _ => topic.title,
        });
        self.intro.set_text(if query.is_empty() {
            topic.intro
        } else {
            "Results across all topics. Replace placeholders before use."
        });
        self.tip.set_text(topic.tip);
        self.source.set_uri(topic.url);
        self.source
            .set_label(&format!("ArchWiki: {} ↗", topic.title));
        for (index, button) in self.topics.iter().enumerate() {
            if index == self.selected.get() {
                button.add_css_class("chosen");
            } else {
                button.remove_css_class("chosen");
            }
        }
        let mut count = 0;
        for entry in ENTRIES {
            if self.mode.get() == 2 && !saved.contains(&key(entry)) {
                continue;
            }
            if self.mode.get() == 0 && query.is_empty() && entry.topic != self.selected.get() {
                continue;
            }
            let text = format!(
                "{} {} {} {}",
                entry.title, entry.description, entry.command, TOPICS[entry.topic].title
            )
            .to_lowercase();
            if !query.split_whitespace().all(|term| text.contains(term)) {
                continue;
            }
            count += 1;
            let card = column(7);
            card.add_css_class("wiki-command-card");
            let top = row(8);
            let title = label(entry.title, "row-title");
            title.set_hexpand(true);
            title.set_wrap(true);
            top.append(&title);
            let effect = label(
                entry.effect,
                if entry.effect == "Read-only" {
                    "wiki-read-badge"
                } else {
                    "wiki-change-badge"
                },
            );
            top.append(&effect);
            card.append(&top);
            if self.mode.get() != 1 {
                card.append(&wrapped(entry.description, "muted"));
            }
            let command_row = row(8);
            command_row.add_css_class("wiki-code-row");
            let code = wrapped("", "wiki-code");
            code.set_selectable(true);
            code.set_hexpand(true);
            code.set_markup(&command_markup(entry.command));
            command_row.append(&code);
            let copy = gtk::Button::from_icon_name("edit-copy-symbolic");
            copy.set_tooltip_text(Some("Copy command"));
            copy.update_property(&[gtk::accessible::Property::Label("Copy command")]);
            copy.connect_clicked(move |button| {
                crate::copy_text(entry.command);
                button.set_tooltip_text(Some("Copied"));
            });
            command_row.append(&copy);
            card.append(&command_row);
            let actions = row(8);
            let reference =
                gtk::LinkButton::with_label(TOPICS[entry.topic].url, "Official guide ↗");
            actions.append(&reference);
            let star = gtk::ToggleButton::with_label(if saved.contains(&key(entry)) {
                "★ Saved"
            } else {
                "☆ Save"
            });
            star.set_active(saved.contains(&key(entry)));
            let weak = Rc::downgrade(self);
            star.connect_toggled(move |button| {
                if let Some(view) = weak.upgrade() {
                    let mut items = view.saved.borrow_mut();
                    if button.is_active() {
                        items.insert(key(entry));
                    } else {
                        items.remove(&key(entry));
                    }
                    let persisted = save(&items);
                    drop(items);
                    match persisted {
                        Ok(()) => {
                            view.message.set_text("Saved commands are stored locally.");
                            if view.mode.get() == 2 {
                                let weak = Rc::downgrade(&view);
                                glib::idle_add_local_once(move || {
                                    if let Some(view) = weak.upgrade() {
                                        view.render();
                                    }
                                });
                            }
                            button.set_label(if button.is_active() {
                                "★ Saved"
                            } else {
                                "☆ Save"
                            });
                        }
                        Err(e) => view.message.set_text(&format!("Could not save: {e}")),
                    };
                }
            });
            actions.append(&star);
            if entry.placeholder {
                actions.append(&label("Replace UPPERCASE placeholders · copy only", "tiny"));
            } else if entry.effect != "Read-only" || entry.command.starts_with("sudo ") {
                let review = button("Review command", false);
                let owner = self.parent.clone();
                review.connect_clicked(move |_| {
                    if let Some(parent) = owner.upgrade() {
                        crate::embedded_terminal::show(
                            parent.upcast_ref(),
                            entry.title,
                            entry.description,
                            entry.command,
                        );
                    }
                });
                actions.append(&review);
            }
            if !entry.placeholder
                && entry.effect == "Read-only"
                && !entry.command.starts_with("sudo ")
            {
                let inspect = button("Inspect", false);
                let owner = self.parent.clone();
                inspect.connect_clicked(move |_| {
                    if let Some(parent) = owner.upgrade() {
                        crate::tools::inspect_cleanup(
                            &parent,
                            entry.title,
                            entry.description,
                            "help-browser-symbolic",
                            entry.command,
                        );
                    }
                });
                actions.append(&inspect);
            }
            card.append(&actions);
            self.cards.append(&card);
            if count <= 6 {
                let jump = button(entry.title, false);
                jump.add_css_class("wiki-toc-link");
                jump.set_tooltip_text(Some(entry.title));
                if let Some(text) = jump.child().and_then(|w| w.downcast::<gtk::Label>().ok()) {
                    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    text.set_max_width_chars(26);
                    text.set_xalign(0.0);
                }
                let target = card.downgrade();
                let container = self.cards.downgrade();
                let scroll = self.scroll.clone();
                jump.connect_clicked(move |_| {
                    if let (Some(target), Some(container)) = (target.upgrade(), container.upgrade())
                        && let Some(rect) = target.compute_bounds(&container)
                    {
                        scroll.vadjustment().set_value(rect.y() as f64);
                    }
                });
                self.contents.append(&jump);
            }
        }
        if count == 0 {
            self.cards.append(&wrapped(
                if self.mode.get() == 2 {
                    "No saved commands match. Save commands while browsing to collect them here."
                } else {
                    "No commands match. Try a package name, task, command, or another topic."
                },
                "muted",
            ));
        }
        self.message.set_text(&format!("{count} commands · quick reference · consult the linked documentation before unfamiliar changes."));
        self.scroll.vadjustment().set_value(0.0);
    }
}
fn command_markup(command: &str) -> String {
    command
        .split_inclusive(char::is_whitespace)
        .map(|word| {
            let token = word.trim();
            let color = if token == "sudo" {
                "#cba6f7"
            } else if token.starts_with('-') {
                "#ffbf69"
            } else if token.chars().any(|c| c.is_ascii_uppercase()) {
                "#89b4fa"
            } else {
                "#a6e3a1"
            };
            format!(
                "<span foreground=\"{color}\">{}</span>",
                glib::markup_escape_text(word)
            )
        })
        .collect()
}
pub(crate) fn page(parent: &gtk::ApplicationWindow) -> gtk::Box {
    let page = column(12);
    page.add_css_class("content");
    page.add_css_class("wiki-page");
    page.set_vexpand(true);
    page.append(&label("Workspace / Wiki", "eyebrow"));
    let header = row(12);
    let text = column(4);
    text.set_hexpand(true);
    text.append(&label("Your Arch field guide.", "page-title"));
    text.append(&label(
        "Find a command. Understand it. Get things done.",
        "subtitle",
    ));
    header.append(&text);
    let official = gtk::LinkButton::with_label("https://wiki.archlinux.org/", "Open ArchWiki ↗");
    official.set_valign(gtk::Align::Center);
    header.append(&official);
    page.append(&header);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search commands, tasks, errors, or topics…")
        .hexpand(true)
        .build();
    search.update_property(&[gtk::accessible::Property::Label("Search Wiki")]);
    page.append(&search);
    let tabs = row(4);
    tabs.add_css_class("wiki-tabs");
    let mut modes = Vec::new();
    for name in ["Browse", "Command cheat sheet", "Saved"] {
        let button = gtk::ToggleButton::with_label(name);
        if let Some(first) = modes.first() {
            button.set_group(Some(first));
        }
        tabs.append(&button);
        modes.push(button);
    }
    modes[0].set_active(true);
    page.append(&tabs);
    let topic_names: Vec<_> = TOPICS.iter().map(|t| t.title).collect();
    let picker = gtk::DropDown::from_strings(&topic_names);
    picker.set_selected(1);
    picker.add_css_class("wiki-topic-picker");
    picker.set_visible(false);
    page.append(&picker);
    let body = row(12);
    body.add_css_class("wiki-columns");
    body.set_vexpand(true);
    let topics = column(4);
    topics.add_css_class("wiki-panel");
    topics.set_width_request(180);
    topics.append(&label("Topics", "section-title"));
    let mut buttons = Vec::new();
    for topic in TOPICS {
        let b = gtk::Button::new();
        b.add_css_class("wiki-topic");
        let line = row(9);
        let icon = gtk::Image::from_icon_name(topic.icon);
        icon.set_pixel_size(18);
        line.append(&icon);
        line.append(&label(topic.title, ""));
        b.set_child(Some(&line));
        topics.append(&b);
        buttons.push(b);
    }
    let topics_scroll = gtk::ScrolledWindow::builder()
        .child(&topics)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(180)
        .max_content_width(180)
        .propagate_natural_width(true)
        .build();
    topics_scroll.add_css_class("wiki-topic-list");
    body.append(&topics_scroll);
    let center = column(8);
    center.add_css_class("wiki-panel");
    center.set_hexpand(true);
    let title = label("", "wiki-topic-title");
    center.append(&title);
    let intro = wrapped("", "subtitle");
    center.append(&intro);
    let cards = column(8);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&cards)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .min_content_height(260)
        .build();
    center.append(&scroll);
    let message = wrapped("", "tiny");
    center.append(&message);
    body.append(&center);
    let aside = column(12);
    aside.set_width_request(245);
    aside.set_hexpand(false);
    aside.set_valign(gtk::Align::Start);
    let toc = column(5);
    toc.add_css_class("wiki-panel");
    toc.append(&label("On this page", "section-title"));
    let contents = column(2);
    toc.append(&contents);
    aside.append(&toc);
    let knowledge = column(8);
    knowledge.add_css_class("wiki-panel");
    knowledge.append(&label("Good to know", "section-title"));
    let tip = wrapped("", "wiki-tip");
    tip.set_max_width_chars(30);
    knowledge.append(&tip);
    aside.append(&knowledge);
    let sources = column(8);
    sources.add_css_class("wiki-panel");
    sources.append(&label("Official reference", "section-title"));
    let source = gtk::LinkButton::with_label("https://wiki.archlinux.org/", "ArchWiki");
    source.set_halign(gtk::Align::Start);
    sources.append(&source);
    let source_note = wrapped(
        "Offline summaries; upstream documentation can change. Check the current guide before modifying your system.",
        "tiny",
    );
    source_note.set_max_width_chars(30);
    sources.append(&source_note);
    for (name, url) in [
        (
            "System maintenance",
            "https://wiki.archlinux.org/title/System_maintenance",
        ),
        (
            "Backup a package list",
            "https://wiki.archlinux.org/title/Pacman/Tips_and_tricks#List_of_installed_packages",
        ),
        (
            "Understand the AUR",
            "https://wiki.archlinux.org/title/Arch_User_Repository",
        ),
    ] {
        let link = gtk::LinkButton::with_label(url, name);
        link.set_halign(gtk::Align::Start);
        sources.append(&link);
    }
    aside.append(&sources);
    let aside_scroll = gtk::ScrolledWindow::builder()
        .child(&aside)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(245)
        .max_content_width(245)
        .propagate_natural_width(true)
        .build();
    aside_scroll.set_hexpand(false);
    body.append(&aside_scroll);
    page.append(&body);
    let view = Rc::new(View {
        parent: parent.downgrade(),
        selected: Cell::new(1),
        mode: Cell::new(0),
        query: RefCell::new(String::new()),
        saved: RefCell::new(saved()),
        cards,
        title,
        intro,
        tip,
        source,
        contents,
        scroll,
        topics: buttons,
        message,
    });
    let weak = Rc::downgrade(&view);
    picker.connect_selected_notify(move |picker| {
        if let Some(view) = weak.upgrade() {
            view.selected
                .set((picker.selected() as usize).min(TOPICS.len() - 1));
            view.render();
        }
    });
    for (i, button) in view.topics.iter().enumerate() {
        let weak = Rc::downgrade(&view);
        let search = search.clone();
        let browse = modes[0].clone();
        let picker = picker.clone();
        button.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.selected.set(i);
                picker.set_selected(i as u32);
                view.mode.set(0);
                view.query.borrow_mut().clear();
                search.set_text("");
                browse.set_active(true);
                view.render();
            }
        });
    }
    for (i, button) in modes.iter().enumerate() {
        let weak = Rc::downgrade(&view);
        button.connect_toggled(move |button| {
            if button.is_active()
                && let Some(view) = weak.upgrade()
            {
                view.mode.set(i as u8);
                view.render();
            }
        });
    }
    let weak = Rc::downgrade(&view);
    search.connect_search_changed(move |search| {
        if let Some(view) = weak.upgrade() {
            *view.query.borrow_mut() = search.text().to_string();
            view.render();
        }
    });
    view.render();
    page.connect_destroy(move |_| {
        let _ = &view;
    });
    page
}
