use super::*;
fn drive() -> Drive {
    Drive {
        target: "/mnt/apps".into(),
        source: "/dev/test-apps".into(),
        uuid: "1111-aaaa".into(),
        fs: "ext4".into(),
        options: "rw,relatime".into(),
        available: Some(100 * 1024 * 1024 * 1024),
        same_root: false,
    }
}
#[test]
fn names_and_app_ids_cannot_inject_commands_or_configuration() {
    for id in [
        "",
        "Extra",
        "../evil",
        "extra\nPath=/tmp",
        "extra;id",
        "extra'",
        "-extra",
    ] {
        assert!(valid_id(id).is_err(), "{id}");
    }
    for id in ["extra", "games-2"] {
        valid_id(id).unwrap();
    }
    for id in [
        "firefox",
        "org.mozilla.firefox;id",
        "org.foo/branch",
        "org..App",
        "org.foo.App\n",
    ] {
        assert!(!valid_app(id), "{id}");
    }
    assert!(valid_app("org.mozilla.firefox"));
    assert!(valid_app("com.valvesoftware.Steam"));
    let command = install_command("extra", "org.mozilla.firefox").unwrap();
    assert!(command.starts_with("/usr/bin/archforge --check-app-drive 'extra' &&"));
    assert!(command.contains("FLATPAK_CONFIG_DIR=/etc/flatpak"));
    assert!(command.contains("--installation='archforge-extra'"));
    assert!(!command.contains("--noninteractive"));
    assert!(install_command("extra", "org.foo.App;id").is_err());
}
#[test]
fn shared_root_btrfs_and_non_linux_or_non_executable_mounts_are_rejected() {
    let rows=mount_rows(r#"{"filesystems":[{"target":"/","source":"/dev/root[/@]","fstype":"btrfs","options":"rw","uuid":"aaa"},{"target":"/home","source":"/dev/root[/@home]","fstype":"btrfs","options":"rw","uuid":"aaa"}]}"#).unwrap();
    assert!(rows[1].same_root);
    assert!(eligibility(&rows[1]).is_err());
    let d = drive();
    eligibility(&d).unwrap();
    for fs in ["ntfs", "vfat", "exfat", "nfs"] {
        let mut other = d.clone();
        other.fs = fs.into();
        assert!(eligibility(&other).is_err());
    }
    for options in ["ro", "rw,noexec", ""] {
        let mut other = d.clone();
        other.options = options.into();
        assert!(eligibility(&other).is_err());
    }
    assert!(mount_rows(r#"{"filesystems":[]}"#).is_err());
}
#[test]
fn configured_identity_prevents_fallback_to_another_drive() {
    let d = drive();
    let config = configuration("extra", &d).unwrap();
    let registered = parse_configuration(&config, "extra").unwrap();
    assert_eq!(registered.path, "/mnt/apps/archforge-apps-extra");
    verify(&registered, std::slice::from_ref(&d)).unwrap();
    assert!(
        verify(&registered, &[])
            .unwrap_err()
            .contains("not mounted")
    );
    let mut wrong = d.clone();
    wrong.uuid = "different".into();
    assert!(
        verify(&registered, &[wrong])
            .unwrap_err()
            .contains("different drive")
    );
    assert!(
        parse_configuration(
            &config.replace(
                "Path=/mnt/apps/archforge-apps-extra",
                "Path=/var/lib/flatpak"
            ),
            "extra"
        )
        .is_err()
    );
    assert!(configuration("bad\nPath=/tmp", &d).is_err());
}
#[test]
fn protected_directories_and_unprivileged_registration_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let actual = tmp.path().join("directory");
    fs::create_dir(&actual).unwrap();
    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&actual, &link).unwrap();
    assert!(protected(&link).is_err());
    if unsafe { libc::geteuid() } != 0 {
        assert!(protected(&actual).is_err());
        assert!(register("extra", "/mnt/apps").unwrap_err().contains("sudo"));
    }
}
#[test]
fn flatpak_reads_generated_named_installation_in_a_temporary_config() {
    if !Path::new("/usr/bin/flatpak").is_file() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let site = tmp.path().join("etc");
    fs::create_dir_all(site.join("installations.d")).unwrap();
    let app_path = tmp.path().join("archforge-apps-extra");
    fs::create_dir(&app_path).unwrap();
    let content = configuration("extra", &drive())
        .unwrap()
        .replace("/mnt/apps", tmp.path().to_str().unwrap());
    fs::write(site.join("installations.d/archforge-extra.conf"), content).unwrap();
    let result = std::process::Command::new("/usr/bin/flatpak")
        .arg("--installations")
        .env("FLATPAK_CONFIG_DIR", &site)
        .env("FLATPAK_SYSTEM_DIR", tmp.path().join("system"))
        .env("FLATPAK_USER_DIR", tmp.path().join("user"))
        .env("XDG_DATA_HOME", tmp.path().join("data"))
        .env("XDG_CONFIG_HOME", tmp.path().join("config"))
        .env("XDG_CACHE_HOME", tmp.path().join("cache"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains(app_path.to_str().unwrap()));
}
