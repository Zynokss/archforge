use std::{
    ffi::OsStr,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

// The proposed command is passed as one argument, never interpolated into this
// script. Readline presents it for editing; Enter is required before execution.
pub(crate) const PROMPT: &str = r#"
printf '\033]0;Arch Forge\007'
printf 'Arch Forge\nReview or edit the command, then press Enter to run it. Ctrl+C cancels.\n\n'
IFS= read -e -r -i "$1" -p '$ ' command || exit 0
if [[ -n "$command" ]]; then
    /usr/bin/bash --noprofile --norc -c "$command"
    result=$?
    printf '\nCommand finished (exit %s).\n\n' "$result"
fi
shell=${SHELL:-/usr/bin/bash}
[[ -x "$shell" ]] || shell=/usr/bin/bash
exec "$shell" -i
"#;

const TERMINALS: &[&str] = &[
    "xdg-terminal-exec",
    "kitty",
    "foot",
    "alacritty",
    "konsole",
    "gnome-terminal",
    "xfce4-terminal",
    "wezterm",
    "ghostty",
    "xterm",
    "st",
];

fn executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn find(name: &OsStr, path: &OsStr) -> Option<PathBuf> {
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return executable(candidate).then(|| candidate.to_path_buf());
    }
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|p| executable(p))
}

pub(crate) fn select(preferred: Option<&OsStr>, path: &OsStr) -> Result<PathBuf, String> {
    if let Some(preferred) = preferred.filter(|p| !p.is_empty()) {
        let terminal = find(preferred, path).ok_or("The terminal configured in TERMINAL could not be found. Set it to a terminal executable, without extra arguments.")?;
        if !TERMINALS
            .iter()
            .any(|name| terminal.file_name() == Some(OsStr::new(name)))
        {
            return Err("The terminal configured in TERMINAL is not supported. Use xdg-terminal-exec, Kitty, Foot, Alacritty, Konsole, GNOME Terminal, Xfce Terminal, WezTerm, Ghostty, XTerm, or st.".into());
        }
        return Ok(terminal);
    }
    TERMINALS.iter().find_map(|name| find(OsStr::new(name), path))
        .ok_or_else(|| "No supported terminal was found. Install a terminal such as Kitty or Konsole, or copy the command to your terminal.".into())
}

pub(crate) fn command(terminal: &Path, proposed: &str) -> Command {
    let mut cmd = Command::new(terminal);
    match terminal.file_name().and_then(OsStr::to_str).unwrap_or("") {
        "xdg-terminal-exec" => {
            cmd.arg("--");
        }
        "kitty" | "foot" | "gnome-terminal" => {
            cmd.args(["--title", "Arch Forge", "--"]);
        }
        "alacritty" | "ghostty" => {
            cmd.args(["--title", "Arch Forge", "-e"]);
        }
        "konsole" => {
            cmd.args(["--separate", "-p", "tabtitle=Arch Forge", "-e"]);
        }
        "xfce4-terminal" => {
            cmd.args(["--disable-server", "--title", "Arch Forge", "-x"]);
        }
        "wezterm" => {
            cmd.args(["start", "--always-new-process", "--"]);
        }
        "st" => {
            cmd.args(["-t", "Arch Forge", "-e"]);
        }
        _ => {
            cmd.args(["-T", "Arch Forge", "-e"]);
        }
    }
    cmd.args([
        "/usr/bin/bash",
        "--noprofile",
        "--norc",
        "-c",
        PROMPT,
        "archforge",
        proposed,
    ])
    .env_remove("BASH_ENV")
    .env_remove("ENV")
    .process_group(0)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    cmd
}

pub fn open(proposed: &str) -> Result<(), String> {
    if proposed.contains("%command%") || proposed.contains(['\n', '\r', '\0']) {
        return Err("This is not a runnable terminal command. Paste Steam launch options into the game's Properties → Launch Options instead.".into());
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let preferred = std::env::var_os("TERMINAL");
    let terminal = select(preferred.as_deref(), &path)?;
    let mut child = command(&terminal, proposed)
        .spawn()
        .map_err(|e| format!("Could not open the terminal: {e}"))?;
    // Catch immediate startup errors, without waiting for the terminal session.
    let deadline = Instant::now() + Duration::from_millis(600);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(format!(
                    "{} could not open a terminal (exit {}). You can still copy the command.",
                    terminal.display(),
                    status.code().unwrap_or(-1)
                ));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(());
            }
        }
    }
}
