use super::*;
use std::{path::Path, process::Command, time::Instant};
#[test]
fn package_preview_summaries_use_reported_values_only() {
    assert_eq!(
        cache_summary("==> no candidate packages found for pruning"),
        Some(CacheSummary {
            count: 0,
            space: "0 B".into()
        })
    );
    assert_eq!(
        cache_summary("==> finished dry run: 6 candidates (disk space saved: 2.50 MiB)"),
        Some(CacheSummary {
            count: 6,
            space: "2.50 MiB".into()
        })
    );
    assert_eq!(
        cache_summary("==> finished: 6 packages removed (disk space saved: 2.50 MiB)"),
        Some(CacheSummary {
            count: 6,
            space: "2.50 MiB".into()
        })
    );
    for text in [
        "",
        "permission denied",
        "unexpected output",
        "finished dry run: unknown candidates (disk space saved: 1 MiB)",
    ] {
        assert!(cache_summary(text).is_none());
    }
}
fn pump() {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(400) {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
pub(super) fn capture(window: &gtk::Window, name: &str) {
    pump();
    let tree = Command::new("xwininfo")
        .args(["-root", "-tree"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&tree.stdout);
    let title = format!("\"{}\"", window.title().unwrap());
    let id = text
        .lines()
        .find(|line| line.contains(&title))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    std::fs::create_dir_all("/tmp/archforge-dialogs").unwrap();
    assert!(
        Command::new("import")
            .args(["-window", id, &format!("/tmp/archforge-dialogs/{name}.png")])
            .status()
            .unwrap()
            .success()
    );
}
pub(super) fn ui(parent: &gtk::ApplicationWindow) {
    let (window, view) = create_job(parent, Job::CacheClean);
    view.authorize.set_sensitive(false);
    window.present();
    capture(&window, "loading");
    view.preview(Ok("==> no candidate packages found for pruning".into()));
    assert_eq!(view.stats[0].text(), "0 packages");
    assert_eq!(view.stats[1].text(), "0 B");
    assert!(!view.authorize.get_visible() && !view.authorize.is_sensitive());
    assert_eq!(view.close.label().as_deref(), Some("Done"));
    assert!(view.status.emblem.has_css_class("success"));
    assert!(view.details.expander.is_expanded());
    capture(&window, "already-tidy");
    view.details.expander.set_expanded(true);
    view.copy.emit_clicked();
    let copied = glib::MainContext::default()
        .block_on(
            gtk::gdk::Display::default()
                .unwrap()
                .clipboard()
                .read_text_future(),
        )
        .unwrap()
        .unwrap();
    assert!(copied.contains("no candidate"));
    capture(&window, "technical-details");
    window.close();
    let (window, view) = create_job(parent, Job::CacheClean);
    window.present();
    view.preview(Ok("/var/cache/pacman/pkg/example.pkg.tar.zst\n==> finished dry run: 6 candidates (disk space saved: 2.50 MiB)".into()));
    assert_eq!(view.stats[0].text(), "6 packages");
    assert!(view.authorize.is_sensitive() && view.authorize.get_visible());
    assert!(view.details.expander.is_expanded());
    capture(&window, "eligible-preview-fixture");
    window.close();
    let (window, view) = create_job(parent, Job::CacheClean);
    window.present();
    view.preview(Err(
        "Permission denied: fixture error; no operation was executed.".into(),
    ));
    assert!(view.status.emblem.has_css_class("error"));
    assert!(!view.authorize.get_visible());
    assert!(view.details.expander.is_expanded());
    capture(&window, "preview-error-fixture");
    window.close();
    let (window, view) = create_job(parent, Job::CacheSize);
    window.present();
    view.finished(Err(
        "Exit 1\n1G /var/cache/pacman/pkg\ndu: Permission denied".into(),
    ));
    assert!(view.status.emblem.has_css_class("warning"));
    assert!(view.retry.get_visible());
    capture(&window, "partial-measurement-fixture");
    window.close();
    let (window, view) = create_job(parent, Job::Failed);
    window.present();
    view.finished(Ok("UNIT LOAD ACTIVE SUB DESCRIPTION\nexample.service loaded failed failed Example\n1 loaded units listed.\n".into()));
    assert!(view.status.emblem.has_css_class("warning"));
    assert_eq!(view.stats[0].text(), "1 unit");
    capture(&window, "failed-units-fixture");
    window.close();
    let (window, view) = create_job(parent, Job::CacheClean);
    window.present();
    view.finished(Ok(
        "==> finished: 1 packages removed (disk space saved: 1 MiB)".into(),
    ));
    assert_eq!(view.stats[0].text(), "1 package");
    assert_eq!(view.stats.captions[0].text(), "Archives processed");
    assert!(view.details.expander.is_expanded());
    assert!(!view.authorize.get_visible());
    window.close();
    let (window, view) = create_job(parent, Job::Vulkan);
    view.finished(Ok("deviceName = Test Radeon\napiVersion = 1.3.0\n".into()));
    assert_eq!(view.stats[0].text(), "Test Radeon");
    assert_eq!(view.stats[1].text(), "1.3.0");
    window.close();
    let (window, view) = create_job(parent, Job::Boot);
    view.finished(Ok(
        "1min 4.2s example.service\n300ms second.service\n".into()
    ));
    assert_eq!(view.stats[0].text(), "example.service");
    assert_eq!(view.stats[1].text(), "1min 4.2s");
    window.close();
    let (window, view) = create_job(parent, Job::CacheSizeAdmin);
    window.set_default_size(940, 510);
    window.present();
    let long = (0..150)
        .map(|i| format!("Line {i}: fixture output\n"))
        .collect::<String>();
    view.finished(Ok(format!("14G /var/cache/pacman/pkg\n{long}")));
    pump();
    let scroll = view
        .details
        .output
        .parent()
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    assert!(scroll.vadjustment().upper() > scroll.vadjustment().page_size());
    assert!(view.details.output.height() >= 80);
    assert!(view.close.is_mapped());
    view.details.expander.set_expanded(false);
    assert!(view.details.expander.is_expanded());
    capture(&window, "long-output-small-window");
    window.close();
    crate::show_setup(parent, "GameMode", "gamemode", false);
    let window = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some("Install GameMode"))
        .unwrap();
    capture(&window, "install-setup");
    window.close();
    println!(
        "Dialog UI passed: empty, eligible, loading, error, partial, technical details, clipboard, setup, and non-default destructive actions."
    );
    assert!(Path::new("/tmp/archforge-dialogs/already-tidy.png").exists());
}
