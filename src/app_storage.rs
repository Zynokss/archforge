use std::{
    fs,
    io::Write,
    os::fd::AsRawFd,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
const CONFIG_DIR: &str = "/etc/flatpak/installations.d";
#[derive(Clone, Debug)]
pub(crate) struct Drive {
    pub target: String,
    pub source: String,
    pub uuid: String,
    pub fs: String,
    pub options: String,
    pub available: Option<u64>,
    pub same_root: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Installation {
    pub id: String,
    pub mount: String,
    pub uuid: String,
    pub path: String,
}
#[derive(Clone, Debug)]
pub(crate) struct Inventory {
    pub drives: Vec<Drive>,
    pub installations: Vec<Installation>,
    pub root_free: Option<u64>,
    pub usr_mount: String,
    pub flatpak: bool,
}
pub(crate) fn available(path: &str) -> Option<u64> {
    let p = std::ffi::CString::new(path).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(p.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    Some(stat.f_bavail.saturating_mul(stat.f_frsize))
}
pub(crate) fn valid_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 32
        || !id.as_bytes()[0].is_ascii_lowercase()
        || id
            .bytes()
            .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit() && b != b'-')
    {
        return Err("Use a drive name beginning with a lowercase letter, followed by lowercase letters, numbers, or hyphens (up to 32 characters).".into());
    }
    Ok(())
}
pub(crate) fn valid_app(id: &str) -> bool {
    let parts: Vec<_> = id.split('.').collect();
    id.len() <= 255
        && parts.len() >= 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
}
pub(crate) fn location(id: &str, mount: &str) -> Result<String, String> {
    valid_id(id)?;
    crate::package_cache::validate_path(mount)?;
    Ok(format!(
        "{}/archforge-apps-{id}",
        mount.trim_end_matches('/')
    ))
}
fn mount_rows(text: &str) -> Result<Vec<Drive>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let rows = value["filesystems"]
        .as_array()
        .ok_or("Mount information was not reported")?;
    let root = rows
        .iter()
        .find(|v| v["target"].as_str() == Some("/"))
        .and_then(|v| v["source"].as_str())
        .map(|s| s.split('[').next().unwrap_or(s))
        .ok_or("The root filesystem was not reported")?;
    Ok(rows
        .iter()
        .filter_map(|v| {
            let target = v["target"].as_str()?;
            let source = v["source"].as_str()?;
            let fs = v["fstype"].as_str()?;
            Some(Drive {
                target: target.into(),
                source: source.into(),
                uuid: v["uuid"].as_str().unwrap_or("").into(),
                fs: fs.into(),
                options: v["options"].as_str().unwrap_or("").into(),
                available: None,
                same_root: source.split('[').next().unwrap_or(source) == root,
            })
        })
        .collect())
}
pub(crate) fn eligibility(d: &Drive) -> Result<(), String> {
    if d.same_root {
        return Err("This shares the root filesystem and would not free space there.".into());
    }
    if !["ext4", "btrfs", "xfs", "f2fs"].contains(&d.fs.as_str()) {
        return Err("Choose an ext4, Btrfs, XFS, or F2FS filesystem. NTFS, FAT, and exFAT are not supported by this setup.".into());
    }
    if !d.options.split(',').any(|o| o == "rw")
        || d.options.split(',').any(|o| o == "ro" || o == "noexec")
    {
        return Err("The selected filesystem must be writable and allow execution.".into());
    }
    if d.uuid.is_empty() {
        return Err("The drive must have a filesystem UUID so its identity can be checked.".into());
    }
    if !["/mnt", "/media", "/home"]
        .iter()
        .any(|p| Path::new(&d.target).starts_with(p))
    {
        return Err("Use a persistent data-drive mount under /mnt, /media, or /home.".into());
    }
    crate::package_cache::validate_path(&d.target)?;
    Ok(())
}
pub(crate) fn configuration(id: &str, d: &Drive) -> Result<String, String> {
    eligibility(d)?;
    let path = location(id, &d.target)?;
    if d.uuid
        .bytes()
        .any(|b| !b.is_ascii_alphanumeric() && b != b'-')
    {
        return Err("Unexpected filesystem UUID.".into());
    }
    Ok(format!(
        "# Managed by Arch Forge\n# ArchForge mount = {}\n# ArchForge UUID = {}\n[Installation \"archforge-{id}\"]\nPath={path}\nDisplayName=Arch Forge - {id}\nStorageType=harddisk\n",
        d.target, d.uuid
    ))
}
fn parse_configuration(text: &str, id: &str) -> Result<Installation, String> {
    valid_id(id)?;
    let key = gtk::glib::KeyFile::new();
    key.load_from_data(text, gtk::glib::KeyFileFlags::NONE)
        .map_err(|e| e.to_string())?;
    let mount = text
        .lines()
        .find_map(|l| l.strip_prefix("# ArchForge mount = "))
        .ok_or("Missing drive mount metadata")?;
    let uuid = text
        .lines()
        .find_map(|l| l.strip_prefix("# ArchForge UUID = "))
        .ok_or("Missing filesystem identity")?;
    let path = key
        .string(&format!("Installation \"archforge-{id}\""), "Path")
        .map_err(|e| e.to_string())?
        .to_string();
    if path != location(id, mount)? {
        return Err(
            "The managed installation path has changed. Review its configuration manually.".into(),
        );
    }
    Ok(Installation {
        id: id.into(),
        mount: mount.into(),
        uuid: uuid.into(),
        path,
    })
}
fn installations_in(dir: &Path) -> Result<Vec<Installation>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(id) = name
            .strip_prefix("archforge-")
            .and_then(|n| n.strip_suffix(".conf"))
        {
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
                return Err(
                    "An Arch Forge drive configuration has unsafe ownership or permissions.".into(),
                );
            }
            let text = fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
            result.push(parse_configuration(&text, id)?);
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}
impl Inventory {
    pub fn load() -> Result<Self, String> {
        let text = crate::system::run(
            "/usr/bin/findmnt",
            &[
                "--json",
                "--list",
                "--real",
                "--output",
                "TARGET,SOURCE,FSTYPE,OPTIONS,UUID",
            ],
            10,
        )?;
        let mut drives = mount_rows(&text)?;
        for drive in &mut drives {
            if ["ext4", "btrfs", "xfs", "f2fs"].contains(&drive.fs.as_str()) {
                drive.available = available(&drive.target);
            }
        }
        let usr_mount = crate::system::run(
            "/usr/bin/findmnt",
            &[
                "--noheadings",
                "--output",
                "TARGET,SOURCE,FSTYPE",
                "--target",
                "/usr",
            ],
            5,
        )?;
        Ok(Self {
            drives,
            installations: installations_in(Path::new(CONFIG_DIR))?,
            root_free: available("/"),
            usr_mount,
            flatpak: Path::new("/usr/bin/flatpak").is_file(),
        })
    }
}
pub(crate) fn verify<'a>(install: &Installation, drives: &'a [Drive]) -> Result<&'a Drive, String> {
    let drive=drives.iter().find(|d|d.target==install.mount).ok_or("The app drive is not mounted at its configured path. Mount it before installing or launching apps.")?;
    eligibility(drive)?;
    if drive.uuid != install.uuid {
        return Err(
            "A different drive is mounted at this path. Reconnect the configured drive.".into(),
        );
    }
    if install.path != location(&install.id, &install.mount)? {
        return Err("The installation path does not match its configured drive.".into());
    }
    Ok(drive)
}
pub(crate) fn check(id: &str) -> Result<String, String> {
    valid_id(id)?;
    let inventory = Inventory::load()?;
    let install = inventory
        .installations
        .iter()
        .find(|i| i.id == id)
        .ok_or("Register this app drive first, then refresh locations.")?;
    verify(install, &inventory.drives)?;
    protected_chain(Path::new(&install.path), false)?;
    Ok(format!("App drive is ready: {}", install.path))
}
fn protected(path: &Path) -> Result<(), String> {
    let m = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 || m.mode() & 0o001 == 0 {
        return Err(format!(
            "{} must be a real, root-owned directory without group/other write access. Use a protected, persistent mount for system-wide apps.",
            path.display()
        ));
    }
    Ok(())
}
fn protected_chain(path: &Path, create: bool) -> Result<(), String> {
    let mut current = PathBuf::from("/");
    protected(&current)?;
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            current.push(name);
            if !current.exists() && create {
                fs::create_dir(&current).map_err(|e| e.to_string())?;
                fs::set_permissions(&current, fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
            protected(&current)?;
        }
    }
    Ok(())
}
pub(crate) fn register(id: &str, mount: &str) -> Result<String, String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("Run app-drive registration with sudo.".into());
    }
    valid_id(id)?;
    let inventory = Inventory::load()?;
    if !inventory.flatpak {
        return Err("Install Flatpak first: sudo pacman -Syu --needed flatpak".into());
    }
    let drive = inventory
        .drives
        .iter()
        .find(|d| d.target == mount)
        .ok_or("Choose an existing mounted data filesystem")?;
    eligibility(drive)?;
    let path = location(id, mount)?;
    protected_chain(Path::new(mount), false)?;
    protected_chain(Path::new(CONFIG_DIR), true)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(Path::new(CONFIG_DIR).join(".archforge.lock"))
        .map_err(|e| e.to_string())?;
    let lock_meta = lock.metadata().map_err(|e| e.to_string())?;
    if !lock_meta.is_file() || lock_meta.uid() != 0 || lock_meta.mode() & 0o022 != 0 {
        return Err("Unsafe app-drive registration lock file.".into());
    }
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("Another app-drive registration is in progress.".into());
    }
    let content = configuration(id, drive)?;
    let filename = Path::new(CONFIG_DIR).join(format!("archforge-{id}.conf"));
    for item in fs::read_dir(CONFIG_DIR).map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        if item.path().extension().is_some_and(|e| e == "conf") {
            let key = gtk::glib::KeyFile::new();
            key.load_from_file(item.path(), gtk::glib::KeyFileFlags::NONE)
                .map_err(|e| e.to_string())?;
            let group = format!("Installation \"archforge-{id}\"");
            if key.has_group(&group) {
                if item.path() == filename
                    && fs::read_to_string(&filename).map_err(|e| e.to_string())? == content
                {
                    return Ok(format!("This app drive is already configured: {path}"));
                }
                return Err("That installation name is already registered. Choose another name; existing apps were not moved.".into());
            }
            for group in key.groups() {
                if group.starts_with("Installation ")
                    && key
                        .string(&group, "Path")
                        .is_ok_and(|p| Path::new(p.as_str()) == Path::new(&path))
                {
                    return Err("This app folder is already used by another installation.".into());
                }
            }
        }
    }
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            protected(Path::new(&path))?;
            if fs::read_dir(&path)
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
            {
                return Err(
                    "The destination folder is not empty. Choose another drive name.".into(),
                );
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&path).map_err(|e| e.to_string())?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        Err(e) => return Err(e.to_string()),
    }
    // Recheck the mount identity immediately before publishing its configuration.
    let fresh = Inventory::load()?;
    let expected = Installation {
        id: id.into(),
        mount: mount.into(),
        uuid: drive.uuid.clone(),
        path: path.clone(),
    };
    verify(&expected, &fresh.drives)?;
    let mut staged = tempfile::Builder::new()
        .prefix(".archforge-drive-")
        .tempfile_in(CONFIG_DIR)
        .map_err(|e| e.to_string())?;
    staged
        .write_all(content.as_bytes())
        .map_err(|e| e.to_string())?;
    staged
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o644))
        .map_err(|e| e.to_string())?;
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    staged
        .persist_noclobber(filename)
        .map_err(|e| e.to_string())?;
    fs::File::open(CONFIG_DIR)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "App drive registered: {path}\nUse --installation=archforge-{id} for apps on this drive. Existing apps were not moved. Keep the drive mounted; sign out and back in if its app shortcuts do not appear."
    ))
}
pub(crate) fn setup_command(id: &str, mount: &str) -> Result<String, String> {
    location(id, mount)?;
    let q = crate::package_cache::quote;
    Ok(format!(
        "sudo /usr/bin/archforge --add-app-drive {} {} && /usr/bin/archforge --check-app-drive {} && /usr/bin/env FLATPAK_CONFIG_DIR=/etc/flatpak /usr/bin/flatpak --installation={} remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo",
        q(id),
        q(mount),
        q(id),
        q(&format!("archforge-{id}"))
    ))
}
pub(crate) fn install_command(id: &str, app: &str) -> Result<String, String> {
    valid_id(id)?;
    if !valid_app(app) {
        return Err("Enter a complete Flatpak application ID, such as org.mozilla.firefox.".into());
    }
    let q = crate::package_cache::quote;
    Ok(format!(
        "/usr/bin/archforge --check-app-drive {} && /usr/bin/env FLATPAK_CONFIG_DIR=/etc/flatpak /usr/bin/flatpak --installation={} install flathub {}",
        q(id),
        q(&format!("archforge-{id}")),
        q(app)
    ))
}

#[cfg(test)]
#[path = "app_storage_tests.rs"]
mod tests;
