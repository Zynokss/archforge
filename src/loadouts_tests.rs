use super::*;
use std::os::unix::fs::PermissionsExt;
fn home() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}
fn profile() -> Profile {
    let mut p = Profile::default();
    p.packages.insert("git".into(), Source::Repo);
    p.packages
        .insert("visual-studio-code-bin".into(), Source::Aur);
    p.packages.insert("my-local-build".into(), Source::Foreign);
    p
}
#[test]
fn profile_roundtrip_and_foreign_sources_are_explicit() {
    let p = profile();
    let q = Profile::decode(&p.encode().unwrap()).unwrap();
    assert_eq!(p.packages, q.packages);
    let command = q.command(false).unwrap();
    assert!(command.starts_with("sudo pacman -Syu --needed -- git"));
    assert!(command.contains("paru -S --needed -- visual-studio-code-bin"));
    assert!(!command.contains("my-local-build"));
    assert!(q.command(true).unwrap().contains("my-local-build"));
    assert!(!command.contains("--noconfirm"));
    assert!(!command.contains("sudo yay"));
    let script = q.script(false).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), script).unwrap();
    assert!(
        std::process::Command::new("/usr/bin/bash")
            .arg("-n")
            .arg(tmp.path())
            .status()
            .unwrap()
            .success()
    );
}
#[test]
fn untrusted_profiles_cannot_inject_paths_packages_or_shell_commands() {
    for name in [
        "git;id",
        "git\nreboot",
        "--overwrite",
        "https://x",
        "$(id)",
        "foo/bar",
    ] {
        assert!(!valid_package(name));
    }
    for path in [
        "../.ssh/id_rsa",
        "/etc/passwd",
        ".config/hypr/../../.ssh/key",
        ".config/hypr//test",
        ".config/hypr",
        ".ssh/config",
        ".config/../zshrc",
    ] {
        assert!(!valid_config(path), "{path}");
    }
    let mut v: Value = serde_json::from_str(&profile().encode().unwrap()).unwrap();
    v["version"] = json!(2);
    assert!(Profile::decode(&v.to_string()).is_err());
    v["version"] = json!(1);
    v["packages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"git","source":"repo"}));
    assert!(Profile::decode(&v.to_string()).is_err());
    let p = Profile {
        name: "bad\0name".into(),
        ..Profile::default()
    };
    assert!(p.encode().is_err());
}
#[test]
fn previews_do_not_write_and_applies_back_up_originals_privately() {
    let home = home();
    let state = home.path().join("state");
    let file = home.path().join(".config/kitty/kitty.conf");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "old colors").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let mut p = Profile::default();
    p.files.insert(
        ".config/kitty/kitty.conf".into(),
        "directory @ARCHFORGE_HOME@/Downloads".into(),
    );
    p.files
        .insert(".config/hypr/new.conf".into(), "new file".into());
    let plan = preview(&p, home.path()).unwrap();
    assert_eq!(plan.changes.len(), 2);
    assert!(!state.exists());
    assert_eq!(fs::read_to_string(&file).unwrap(), "old colors");
    let result = apply(plan, &state).unwrap();
    assert!(result.contains("Applied 2 dotfiles"));
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains(home.path().to_str().unwrap())
    );
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = fs::read_dir(state.join("archforge/loadouts/backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup_text = fs::read_to_string(&backup).unwrap();
    assert!(backup_text.contains("created_files"));
    let old = Profile::decode(&backup_text).unwrap();
    assert_eq!(old.files[".config/kitty/kitty.conf"], "old colors");
    assert_eq!(old.modes[".config/kitty/kitty.conf"], 0o600);
    apply(preview(&old, home.path()).unwrap(), &state).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "old colors");
}
#[test]
fn changed_files_abort_the_whole_preflight_before_writing() {
    let home = home();
    let file = home.path().join(".config/hypr/z.conf");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "before").unwrap();
    let mut p = Profile::default();
    p.files.insert(".config/hypr/a.conf".into(), "new".into());
    p.files
        .insert(".config/hypr/z.conf".into(), "desired".into());
    let plan = preview(&p, home.path()).unwrap();
    fs::write(&file, "changed after preview").unwrap();
    let state = home.path().join("state");
    assert!(
        apply(plan, &state)
            .unwrap_err()
            .contains("changed after preview")
    );
    assert!(!home.path().join(".config/hypr/a.conf").exists());
    assert!(!state.exists());
}
#[test]
fn symlinked_targets_and_parents_cannot_escape_the_home() {
    let home = home();
    let outside = tempfile::tempdir().unwrap();
    let victim = outside.path().join("victim");
    fs::write(&victim, "do not touch").unwrap();
    fs::create_dir_all(home.path().join(".config/kitty")).unwrap();
    std::os::unix::fs::symlink(&victim, home.path().join(".config/kitty/kitty.conf")).unwrap();
    let mut p = Profile::default();
    p.files
        .insert(".config/kitty/kitty.conf".into(), "bad".into());
    assert!(preview(&p, home.path()).is_err());
    fs::remove_file(home.path().join(".config/kitty/kitty.conf")).unwrap();
    fs::remove_dir(home.path().join(".config/kitty")).unwrap();
    std::os::unix::fs::symlink(outside.path(), home.path().join(".config/kitty")).unwrap();
    assert!(preview(&p, home.path()).is_err());
    assert_eq!(fs::read_to_string(&victim).unwrap(), "do not touch");
}
#[test]
fn parent_swap_after_review_and_backup_symlinks_are_refused() {
    let home = home();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".config/kitty")).unwrap();
    let mut p = Profile::default();
    p.files
        .insert(".config/kitty/kitty.conf".into(), "new".into());
    let plan = preview(&p, home.path()).unwrap();
    fs::remove_dir(home.path().join(".config/kitty")).unwrap();
    std::os::unix::fs::symlink(outside.path(), home.path().join(".config/kitty")).unwrap();
    assert!(apply(plan, &home.path().join("state")).is_err());
    assert!(!outside.path().join("kitty.conf").exists());
    fs::remove_file(home.path().join(".config/kitty")).unwrap();
    let plan = preview(&p, home.path()).unwrap();
    let state = home.path().join("state-alias");
    std::os::unix::fs::symlink(outside.path(), &state).unwrap();
    assert!(apply(plan, &state).is_err());
    assert!(!home.path().join(".config/kitty/kitty.conf").exists());
}
#[test]
fn optional_capture_only_collects_selected_text_configs_and_rebases_home() {
    let home = home();
    let kitty = home.path().join(".config/kitty");
    fs::create_dir_all(&kitty).unwrap();
    fs::write(
        kitty.join("kitty.conf"),
        format!("path {}/Downloads", home.path().display()),
    )
    .unwrap();
    fs::write(kitty.join("binary.png"), [0, 1, 2, 3]).unwrap();
    fs::write(kitty.join("secrets.conf"), "private").unwrap();
    fs::create_dir_all(home.path().join(".ssh")).unwrap();
    fs::write(home.path().join(".ssh/id_rsa"), "private key").unwrap();
    std::os::unix::fs::symlink(home.path().join(".ssh/id_rsa"), kitty.join("alias")).unwrap();
    let mut p = Profile::default();
    let skip = capture_files(&mut p, home.path(), &[4]).unwrap();
    assert_eq!(p.files.len(), 1);
    assert!(p.files[".config/kitty/kitty.conf"].contains(HOME_TOKEN));
    assert!(!p.encode().unwrap().contains(home.path().to_str().unwrap()));
    assert!(skip.len() >= 3);
}
#[test]
fn mode_only_changes_and_size_limits_are_checked() {
    let home = home();
    let file = home.path().join(".zshrc");
    fs::write(&file, "same").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    let mut p = Profile::default();
    p.files.insert(".zshrc".into(), "same".into());
    p.modes.insert(".zshrc".into(), 0o600);
    assert_eq!(preview(&p, home.path()).unwrap().changes.len(), 1);
    p.files.insert(".zshrc".into(), "a".repeat(MAX_TEXT + 1));
    assert!(p.validate().is_err());
}
#[test]
fn moved_home_identity_aborts() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    fs::create_dir(&home).unwrap();
    let mut p = Profile::default();
    p.files.insert(".zshrc".into(), "hello".into());
    let plan = preview(&p, &home).unwrap();
    fs::rename(&home, root.path().join("moved")).unwrap();
    fs::create_dir(&home).unwrap();
    assert!(
        apply(plan, &root.path().join("state"))
            .unwrap_err()
            .contains("home directory changed")
    );
    assert!(!home.join(".zshrc").exists());
}
#[test]
fn catalog_and_every_desktop_kit_have_valid_reproducible_packages() {
    let catalog = crate::loadout_styles::catalog();
    assert_eq!(catalog.len(), 659);
    let mut names = BTreeSet::new();
    for app in catalog {
        assert!(valid_package(&app.name));
        assert!(CATEGORIES.contains(&app.category.as_str()));
        assert!(app.priority <= 2);
        assert!(app.popularity.is_none_or(|n| n <= 10000));
        assert!(names.insert(app.name));
    }
    for (i, _) in crate::loadout_styles::KITS.iter().enumerate() {
        let p = crate::loadout_styles::style(Some(i), true);
        p.validate().unwrap();
        assert!(!p.files.is_empty());
        assert!(p.command(false).unwrap().contains("--needed --"));
        for (path, text) in p.files {
            if path.ends_with("config.jsonc") {
                serde_json::from_str::<Value>(&text).unwrap();
            }
        }
    }
    let mut p = profile();
    p.packages.insert("code".into(), Source::Repo);
    assert!(p.command(false).unwrap_err().contains("Choose Code"));
}

