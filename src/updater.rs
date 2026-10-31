use crate::{button, column, label, row, system, wrapped};
use gtk::{glib, prelude::*};
use semver::Version;
use serde_json::Value;
use std::{
    cell::Cell,
    fs::File,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

const API: &str = "https://api.github.com/repos/Zynokss/archforge/releases/latest";
const DOWNLOADS: &str = "https://github.com/Zynokss/archforge/releases/download";
const RELEASES: &str = "https://github.com/Zynokss/archforge/releases";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const MAX_PACKAGE: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct Asset {
    pub name: String,
    pub url: String,
    pub digest: String,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct Release {
    pub version: Version,
    pub notes: String,
    pub asset: Option<Asset>,
    pub page: String,
}

pub(crate) fn parse_release(json: &str, arch: &str) -> Result<Release, String> {
    let data: Value = serde_json::from_str(json)
        .map_err(|_| "GitHub returned an unreadable release.".to_string())?;
    if data["draft"] != false || data["prerelease"] != false {
        return Err("No stable release is available.".into());
    }
    let tag = data["tag_name"]
        .as_str()
        .ok_or("The release has no version tag.")?;
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|_| "The release has an invalid version.".to_string())?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err("Only stable version tags are supported.".into());
    }
    let prefix = format!("archforge-{version}-");
    let suffix = format!("-{arch}.pkg.tar.zst");
    let mut asset = None;
    for item in data["assets"]
        .as_array()
        .ok_or("The release has no asset list.")?
    {
        let Some(name) = item["name"].as_str() else {
            continue;
        };
        let Some(rel) = name
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(&suffix))
        else {
            continue;
        };
        if rel.is_empty() || !rel.bytes().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let url = format!("{DOWNLOADS}/{tag}/{name}");
        if item["browser_download_url"].as_str() != Some(url.as_str())
            || item["state"] != "uploaded"
        {
            return Err(
                "The release package has an unexpected download location or is still uploading."
                    .into(),
            );
        }
        let digest = item["digest"]
            .as_str()
            .and_then(|s| s.strip_prefix("sha256:"))
            .filter(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))
            .ok_or(
                "GitHub has not provided a SHA-256 checksum for this package. Try again later.",
            )?;
        let size = item["size"]
            .as_u64()
            .filter(|s| *s > 0 && *s <= MAX_PACKAGE)
            .ok_or("The release package has an invalid size.")?;
        if asset.is_some() {
            return Err("The release contains more than one package for this architecture.".into());
        }
        asset = Some(Asset {
            name: name.into(),
            url,
            digest: digest.to_ascii_lowercase(),
            size,
        });
    }
    Ok(Release {
        version,
        notes: data["body"]
            .as_str()
            .unwrap_or("No release notes provided.")
            .chars()
            .take(12_000)
            .collect(),
        asset,
        page: format!("{RELEASES}/tag/{tag}"),
    })
}

fn curl_args() -> Vec<&'static str> {
    vec![
        "--disable",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "10",
        "--user-agent",
        concat!("Arch-Forge/", env!("CARGO_PKG_VERSION")),
    ]
}

pub(crate) fn check() -> Result<Option<Release>, String> {
    let mut args = curl_args();
    args.extend([
        "--max-time",
        "30",
        "--max-filesize",
        "1048576",
        "--header",
        "Accept: application/vnd.github+json",
        "--header",
        "X-GitHub-Api-Version: 2022-11-28",
        "--write-out",
        "\n%{http_code}",
        API,
    ]);
    let response = system::run("/usr/bin/curl", &args, 40).map_err(|e| {
        format!("Could not reach GitHub. Check your connection and try again.\n{e}")
    })?;
    let (body, status) = response
        .trim_end()
        .rsplit_once('\n')
        .ok_or("GitHub returned an incomplete response.")?;
    match status {
        "404" => Ok(None),
        "200" => parse_release(body, std::env::consts::ARCH).map(Some),
        "403" | "429" => Err("GitHub's request limit was reached. Please try again later.".into()),
        _ => Err(format!(
            "GitHub returned HTTP {status}. Please try again later."
        )),
    }
}

