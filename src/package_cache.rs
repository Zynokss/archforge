use std::os::fd::AsRawFd;
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

const START: &str = "# Arch Forge package cache: begin";
const END: &str = "# Arch Forge package cache: end";
pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
pub(crate) fn validate_path(value: &str) -> Result<PathBuf, String> {
    let p = Path::new(value);
    if !p.is_absolute()
        || value == "/"
        || value.len() > 4096
        || value
            .bytes()
            .any(|b| !b.is_ascii_alphanumeric() && !b"/._-".contains(&b))
        || p.components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        || value.split('/').any(|c| c == "." || c == "..")
    {
        return Err("Use an absolute Linux path with letters, numbers, /, dots, underscores, or hyphens. Do not select / or use spaces, ~, or .. .".into());
    }
    if [
        "/etc", "/usr", "/bin", "/sbin", "/boot", "/proc", "/sys", "/dev", "/run", "/var/lib",
    ]
    .iter()
    .any(|base| p.starts_with(base))
    {
        return Err("Choose a cache directory under /var/cache or on a mounted Linux data drive, outside system configuration and package databases.".into());
    }
    Ok(p.to_path_buf())
}
pub(crate) fn directories() -> Result<Vec<String>, String> {
    let output = crate::system::run("/usr/bin/pacman-conf", &["CacheDir"], 5)?;
    let dirs: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    if dirs.is_empty()
        || dirs
            .iter()
            .any(|s| !Path::new(s).is_absolute() || s.contains(['\0', '\r', '\n']))
    {
        return Err("Pacman did not report valid cache directories.".into());
    }
    Ok(dirs)
}
pub(crate) fn measured_size(output: &str) -> Option<&str> {
    let rows: Vec<_> = output
        .lines()
        .filter_map(|line| line.split_once('\t').or_else(|| line.split_once(' ')))
        .collect();
    rows.iter()
        .rev()
        .find(|(_, p)| p.trim() == "total")
        .or_else(|| rows.iter().find(|(_, p)| p.trim().starts_with('/')))
        .map(|(size, _)| size.trim())
}
pub(crate) fn rewrite(config: &str, destination: Option<&str>) -> Result<String, String> {
    if let Some(path) = destination {
        validate_path(path)?;
    }
    let mut clean = String::new();
    let mut managed = false;
    let mut seen = false;
    for line in config.split_inclusive('\n') {
        if line.trim() == START {
            if managed || seen {
                return Err(
                    "Duplicate cache markers in pacman.conf. Review the file manually.".into(),
                );
            }
            managed = true;
            seen = true;
            continue;
        }
        if line.trim() == END {
            if !managed {
                return Err("Unmatched cache marker in pacman.conf.".into());
            }
            managed = false;
            continue;
        }
        if !managed {
            clean.push_str(line);
        }
    }
    if managed {
        return Err("Incomplete cache block in pacman.conf. Review it manually.".into());
    }
    if destination.is_none() {
        return Ok(clean);
    }
    let mut result = String::new();
    let mut found = false;
    for line in clean.split_inclusive('\n') {
        result.push_str(line);
        if line.trim() == "[options]" {
            if found {
                return Err("Multiple [options] sections are not supported.".into());
            }
            found = true;
            if !line.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(&format!(
                "{START}\nCacheDir = {}\n{END}\n",
                destination.unwrap()
            ));
        }
    }
    if !found {
        return Err("No [options] section found in pacman.conf.".into());
    }
    Ok(result)
}
fn protected_directory(path: &Path) -> Result<(), String> {
    let m = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !m.is_dir()
        || m.uid() != 0
        || m.permissions().mode() & 0o022 != 0
        || m.permissions().mode() & 0o001 == 0
    {
        return Err(format!(
            "{} must be a real, root-owned directory with no group/other write access and traversal for pacman's download user. Choose a protected directory on a mounted Linux filesystem.",
            path.display()
        ));
    }
    Ok(())
}
fn prepare_directory(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::from("/");
    protected_directory(&current)?;
    let parent = path
        .parent()
        .ok_or("Select a directory with an existing parent")?;
    for part in parent.components() {
        if let Component::Normal(name) = part {
            current.push(name);
            protected_directory(&current)?;
        }
    }
    if path.starts_with("/mnt") || path.starts_with("/media") {
        let mount = crate::system::run(
            "/usr/bin/findmnt",
            &[
                "--noheadings",
                "--output",
                "TARGET",
                "--target",
                parent.to_str().ok_or("Invalid parent path")?,
            ],
            5,
        )?;
        if mount.trim() == "/" {
            return Err("The selected data drive does not appear to be mounted. Mount it before configuring its package cache.".into());
        }
    }
    match fs::symlink_metadata(path) {
        Ok(_) => protected_directory(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|e| e.to_string())?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
            protected_directory(path)
        }
        Err(e) => Err(e.to_string()),
    }
}
// Called only by the explicit, administrator-reviewed CLI operation. No GUI
// process elevates itself or writes system configuration automatically.
pub(crate) fn apply(destination: Option<&str>) -> Result<String, String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("Run this configuration action with sudo.".into());
    }
    if Path::new("/var/lib/pacman/db.lck").exists() {
        return Err(
            "Pacman is busy. Finish the package transaction before changing cache settings.".into(),
        );
    }
    let path = Path::new("/etc/pacman.conf");
    let mut source = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    let meta = source.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.uid() != 0 || meta.permissions().mode() & 0o022 != 0 {
        return Err(
            "pacman.conf must be a regular, root-owned file without group/other write access."
                .into(),
        );
    }
    if unsafe { libc::flock(source.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("Another cache configuration is in progress.".into());
    }
    let mut original = String::new();
    source
        .read_to_string(&mut original)
        .map_err(|e| e.to_string())?;
    let content = rewrite(&original, destination)?;
    if content == original {
        return Ok("No configuration changes needed.".into());
    }
    let mut staged = tempfile::Builder::new()
        .prefix(".archforge-pacman-")
        .tempfile_in("/etc")
        .map_err(|e| e.to_string())?;
    staged
        .write_all(content.as_bytes())
        .map_err(|e| e.to_string())?;
    staged.flush().map_err(|e| e.to_string())?;
    let proposed = crate::system::run(
        "/usr/bin/pacman-conf",
        &[
            "--config",
            staged.path().to_str().ok_or("Invalid configuration path")?,
            "CacheDir",
        ],
        5,
    )?;
    if let Some(value) = destination
        && proposed.lines().next().map(|p| p.trim_end_matches('/'))
            != Some(value.trim_end_matches('/'))
    {
        return Err("Pacman did not accept the proposed cache directory. The configuration was not changed.".into());
    }
    if let Some(value) = destination {
        prepare_directory(&validate_path(value)?)?;
    }
    let current = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if current.ino() != meta.ino()
        || current.dev() != meta.dev()
        || fs::read_to_string(path).map_err(|e| e.to_string())? != original
    {
        return Err("pacman.conf changed during review. Try again.".into());
    }
    let mut backup = tempfile::Builder::new()
        .prefix("pacman.conf.archforge-backup-")
        .tempfile_in("/etc")
        .map_err(|e| e.to_string())?;
    backup
        .write_all(original.as_bytes())
        .map_err(|e| e.to_string())?;
    backup.as_file().sync_all().map_err(|e| e.to_string())?;
    let (_, backup_path) = backup.keep().map_err(|e| e.to_string())?;
    staged
        .as_file()
        .set_permissions(fs::Permissions::from_mode(
            meta.permissions().mode() & 0o777,
        ))
        .map_err(|e| e.to_string())?;
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    staged.persist(path).map_err(|e| e.to_string())?;
    fs::File::open("/etc")
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "Package cache configuration saved.\nBackup: {}\nEffective cache directories:\n{}\nExisting archives were not moved or deleted. The Cleanup page follows pacman's configuration.",
        backup_path.display(),
        proposed
    ))
}

#[cfg(test)]
#[path = "package_cache_tests.rs"]
mod tests;
