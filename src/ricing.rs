use std::{path::Path, thread};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Project {
    pub name: &'static str,
    pub repository: &'static str,
    pub desktop: &'static str,
    pub style: &'static str,
    pub description: &'static str,
    pub requirements: &'static str,
    pub impact: &'static str,
    pub command: &'static str,
}

pub(crate) const PROJECTS: &[Project] = &[
    Project {
        name: "end-4's Hyprland dotfiles",
        repository: "end-4/dots-hyprland",
        desktop: "Hyprland",
        style: "Material · shell",
        description: "Usability-first Hyprland configuration with a custom shell, widgets, and a transparent interactive setup.",
        requirements: "Arch Linux · Hyprland 0.55-compatible system · Quickshell",
        impact: "User-level setup. Review the repository's compatibility note and every install step before continuing.",
        command: "bash <(curl -s https://ii.clsty.link/get)",
    },
    Project {
        name: "HyDE",
        repository: "HyDE-Project/HyDE",
        desktop: "Hyprland",
        style: "Dynamic · full desktop",
        description: "A complete themed environment with selectable styles, wallpapers, and desktop components.",
        requirements: "Arch Linux · Git · base-devel · review GPU and bootloader notes",
        impact: "Broad system setup. The upstream installer can change the display manager, package configuration, and bootloader settings.",
        command: "sudo pacman -S --needed git base-devel && git clone --depth 1 https://github.com/HyDE-Project/HyDE \"$HOME/HyDE\" && cd \"$HOME/HyDE/Scripts\" && ./install.sh",
    },
    Project {
        name: "ML4W Hyprland dotfiles",
        repository: "mylinuxforwork/dotfiles",
        desktop: "Hyprland",
        style: "Polished · configurable",
        description: "An established Hyprland configuration with an interactive setup, backup flow, and settings tools.",
        requirements: "Arch Linux · curl · upstream stable installer",
        impact: "Changes user configuration and installs desktop components. Upstream recommends backing up ~/.config first.",
        command: "bash <(curl -s https://ml4w.com/os/stable)",
    },
    Project {
        name: "end4-pC QuickShell setup",
        repository: "pctrade/end4-pC",
        desktop: "Hyprland",
        style: "Material · shell add-on",
        description: "A maintained end-4 QuickShell fork with expanded widgets and community presets. This is an add-on, not a complete desktop setup.",
        requirements: "end-4 dots installed and running · QuickShell · Git",
        impact: "Clones a separate QuickShell configuration under ~/.config/quickshell/end4-pC. It does not install the base desktop.",
        command: "cd \"$HOME/.config/quickshell\" && git clone https://github.com/pctrade/end4-pC.git && { killall qs 2>/dev/null; qs -c end4-pC > /dev/null 2>&1 & disown; }",
    },
    Project {
        name: "KoolDots for Arch",
        repository: "LinuxBeginnings/Arch-Hyprland",
        desktop: "Hyprland",
        style: "Full installer · curated",
        description: "Arch-focused Hyprland setup scripts with optional preconfigured community dotfiles. The older JaKooLit installer now points to this maintainer.",
        requirements: "A minimal Arch install · Git · backup or system snapshot recommended",
        impact: "Full system installer. Upstream may replace audio packages, configure a login manager, graphics drivers, or other system settings.",
        command: "git clone --depth=1 https://github.com/LinuxBeginnings/Arch-Hyprland.git \"$HOME/Arch-Hyprland\" && cd \"$HOME/Arch-Hyprland\" && chmod +x install.sh && ./install.sh",
    },
];

#[derive(Clone, Debug)]
pub(crate) struct Details {
    pub readme: String,
    pub images: Vec<(String, Vec<u8>)>,
}

fn curl_args() -> Vec<&'static str> {
    vec![
        "--disable",
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "8",
        "--user-agent",
        concat!("Arch-Forge/", env!("CARGO_PKG_VERSION")),
    ]
}

