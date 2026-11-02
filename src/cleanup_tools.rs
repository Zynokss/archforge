//! Additional cleanup tasks. Inspect commands never delete; changes remain interactive.
use crate::{button, column, label, row, wrapped};
use gtk::prelude::*;

struct Task {
    title: &'static str,
    description: &'static str,
    icon: &'static str,
    requires: &'static str,
    inspect: &'static str,
    change: Option<&'static str>,
}
const STORAGE: &[Task] = &[
    Task {
        title: "Largest installed packages",
        description: "Rank the 50 largest installed packages by size. Inspect dependencies before deciding what to uninstall.",
        icon: "package-x-generic-symbolic",
        requires: "pacman",
        inspect: "LC_ALL=C pacman -Qi | awk '/^Name[[:space:]]*:/ {name=$3} /^Installed Size[[:space:]]*:/ {size=$4; if ($5 == \"GiB\") size*=1024; else if ($5 == \"KiB\") size/=1024; else if ($5 == \"B\") size/=1048576; printf \"%.2f MiB\\t%s\\n\", size, name}' | sort -nr | head -n 50",
        change: None,
    },
    Task {
        title: "Home cache breakdown",
        description: "See which app caches use the most space. This does not clear your entire cache.",
        icon: "folder-symbolic",
        requires: "du",
        inspect: "cache=${XDG_CACHE_HOME:-\"$HOME/.cache\"}; [[ $cache = /* ]] || cache=\"$HOME/.cache\"; du -x -h --max-depth=1 -- \"$cache\" | sort -hr",
        change: None,
    },
    Task {
        title: "Large files in your home",
        description: "Find files larger than 1 GiB on your home filesystem. Other mounted filesystems are excluded; nothing is deleted.",
        icon: "drive-harddisk-symbolic",
        requires: "find",
        inspect: "[[ $HOME = /* && $HOME != / ]] || exit 1; find \"$HOME\" -xdev -type f -size +1G -printf '%s\\t%p\\n' | sort -nr | head -n 50 | numfmt --field=1 --to=iec",
        change: None,
    },
    Task {
        title: "Pacman configuration leftovers",
        description: "List .pacnew and .pacsave files in /etc. Merge or inspect them individually; they can contain useful configuration.",
        icon: "document-properties-symbolic",
        requires: "find",
        inspect: "find /etc -xdev -type f \\( -name '*.pacnew' -o -name '*.pacsave' \\) -print",
        change: None,
    },
];
const APPS: &[Task] = &[
    Task {
        title: "Docker dangling images",
        description: "Inspect Docker disk use and dangling images. Cleanup asks for confirmation and leaves volumes and containers alone. Requires access to the Docker daemon.",
        icon: "package-x-generic-symbolic",
        requires: "docker",
        inspect: "docker system df && docker image ls --filter dangling=true",
        change: Some(
            "docker system df && docker image ls --filter dangling=true && docker image prune",
        ),
    },
    Task {
        title: "Unused Flatpak runtimes · your account",
        description: "Inspect user runtimes, then let Flatpak identify unused refs and show its removal transaction. Named custom installations are outside this action.",
        icon: "application-x-executable-symbolic",
        requires: "flatpak",
        inspect: "flatpak list --user --runtime --columns=application,branch,size",
        change: Some("flatpak uninstall --user --unused"),
    },
    Task {
        title: "Unused Flatpak runtimes · system",
        description: "Inspect system runtimes, then review unused refs in Flatpak's transaction. App data is kept; administrative authentication may be requested.",
        icon: "application-x-executable-symbolic",
        requires: "flatpak",
        inspect: "flatpak list --system --runtime --columns=application,branch,size",
        change: Some("flatpak uninstall --system --unused"),
    },
    Task {
        title: "VirtualBox storage inventory",
        description: "List registered virtual machines and virtual disks. Manage unwanted VMs in VirtualBox after reviewing them; this action removes nothing.",
        icon: "computer-symbolic",
        requires: "VBoxManage",
        inspect: "VBoxManage list vms && VBoxManage list hdds",
        change: None,
    },
];
const SYSTEM: &[Task] = &[
    Task {
        title: "Expired user temporary files",
        description: "Use systemd's configured age rules for your account. Preview is a dry run; cleanup does not blindly empty temporary directories.",
        icon: "user-trash-symbolic",
        requires: "systemd-tmpfiles",
        inspect: "systemd-tmpfiles --user --clean --dry-run",
        change: Some(
            "systemd-tmpfiles --user --clean --dry-run && read -r -p 'Apply the configured user temporary-file cleanup? [y/N] ' reply && [[ $reply = y || $reply = Y ]] && systemd-tmpfiles --user --clean",
        ),
    },
    Task {
        title: "Expired system temporary files",
        description: "Clean only entries eligible under systemd's age rules. Review the authenticated dry run in the embedded terminal before applying.",
        icon: "user-trash-symbolic",
        requires: "systemd-tmpfiles",
        inspect: "systemd-tmpfiles --clean --dry-run",
        change: Some(
            "sudo systemd-tmpfiles --clean --dry-run && read -r -p 'Apply the configured system temporary-file cleanup? [y/N] ' reply && [[ $reply = y || $reply = Y ]] && sudo systemd-tmpfiles --clean",
        ),
    },
    Task {
        title: "Journal size budget · 50 MiB",
        description: "An alternative to the existing 7-day policy: remove oldest archived journals to target 50 MiB. Active journals remain, so total usage can exceed the target.",
        icon: "text-x-generic-symbolic",
        requires: "journalctl",
        inspect: "journalctl --disk-usage",
        change: Some(
            "journalctl --disk-usage && read -r -p 'Remove archived journal history beyond 50 MiB? [y/N] ' reply && [[ $reply = y || $reply = Y ]] && sudo journalctl --vacuum-size=50M",
        ),
    },
    Task {
        title: "Timeshift snapshot before maintenance",
        description: "List snapshots or create an on-demand snapshot using an already configured Timeshift installation. Personal files may be excluded by your configuration.",
        icon: "document-save-symbolic",
        requires: "timeshift",
        inspect: "sudo timeshift --list",
        change: Some("sudo timeshift --create --comments 'Before Arch Forge maintenance' --tags O"),
    },
];

