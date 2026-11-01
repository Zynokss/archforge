use serde_json::{Value, json};
use std::os::unix::fs::OpenOptionsExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    },
    path::{Component, Path, PathBuf},
};
pub(crate) const MAX_PROFILE: usize = 16 * 1024 * 1024;
const MAX_TEXT: usize = 1024 * 1024;
const HOME_TOKEN: &str = "@ARCHFORGE_HOME@";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Repo,
    Aur,
    Foreign,
}
impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Self::Repo => "Repository",
            Self::Aur => "AUR",
            Self::Foreign => "Foreign",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Repo => "repo",
            Self::Aur => "aur",
            Self::Foreign => "foreign",
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct App {
    pub name: String,
    pub title: String,
    pub category: String,
    pub description: String,
    pub icon: String,
    pub source: Source,
}
pub(crate) const CATEGORIES: &[&str] = &[
    "All packages",
    "Essentials",
    "Browsers",
    "Development",
    "Gaming",
    "Media",
    "Communication",
    "System utilities",
    "Desktop & styling",
    "Imported / custom",
];
#[derive(Clone, Debug)]
pub(crate) struct Profile {
    pub name: String,
    pub packages: BTreeMap<String, Source>,
    pub repositories: Vec<String>,
    pub files: BTreeMap<String, String>,
    pub executable: BTreeSet<String>,
    pub modes: BTreeMap<String, u32>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "My Arch loadout".into(),
            packages: BTreeMap::new(),
            repositories: Vec::new(),
            files: BTreeMap::new(),
            executable: BTreeSet::new(),
            modes: BTreeMap::new(),
        }
    }
}
pub(crate) fn valid_package(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
}
pub(crate) const CONFIGS: &[(&str, &str, bool)] = &[
    ("Hyprland", ".config/hypr", true),
    ("Sway", ".config/sway", true),
    ("Waybar", ".config/waybar", true),
    ("Quickshell", ".config/quickshell", true),
    ("Kitty", ".config/kitty", true),
    ("Alacritty", ".config/alacritty", true),
    ("Fish", ".config/fish", true),
    ("Starship", ".config/starship.toml", false),
    ("Zsh", ".zshrc", false),
    ("GTK 3", ".config/gtk-3.0/settings.ini", false),
    ("GTK 4", ".config/gtk-4.0/settings.ini", false),
    ("KDE colors", ".config/kdeglobals", false),
    ("KWin", ".config/kwinrc", false),
    ("KDE color schemes", ".local/share/color-schemes", true),
];
pub(crate) fn valid_config(path: &str) -> bool {
    path.len() <= 4096
        && path.split('/').count() <= 20
        && (!path.starts_with(".local/share/color-schemes/")
            || (path.ends_with(".colors") && path.split('/').count() == 4))
        && !path.contains(['\0', '\r', '\n'])
        && !path.starts_with('/')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && !path
            .split('/')
            .any(|p| p == ".." || p == "." || p.is_empty())
        && CONFIGS.iter().any(|(_, root, dir)| {
            if *dir {
                path.starts_with(&format!("{root}/"))
            } else {
                path == *root
            }
        })
}
impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 100
            || self.name.chars().any(char::is_control)
        {
            return Err("Give the profile a name of 1–100 characters.".into());
        }
        if self.repositories.len() > 64 || self.repositories.iter().any(|name| !valid_package(name))
        {
            return Err("Invalid repository-name metadata.".into());
        }
        if self.packages.len() > 5000 || self.packages.keys().any(|n| !valid_package(n)) {
            return Err("Invalid package name or too many selected packages.".into());
        }
        if self.files.len() > 2000
            || self
                .files
                .iter()
                .any(|(p, t)| !valid_config(p) || t.len() > MAX_TEXT || t.contains('\0'))
            || self.files.values().map(String::len).sum::<usize>() > 8 * 1024 * 1024
            || self.executable.iter().any(|p| !self.files.contains_key(p))
            || self
                .modes
                .iter()
                .any(|(p, m)| !self.files.contains_key(p) || m & !0o777 != 0 || m & 0o400 == 0)
        {
            return Err("The dotfile bundle contains an unsupported path, invalid text, or exceeds its size limits.".into());
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<String, String> {
        self.validate()?;
        let text = serde_json::to_string_pretty(&json!({"format":"archforge-loadout","version":1,"name":self.name,"repositories":self.repositories,"packages":self.packages.iter().map(|(name,source)|json!({"name":name,"source":source.id()})).collect::<Vec<_>>(),"dotfiles":self.files,"executable":self.executable,"modes":self.modes})).map_err(|e|e.to_string())?;
        if text.len() > MAX_PROFILE {
            return Err("The encoded profile exceeds 16 MiB. Select fewer dotfiles.".into());
        }
        Ok(text)
    }
    pub fn decode(text: &str) -> Result<Self, String> {
        if text.len() > MAX_PROFILE {
            return Err("Profile exceeds the 16 MiB limit.".into());
        }
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if v["format"] != "archforge-loadout" || v["version"] != 1 {
            return Err("This is not a supported Arch Forge loadout profile.".into());
        }
        let mut p = Self {
            name: v["name"].as_str().ok_or("Missing profile name")?.into(),
            ..Self::default()
        };
        if let Some(repositories) = v.get("repositories") {
            for repo in repositories.as_array().ok_or("Invalid repository list")? {
                p.repositories
                    .push(repo.as_str().ok_or("Invalid repository name")?.into());
            }
        }
        let packages = v["packages"].as_array().ok_or("Missing package list")?;
        if packages.len() > 5000 {
            return Err("Too many packages in this profile.".into());
        }
        for item in packages {
            let name = item["name"].as_str().ok_or("Invalid package entry")?;
            let source = match item["source"].as_str() {
                Some("repo") => Source::Repo,
                Some("aur") => Source::Aur,
                Some("foreign") => Source::Foreign,
                _ => return Err("Unknown package source".into()),
            };
            if p.packages.insert(name.into(), source).is_some() {
                return Err("Duplicate package entries are not allowed.".into());
            }
        }
        let files = v["dotfiles"].as_object().ok_or("Missing dotfile map")?;
        if files.len() > 2000 {
            return Err("Too many dotfiles in this profile.".into());
        }
        for (path, text) in files {
            p.files.insert(
                path.clone(),
                text.as_str().ok_or("Dotfile content must be text")?.into(),
            );
        }
        if let Some(entries) = v.get("executable") {
            for item in entries.as_array().ok_or("Invalid executable-file list")? {
                p.executable
                    .insert(item.as_str().ok_or("Invalid executable path")?.into());
            }
        }
        if let Some(modes) = v.get("modes") {
            for (path, mode) in modes.as_object().ok_or("Invalid file modes")? {
                p.modes.insert(
                    path.clone(),
                    mode.as_u64()
                        .filter(|m| *m <= 0o777)
                        .ok_or("Invalid file mode")? as u32,
                );
            }
        }
        p.validate()?;
        Ok(p)
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        let mut text = String::new();
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Choose a regular JSON profile file.".into());
        }
        file.take((MAX_PROFILE + 1) as u64)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        Self::decode(&text)
    }
    pub fn command(&self, foreign_as_aur: bool) -> Result<String, String> {
        self.validate()?;
        if self.packages.contains_key("code")
            && self.packages.contains_key("visual-studio-code-bin")
        {
            return Err(
                "Choose Code - OSS or Visual Studio Code. Their package files can conflict.".into(),
            );
        }
        let mut repo = Vec::new();
        let mut aur = Vec::new();
        for (name, source) in &self.packages {
            match source {
                Source::Repo => repo.push(name.as_str()),
                Source::Aur => aur.push(name.as_str()),
                Source::Foreign if foreign_as_aur => aur.push(name.as_str()),
                _ => {}
            }
        }
        let mut commands = Vec::new();
        if !repo.is_empty() {
            commands.push(format!("sudo pacman -Syu --needed -- {}", repo.join(" ")));
        }
        if !aur.is_empty() {
            let packages = aur.join(" ");
            commands.push(format!("(if command -v paru >/dev/null 2>&1; then paru -S --needed -- {packages}; elif command -v yay >/dev/null 2>&1; then yay -S --needed -- {packages}; else printf '%s\\n' 'Install and review an AUR helper (paru or yay), then rerun this command.' >&2; exit 1; fi)"));
        }
        if commands.is_empty() {
            return Err("Select repository or AUR packages. Foreign packages need an explicit source decision before bulk installation.".into());
        }
        Ok(commands.join(" && "))
    }
    pub fn script(&self, foreign_as_aur: bool) -> Result<String, String> {
        let command = self.command(foreign_as_aur)?;
        Ok(format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n# Generated by Arch Forge. Packages only; dotfiles require a separate review.\nif (( EUID == 0 )); then printf '%s\\n' 'Run this script as your regular user; pacman uses sudo.' >&2; exit 1; fi\n{command}\n"
        ))
    }
}
fn query(args: &[&str]) -> Result<Vec<String>, String> {
    match crate::system::run("/usr/bin/pacman", args, 30) {
        Ok(s) => Ok(s
            .lines()
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect()),
        Err(e) if e.trim() == "Exit 1" => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}
pub(crate) fn capture_packages() -> Result<Profile, String> {
    let mut p = Profile::default();
    for name in query(&["-Qqen"])? {
        p.packages.insert(name, Source::Repo);
    }
    for name in query(&["-Qqem"])? {
        p.packages.insert(name, Source::Foreign);
    }
    p.repositories = crate::system::run("/usr/bin/pacman-conf", &["--repo-list"], 5)?
        .lines()
        .map(str::to_owned)
        .collect();
    p.validate()?;
    Ok(p)
}
fn cstr(s: &std::ffi::OsStr) -> Result<std::ffi::CString, String> {
    std::ffi::CString::new(s.as_bytes()).map_err(|e| e.to_string())
}
fn open_root(path: &Path) -> Result<File, String> {
    let p = cstr(path.as_os_str())?;
    let fd = unsafe {
        libc::open(
            p.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(format!(
            "{}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn dir_at(root: &File, path: &Path, create: bool) -> Result<Option<File>, String> {
    let mut dir = root.try_clone().map_err(|e| e.to_string())?;
    for part in path.components() {
        let Component::Normal(name) = part else {
            return Err("Unsafe directory component".into());
        };
        let name = cstr(name)?;
        let mut fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
            if !create {
                return Ok(None);
            }
            if unsafe { libc::mkdirat(dir.as_raw_fd(), name.as_ptr(), 0o700) } != 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
        }
        if fd < 0 {
            return Err(format!(
                "Refusing an inaccessible or symlinked directory: {}",
                std::io::Error::last_os_error()
            ));
        }
        dir = unsafe { File::from_raw_fd(fd) };
    }
    Ok(Some(dir))
}
fn read_at(dir: &File, name: &std::ffi::OsStr) -> Result<Option<(String, u32)>, String> {
    use std::os::unix::fs::MetadataExt;
    let name = cstr(name)?;
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err("Refusing a symlinked or unreadable dotfile.".into());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let m = file.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > MAX_TEXT as u64 {
        return Err("Only text files up to 1 MiB are supported.".into());
    }
    let mut text = String::new();
    file.take((MAX_TEXT + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|_| "A selected configuration is not UTF-8 text".to_string())?;
    if text.len() > MAX_TEXT || text.contains('\0') {
        return Err("A selected configuration is too large or not text.".into());
    }
    Ok(Some((text, m.mode() & 0o777)))
}
fn read_relative(root: &File, path: &str) -> Result<Option<(String, u32)>, String> {
    let p = Path::new(path);
    let Some(dir) = dir_at(root, p.parent().unwrap_or(Path::new("")), false)? else {
        return Ok(None);
    };
    read_at(&dir, p.file_name().ok_or("Missing filename")?)
}
pub(crate) fn capture_files(
    profile: &mut Profile,
    home: &Path,
    indices: &[usize],
) -> Result<Vec<String>, String> {
    let root = open_root(home)?;
    let mut skipped = Vec::new();
    for index in indices {
        let (_, path, is_dir) = CONFIGS
            .get(*index)
            .ok_or("Invalid configuration selection")?;
        if *is_dir {
            match dir_at(&root, Path::new(path), false) {
                Ok(Some(dir)) => collect(&dir, path, profile, &mut skipped, home, 0)?,
                Ok(None) => {}
                Err(e) => skipped.push(format!("{path}: {e}")),
            }
        } else {
            match read_relative(&root, path) {
                Ok(Some((text, mode))) => {
                    profile.modes.insert(path.to_string(), mode);
                    profile.files.insert(
                        path.to_string(),
                        text.replace(home.to_str().ok_or("HOME is not UTF-8")?, HOME_TOKEN),
                    );
                    if mode & 0o111 != 0 {
                        profile.executable.insert(path.to_string());
                    }
                }
                Ok(None) => {}
                Err(e) => skipped.push(format!("{path}: {e}")),
            }
        }
    }
    profile.validate()?;
    Ok(skipped)
}
fn collect(
    dir: &File,
    prefix: &str,
    p: &mut Profile,
    skipped: &mut Vec<String>,
    home: &Path,
    depth: usize,
) -> Result<(), String> {
    if depth > 16 {
        return Err("Dotfile directory nesting exceeds the supported depth.".into());
    }
    for entry in
        fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd())).map_err(|e| e.to_string())?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let Some(name_text) = name.to_str() else {
            continue;
        };
        let path = format!("{prefix}/{name_text}");
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if name_text == ".git"
            || name_text == "node_modules"
            || name_text == "cache"
            || name_text == ".env"
            || name_text.to_lowercase().contains("secret")
        {
            skipped.push(path);
            continue;
        }
        if kind.is_dir() {
            if let Some(child) = dir_at(dir, Path::new(&name), false)? {
                collect(&child, &path, p, skipped, home, depth + 1)?;
            }
        } else if kind.is_file() {
            match read_at(dir, &name) {
                Ok(Some((text, mode))) => {
                    p.modes.insert(path.clone(), mode);
                    p.files.insert(
                        path.clone(),
                        text.replace(home.to_str().ok_or("HOME is not UTF-8")?, HOME_TOKEN),
                    );
                    if mode & 0o111 != 0 {
                        p.executable.insert(path);
                    }
                }
                Ok(None) => {}
                Err(e) => skipped.push(format!("{path}: {e}")),
            }
        } else {
            skipped.push(format!("{path}: symlink or special file omitted"));
        }
        p.validate()?;
    }
    Ok(())
}
pub(crate) struct Change {
    pub path: String,
    pub text: String,
    pub previous: Option<String>,
    pub previous_mode: Option<u32>,
    pub mode: u32,
}
pub(crate) struct Plan {
    home: File,
    home_path: PathBuf,
    pub changes: Vec<Change>,
}
pub(crate) fn preview(profile: &Profile, home: &Path) -> Result<Plan, String> {
    use std::os::unix::fs::MetadataExt;
    profile.validate()?;
    let root = open_root(home)?;
    let mut changes = Vec::new();
    let mut existing_bytes = 0usize;
    let mut expanded_bytes = 0usize;
    for (path, text) in &profile.files {
        let current = read_relative(&root, path)?;
        let home_text = home.to_str().ok_or("HOME is not UTF-8")?;
        let count = text.matches(HOME_TOKEN).count();
        let expanded = text
            .len()
            .saturating_sub(count * HOME_TOKEN.len())
            .saturating_add(count.saturating_mul(home_text.len()));
        expanded_bytes = expanded_bytes.saturating_add(expanded);
        existing_bytes =
            existing_bytes.saturating_add(current.as_ref().map(|(t, _)| t.len()).unwrap_or(0));
        if expanded > MAX_TEXT
            || expanded_bytes > 8 * 1024 * 1024
            || existing_bytes > 8 * 1024 * 1024
        {
            return Err("The resolved files or backup exceed the dotfile size limits.".into());
        }
        let desired = text.replace(HOME_TOKEN, home_text);
        let previous_mode = current.as_ref().map(|(_, m)| *m);
        let mode = profile.modes.get(path).copied().unwrap_or_else(|| {
            if profile.executable.contains(path) {
                0o755
            } else {
                previous_mode.unwrap_or(0o644)
            }
        });
        if current
            .as_ref()
            .is_some_and(|(text, current_mode)| text == &desired && *current_mode == mode)
        {
            continue;
        }
        if let Some(parent) = dir_at(
            &root,
            Path::new(path).parent().unwrap_or(Path::new("")),
            false,
        )? {
            let name = cstr(Path::new(path).file_name().unwrap())?;
            let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
            if unsafe {
                libc::fstatat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    st.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } == 0
                && unsafe { st.assume_init() }.st_uid != unsafe { libc::geteuid() }
            {
                return Err(format!(
                    "{path} is owned by another account; it will not be replaced."
                ));
            }
        }
        changes.push(Change {
            path: path.clone(),
            text: desired,
            previous: current.map(|(t, _)| t),
            previous_mode,
            mode,
        });
    }
    let m = root.metadata().map_err(|e| e.to_string())?;
    if m.uid() != unsafe { libc::geteuid() } || home == Path::new("/") {
        return Err("Dotfiles can only be applied to your own home directory.".into());
    }
    Ok(Plan {
        home: root,
        home_path: home.into(),
        changes,
    })
}
fn same_previous(current: Option<(String, u32)>, change: &Change) -> bool {
    match (current, change.previous.as_ref(), change.previous_mode) {
        (None, None, _) => true,
        (Some((text, mode)), Some(old), Some(old_mode)) => text == *old && mode == old_mode,
        _ => false,
    }
}
fn atomic_at(dir: &File, name: &std::ffi::OsStr, text: &str, mode: u32) -> Result<(), String> {
    let target = cstr(name)?;
    let tmp = std::ffi::CString::new(format!(
        ".archforge-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ))
    .unwrap();
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            tmp.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let result = (|| {
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        if unsafe { libc::fchmod(file.as_raw_fd(), mode as libc::mode_t) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        file.sync_all().map_err(|e| e.to_string())?;
        if unsafe {
            libc::renameat(
                dir.as_raw_fd(),
                tmp.as_ptr(),
                dir.as_raw_fd(),
                target.as_ptr(),
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        dir.sync_all().map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        unsafe { libc::unlinkat(dir.as_raw_fd(), tmp.as_ptr(), 0) };
    }
    result
}
pub(crate) fn apply(plan: Plan, state: &Path) -> Result<String, String> {
    use std::os::unix::fs::MetadataExt;
    let fresh = fs::symlink_metadata(&plan.home_path).map_err(|e| e.to_string())?;
    let original = plan.home.metadata().map_err(|e| e.to_string())?;
    if fresh.ino() != original.ino() || fresh.dev() != original.dev() || !fresh.is_dir() {
        return Err("The home directory changed after preview.".into());
    }
    if plan.changes.is_empty() {
        return Ok("Your selected dotfiles already match. No changes needed.".into());
    }
    for change in &plan.changes {
        if !same_previous(read_relative(&plan.home, &change.path)?, change) {
            return Err(format!(
                "{} changed after preview. Nothing was applied; review again.",
                change.path
            ));
        }
    }
    if !state.is_absolute()
        || state
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(
            "The backup state directory must be an absolute path without symlinks or traversal."
                .into(),
        );
    }
    let slash = open_root(Path::new("/"))?;
    let relative = state
        .strip_prefix("/")
        .map_err(|e| e.to_string())?
        .join("archforge/loadouts/backups");
    let backup_dir = dir_at(&slash, &relative, true)?.ok_or("Could not create backup directory")?;
    let metadata = backup_dir.metadata().map_err(|e| e.to_string())?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
        return Err(
            "The backup directory must be owned by your account without group/other write access."
                .into(),
        );
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let backup_name = format!("before-dotfiles-{stamp}.json");
    let mut backup = Profile {
        name: "Before dotfile changes".into(),
        ..Profile::default()
    };
    for c in &plan.changes {
        if let Some(text) = &c.previous {
            backup
                .modes
                .insert(c.path.clone(), c.previous_mode.unwrap_or(0o600));
            backup.files.insert(
                c.path.clone(),
                text.replace(
                    plan.home_path.to_str().ok_or("HOME is not UTF-8")?,
                    HOME_TOKEN,
                ),
            );
        }
    }
    let created: Vec<_> = plan
        .changes
        .iter()
        .filter(|c| c.previous.is_none())
        .map(|c| c.path.clone())
        .collect();
    let mut backup_json: Value =
        serde_json::from_str(&backup.encode()?).map_err(|e| e.to_string())?;
    backup_json["created_files"] = json!(created);
    let backup_text = serde_json::to_string_pretty(&backup_json).map_err(|e| e.to_string())?;
    if backup_text.len() > MAX_PROFILE {
        return Err("The backup metadata exceeds its size limit. Select fewer files; no dotfiles were changed.".into());
    }
    atomic_at(
        &backup_dir,
        std::ffi::OsStr::new(&backup_name),
        &backup_text,
        0o600,
    )?;
    let backup_path = state.join("archforge/loadouts/backups").join(backup_name);
    let mut applied = 0;
    for c in &plan.changes {
        let result = (|| {
            let path = Path::new(&c.path);
            let parent = dir_at(&plan.home, path.parent().unwrap_or(Path::new("")), true)?
                .ok_or("Missing configuration directory")?;
            if !same_previous(read_at(&parent, path.file_name().unwrap())?, c) {
                return Err("The file changed after preview".into());
            }
            atomic_at(&parent, path.file_name().unwrap(), &c.text, c.mode)
        })();
        if let Err(e) = result {
            return Err(format!(
                "Stopped while applying {}: {e}\n{applied} earlier files completed. The current file may already have changed.\nBackup: {}",
                c.path,
                backup_path.display()
            ));
        }
        applied += 1;
    }
    Ok(format!(
        "Applied {applied} dotfiles.\nBackup of replaced files: {}\nNew files (recorded in the backup metadata; remove manually to completely undo): {}. Desktop configurations may be picked up by the running desktop. No reload command was sent.",
        backup_path.display(),
        created.join(", ")
    ))
}
pub(crate) fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p != Path::new("/"))
        .ok_or_else(|| "HOME must point to your home directory".into())
}
pub(crate) fn state_home() -> Result<PathBuf, String> {
    Ok(std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".local/state")))
}