pub(crate) fn verify_file(path: &Path, asset: &Asset) -> Result<(), String> {
    let mut file =
        File::open(path).map_err(|e| format!("Cannot read the downloaded package: {e}"))?;
    if file.metadata().map_err(|e| e.to_string())?.len() != asset.size {
        return Err("The download is incomplete. No changes were made.".into());
    }
    let mut checksum =
        glib::Checksum::new(glib::ChecksumType::Sha256).ok_or("SHA-256 is unavailable.")?;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        checksum.update(&buffer[..count]);
    }
    if checksum.string().as_deref() != Some(&asset.digest) {
        return Err("The package checksum does not match GitHub. No changes were made.".into());
    }
    Ok(())
}

pub(crate) fn verify_metadata(
    metadata: &str,
    version: &Version,
    arch: &str,
) -> Result<String, String> {
    let field = |name: &str| -> Result<&str, String> {
        let values: Vec<_> = metadata
            .lines()
            .filter_map(|line| {
                let (key, value) = line.split_once(':')?;
                (key.trim() == name).then_some(value.trim())
            })
            .collect();
        if values.len() != 1 {
            return Err(format!("The package has an invalid {name} field."));
        }
        Ok(values[0])
    };
    if field("Name")? != "archforge" || field("Architecture")? != arch {
        return Err("The downloaded file is not an Arch Forge package for this computer.".into());
    }
    let package_version = field("Version")?;
    let (pkgver, pkgrel) = package_version
        .rsplit_once('-')
        .ok_or("The package version is invalid.")?;
    if pkgver != version.to_string()
        || pkgrel.is_empty()
        || !pkgrel.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("The package version does not match the release.".into());
    }
    Ok(package_version.to_string())
}

pub(crate) struct Download {
    // Keep the private directory alive until pacman has finished reading it.
    _directory: tempfile::TempDir,
    pub path: std::path::PathBuf,
}