fn source(project: Project) -> (&'static str, &'static str, &'static [&'static str]) {
    match project.repository {
        "end-4/dots-hyprland" => (
            "main",
            ".github/README.md",
            &[
                "https://github.com/user-attachments/assets/5d4e7d07-d0b4-4406-a4c9-ed7ba90e3fe4",
                "https://github.com/user-attachments/assets/6a32395f-9437-4192-8faf-2951a9e84cbe",
                "https://github.com/user-attachments/assets/c51bed8b-3670-4d4c-9074-873be224fb8e",
            ],
        ),
        "HyDE-Project/HyDE" => (
            "main",
            "README.md",
            &[
                "https://raw.githubusercontent.com/prasanthrangan/hyprdots/main/Source/assets/theme_select_1.png",
            ],
        ),
        "mylinuxforwork/dotfiles" => ("main", "README.md", &["https://ml4w.com/os/screen-216.jpg"]),
        "pctrade/end4-pC" => (
            "main",
            "README.md",
            &[
                "https://raw.githubusercontent.com/pctrade/end4-pC/main/screenshots/1.png",
                "https://raw.githubusercontent.com/pctrade/end4-pC/main/screenshots/2.png",
            ],
        ),
        "LinuxBeginnings/Arch-Hyprland" => (
            "main",
            "README.md",
            &[
                "https://raw.githubusercontent.com/LinuxBeginnings/Hyprland-Dots/main/config/quickshell/qs-hyprview/screenshots/hero.jpeg",
            ],
        ),
        _ => ("main", "README.md", &[]),
    }
}

fn cache_path(url: &str) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))?;
    let dir = base.join("archforge/ricing-v2");
    std::fs::create_dir_all(&dir).ok()?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hash);
    Some(dir.join(format!("{:016x}", hash.finish())))
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let cache = cache_path(url);
    if let Some(bytes) = cache.as_ref().and_then(|p| std::fs::read(p).ok()) {
        return Ok(bytes);
    }
    let file = tempfile::NamedTempFile::new().map_err(|_| "Cannot create preview file")?;
    let mut args = curl_args();
    args.extend([
        "--max-time",
        "20",
        "--max-filesize",
        "16777216",
        "--output",
        file.path().to_str().ok_or("Invalid preview path")?,
        url,
    ]);
    crate::system::run("/usr/bin/curl", &args, 24).map_err(|_| {
        "Could not download from the repository. Check your connection and retry.".to_string()
    })?;
    let bytes = std::fs::read(file.path()).map_err(|_| "Cannot read preview")?;
    if bytes.is_empty() {
        return Err("Repository returned an empty response.".into());
    }
    if let Some(path) = cache {
        let _ = std::fs::write(path, &bytes);
    }
    Ok(bytes)
}

fn clean_url(url: &str, project: Project, branch: &str) -> Option<String> {
    let trimmed = url.trim().trim_matches(['<', '>']);
    if let Some(rest) = trimmed.strip_prefix("https://") {
        let (host, path) = rest.split_once('/')?;
        if [
            "raw.githubusercontent.com",
            "user-images.githubusercontent.com",
            "images.githubusercontent.com",
            "ml4w.com",
        ]
        .contains(&host)
        {
            return Some(trimmed.replace("&amp;", "&"));
        }
        if host == "github.com" {
            if path.starts_with("user-attachments/assets/") || path.contains("/assets/") {
                return Some(trimmed.into());
            }
            if let Some((repo, file)) = path.split_once("/blob/") {
                return Some(format!("https://raw.githubusercontent.com/{repo}/{file}"));
            }
        }
        return None;
    }
    if trimmed.contains(':') || trimmed.starts_with("//") {
        return None;
    }
    let mut parts: Vec<&str> = branch.split('/').collect();
    if trimmed.starts_with('/') {
        parts.truncate(1);
    }
    for part in trimmed.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() <= 1 {
                    return None;
                }
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    Some(format!(
        "https://raw.githubusercontent.com/{}/{}",
        project.repository,
        parts.join("/").replace(' ', "%20")
    ))
}

fn is_image(url: &str) -> bool {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".webp"]
        .iter()
        .any(|ext| path.ends_with(ext))
        || url.starts_with("https://github.com/user-attachments/assets/")
        || url.starts_with("https://user-images.githubusercontent.com/")
}