fn task_row(parent: &gtk::ApplicationWindow, task: &'static Task) -> gtk::Box {
    let line = row(12);
    line.add_css_class("tool-row");
    let icon = gtk::Image::from_icon_name(task.icon);
    icon.set_pixel_size(22);
    line.append(&icon);
    let text = column(3);
    text.set_hexpand(true);
    text.append(&label(task.title, "row-title"));
    text.append(&wrapped(task.description, "row-sub"));
    line.append(&text);
    let available = gtk::glib::find_program_in_path(task.requires).is_some();
    let inspect = button("Inspect", false);
    let owner = parent.downgrade();
    inspect.connect_clicked(move |_| {
        if let Some(parent) = owner.upgrade() {
            if task.requires == "timeshift" {
                crate::tools::terminal_review(&parent, task.title, "List existing snapshots with administrator authentication. This does not create or remove snapshots.", task.inspect);
                return;
            }
            crate::tools::inspect_cleanup(
                &parent,
                task.title,
                task.description,
                task.icon,
                task.inspect,
            );
        }
    });
    inspect.set_sensitive(available);
    line.append(&inspect);
    if let Some(command) = task.change {
        let review = button(
            if task.requires == "timeshift" {
                "Create snapshot"
            } else {
                "Review & clean"
            },
            false,
        );
        review.set_sensitive(available);
        let owner = parent.downgrade();
        review.connect_clicked(move |_| {
            if let Some(parent) = owner.upgrade() {
                crate::tools::terminal_review(&parent, task.title, task.description, command);
            }
        });
        line.append(&review);
    }
    if !available {
        let missing = label(&format!("Requires {}", task.requires), "tiny");
        text.append(&missing);
        line.set_tooltip_text(Some(&format!(
            "Install and configure {} to use this optional tool.",
            task.requires
        )));
    }
    line
}

pub(crate) fn panel(parent: &gtk::ApplicationWindow) -> gtk::Box {
    let panel = column(10);
    panel.add_css_class("panel");
    panel.append(&label("More ways to reclaim space", "section-title"));
    panel.append(&wrapped(
        "Inspect first, then choose a specific cleanup. Optional tools are enabled when installed.",
        "muted",
    ));
    let stack = gtk::Stack::new();
    stack.set_vhomogeneous(false);
    let tabs = gtk::StackSwitcher::new();
    tabs.set_stack(Some(&stack));
    tabs.set_halign(gtk::Align::Start);
    panel.append(&tabs);
    for (name, tasks) in [
        ("Storage review", STORAGE),
        ("Apps & containers", APPS),
        ("Logs, temp & snapshots", SYSTEM),
    ] {
        let group = column(6);
        for task in tasks {
            group.append(&task_row(parent, task));
        }
        stack.add_titled(&group, Some(name), name);
    }
    panel.append(&stack);
    panel
}
