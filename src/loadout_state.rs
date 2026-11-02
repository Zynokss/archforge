use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub ready: bool,
    pub updates_checked: bool,
    pub installed: BTreeMap<String, String>,
    pub cached: BTreeSet<String>,
    pub foreign: Option<BTreeSet<String>>,
    pub updates: BTreeMap<String, (String, String)>,
    pub update_note: String,
    pub cache_note: String,
}
impl Snapshot {
    /// Exact names take priority over unambiguous packaging variants.
    pub fn installed_variant<'a>(&'a self, name: &'a str) -> &'a str {
        crate::package_variants::installed_name(&self.installed, name)
    }
    pub fn variant_source(
        &self,
        actual: &str,
        requested: &str,
        source: crate::loadouts::Source,
    ) -> crate::loadouts::Source {
        use crate::loadouts::Source;
        if actual == requested || source == Source::Foreign {
            return source;
        }
        match &self.foreign {
            Some(foreign) if !foreign.contains(actual) => Source::Repo,
            _ => crate::loadout_styles::catalog()
                .iter()
                .find(|app| app.name == actual && app.source == Source::Aur)
                .map(|app| app.source)
                .unwrap_or(Source::Foreign),
        }
    }
    pub fn describe(&self, name: &str) -> (String, &'static str, String) {
        if !self.ready {
            return (
                "Not scanned".into(),
                "package-unknown",
                "Installation status is unavailable until the package scan succeeds.".into(),
            );
        }
        let requested = name;
        let name = self.installed_variant(name);
        if let Some(version) = self.installed.get(name) {
            if let Some((old, new)) = self.updates.get(name) {
                return (
                    "Update available".into(),
                    "package-update",
                    format!("Installed {old} → repository {new}"),
                );
            }
            return (
                "Installed".into(),
                "package-installed",
                format!(
                    "Installed package: {name} · version: {version}. {}{}",
                    if requested != name {
                        "Packaging variant detected. "
                    } else {
                        ""
                    },
                    self.update_note
                ),
            );
        }
        if self.cached.contains(name) {
            return ("Cached · not installed".into(), "package-cached", "An archive exists in a configured pacman cache. Its version may be older; pacman decides whether it can reuse it and verifies it before installing.".into());
        }
        (
            "Not installed".into(),
            "package-unknown",
            format!(
                "Not registered in pacman's installed package database. {}",
                self.cache_note
            ),
        )
    }
}
fn archive_name(filename: &str) -> Option<&str> {
    let (stem, compression) = filename.rsplit_once(".pkg.tar")?;
    if !["", ".zst", ".xz", ".gz", ".bz2", ".lrz", ".lzo", ".Z"].contains(&compression) {
        return None;
    }
    let mut parts = stem.rsplitn(4, '-');
    let arch = parts.next()?;
    let release = parts.next()?;
    let version = parts.next()?;
    let name = parts.next()?;
    if arch.is_empty()
        || release.is_empty()
        || version.is_empty()
        || !crate::loadouts::valid_package(name)
    {
        return None;
    }
    Some(name)
}
fn parse_updates(text: &str) -> Result<BTreeMap<String, (String, String)>, String> {
    let mut updates = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 4 || fields[2] != "->" || !crate::loadouts::valid_package(fields[0]) {
            return Err("Unexpected update output; no update status was inferred.".into());
        }
        updates.insert(fields[0].into(), (fields[1].into(), fields[3].into()));
    }
    Ok(updates)
}
pub(crate) fn scan(check_updates: bool) -> Result<Snapshot, String> {
    let output = crate::system::run("/usr/bin/pacman", &["-Q"], 30)?;
    let mut state = Snapshot {
        ready: true,
        update_note: "Updates not checked yet".into(),
        ..Snapshot::default()
    };
    for line in output.lines().filter(|l| !l.trim().is_empty()) {
        let Some((name, version)) = line.split_once(' ') else {
            return Err("Unexpected installed package output".into());
        };
        if !crate::loadouts::valid_package(name) || version.is_empty() {
            return Err("Unexpected installed package output".into());
        }
        state.installed.insert(name.into(), version.into());
    }
    state.foreign = match crate::system::run("/usr/bin/pacman", &["-Qmq"], 30) {
        Ok(text) => Some(text.lines().map(str::to_owned).collect()),
        Err(e) if e.trim() == "Exit 1" => Some(BTreeSet::new()),
        Err(_) => None,
    };
    match crate::package_cache::directories() {
        Ok(dirs) => {
            let mut unreadable = 0;
            for dir in dirs {
                let entries = match std::fs::read_dir(dir) {
                    Ok(e) => e,
                    Err(_) => {
                        unreadable += 1;
                        continue;
                    }
                };
                for entry in entries {
                    let Ok(entry) = entry else {
                        unreadable += 1;
                        continue;
                    };
                    if !entry.file_type().is_ok_and(|t| t.is_file()) {
                        continue;
                    }
                    if let Some(name) = entry.file_name().to_str().and_then(archive_name) {
                        state.cached.insert(name.into());
                    }
                }
            }
            if unreadable > 0 {
                state.cache_note =
                    "Some cache paths could not be read; cached status may be incomplete.".into();
            }
        }
        Err(_) => {
            state.cache_note =
                "Cache status unavailable; pacman will check archives when installing.".into()
        }
    }
    if check_updates {
        let result = if std::path::Path::new("/usr/bin/checkupdates").is_file() {
            match crate::system::run("/usr/bin/checkupdates", &["--nocolor"], 120) {
                Ok(text) => parse_updates(&text),
                Err(e) if e.trim() == "Exit 2" => Ok(BTreeMap::new()),
                Err(e) => Err(e),
            }
        } else {
            Err("Install pacman-contrib to check repository updates.".into())
        };
        match result {
            Ok(updates) => {
                state.updates_checked = true;
                state.updates = updates;
                state.update_note = "Repository updates checked · AUR not checked".into();
            }
            Err(e) => state.update_note = format!("Update check unavailable: {e}"),
        }
        state
            .updates
            .retain(|name, (old, _)| state.installed.get(name) == Some(old));
    }
    Ok(state)
}
pub(crate) fn remove_command(names: &[String]) -> Result<String, String> {
    if names.is_empty() || names.iter().any(|n| !crate::loadouts::valid_package(n)) {
        return Err("Choose installed packages with valid names.".into());
    }
    Ok(format!(
        "sudo pacman -R -- {}",
        names
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(" ")
    ))
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RemovalBlocker {
    pub package: String,
    pub dependency: String,
    pub required_by: String,
}
pub(crate) struct RemovalPreview {
    pub blockers: Vec<RemovalBlocker>,
    pub output: String,
}
pub(crate) fn removal_preview_args(names: &[String]) -> Result<Vec<String>, String> {
    remove_command(names)?;
    let mut args: Vec<String> = ["-R", "--print", "--print-format", "%n", "--"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    args.extend(names.iter().cloned().collect::<BTreeSet<_>>());
    Ok(args)
}
fn removal_result(
    names: &[String],
    result: Result<String, String>,
) -> Result<RemovalPreview, String> {
    match result {
        Ok(output) => {
            let targets: BTreeSet<_> = output
                .lines()
                .map(str::trim)
                .filter(|s| crate::loadouts::valid_package(s))
                .collect();
            let requested: BTreeSet<_> = names.iter().map(String::as_str).collect();
            if targets != requested {
                return Err(format!(
                    "Pacman did not confirm the exact selected packages. Nothing has been removed.\n{output}"
                ));
            }
            Ok(RemovalPreview {
                blockers: vec![],
                output,
            })
        }
        Err(output) => {
            let blockers: BTreeSet<_> = output
                .lines()
                .filter_map(|line| {
                    let line = line.trim().strip_prefix(":: removing ")?;
                    let (package, rest) = line.split_once(" breaks dependency '")?;
                    let (dependency, required_by) = rest.split_once("' required by ")?;
                    if !crate::loadouts::valid_package(package)
                        || !crate::loadouts::valid_package(required_by)
                    {
                        return None;
                    }
                    Some(RemovalBlocker {
                        package: package.into(),
                        dependency: dependency.into(),
                        required_by: required_by.into(),
                    })
                })
                .collect();
            if blockers.is_empty() {
                return Err(output);
            }
            Ok(RemovalPreview {
                blockers: blockers.into_iter().collect(),
                output,
            })
        }
    }
}
pub(crate) fn preview_removal(names: &[String]) -> Result<RemovalPreview, String> {
    let args = removal_preview_args(names)?;
    let refs: Vec<_> = args.iter().map(String::as_str).collect();
    removal_result(names, crate::system::run("/usr/bin/pacman", &refs, 30))
}

#[cfg(test)]
#[path = "loadout_state_tests.rs"]
mod tests;