fn image_urls(readme: &str, project: Project, branch: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut cursor = 0;
    while let Some(start) = readme[cursor..].find("![") {
        let start = cursor + start;
        let Some(open) = readme[start..].find("](").map(|n| start + n + 2) else {
            cursor = start + 2;
            continue;
        };
        let Some(end) = readme[open..].find(')').map(|n| open + n) else {
            break;
        };
        let raw = readme[open..end]
            .split_whitespace()
            .next()
            .unwrap_or_default();
        if let Some(url) = clean_url(raw, project, branch)
            && is_image(&url)
            && !found.contains(&url)
        {
            found.push(url);
            if found.len() == 4 {
                break;
            }
        }
        cursor = end + 1;
    }
    let mut cursor = 0;
    while found.len() < 4 {
        let Some(start) = readme[cursor..].find("<img") else {
            break;
        };
        let start = cursor + start;
        let Some(end) = readme[start..].find('>').map(|n| start + n) else {
            break;
        };
        let normalized = readme[start..end]
            .replace("src =", "src=")
            .replace("src= ", "src=");
        let tag = &normalized;
        let Some(src) = tag.find("src=").map(|n| n + 4) else {
            cursor = end + 1;
            continue;
        };
        let rest = tag[src..].trim_start();
        let quote = rest.chars().next().unwrap_or(' ');
        if !matches!(quote, '\'' | '"') {
            cursor = end + 1;
            continue;
        }
        let value = &rest[1..];
        let Some(value_end) = value.find(quote) else {
            cursor = end + 1;
            continue;
        };
        if let Some(url) = clean_url(&value[..value_end], project, branch)
            && is_image(&url)
            && !found.contains(&url)
        {
            found.push(url);
        }
        cursor = end + 1;
    }
    found
}

fn fetch_image(url: &str) -> Option<Vec<u8>> {
    use gtk::prelude::*;
    let bytes = download(url).ok()?;
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    loader.connect_size_prepared(|loader, w, h| {
        if w > 1600 {
            loader.set_size(1600, (h as i64 * 1600 / w as i64).max(1) as i32);
        }
    });
    if loader.write(&bytes).is_err() || loader.close().is_err() || loader.pixbuf().is_none() {
        if let Some(path) = cache_path(url) {
            let _ = std::fs::remove_file(path);
        }
        return None;
    }
    Some(bytes)
}

pub(crate) fn fetch(project: Project) -> Result<Details, String> {
    fetch_uncached(project)
}

fn fetch_uncached(project: Project) -> Result<Details, String> {
    let (branch, path, explicit) = source(project);
    let readme_url = format!(
        "https://raw.githubusercontent.com/{}/{branch}/{path}",
        project.repository
    );
    let raw_readme = download(&readme_url)
        .ok()
        .and_then(|b| String::from_utf8(b).ok());
    let mut urls: Vec<String> = explicit.iter().map(|u| u.to_string()).collect();
    if let Some(readme) = &raw_readme {
        let resolved_branch = path
            .rsplit_once('/')
            .map(|(dir, _)| format!("{branch}/{dir}"))
            .unwrap_or_else(|| branch.into());
        for url in image_urls(readme, project, &resolved_branch) {
            if !urls.contains(&url) && urls.len() < 4 {
                urls.push(url);
            }
        }
    }
    let images: Vec<(String, Vec<u8>)> = thread::scope(|scope| {
        let joins: Vec<_> = urls
            .iter()
            .map(|url| scope.spawn(move || fetch_image(url).map(|bytes| (url.clone(), bytes))))
            .collect();
        joins
            .into_iter()
            .filter_map(|join| join.join().ok().flatten())
            .collect()
    });
    if images.is_empty() {
        return Err("Screenshots could not be downloaded. Check your connection and retry.".into());
    }
    Ok(Details {
        readme: raw_readme
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| {
                "README unavailable. Open the repository for installation documentation.".into()
            }),
        images,
    })
}

pub(crate) fn document_link(url: &str, project: Project) -> Option<String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        return Some(url.into());
    }
    if url.starts_with('#') {
        return None;
    }
    let (branch, path, _) = source(project);
    let base = path
        .rsplit_once('/')
        .map(|(dir, _)| format!("{branch}/{dir}"))
        .unwrap_or_else(|| branch.into());
    clean_url(url, project, &base).map(|u| {
        u.replace("https://raw.githubusercontent.com/", "https://github.com/")
            .replacen(
                &format!("{}/", project.repository),
                &format!("{}/blob/", project.repository),
                1,
            )
    })
}
pub(crate) fn document_image(url: &str, project: Project) -> Option<Vec<u8>> {
    let (branch, path, _) = source(project);
    let base = path
        .rsplit_once('/')
        .map(|(dir, _)| format!("{branch}/{dir}"))
        .unwrap_or_else(|| branch.into());
    fetch_image(&clean_url(url, project, &base)?)
}

