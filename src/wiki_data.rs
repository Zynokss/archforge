//! Shared offline guide data for the desktop and terminal interfaces.
// Each interface consumes different topic presentation fields.
#[allow(dead_code)]
pub(crate) struct Topic {
    pub(crate) title: &'static str,
    pub(crate) intro: &'static str,
    pub(crate) tip: &'static str,
    pub(crate) url: &'static str,
    pub(crate) icon: &'static str,
}
pub(crate) const TOPICS: &[Topic] = &[
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
pub(crate) struct Entry {
    pub(crate) topic: usize,
    pub(crate) title: &'static str,
    pub(crate) command: &'static str,
    pub(crate) description: &'static str,
    pub(crate) effect: &'static str,
    pub(crate) placeholder: bool,
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
pub(crate) const ENTRIES: &[Entry] = &[
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
