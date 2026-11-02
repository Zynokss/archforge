use std::{thread, time::Instant};
pub(super) static INTERCEPT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
pub(super) static RESTART_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
use super::*;
use serde_json::json;
use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};

fn release_json(version: &str) -> Value {
    json!({"draft": false, "prerelease": false, "tag_name": format!("v{version}"), "body":"Release notes", "assets":[{
        "name":format!("archforge-{version}-1-x86_64.pkg.tar.zst"),
        "browser_download_url":format!("{DOWNLOADS}/v{version}/archforge-{version}-1-x86_64.pkg.tar.zst"),
        "digest":format!("sha256:{}", "a".repeat(64)), "size": 1234, "state":"uploaded"
    }]})
}

#[test]
fn versions_assets_and_untrusted_metadata() {
    let data = release_json("1.0.0");
    let release = parse_release(&data.to_string(), "x86_64").unwrap();
    assert!(release.version > Version::parse(CURRENT).unwrap());
    assert!(
        parse_release(&data.to_string(), "aarch64")
            .unwrap()
            .asset
            .is_none()
    );
    assert!(Version::parse("0.10.0").unwrap() > Version::parse("0.9.0").unwrap());
    for bad in ["garbage", "{}", "[]"] {
        assert!(parse_release(bad, "x86_64").is_err());
    }
    for (key, value) in [
        ("browser_download_url", json!("https://example.org/evil")),
        ("digest", json!(null)),
        ("digest", json!("sha256:bad")),
        ("size", json!(0)),
        ("size", json!(MAX_PACKAGE + 1)),
        ("state", json!("new")),
    ] {
        let mut bad = data.clone();
        bad["assets"][0][key] = value;
        assert!(parse_release(&bad.to_string(), "x86_64").is_err(), "{key}");
    }
    for (key, value) in [
        ("draft", json!(true)),
        ("prerelease", json!(true)),
        ("tag_name", json!("v1.0.0-rc.1")),
        ("tag_name", json!("../../evil")),
    ] {
        let mut bad = data.clone();
        bad[key] = value;
        assert!(parse_release(&bad.to_string(), "x86_64").is_err());
    }
    let mut duplicate = data.clone();
    duplicate["assets"]
        .as_array_mut()
        .unwrap()
        .push(data["assets"][0].clone());
    assert!(parse_release(&duplicate.to_string(), "x86_64").is_err());
}