pub(crate) fn installation_document(project: Project) -> String {
    let (notes, source) = match project.repository {
        "end-4/dots-hyprland" => (
            "## Hyprland 0.55 compatibility\n\nIf your distribution has not shipped Hyprland 0.55, or you are not ready to move to it, upstream directs you to the Pre-Hyprland Luaification release. Read the wiki before installing.\n\n## Manual alternative\n\nClone the repository, change into its directory, and run:\n\n```sh\n./setup install\n```\n\n## Keybindings\n\nSuper+/ opens the keybinding list. Super+Enter opens a terminal.",
            "https://ii.clsty.link/en/ii-qs/01setup/",
        ),
        "HyDE-Project/HyDE" => (
            "Run the installer from **HyDE/Scripts**, as upstream documents. The command installs Git and base-devel first. Review the package, graphics driver, display manager and bootloader changes. Upstream recommends rebooting after the first installation.",
            "https://hydeproject.pages.dev/en/getting-started/installation",
        ),
        "mylinuxforwork/dotfiles" => (
            "This uses the documented **stable** installer. The previous local setup-script path is no longer the documented installation method.\n\n## Rolling release alternative\n\n```sh\nbash <(curl -s https://ml4w.com/os/rolling)\n```\n\nChoose one release channel and follow its interactive installer.",
            "https://ml4w.com/os/",
        ),
        "pctrade/end4-pC" => (
            "Requires **illogical-impulse installed and running** and an existing ~/.config/quickshell directory. Upstream clones the add-on there, stops current qs processes and launches end4-pC. **This restarts your Quickshell shell.** Default-shell configuration is a separate optional step documented in the README.",
            "https://github.com/pctrade/end4-pC#-installation",
        ),
        _ => (
            "Upstream's manual method clones into ~/Arch-Hyprland, changes into that directory, makes install.sh executable, and runs it. This is a system installer intended for a minimal Arch installation. Review its configuration and backup instructions first.",
            "https://github.com/LinuxBeginnings/Arch-Hyprland#-how-to-use-this-script",
        ),
    };
    format!(
        "# Installation\n\nDocumented upstream method · checked 2026-11-01.\n\n```bash\n{}\n```\n\n{}\n\n{}\n\n[Upstream installation instructions]({})",
        project.command, notes, project.impact, source
    )
}

pub(crate) fn backup_files() -> Vec<(String, u64, String)> {
    let Ok(directory) = crate::loadouts::state_home().map(|p| p.join("archforge/loadouts/backups"))
    else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(Path::new(&directory)) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with("before-dotfiles-") || !name.ends_with(".json") {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            metadata
                .is_file()
                .then_some((name, metadata.len(), metadata.modified().ok()))
        })
        .map(|(name, size, modified)| {
            let stamp = modified
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default();
            (name, size, stamp)
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| b.2.cmp(&a.2));
    files.truncate(40);
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curated_installation_commands_are_valid_bash() {
        for project in PROJECTS {
            assert!(
                std::process::Command::new("/usr/bin/bash")
                    .args(["-n", "-c", project.command])
                    .status()
                    .unwrap()
                    .success(),
                "{}",
                project.repository
            );
            assert!(installation_document(*project).contains(project.command));
        }
        assert_eq!(
            PROJECTS[0].command,
            "bash <(curl -s https://ii.clsty.link/get)"
        );
        assert_eq!(
            PROJECTS[2].command,
            "bash <(curl -s https://ml4w.com/os/stable)"
        );
    }
    #[test]
    fn resolves_readme_image_sources() {
        let project = PROJECTS[0];
        let readme = "![Desktop](../screenshots/desktop.png) <img src = \"images/widgets.webp\"> ![Other](https://github.com/pctrade/end4-pC/blob/main/screenshots/1.png)";
        let urls = image_urls(readme, project, "main/.github");
        assert!(urls.contains(&"https://raw.githubusercontent.com/end-4/dots-hyprland/main/screenshots/desktop.png".into()));
        assert!(urls.contains(&"https://raw.githubusercontent.com/end-4/dots-hyprland/main/.github/images/widgets.webp".into()));
        assert!(urls.contains(
            &"https://raw.githubusercontent.com/pctrade/end4-pC/main/screenshots/1.png".into()
        ));
        assert!(clean_url("http://example.com/image.png", project, "main").is_none());
    }
    #[test]
    #[ignore]
    fn curated_screenshots_download_and_cache() {
        for project in PROJECTS {
            let details = fetch(*project).unwrap_or_else(|e| panic!("{}: {e}", project.repository));
            assert!(!details.images.is_empty(), "{}", project.repository);
            for (url, bytes) in details.images {
                assert_eq!(std::fs::read(cache_path(&url).unwrap()).unwrap(), bytes);
            }
        }
    }
}