pub(crate) fn private_download_dir() -> Result<tempfile::TempDir, String> {
    tempfile::Builder::new()
        .prefix("archforge-update-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .map_err(|e| format!("Cannot create a private download folder: {e}"))
}

pub(crate) fn download(release: &Release) -> Result<Download, String> {
    let asset = release
        .asset
        .as_ref()
        .ok_or("No package is available for this architecture.")?;
    let directory = private_download_dir()?;
    let path = directory.path().join(&asset.name);
    let path_text = path.to_str().ok_or("The download path cannot be used.")?;
    let mut args = curl_args();
    args.extend([
        "--fail",
        "--max-time",
        "300",
        "--max-filesize",
        "67108864",
        "--output",
        path_text,
        &asset.url,
    ]);
    system::run("/usr/bin/curl", &args, 310)
        .map_err(|e| format!("Download failed. No changes were made.\n{e}"))?;
    verify_file(&path, asset)?;
    let metadata = system::run("/usr/bin/pacman", &["-Qip", path_text], 20)?;
    let candidate = verify_metadata(&metadata, &release.version, std::env::consts::ARCH)?;
    if let Ok(installed) = system::run("/usr/bin/pacman", &["-Q", "archforge"], 10) {
        let installed = installed
            .split_whitespace()
            .nth(1)
            .ok_or("Cannot read the installed version.")?;
        let order = system::run("/usr/bin/vercmp", &[&candidate, installed], 5)?;
        if order.trim() != "1" {
            return Err("This version is already installed, or a newer version is installed. Close and reopen Arch Forge.".into());
        }
    }
    Ok(Download {
        _directory: directory,
        path,
    })
}

pub(crate) fn install_command(path: &Path) -> Command {
    let mut command = Command::new("/usr/bin/pkexec");
    command
        .args(["/usr/bin/pacman", "-U", "--noconfirm", "--needed"])
        .arg(path)
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    command
}

fn install(download: Download) -> Result<(), String> {
    // Do not time out or kill a package transaction. The window stays responsive
    // while pacman owns its lock and performs the upgrade in a worker thread.
    let output = install_command(&download.path)
        .output()
        .map_err(|e| format!("Cannot start the installer: {e}"))?;
    install_result(output)
}

pub(crate) fn install_result(output: std::process::Output) -> Result<(), String> {
    match output.status.code() {
        Some(0) => Ok(()),
        Some(126) | Some(127) => Err(
            "Installation was cancelled or authentication was unavailable. You can try again."
                .into(),
        ),
        _ => Err(format!(
            "Pacman could not install the update. Resolve the issue below and try again.\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )),
    }
}

struct View {
    launcher: gtk::Button,
    dialog: gtk::Window,
    status: gtk::Label,
    detail: gtk::Label,
    notes: gtk::TextBuffer,
    spinner: gtk::Spinner,
    check: gtk::Button,
    update: gtk::Button,
    release_link: gtk::LinkButton,
    busy: Rc<Cell<bool>>,
    installing: Rc<Cell<bool>>,
    available: Rc<std::cell::RefCell<Option<Release>>>,
}

impl View {
    fn activity(&self, text: &str, detail: &str) {
        self.busy.set(true);
        self.spinner.start();
        self.status.set_text(text);
        self.detail.set_text(detail);
        self.check.set_sensitive(false);
        self.update.set_sensitive(false);
    }

    fn idle(&self) {
        self.installing.set(false);
        self.busy.set(false);
        self.spinner.stop();
        self.check.set_sensitive(true);
        self.update.set_sensitive(
            self.available
                .borrow()
                .as_ref()
                .is_some_and(|r| r.asset.is_some()),
        );
    }

    fn check(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        self.activity(
            "Checking for updates…",
            "Looking for the latest stable Arch Forge release on GitHub.",
        );
        self.update.set_visible(false);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(check());
        });
        let view = self.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let result = match rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("The update check stopped unexpectedly. Please try again.".into()),
            };
            view.checked(result);
            glib::ControlFlow::Break
        });
    }

    fn checked(&self, result: Result<Option<Release>, String>) {
        *self.available.borrow_mut() = None;
        self.update.set_visible(false);
        self.launcher.set_label("App updates");
        self.launcher.remove_css_class("primary");
        self.release_link.set_uri(RELEASES);
        self.notes.set_text("");
        match result {
            Ok(Some(release))
                if release.version > Version::parse(CURRENT).expect("Cargo version") =>
            {
                self.status
                    .set_text(&format!("Arch Forge {} is available", release.version));
                self.detail.set_text(if release.asset.is_some() {
                        "Update directly from GitHub. Your desktop will ask for administrator authentication before pacman installs the package."
                    } else {
                        "A new version is available, but there is no downloadable package for this architecture yet. See the release page for options."
                    });
                self.notes.set_text(&release.notes);
                self.release_link.set_uri(&release.page);
                self.launcher.set_label("Update available");
                self.launcher.add_css_class("primary");
                self.update
                    .set_label(&format!("Update to {}", release.version));
                self.update.set_visible(release.asset.is_some());
                *self.available.borrow_mut() = Some(release);
            }
            Ok(Some(_)) => {
                self.status.set_text("You're up to date");
                self.detail.set_text(&format!(
                    "Arch Forge {CURRENT} is the latest version for this installation."
                ));
            }
            Ok(None) => {
                self.status.set_text("No releases published yet");
                self.detail.set_text("Arch Forge will offer an update here when a stable GitHub release is published.");
            }
            Err(error) => {
                self.status.set_text("Could not check for updates");
                self.detail.set_text(&error);
            }
        }
        self.idle();
    }

    fn update(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        let Some(release) = self.available.borrow().clone() else {
            return;
        };
        self.activity("Downloading update…", "Downloading and verifying the Arch package. Installation will request administrator authentication.");
        self.launcher.set_label("Updating…");
        let (tx, rx) = mpsc::channel();
        let version = release.version.clone();
        std::thread::spawn(move || {
            let result = download(&release);
            let _ = tx.send(result);
        });
        let view = self.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let result = match rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("The download stopped unexpectedly. Please try again.".into()),
            };
            match result {
                Ok(download) => view.install(download, version.clone()),
                Err(error) => view.failed(&error),
            }
            glib::ControlFlow::Break
        });
    }

    fn failed(&self, error: &str) {
        self.status.set_text("Update not installed");
        self.detail.set_text(error);
        self.launcher.set_label("Update available");
        self.idle();
    }

    fn install(self: &Rc<Self>, download: Download, version: Version) {
        self.installing.set(true);
        self.status.set_text("Installing update…");
        self.detail.set_text("Complete your desktop's authentication prompt. Keep Arch Forge open until pacman finishes.");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(install(download));
        });
        let view = self.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let result = match rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err(
                    "The installer stopped unexpectedly. Check pacman's status before retrying."
                        .into(),
                ),
            };
            view.installed(result, &version);
            glib::ControlFlow::Break
        });
    }
    fn installed(&self, result: Result<(), String>, version: &Version) {
        match result {
            Ok(()) => {
                *self.available.borrow_mut() = None;
                self.status
                    .set_text(&format!("Arch Forge {version} is installed"));
                self.detail
                    .set_text("Close and reopen Arch Forge to use the new version.");
                self.launcher.set_label("Updated · reopen app");
                self.update.set_visible(false);
                self.idle();
                self.check.set_sensitive(false);
            }
            Err(error) => self.failed(&error),
        }
    }
}

