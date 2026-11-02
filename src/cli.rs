//! Line-oriented terminal interface: no GTK initialization, raw mode or shell evaluation.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{self, IsTerminal, Write},
    process::{Command, ExitStatus},
};

fn heading(title: &str) {
    if io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        println!("\n\x1b[1;33m  ARCH FORGE  /  {title}\x1b[0m\n");
    } else {
        println!("\n  ARCH FORGE / {title}\n");
    }
}
fn input(prompt: &str) -> Result<String, String> {
    print!("  {prompt} ");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    if io::stdin()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?
        == 0
    {
        return Err("Input closed; nothing else will run.".into());
    }
    Ok(line.trim().into())
}
pub(crate) fn output(program: &str, args: &[&str]) -> Result<String, String> {
    crate::system::run(program, args, 30)
}
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
}
pub(crate) fn names(text: &str) -> Result<Vec<String>, String> {
    let result: BTreeSet<String> = text.split_whitespace().map(str::to_owned).collect();
    if result.is_empty() || result.len() > 5000 || result.iter().any(|n| !valid_name(n)) {
        return Err(
            "Enter exact package names separated by spaces (no options or shell commands).".into(),
        );
    }
    Ok(result.into_iter().collect())
}
pub(crate) fn installed() -> Result<BTreeMap<String, String>, String> {
    Ok(output("/usr/bin/pacman", &["-Q"])?
        .lines()
        .filter_map(|line| {
            line.split_once(' ')
                .map(|(n, v)| (n.to_owned(), v.to_owned()))
        })
        .collect())
}
fn status_message(status: ExitStatus) {
    if status.success() {
        println!("\n  Completed.");
    } else {
        println!(
            "\n  Command exited with {status}. Read the output above; the operation may be incomplete."
        );
    }
}
fn inspect(description: &str, program: &str, args: &[&str]) -> Result<(), String> {
    heading(description);
    println!("  Read-only · {program} {}\n", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("{program}: {e}"))?;
    status_message(status);
    Ok(())
}
// Keep the parent alive during Ctrl+C so credential cleanup still runs.
// A caught signal handler resets to the default disposition in executed children.
struct InterruptGuard(libc::sigaction);
extern "C" fn keep_parent_alive(_: libc::c_int) {}
impl InterruptGuard {
    fn new() -> Result<Self, String> {
        // SAFETY: sigaction structures are initialized before use and pointers are valid.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            let mut previous: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = keep_parent_alive as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            if libc::sigaction(libc::SIGINT, &action, &mut previous) != 0 {
                return Err(io::Error::last_os_error().to_string());
            }
            Ok(Self(previous))
        }
    }
}
impl Drop for InterruptGuard {
    fn drop(&mut self) {
        // SAFETY: restore the previously returned signal action.
        unsafe {
            libc::sigaction(libc::SIGINT, &self.0, std::ptr::null_mut());
        }
    }
}
struct Credentials(std::path::PathBuf);
impl Drop for Credentials {
    fn drop(&mut self) {
        let _ = Command::new(&self.0).arg("-k").status();
    }
}
fn change(description: &str, program: &str, args: &[String], admin: bool) -> Result<(), String> {
    heading("Review command");
    println!(
        "  {description}\n\n  {}{program} {}\n",
        if admin { "sudo " } else { "" },
        args.join(" ")
    );
    println!("  Read the command and transaction. Enter RUN to continue; anything else cancels.");
    if input("Confirm:")? != "RUN" {
        println!("  Cancelled. Nothing was run.");
        return Ok(());
    }
    status_message(execute_reviewed(program, args, admin, None)?);
    Ok(())
}
pub(crate) fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}
pub(crate) fn execute_reviewed(
    program: &str,
    args: &[String],
    admin: bool,
    transcript: Option<&std::path::Path>,
) -> Result<ExitStatus, String> {
    execute_with("/usr/bin/sudo", program, args, admin, transcript)
}
fn execute_with(
    sudo: &str,
    program: &str,
    args: &[String],
    admin: bool,
    transcript: Option<&std::path::Path>,
) -> Result<ExitStatus, String> {
    let _interrupt = InterruptGuard::new()?;
    if let Some(log) = transcript {
        // Authentication and command share the SAME PTY, including sudo's tty timestamp.
        // script records OUTPUT only. Never enable --log-in or --log-io.
        const SESSION: &str = r#"sudo=$1; shift
trap '"$sudo" -k >/dev/null 2>&1' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
if "$sudo" -n -k -v >/dev/null 2>&1; then
  printf '%s\n' 'Passwordless sudo validation is enabled. Command cancelled.' >&2
  exit 77
fi
"$sudo" -k || exit 78
"$sudo" -v || { printf '%s\n' 'Authentication failed or cancelled. Nothing was run.' >&2; exit 78; }
"$sudo" -- "$@"
"#;
        let mut words: Vec<&str> = if admin {
            vec![
                "/usr/bin/bash",
                "--noprofile",
                "--norc",
                "-c",
                SESSION,
                "archforge-session",
                sudo,
                program,
            ]
        } else {
            vec![program]
        };
        words.extend(args.iter().map(String::as_str));
        let shell = words
            .into_iter()
            .map(shell_quote)
            .collect::<Vec<_>>()
            .join(" ");
        return Command::new("/usr/bin/script")
            .args([
                "-q",
                "-e",
                "-f",
                "-c",
                &format!("exec {shell}"),
                "--log-out",
            ])
            .arg(log)
            .status()
            .map_err(|e| format!("Could not start interactive terminal session: {e}"));
    }
    let _credentials = if admin {
        // Match GUI policy: discard cached credentials and reject passwordless validation.
        let probe = Command::new(sudo)
            .args(["-n", "-k", "-v"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        let guard = Credentials(sudo.into());
        if probe.success() {
            return Err("Passwordless sudo validation is enabled. Fresh interactive authentication is required; command cancelled.".into());
        }
        if !Command::new(sudo)
            .arg("-k")
            .status()
            .map_err(|e| e.to_string())?
            .success()
            || !Command::new(sudo)
                .arg("-v")
                .status()
                .map_err(|e| e.to_string())?
                .success()
        {
            return Err("Authentication failed or cancelled. The command was not run.".into());
        }
        Some(guard)
    } else {
        None
    };
    let mut cmd = if admin {
        let mut c = Command::new(sudo);
        c.arg("--").arg(program);
        c
    } else {
        Command::new(program)
    };
    cmd.args(args);
    cmd.status().map_err(|e| e.to_string())
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}
fn overview() -> Result<(), String> {
    heading("System overview");
    for (title, program, args) in [
        ("Kernel", "/usr/bin/uname", vec!["-sr"]),
        ("Uptime", "/usr/bin/uptime", vec!["-p"]),
        ("Memory", "/usr/bin/free", vec!["-h"]),
        ("Storage", "/usr/bin/df", vec!["-h", "/"]),
    ] {
        println!(
            "  {title}\n{}",
            output(program, &args).unwrap_or_else(|e| e)
        );
    }
    println!("  Installed packages: {}", installed()?.len());
    Ok(())
}
fn install(selected: Vec<String>) -> Result<(), String> {
    let have = installed()?;
    let selected = selected
        .into_iter()
        .map(|n| crate::package_variants::installed_name(&have, &n).to_owned())
        .collect::<BTreeSet<_>>();
    let missing: Vec<_> = selected
        .into_iter()
        .filter(|n| {
            if have.contains_key(n) {
                println!("  Skipping {n}: already installed.");
                false
            } else {
                true
            }
        })
        .collect();
    if missing.is_empty() {
        println!("  Everything selected is already installed.");
        return Ok(());
    }
    let available: BTreeSet<String> = output("/usr/bin/pacman", &["-Slq"])?
        .lines()
        .map(str::to_owned)
        .collect();
    let unavailable: Vec<_> = missing.iter().filter(|n| !available.contains(*n)).collect();
    if !unavailable.is_empty() {
        return Err(format!(
            "Not found in configured repository databases: {}. Nothing was run. AUR recipes require separate source review; this installer does not build them.",
            unavailable
                .iter()
                .map(|n| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut args = strings(&["-Syu", "--needed", "--"]);
    args.extend(missing);
    change(
        "Refreshes repositories, upgrades the whole system and installs the selected missing packages. Pacman will ask you to confirm its transaction.",
        "/usr/bin/pacman",
        &args,
        true,
    )
}
fn packages() -> Result<(), String> {
    heading("Packages");
    println!(
        "  1  Search repositories\n  2  Find installed package\n  3  Install packages\n  4  Remove packages\n  5  Check repository updates\n  6  Full system upgrade\n  7  List foreign / AUR packages\n  0  Back"
    );
    match input("Choose:")?.as_str() {
        "1" => {
            let q = input("Search:")?;
            if !q.is_empty() {
                inspect("Repository search", "/usr/bin/pacman", &["-Ss", "--", &q])?;
            }
        }
        "2" => {
            let q = input("Search installed:")?;
            if !q.is_empty() {
                inspect("Installed packages", "/usr/bin/pacman", &["-Qs", "--", &q])?;
            }
        }
        "3" => install(names(&input("Package names:")?)?)?,
        "4" => {
            let selected = names(&input("Exact installed package names to remove:")?)?;
            let have = installed()?;
            if selected.iter().any(|n| !have.contains_key(n)) {
                return Err("One or more names are not installed. Nothing was run.".into());
            }
            let mut args = strings(&["-R", "--"]);
            args.extend(selected);
            change(
                "Removes only these packages. Pacman blocks removal if other packages depend on them and asks for confirmation. Dependencies are not force-removed.",
                "/usr/bin/pacman",
                &args,
                true,
            )?;
        }
        "5" => {
            heading("Repository updates");
            println!("  Uses checkupdates' separate database; does not check AUR recipes.\n");
            let status = Command::new("/usr/bin/checkupdates")
                .status()
                .map_err(|e| format!("Install pacman-contrib to use checkupdates: {e}"))?;
            if status.code() == Some(2) {
                println!("  No repository updates available.");
            } else {
                status_message(status);
            }
        }
        "6" => change(
            "Refreshes repository databases and upgrades ALL system packages. Review pacman's transaction.",
            "/usr/bin/pacman",
            &strings(&["-Syu"]),
            true,
        )?,
        "7" => inspect("Foreign / AUR packages", "/usr/bin/pacman", &["-Qm"])?,
        "0" => (),
        _ => println!("  Unknown choice."),
    }
    Ok(())
}
fn cleanup() -> Result<(), String> {
    heading("Cleanup");
    println!(
        "  1  Preview older cached packages (keep 2 versions)\n  2  Remove older cached packages (keep 2 versions)\n  3  Measure journal storage\n  4  Keep 7 days of archived journals\n  5  Inspect orphan dependencies\n  0  Back"
    );
    match input("Choose:")?.as_str() {
        "1" => inspect(
            "Cache cleanup candidates",
            "/usr/bin/paccache",
            &["-d", "-k", "2"],
        )?,
        "2" => change(
            "Permanently deletes older package archives, keeping the latest 2 versions. Installed packages are not removed. Use Preview first.",
            "/usr/bin/paccache",
            &strings(&["-r", "-k", "2"]),
            true,
        )?,
        "3" => inspect("Journal storage", "/usr/bin/journalctl", &["--disk-usage"])?,
        "4" => change(
            "Permanently deletes archived journal files older than 7 days. Active journal files are not removed.",
            "/usr/bin/journalctl",
            &strings(&["--vacuum-time=7d"]),
            true,
        )?,
        "5" => {
            heading("Orphan dependencies");
            let out = Command::new("/usr/bin/pacman")
                .arg("-Qtdq")
                .output()
                .map_err(|e| e.to_string())?;
            if out.status.code() == Some(1) && out.stdout.is_empty() && out.stderr.is_empty() {
                println!("  No orphan dependencies found.");
            } else {
                print!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                status_message(out.status);
            }
            println!(
                "  Review names before using Packages → Remove. Orphans are not automatically deleted."
            );
        }
        "0" => (),
        _ => println!("  Unknown choice."),
    }
    Ok(())
}
fn diagnostics() -> Result<(), String> {
    heading("Diagnostics");
    println!(
        "  1  Failed system services\n  2  Boot timing\n  3  Current boot warnings\n  4  Drives and filesystems\n  5  Network interfaces\n  6  Listening sockets\n  7  Package file check\n  0  Back"
    );
    let (title, program, args) = match input("Choose:")?.as_str() {
        "1" => (
            "Failed services",
            "/usr/bin/systemctl",
            vec!["--failed", "--no-pager"],
        ),
        "2" => (
            "Boot timing",
            "/usr/bin/systemd-analyze",
            vec!["blame", "--no-pager"],
        ),
        "3" => (
            "Boot warnings",
            "/usr/bin/journalctl",
            vec!["-b", "-p", "warning", "-n", "100", "--no-pager"],
        ),
        "4" => ("Drives", "/usr/bin/lsblk", vec!["-f"]),
        "5" => (
            "Network interfaces",
            "/usr/bin/ip",
            vec!["-brief", "address"],
        ),
        "6" => ("Listening sockets", "/usr/bin/ss", vec!["-tuln"]),
        "7" => ("Package file check", "/usr/bin/pacman", vec!["-Qk"]),
        "0" => return Ok(()),
        _ => return Err("Unknown choice.".into()),
    };
    inspect(title, program, &args)
}

pub(crate) fn catalog() -> Vec<Vec<&'static str>> {
    // Curated terminal-capable tools; do not assume whole GUI catalog categories are headless.
    const HEADLESS: &[&str] = &[
        "git",
        "base-devel",
        "curl",
        "wget",
        "openssh",
        "sudo",
        "less",
        "unzip",
        "zip",
        "7zip",
        "man-db",
        "man-pages",
        "bash-completion",
        "nano",
        "vim",
        "neovim",
        "emacs-nox",
        "tmux",
        "zsh",
        "fish",
        "starship",
        "fzf",
        "ripgrep",
        "fd",
        "bat",
        "eza",
        "zoxide",
        "btop",
        "htop",
        "fastfetch",
        "tree",
        "which",
        "jq",
        "yq",
        "rsync",
        "restic",
        "borg",
        "rclone",
        "testdisk",
        "smartmontools",
        "nvme-cli",
        "ncdu",
        "duf",
        "dust",
        "lsof",
        "strace",
        "tcpdump",
        "nmap",
        "bind",
        "mtr",
        "traceroute",
        "wireguard-tools",
        "tailscale",
        "ufw",
        "firewalld",
        "pacman-contrib",
        "reflector",
        "archlinux-keyring",
        "rust",
        "go",
        "python",
        "python-pip",
        "nodejs",
        "npm",
        "pnpm",
        "docker",
        "docker-compose",
        "podman",
        "cmake",
        "make",
        "gcc",
        "gdb",
        "lldb",
        "shellcheck",
        "shfmt",
        "just",
        "ansible",
        "terraform",
        "ipython",
        "python-numpy",
        "python-pandas",
        "ollama",
        "aichat",
        "openai-codex",
        "gemini-cli",
        "claude-code",
        "aider-chat",
    ];
    let mut apps: Vec<_> = include_str!("../data/loadouts/catalog.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split('|').collect::<Vec<_>>())
        .filter(|f| f.len() == 8 && HEADLESS.contains(&f[1]))
        .collect();
    apps.sort_by_key(|f| {
        (
            f[6].parse::<u8>().unwrap_or(2),
            std::cmp::Reverse(f[7].parse::<u32>().unwrap_or(0)),
            f[2],
        )
    });
    apps
}
fn browse() -> Result<(), String> {
    heading("Terminal package library");
    println!(
        "  Curated tools from the desktop catalog. Recommended choices appear first.\n  AUR entries are shown for reference; automated installation uses configured repositories only."
    );
    let q = input("Search name/category (Enter for all):")?.to_lowercase();
    let have = installed()?;
    let apps: Vec<_> = catalog()
        .into_iter()
        .filter(|f| {
            format!("{} {} {} {}", f[0], f[1], f[2], f[3])
                .to_lowercase()
                .contains(&q)
        })
        .collect();
    for (i, f) in apps.iter().enumerate() {
        let actual = crate::package_variants::installed_name(&have, f[1]);
        let state = if have.contains_key(actual) {
            format!("installed: {actual}")
        } else {
            "not installed".into()
        };
        println!(
            "  {:>3}  {:<23} {:<20} [{} · {}]\n       {}",
            i + 1,
            f[1],
            f[0],
            state,
            f[5],
            f[3]
        );
    }
    if apps.is_empty() {
        println!("  No matches.");
        return Ok(());
    }
    let choice = input("Select numbers separated by spaces to install (Enter to go back):")?;
    if choice.is_empty() {
        return Ok(());
    }
    let mut selected = BTreeSet::new();
    for number in choice.split_whitespace() {
        let i = number
            .parse::<usize>()
            .ok()
            .filter(|i| *i > 0)
            .and_then(|i| apps.get(i - 1))
            .ok_or("Invalid selection. Nothing was run.")?;
        selected.insert(crate::package_variants::installed_name(&have, i[1]).to_owned());
    }
    install(selected.into_iter().collect())
}
fn capture() -> Result<(), String> {
    heading("Capture package loadout");
    println!(
        "  Saves explicitly installed package names in a desktop-compatible JSON profile.\n  No dotfiles, credentials, repository URLs or personal files are included."
    );
    let explicit = output("/usr/bin/pacman", &["-Qqe"])?;
    let repos: BTreeSet<_> = output("/usr/bin/pacman", &["-Qnq"])?
        .lines()
        .map(str::to_owned)
        .collect();
    let selected = names(&explicit)?;
    let foreign = selected.iter().filter(|n| !repos.contains(*n)).count();
    println!(
        "  {} packages · {foreign} foreign/local packages (require source review on import).",
        selected.len()
    );
    let path = input("New output file (Enter cancels):")?;
    if path.is_empty() {
        return Ok(());
    }
    let profile = serde_json::json!({"format":"archforge-loadout","version":1,"name":"My terminal loadout","repositories":[],"packages":selected.iter().map(|n|serde_json::json!({"name":n,"source":if repos.contains(n){"repo"}else{"foreign"}})).collect::<Vec<_>>(),"dotfiles":{},"executable":[],"modes":{}});
    let text = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| {
            format!("Could not create {path}: {e}. Existing files are never overwritten.")
        })?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    println!("  Saved {path}. Keep a copy on another drive.");
    Ok(())
}
pub(crate) fn profile_names(text: &str) -> Result<(Vec<String>, usize), String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if value["format"] != "archforge-loadout" || value["version"] != 1 {
        return Err("Unsupported loadout format.".into());
    }
    let entries = value["packages"].as_array().ok_or("Missing packages")?;
    if entries.len() > 5000 {
        return Err("Too many packages.".into());
    }
    let mut selected = BTreeSet::new();
    let mut skipped = 0;
    for entry in entries {
        let name = entry["name"].as_str().ok_or("Invalid package entry")?;
        if !valid_name(name) {
            return Err("Invalid package name. Nothing was run.".into());
        }
        match entry["source"].as_str() {
            Some("repo") => {
                selected.insert(name.to_owned());
            }
            Some("aur" | "foreign") => skipped += 1,
            _ => return Err("Unknown package source. Nothing was run.".into()),
        }
    }
    Ok((selected.into_iter().collect(), skipped))
}
fn import() -> Result<(), String> {
    heading("Import package loadout");
    let path = input("Profile file (Enter cancels):")?;
    if path.is_empty() {
        return Ok(());
    }
    use std::io::Read;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut text = String::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 16 * 1024 * 1024 {
        return Err("Profile exceeds 16 MiB.".into());
    }
    let (selected, skipped) = profile_names(&text)?;
    println!(
        "  {} repository packages. {skipped} AUR/foreign entries excluded.\n  Dotfiles and desktop configuration are not applied.",
        selected.len()
    );
    if selected.is_empty() {
        return Ok(());
    }
    println!("  {}", selected.join(" "));
    install(selected)
}
fn guide() -> Result<(), String> {
    heading("Wiki / command reference");
    println!(
        "  Reference only. UPPERCASE words are placeholders; review upstream documentation.\n"
    );
    let q = input("Search commands/topics (Enter for all):")?.to_lowercase();
    let mut count = 0;
    for e in crate::wiki_data::ENTRIES {
        let topic = &crate::wiki_data::TOPICS[e.topic];
        if e.topic == 7 || e.command.starts_with("glxinfo ") {
            continue;
        } // No compositor tools in the terminal-only guide.
        if !format!(
            "{} {} {} {}",
            topic.title, e.title, e.command, e.description
        )
        .to_lowercase()
        .contains(&q)
        {
            continue;
        }
        println!(
            "  {} / {} [{}]\n    {}\n    {}{}\n    {}\n",
            topic.title,
            e.title,
            e.effect,
            e.command,
            e.description,
            if e.placeholder {
                " Replace placeholders first."
            } else {
                ""
            },
            topic.url
        );
        count += 1;
    }
    println!("  {count} matches. Commands here are never executed automatically.");
    Ok(())
}
fn plain_report(topic: &str) -> bool {
    let result = match topic {
        "packages" => output("/usr/bin/pacman", &["-Q"]).map(|s| print!("{s}")),
        "diagnostics" => {
            output("/usr/bin/systemctl", &["--failed", "--no-pager"]).map(|s| print!("{s}"))
        }
        "wiki" => {
            for e in crate::wiki_data::ENTRIES
                .iter()
                .filter(|e| e.topic != 7 && !e.command.starts_with("glxinfo "))
            {
                println!("{}\n  {}\n  {}\n", e.title, e.command, e.description);
            }
            Ok(())
        }
        _ => overview(),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        false
    } else {
        true
    }
}
pub(crate) fn run() -> bool {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|s| s == "--help" || s == "-h") {
        println!(
            "Arch Forge terminal {}\n\nUsage: archforge-cli [--help | --version | --plain [overview|packages|diagnostics|wiki]]\n       archforge --tui\n\nKeyboard-driven TUI for system overview, packages, cleanup, diagnostics,\nterminal tools, package loadouts and an offline Wiki.\nInteractive mode uses Ratatui. Redirected input/output produces a plain read-only report.\nRequires Arch Linux. No graphical session needed.\nNO_COLOR disables interface colors.",
            env!("CARGO_PKG_VERSION")
        );
        return true;
    }
    if args.iter().any(|s| s == "--version" || s == "-V") {
        println!("archforge-cli {}", env!("CARGO_PKG_VERSION"));
        return true;
    }
    if args.iter().any(|s| {
        ![
            "--tui",
            "--plain",
            "overview",
            "packages",
            "diagnostics",
            "wiki",
        ]
        .contains(&s.as_str())
    }) {
        eprintln!("Unknown option. Use archforge-cli --help.");
        return false;
    }
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("Run as your regular account. Reviewed changes ask for sudo authentication.");
        return false;
    }
    let release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    if !release
        .lines()
        .any(|line| line == "ID=arch" || line == "ID=\"arch\"")
    {
        eprintln!("Arch Forge supports Arch Linux only (ID=arch).");
        return false;
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return plain_report(
            args.iter()
                .find(|s| ["overview", "packages", "diagnostics", "wiki"].contains(&s.as_str()))
                .map(String::as_str)
                .unwrap_or("overview"),
        );
    }
    if !args.iter().any(|s| s == "--plain") {
        return match crate::tui::run() {
            Ok(()) => true,
            Err(e) => {
                eprintln!("Terminal interface: {e}");
                false
            }
        };
    }
    if let Some(topic) = args
        .iter()
        .find(|s| ["overview", "packages", "diagnostics", "wiki"].contains(&s.as_str()))
    {
        return plain_report(topic);
    }
    loop {
        heading("Terminal workbench");
        println!(
            "  1  Overview          System, memory and storage\n  2  Packages          Search, install, remove and update\n  3  Cleanup           Review caches and archived logs\n  4  Diagnostics       Services, disks and networking\n  5  Build loadout     Browse terminal tools and bulk install\n  6  Capture loadout   Save this system's package selection\n  7  Import loadout    Review repository packages from a profile\n  8  Wiki              Search the offline command guide\n  0  Quit\n"
        );
        let choice = match input("Choose:") {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                return false;
            }
        };
        let result = match choice.as_str() {
            "0" | "q" | "quit" => return true,
            "1" => overview(),
            "2" => packages(),
            "3" => cleanup(),
            "4" => diagnostics(),
            "5" => browse(),
            "6" => capture(),
            "7" => import(),
            "8" => guide(),
            _ => {
                println!("  Choose a number from 0 to 8.");
                continue;
            }
        };
        if let Err(e) = result {
            eprintln!("\n  {e}");
        }
        if input("Press Enter to return to the menu.").is_err() {
            return false;
        }
    }
}
