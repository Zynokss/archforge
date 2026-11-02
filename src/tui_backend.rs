//! Operation descriptions shared by rendering, review and execution. No action runs on selection.
use crate::{cli, system};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
};
#[derive(Clone, Debug)]
pub(super) struct Action {
    pub title: String,
    pub description: String,
    pub program: String,
    pub args: Vec<String>,
    pub admin: bool,
}
impl Action {
    pub fn new(title: &str, description: &str, program: &str, args: &[&str], admin: bool) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            admin,
        }
    }
    pub fn command(&self) -> String {
        format!(
            "{}{} {}",
            if self.admin { "sudo " } else { "" },
            self.program,
            self.args
                .iter()
                .map(|s| cli::shell_quote(s))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
    pub fn read(&self) -> Result<String, String> {
        debug_assert!(!self.admin);
        match system::run(
            &self.program,
            &self.args.iter().map(String::as_str).collect::<Vec<_>>(),
            60,
        ) {
            Err(e) if self.program.ends_with("checkupdates") && e.trim() == "Exit 2" => {
                Ok("No repository updates available.".into())
            }
            result => result,
        }
    }
}
pub(super) fn update() -> Action {
    Action::new(
        "Full system upgrade",
        "Refreshes repositories and upgrades ALL system packages, not just selected packages. Pacman will ask for transaction confirmation.",
        "/usr/bin/pacman",
        &["-Syu"],
        true,
    )
}
pub(super) fn check_updates() -> Action {
    Action::new(
        "Check repository updates",
        "Uses checkupdates' separate database. Requires pacman-contrib. Does not check AUR updates.",
        "/usr/bin/checkupdates",
        &[],
        false,
    )
}
pub(super) fn cleanup() -> Vec<Action> {
    vec![
        Action::new(
            "Preview package cache cleanup",
            "Lists older archives eligible for removal, keeping the latest two versions. No archives are removed.",
            "/usr/bin/paccache",
            &["-d", "-k", "2"],
            false,
        ),
        Action::new(
            "Clean older package versions",
            "Permanently deletes older cached package archives, retaining two versions. Does not uninstall packages. Preview candidates first.",
            "/usr/bin/paccache",
            &["-r", "-k", "2"],
            true,
        ),
        Action::new(
            "Measure journal storage",
            "Shows stored journal disk usage accessible to your account.",
            "/usr/bin/journalctl",
            &["--disk-usage"],
            false,
        ),
        Action::new(
            "Keep 7 days of archived logs",
            "Permanently deletes archived journal files older than 7 days. Active journal files are not removed.",
            "/usr/bin/journalctl",
            &["--vacuum-time=7d"],
            true,
        ),
        Action::new(
            "Inspect orphan dependencies",
            "Lists packages installed as dependencies that are no longer required. Nothing is removed; an empty query may return exit 1.",
            "/usr/bin/pacman",
            &["-Qtdq"],
            false,
        ),
    ]
}
pub(super) fn diagnostics() -> Vec<Action> {
    vec![
        Action::new(
            "Failed system services",
            "Lists failed system units from this boot.",
            "/usr/bin/systemctl",
            &["--failed", "--no-pager"],
            false,
        ),
        Action::new(
            "Boot timing",
            "Lists service startup times. Requires systemd boot timing data.",
            "/usr/bin/systemd-analyze",
            &["blame", "--no-pager"],
            false,
        ),
        Action::new(
            "Current boot warnings",
            "Shows the latest 100 accessible warnings and errors from this boot.",
            "/usr/bin/journalctl",
            &["-b", "-p", "warning", "-n", "100", "--no-pager"],
            false,
        ),
        Action::new(
            "Drives and filesystems",
            "Shows block devices and mountpoints. Does not change disks.",
            "/usr/bin/lsblk",
            &["-f"],
            false,
        ),
        Action::new(
            "Network interfaces",
            "Shows interface addresses. Requires iproute2.",
            "/usr/bin/ip",
            &["-brief", "address"],
            false,
        ),
        Action::new(
            "Listening sockets",
            "Shows listening TCP/UDP sockets. Requires iproute2.",
            "/usr/bin/ss",
            &["-tuln"],
            false,
        ),
        Action::new(
            "Package file check",
            "Checks installed package files for missing paths. Can produce a long log.",
            "/usr/bin/pacman",
            &["-Qk"],
            false,
        ),
    ]
}
#[derive(Default)]
pub(super) struct Scan {
    pub inventory: system::Inventory,
    pub installed: BTreeMap<String, String>,
    pub available: BTreeSet<String>,
    pub repository_error: Option<String>,
}
pub(super) fn scan() -> Scan {
    let inventory = system::Inventory::load();
    let installed = inventory
        .packages
        .iter()
        .map(|p| (p.name.clone(), p.version.clone()))
        .collect();
    let repositories = cli::output("/usr/bin/pacman", &["-Slq"]);
    Scan {
        inventory,
        installed,
        available: repositories
            .as_ref()
            .map(|s| s.lines().map(str::to_owned).collect())
            .unwrap_or_default(),
        repository_error: repositories.err(),
    }
}
pub(super) fn install_plan(names: &BTreeSet<String>, scan: &Scan) -> Result<Action, String> {
    if names.len() > 5000 {
        return Err("Selection exceeds 5,000 packages. Narrow it before continuing.".into());
    }
    if scan.inventory.error.is_some() {
        return Err("Installed package scan failed. Rescan before installing.".into());
    }
    let missing: BTreeSet<_> = names
        .iter()
        .map(|n| crate::package_variants::installed_name(&scan.installed, n).to_owned())
        .filter(|n| !scan.installed.contains_key(n))
        .collect();
    if missing.is_empty() {
        return Err("No missing packages selected. Installed packages are skipped.".into());
    }
    let unknown: Vec<_> = missing
        .iter()
        .filter(|n| !cli::valid_name(n) || !scan.available.contains(*n))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "Not in configured repository databases: {}. AUR/foreign recipes require separate source review. Nothing was run.",
            unknown.join(", ")
        ));
    }
    let mut action = update();
    action.title = format!("Install {} missing packages", missing.len());
    action.description="Refreshes repositories, performs a FULL system upgrade and installs selected missing packages. Already-installed packages are skipped. Review pacman's transaction.".into();
    action.args.extend(["--needed".into(), "--".into()]);
    action.args.extend(missing);
    Ok(action)
}
pub(super) fn removal(names: &BTreeSet<String>, scan: &Scan) -> Result<Action, String> {
    if names.len() > 5000 {
        return Err("Selection exceeds 5,000 packages. Narrow it before continuing.".into());
    }
    if scan.inventory.error.is_some() {
        return Err("Installed package scan failed. Rescan before removing.".into());
    }
    let selected: BTreeSet<_> = names
        .iter()
        .map(|n| crate::package_variants::installed_name(&scan.installed, n).to_owned())
        .collect();
    if selected.is_empty()
        || selected
            .iter()
            .any(|n| !cli::valid_name(n) || !scan.installed.contains_key(n))
    {
        return Err("Select only exact installed packages for removal. Nothing was run.".into());
    }
    let mut a = Action::new(
        "Uninstall selected packages",
        "Removes these packages after pacman's own confirmation. Pacman blocks removal if other packages require them. Dependencies are never force-removed.",
        "/usr/bin/pacman",
        &["-R", "--"],
        true,
    );
    a.args.extend(selected);
    Ok(a)
}
pub(super) fn write_new(path: &str, text: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Choose a filename.".into());
    }
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("{path}: {e}. Existing files are never overwritten."))?;
    f.write_all(text.as_bytes()).map_err(|e| e.to_string())
}
/// Saved selections retain real installed identities, never a guessed AUR source.
pub(super) fn selection_profile(names: &BTreeSet<String>, scan: &Scan) -> Result<String, String> {
    if names.len() > 5000 || names.iter().any(|n| !cli::valid_name(n)) {
        return Err("Choose at most 5,000 valid package names before exporting.".into());
    }
    let actual: BTreeSet<_> = names
        .iter()
        .map(|n| crate::package_variants::installed_name(&scan.installed, n).to_owned())
        .collect();
    let packages:Vec<_>=actual.iter().map(|n|serde_json::json!({"name":n,"source":if scan.repository_error.is_none() && scan.available.contains(n){"repo"}else{"foreign"}})).collect();
    serde_json::to_string_pretty(&serde_json::json!({"format":"archforge-loadout","version":1,"name":"My terminal loadout","repositories":[],"packages":packages,"dotfiles":{},"executable":[],"modes":{}})).map_err(|e|e.to_string())
}
pub(super) fn capture(path: &str) -> Result<String, String> {
    let explicit = cli::names(&cli::output("/usr/bin/pacman", &["-Qqe"])?)?;
    let native: BTreeSet<_> = cli::output("/usr/bin/pacman", &["-Qnq"])?
        .lines()
        .map(str::to_owned)
        .collect();
    let json = serde_json::json!({"format":"archforge-loadout","version":1,"name":"My terminal loadout","repositories":[],"packages":explicit.iter().map(|n|serde_json::json!({"name":n,"source":if native.contains(n){"repo"}else{"foreign"}})).collect::<Vec<_>>(),"dotfiles":{},"executable":[],"modes":{}});
    write_new(
        path,
        &serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?,
    )?;
    Ok(format!(
        "Saved {} package names to {path}. No dotfiles or credentials included. Keep a copy on another drive.",
        explicit.len()
    ))
}
pub(super) fn import(path: &str) -> Result<(Vec<String>, usize), String> {
    let mut text = String::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(16 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 16 * 1024 * 1024 {
        return Err("Profile exceeds 16 MiB.".into());
    }
    cli::profile_names(&text)
}
pub(super) fn clean_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&strip_ansi_escapes::strip(bytes))
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}