#[test]
fn checksum_metadata_and_private_download_directory() {
    let directory = private_download_dir().unwrap();
    assert_eq!(
        std::fs::metadata(directory.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let file = directory.path().join("package.pkg.tar.zst");
    std::fs::write(&file, b"abc").unwrap();
    let asset = Asset {
        name: "package".into(),
        url: "unused".into(),
        size: 3,
        digest: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
    };
    verify_file(&file, &asset).unwrap();
    std::fs::write(&file, b"abd").unwrap();
    assert!(verify_file(&file, &asset).unwrap_err().contains("checksum"));
    std::fs::write(&file, b"a").unwrap();
    assert!(
        verify_file(&file, &asset)
            .unwrap_err()
            .contains("incomplete")
    );
    let meta = "Name : archforge\nVersion : 1.0.0-1\nArchitecture : x86_64\n";
    let version = Version::parse("1.0.0").unwrap();
    assert_eq!(
        verify_metadata(meta, &version, "x86_64").unwrap(),
        "1.0.0-1"
    );
    for bad in [
        meta.replace("archforge", "other"),
        meta.replace("1.0.0", "1.1.0"),
        meta.replace("x86_64", "aarch64"),
        format!("{meta}Name : archforge\n"),
        meta.replace("-1", "-1;evil"),
    ] {
        assert!(verify_metadata(&bad, &version, "x86_64").is_err());
    }
    let folder = directory.path().to_path_buf();
    drop(directory);
    assert!(!folder.exists());
}

#[test]
fn installer_uses_pacman_and_handles_authentication_and_errors() {
    let path = Path::new("/tmp/a path/package.pkg.tar.zst");
    let cmd = install_command(path);
    assert_eq!(cmd.get_program(), "/usr/bin/pkexec");
    assert_eq!(
        cmd.get_args().collect::<Vec<_>>(),
        [
            "/usr/bin/pacman",
            "-U",
            "--noconfirm",
            "--needed",
            "/tmp/a path/package.pkg.tar.zst"
        ]
    );
    for (code, ok) in [(0, true), (126, false), (127, false), (1, false)] {
        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: vec![],
            stderr: b"database is locked".to_vec(),
        };
        let result = install_result(output);
        assert_eq!(result.is_ok(), ok);
        if code == 1 {
            assert!(result.unwrap_err().contains("database is locked"));
        }
    }
}

pub(super) fn ui(parent: &gtk::ApplicationWindow) {
    let view = create_view(parent);
    let release = parse_release(&release_json("1.0.0").to_string(), "x86_64").unwrap();
    view.checked(Ok(Some(release.clone())));
    let content = view.launcher.child().unwrap();
    assert!(content.first_child().unwrap().is::<gtk::Image>());
    let caption = content
        .last_child()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    assert_eq!(caption.text(), "Update available");
    assert!(view.update.get_visible() && view.update.is_sensitive());
    view.launcher.emit_clicked();
    crate::dialogs::snapshot(&view.dialog, "update-available-fixture");
    assert!(view.dialog.is_visible());
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::process::Command::new("import")
        .args(["-window", "root", "/tmp/archforge-updater-dialog.png"])
        .status()
        .unwrap();
    view.activity("Downloading update…", "Checking package");
    assert!(!view.update.is_sensitive() && !view.check.is_sensitive());
    view.failed("The package checksum does not match GitHub. No changes were made.");
    assert!(view.update.is_sensitive() && view.check.is_sensitive());
    assert!(view.detail.text().contains("checksum"));
    view.installing.set(true);
    assert!(parent.emit_by_name::<bool>("close-request", &[]));
    view.installed(Err("Authentication cancelled".into()), &release.version);
    assert!(!view.installing.get());
    assert!(view.update.is_sensitive());
    RESTART_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    let guard = crate::Operation::new();
    view.installed(Ok(()), &release.version);
    assert!(view.status.text().contains("1.0.0 is installed"));
    assert!(!view.update.get_visible() && !view.check.is_sensitive());
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(450) {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(RESTART_CALLS.load(std::sync::atomic::Ordering::SeqCst), 0);
    drop(guard);
    let deadline = Instant::now() + Duration::from_secs(2);
    while RESTART_CALLS.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    assert!(view.restart.get_visible());
    assert_eq!(restart_command().get_program(), "/usr/bin/archforge");
    view.checked(Ok(Some(
        parse_release(&release_json(CURRENT).to_string(), "x86_64").unwrap(),
    )));
    assert_eq!(view.status.text(), "You're up to date");
    crate::dialogs::snapshot(&view.dialog, "up-to-date-fixture");
    assert!(!view.update.get_visible());
    view.checked(Ok(None));
    assert_eq!(view.status.text(), "No releases published yet");
    view.checked(Err("Offline".into()));
    assert_eq!(view.detail.text(), "Offline");
    let mut no_asset = release;
    no_asset.asset = None;
    view.checked(Ok(Some(no_asset)));
    assert!(!view.update.get_visible() && !view.update.is_sensitive());
    view.dialog.destroy();
    println!(
        "Updater UI: available, current, no release, offline, unsupported architecture, retry, authentication cancellation, success, and install close guard passed."
    );
}

#[test]
#[ignore]
fn download_published_release_without_installing() {
    let release = check().unwrap().unwrap();
    let downloaded = download(&release).unwrap();
    assert!(downloaded.path.exists());
    let path = downloaded.path.clone();
    drop(downloaded);
    assert!(!path.exists());
}

#[test]
#[ignore]
fn restart_handoff_child() {
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id(crate::APP_ID)
        .build();
    app.connect_activate(|app| {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Restart handoff probe")
            .default_width(400)
            .default_height(180)
            .build();
        window.present();
        glib::timeout_add_local_once(Duration::from_millis(300), || {
            INTERCEPT.store(false, std::sync::atomic::Ordering::SeqCst);
            panic!("exec failed: {}", super::restart());
        });
    });
    app.run_with_args(&["archforge"]);
}
