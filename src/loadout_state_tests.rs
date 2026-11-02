use super::*;
#[test]
fn cache_names_do_not_confuse_versions_signatures_or_partial_downloads() {
    assert_eq!(
        archive_name("visual-studio-code-bin-1.2.3-4-x86_64.pkg.tar.zst"),
        Some("visual-studio-code-bin")
    );
    assert_eq!(archive_name("git-2.50-1-x86_64.pkg.tar.xz"), Some("git"));
    for name in [
        "git-2.50-1-x86_64.pkg.tar.zst.sig",
        "git-2.50-1-x86_64.pkg.tar.zst.part",
        "download-123",
        "git.pkg.tar.zst",
    ] {
        assert_eq!(archive_name(name), None);
    }
}
#[test]
fn update_parser_and_status_distinguish_unknown_missing_cached_and_installed() {
    let mut state = Snapshot::default();
    assert_eq!(state.describe("git").0, "Not scanned");
    state.ready = true;
    assert_eq!(state.describe("git").0, "Not installed");
    state.cached.insert("git".into());
    assert_eq!(state.describe("git").0, "Cached · not installed");
    state.installed.insert("git".into(), "1:2.49-1".into());
    assert_eq!(state.describe("git").0, "Installed");
    state.updates = parse_updates("git 1:2.49-1 -> 1:2.50-2\n").unwrap();
    assert_eq!(state.describe("git").0, "Update available");
    assert!(
        state
            .describe("git")
            .2
            .contains("1:2.49-1 → repository 1:2.50-2")
    );
    assert!(parse_updates("error: failed to sync databases").is_err());
    assert!(parse_updates("git 1 2").is_err());
    assert!(parse_updates("").unwrap().is_empty());
}
#[test]
fn removal_is_explicit_interactive_and_rejects_shell_input() {
    assert_eq!(
        remove_command(&["git".into(), "curl".into(), "git".into()]).unwrap(),
        "sudo pacman -R -- curl git"
    );
    assert!(remove_command(&[]).is_err());
    for name in ["--noconfirm", "git;id", "$(id)", "git curl", "/tmp/pkg"] {
        assert!(remove_command(&[name.into()]).is_err());
    }
}

#[test]
fn removal_preflight_is_read_only_and_has_no_dependency_bypass() {
    let args = removal_preview_args(&["7zip".into(), "git".into(), "7zip".into()]).unwrap();
    assert_eq!(
        args,
        ["-R", "--print", "--print-format", "%n", "--", "7zip", "git"]
    );
    assert!(removal_preview_args(&[]).is_err());
    assert!(removal_preview_args(&["--nodeps".into()]).is_err());
}
#[test]
fn removal_preflight_reports_dependents_and_keeps_other_errors_distinct() {
    let names = vec!["7zip".into()];
    let output = "Exit 1\nerror: failed to prepare transaction (could not satisfy dependencies)\n:: removing 7zip breaks dependency '7zip' required by goverlay\n:: removing 7zip breaks dependency '7zip>=25' required by lutris-git\n";
    let plan = removal_result(&names, Err(output.into())).unwrap();
    assert_eq!(plan.blockers.len(), 2);
    assert_eq!(plan.blockers[0].required_by, "goverlay");
    assert_eq!(plan.blockers[1].required_by, "lutris-git");
    assert_eq!(plan.blockers[1].dependency, "7zip>=25");
    assert!(removal_result(&names, Err("error: database is locked".into())).is_err());
    assert!(removal_result(&names, Err("error: target not found: 7zip".into())).is_err());
    assert!(removal_result(&names, Ok("other-package\n".into())).is_err());
    assert!(removal_result(&names, Ok("".into())).is_err());
    assert!(
        removal_result(&names, Ok("7zip\n".into()))
            .unwrap()
            .blockers
            .is_empty()
    );
    let selected = vec!["7zip".into(), "goverlay".into(), "lutris-git".into()];
    assert!(
        removal_result(&selected, Ok("lutris-git\ngoverlay\n7zip\n".into()))
            .unwrap()
            .blockers
            .is_empty()
    );
}

#[test]
fn installed_application_variants_use_the_real_package_name() {
    let mut state = Snapshot::default();
    state.installed.insert("vesktop".into(), "1.6.8-1".into());
    assert_eq!(state.installed_variant("vesktop-bin"), "vesktop");
    assert_eq!(state.installed_variant("vesktop"), "vesktop");
    assert_eq!(state.installed_variant("unknown-bin"), "unknown-bin");
    state.installed.insert("lutris-git".into(), "1.0".into());
    state
        .installed
        .insert("adw-gtk-theme-git".into(), "1.0".into());
    assert_eq!(state.installed_variant("lutris"), "lutris-git");
    assert_eq!(
        state.installed_variant("adw-gtk-theme"),
        "adw-gtk-theme-git"
    );
    state
        .installed
        .insert("vesktop-bin".into(), "1.6.8-1".into());
    assert_eq!(state.installed_variant("vesktop-bin"), "vesktop-bin");
}

#[test]
fn generic_variants_preserve_real_identity_and_do_not_invent_sources() {
    use crate::loadouts::Source;
    let mut state = Snapshot {
        ready: true,
        ..Default::default()
    };
    state.installed.insert("firefox-bin".into(), "144-1".into());
    state
        .installed
        .insert("obs-studio-git".into(), "32-1".into());
    assert_eq!(state.installed_variant("firefox"), "firefox-bin");
    assert!(state.describe("firefox").2.contains("firefox-bin"));
    assert_eq!(state.describe("obs-studio").0, "Installed");
    assert_eq!(
        state.variant_source("firefox-bin", "firefox", Source::Repo),
        Source::Foreign
    );
    state.foreign = Some(BTreeSet::new());
    assert_eq!(
        state.variant_source("firefox-bin", "firefox", Source::Repo),
        Source::Repo
    );
    assert_eq!(
        remove_command(&[state.installed_variant("obs-studio").into()]).unwrap(),
        "sudo pacman -R -- obs-studio-git"
    );
}