#[test]
fn fifo_profiles_and_changed_permissions_do_not_get_applied() {
    let tmp = tempfile::tempdir().unwrap();
    let fifo = tmp.path().join("profile.json");
    let c = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    assert!(Profile::read(&fifo).is_err());
    let file = tmp.path().join(".zshrc");
    fs::write(&file, "original").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let mut p = Profile::default();
    p.files.insert(".zshrc".into(), "desired".into());
    let plan = preview(&p, tmp.path()).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        apply(plan, &tmp.path().join("state"))
            .unwrap_err()
            .contains("changed after preview")
    );
    assert_eq!(fs::read_to_string(file).unwrap(), "original");
}
#[test]
fn home_expansion_and_existing_backup_totals_are_bounded() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Profile::default();
    p.files.insert(
        ".zshrc".into(),
        HOME_TOKEN.repeat(MAX_TEXT / HOME_TOKEN.len()),
    );
    let long_home = tmp.path().join("a-long-home-name-for-expansion-bounds");
    fs::create_dir(&long_home).unwrap();
    assert!(preview(&p, &long_home).is_err());
    p.files.clear();
    fs::create_dir_all(tmp.path().join(".config/hypr")).unwrap();
    for i in 0..9 {
        let path = format!(".config/hypr/{i}.conf");
        fs::write(tmp.path().join(&path), "x".repeat(MAX_TEXT)).unwrap();
        p.files.insert(path, "small replacement".into());
    }
    assert!(preview(&p, tmp.path()).is_err());
}
#[test]
fn generated_hyprland_lua_parses_without_starting_a_compositor() {
    if !Path::new("/usr/bin/luac").is_file() {
        return;
    }
    let p = crate::loadout_styles::style(Some(0), true);
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), &p.files[".config/hypr/hyprland.lua"]).unwrap();
    assert!(
        std::process::Command::new("/usr/bin/luac")
            .arg("-p")
            .arg(file.path())
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn plasma_palette_and_repository_names_roundtrip() {
    let mut p = crate::loadout_styles::style(Some(1), true);
    assert!(
        p.files
            .contains_key(".local/share/color-schemes/ArchForgeAmber.colors")
    );
    let key = gtk::glib::KeyFile::new();
    key.load_from_data(
        &p.files[".config/kdeglobals"],
        gtk::glib::KeyFileFlags::NONE,
    )
    .unwrap();
    assert_eq!(
        key.string("Colors:Selection", "BackgroundNormal").unwrap(),
        "255,180,84"
    );
    p.repositories = vec!["core".into(), "extra".into(), "custom-repo".into()];
    let loaded = Profile::decode(&p.encode().unwrap()).unwrap();
    assert_eq!(loaded.repositories, p.repositories);
    assert!(!valid_config(
        ".local/share/color-schemes/autostart/script.sh"
    ));
}

#[test]
fn expanded_catalog_has_public_provenance_for_every_entry() {
    let data: Value =
        serde_json::from_str(include_str!("../data/loadouts/catalog-metadata.json")).unwrap();
    let packages = data["packages"].as_object().unwrap();
    let apps = crate::loadout_styles::catalog();
    assert_eq!(packages.len(), apps.len());
    let mut official = 0;
    let mut community = 0;
    for app in apps {
        let meta = &packages[&app.name];
        assert_eq!(
            app.popularity,
            meta["pkgstats_percent"]
                .as_f64()
                .map(|n| (n * 100.).round() as u32)
        );

        assert!(meta["upstream"].as_str().unwrap().starts_with("http"));
        assert!(!meta["version"].as_str().unwrap().is_empty());
        match app.source {
            Source::Repo => {
                official += 1;
                assert!(
                    ["core", "extra", "multilib"].contains(&meta["repository"].as_str().unwrap())
                );
                assert!(
                    meta["package_page"]
                        .as_str()
                        .unwrap()
                        .starts_with("https://archlinux.org/packages/")
                );
            }
            Source::Aur => {
                community += 1;
                assert!(!meta["maintainer"].as_str().unwrap().is_empty());
                assert_eq!(meta["recipe_sha256"].as_str().unwrap().len(), 64);
                assert!(
                    meta["package_page"]
                        .as_str()
                        .unwrap()
                        .starts_with("https://aur.archlinux.org/packages/")
                );
            }
            Source::Foreign => panic!("Personal packages must not enter the bundled catalog"),
        }
    }
    assert_eq!((official, community), (621, 38));
    let ai: Vec<_> = crate::loadout_styles::catalog()
        .into_iter()
        .filter(|a| a.category == "AI & ML")
        .collect();
    assert_eq!(ai.len(), 32);
    for name in [
        "openai-codex",
        "ollama-vulkan",
        "claude-code",
        "lmstudio-bin",
        "jupyterlab",
    ] {
        assert!(ai.iter().any(|a| a.name == name));
    }
}
#[test]
fn recommendation_and_popularity_sort_are_distinct_and_installed_order_is_local() {
    let catalog = crate::loadout_styles::catalog();
    let mut common = catalog.iter().find(|a| a.name == "git").unwrap().clone();
    common.priority = 0;
    common.popularity = Some(2500);
    let mut alternative = catalog.iter().find(|a| a.name == "nano").unwrap().clone();
    alternative.priority = 1;
    alternative.popularity = Some(9000);
    let mut unmeasured = catalog.iter().find(|a| a.name == "fzf").unwrap().clone();
    unmeasured.priority = 2;
    unmeasured.popularity = None;
    let mut apps = vec![alternative.clone(), unmeasured.clone(), common.clone()];
    let installed = BTreeMap::from([("fzf".to_owned(), "1.0".to_owned())]);
    crate::loadout_styles::sort_catalog(&mut apps, 0, &installed);
    assert_eq!(
        apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["git", "nano", "fzf"]
    );
    crate::loadout_styles::sort_catalog(&mut apps, 1, &installed);
    assert_eq!(
        apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["nano", "git", "fzf"]
    );
    crate::loadout_styles::sort_catalog(&mut apps, 4, &installed);
    assert_eq!(apps[0].name, "fzf");
    assert!(crate::loadout_styles::ranking_note(&unmeasured).contains("No pkgstats figure"));
    assert!(crate::loadout_styles::ranking_note(&common).contains("25.00%"));
}
#[test]
fn bulk_install_rejects_conflicting_variants_but_keeps_ollama_accelerators_valid() {
    for names in [
        ["libreoffice-fresh", "libreoffice-still"],
        ["wine", "wine-staging"],
        ["python-pytorch", "python-pytorch-cuda"],
        ["python-pytorch-cuda", "python-pytorch-rocm"],
        ["python-tensorflow", "python-tensorflow-cuda"],
    ] {
        let mut p = Profile::default();
        for n in names {
            p.packages.insert(n.into(), Source::Repo);
        }
        assert!(p.command(false).unwrap_err().contains("Choose one"));
    }
    let mut p = Profile::default();
    p.packages.insert("ollama".into(), Source::Repo);
    p.packages.insert("ollama-vulkan".into(), Source::Repo);
    assert!(p.command(false).is_ok());
}