fn create_view(parent: &gtk::ApplicationWindow) -> Rc<View> {
    let launcher = button("App updates", false);
    launcher.set_margin_top(8);
    let dialog = gtk::Window::builder()
        .title("Arch Forge updates")
        .transient_for(parent)
        .modal(true)
        .hide_on_close(true)
        .destroy_with_parent(true)
        .default_width(600)
        .default_height(500)
        .build();
    let body = column(16);
    for set_margin in [
        gtk::prelude::WidgetExt::set_margin_top,
        gtk::prelude::WidgetExt::set_margin_bottom,
        gtk::prelude::WidgetExt::set_margin_start,
        gtk::prelude::WidgetExt::set_margin_end,
    ] {
        set_margin(body.upcast_ref::<gtk::Widget>(), 24);
    }
    body.append(&label("Arch Forge updates", "section-title"));
    body.append(&label(&format!("Current version  {CURRENT}"), "muted"));
    let status = wrapped("Ready to check", "row-title");
    body.append(&status);
    let detail = wrapped("Updates come from Zynokss/archforge on GitHub.", "muted");
    detail.set_selectable(true);
    body.append(&detail);
    let notes_view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(12)
        .bottom_margin(12)
        .left_margin(12)
        .right_margin(12)
        .build();
    let notes = notes_view.buffer();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&notes_view)
        .vexpand(true)
        .min_content_height(130)
        .build();
    scroll.add_css_class("console");
    body.append(&scroll);
    let release_link = gtk::LinkButton::with_label(RELEASES, "Release notes on GitHub");
    release_link.set_halign(gtk::Align::Start);
    body.append(&release_link);
    let actions = row(10);
    let spinner = gtk::Spinner::new();
    actions.append(&spinner);
    let check = button("Check again", false);
    actions.append(&check);
    let update = button("Update", true);
    update.set_visible(false);
    actions.append(&update);
    let close = button("Close", false);
    actions.append(&close);
    body.append(&actions);
    dialog.set_child(Some(&body));
    let view = Rc::new(View {
        launcher: launcher.clone(),
        dialog,
        status,
        detail,
        notes,
        spinner,
        check,
        update,
        release_link,
        busy: Rc::new(Cell::new(false)),
        installing: Rc::new(Cell::new(false)),
        available: Rc::new(std::cell::RefCell::new(None)),
    });
    // Closing the main app must not interrupt an authenticated transaction.
    let busy = view.installing.clone();
    let weak = view.dialog.downgrade();
    parent.connect_close_request(move |_| {
        if busy.get() {
            if let Some(dialog) = weak.upgrade() {
                dialog.present();
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    let weak = view.dialog.downgrade();
    close.connect_clicked(move |_| {
        if let Some(dialog) = weak.upgrade() {
            dialog.set_visible(false);
        }
    });
    let weak = Rc::downgrade(&view);
    launcher.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            v.dialog.present();
        }
    });
    let weak = Rc::downgrade(&view);
    view.check.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            v.check();
        }
    });
    let weak = Rc::downgrade(&view);
    view.update.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            v.update();
        }
    });
    // The parent owns the view; callbacks use weak references to avoid cycles.
    let owned = view.clone();
    parent.connect_destroy(move |_| owned.dialog.destroy());
    // Check quietly once per launch. Opening the dialog is always user initiated.
    let parent = parent.downgrade();
    let initial = view.clone();
    glib::timeout_add_local_once(Duration::from_secs(3), move || {
        if parent.upgrade().is_some_and(|p| p.is_visible()) {
            initial.check();
        }
    });
    view
}

pub fn launcher(parent: &gtk::ApplicationWindow) -> gtk::Button {
    create_view(parent).launcher.clone()
}
