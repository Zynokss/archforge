use super::*;
use std::{os::unix::fs::symlink, sync::Mutex, time::Instant};
pub(super) static OVERRIDE: Mutex<Option<Vec<PathBuf>>> = Mutex::new(None);

#[test]
fn cleanup_is_reviewed_bounded_and_does_not_follow_links() {
    let temp = tempfile::tempdir().unwrap();
    let cache = temp.path().join("cache");
    let outside = temp.path().join("outside");
    fs::create_dir_all(cache.join("nested/sub")).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(cache.join(".hidden"), "hidden").unwrap();
    fs::write(cache.join("nested/sub/download"), "cache").unwrap();
    fs::write(outside.join("keep"), "safe").unwrap();
    symlink(&outside, cache.join("linked")).unwrap();
    let plan = Plan::scan(vec![cache.clone()]).unwrap();
    assert_eq!(plan.count(), 3);
    assert!(cache.join(".hidden").exists(), "Preview deleted a file");
    fs::write(cache.join("created-after-review"), "keep").unwrap();
    plan.execute().unwrap();
    assert!(!cache.join(".hidden").exists());
    assert!(!cache.join("nested").exists());
    assert!(cache.join("created-after-review").exists());
    assert_eq!(fs::read_to_string(outside.join("keep")).unwrap(), "safe");
    symlink(&outside, temp.path().join("linked-root")).unwrap();
    assert!(Plan::scan(vec![temp.path().join("linked-root")]).is_err());
    assert!(Plan::scan(vec![PathBuf::from("/")]).is_err());
    assert!(Plan::scan(vec![glib::home_dir()]).is_err());
    symlink(glib::home_dir(), temp.path().join("home-alias")).unwrap();
    assert!(Plan::scan(vec![temp.path().join("home-alias/.")]).is_err());
    assert!(
        Plan::scan(vec![temp.path().join("does-not-exist")])
            .unwrap()
            .folders
            .is_empty()
    );
}
#[test]
fn replaced_root_and_nested_symlinks_are_kept_safe() {
    let temp = tempfile::tempdir().unwrap();
    let cache = temp.path().join("cache");
    let moved = temp.path().join("moved");
    fs::create_dir(&cache).unwrap();
    fs::write(cache.join("keep"), "saved").unwrap();
    let plan = Plan::scan(vec![cache.clone()]).unwrap();
    fs::rename(&cache, &moved).unwrap();
    fs::create_dir(&cache).unwrap();
    fs::write(cache.join("new"), "new").unwrap();
    assert!(plan.execute().unwrap_err().contains("changed since review"));
    assert!(moved.join("keep").exists());
    assert!(cache.join("new").exists());
    let plan = Plan::scan(vec![cache.clone()]).unwrap();
    fs::remove_file(cache.join("new")).unwrap();
    symlink(&moved, cache.join("new")).unwrap();
    plan.execute().unwrap();
    assert!(moved.join("keep").exists());
    assert!(!cache.join("new").exists());
}
#[test]
fn user_paths_and_orphan_commands_handle_other_accounts_and_empty_results() {
    let fallback = Path::new("/home/alex/.cache");
    assert_eq!(
        paths_for(Action::Trash, Path::new("/home/alex"), None, None, None).unwrap(),
        vec![
            PathBuf::from("/home/alex/.local/share/Trash/files"),
            PathBuf::from("/home/alex/.local/share/Trash/info")
        ]
    );
    assert_eq!(
        paths_for(
            Action::YayCache,
            Path::new("/home/sam"),
            Some("/custom/cache".into()),
            None,
            None
        )
        .unwrap(),
        vec![PathBuf::from("/custom/cache/yay")]
    );
    assert_eq!(
        paths_for(
            Action::PipCache,
            Path::new("/home/sam"),
            None,
            None,
            Some("/pip/downloads".into())
        )
        .unwrap(),
        vec![PathBuf::from("/pip/downloads")]
    );
    assert_eq!(xdg(None, fallback), fallback);
    assert_eq!(xdg(Some("relative".into()), fallback), fallback);
    assert_eq!(xdg(Some("/".into()), fallback), fallback);
    assert_eq!(
        xdg(Some("/custom/cache".into()), fallback),
        Path::new("/custom/cache")
    );
    assert!(orphan_command("").unwrap().is_none());
    assert!(orphan_command(" \n").unwrap().is_none());
    assert_eq!(
        orphan_command("liba\nlibb\n").unwrap().unwrap(),
        "sudo pacman -Rs -- liba libb"
    );
    assert!(orphan_command("-bad").is_err());
    assert!(orphan_command("liba;rm").is_err());
    assert!(
        !orphan_command("liba")
            .unwrap()
            .unwrap()
            .contains("--noconfirm")
    );
    assert!(crate::jobs::Job::UninstalledClean.destructive());
    assert!(
        crate::jobs::Job::UninstalledClean
            .display_command()
            .contains("-r -u -k 0")
    );
    assert!(
        crate::jobs::Job::CacheClean
            .display_command()
            .contains("-k 2")
    );
    assert!(
        crate::jobs::Job::JournalClean
            .display_command()
            .contains("7d")
    );
}
fn walk(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut out = vec![widget.clone()];
    let mut child = widget.first_child();
    while let Some(w) = child {
        out.extend(walk(&w));
        child = w.next_sibling();
    }
    out
}
fn button_named(root: &gtk::Widget, name: &str) -> gtk::Button {
    walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some(name))
        .unwrap()
}
fn dialog() -> gtk::Window {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some("Empty home Trash"))
        .unwrap()
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        if condition() {
            return;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
}
pub(super) fn ui(parent: &gtk::ApplicationWindow) {
    let temp = tempfile::tempdir().unwrap();
    let cache = temp.path().join("test-trash");
    fs::create_dir(&cache).unwrap();
    fs::write(cache.join(".hidden"), "test").unwrap();
    *OVERRIDE.lock().unwrap() = Some(vec![cache.clone()]);
    show(parent, Action::Trash);
    let dialog = dialog();
    let confirm = button_named(dialog.upcast_ref(), "Permanently clean");
    assert!(!confirm.is_sensitive());
    wait(|| confirm.is_sensitive());
    assert!(cache.join(".hidden").exists());
    crate::dialogs::snapshot(&dialog, "personal-cleanup-review");
    confirm.emit_clicked();
    wait(|| !cache.join(".hidden").exists());
    wait(|| {
        walk(dialog.upcast_ref()).iter().any(|w| {
            w.downcast_ref::<gtk::Label>()
                .is_some_and(|l| l.text() == "Cleanup completed")
        })
    });
    dialog.close();
    *OVERRIDE.lock().unwrap() = None;
    // Refreshing replaces page contents without accumulating shortcut controllers.
    let stack = walk(parent.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::Stack>().ok())
        .unwrap();
    let inv = crate::system::Inventory::load();
    let metrics = Rc::new(RefCell::new(crate::system::Metrics::default()));
    stack.set_visible_child_name("packages");
    let entry = walk(stack.child_by_name("packages").unwrap().as_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::SearchEntry>().ok())
        .unwrap();
    entry.set_text("pacman");
    for _ in 0..2 {
        crate::refresh_pages(parent, &stack, &inv, metrics.clone());
    }
    assert_eq!(stack.visible_child_name().as_deref(), Some("packages"));
    let entry = walk(stack.child_by_name("packages").unwrap().as_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::SearchEntry>().ok())
        .unwrap();
    assert_eq!(entry.text(), "pacman");
    let controllers = parent.observe_controllers();
    let count = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .filter_map(|w| w.downcast::<gtk::EventController>().ok())
        .filter(|c| c.name().as_deref() == Some("archforge-package-search"))
        .count();
    assert_eq!(count, 1);
    println!(
        "Cleanup UI: read-only preview, explicit confirmation, sandbox-only deletion, live page refresh, search preservation, and shortcut controller replacement passed."
    );
}

#[test]
fn deeply_nested_cache_is_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let cache = temp.path().join("cache");
    let mut deepest = cache.clone();
    for _ in 0..260 {
        deepest = deepest.join("x");
    }
    fs::create_dir_all(&deepest).unwrap();
    fs::write(deepest.join("keep"), "safe").unwrap();
    let result = Plan::scan(vec![cache]).unwrap().execute();
    assert!(result.unwrap_err().contains("deeply nested"));
    assert!(deepest.join("keep").exists());
}

#[test]
fn failed_trash_payload_cleanup_retains_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let files = temp.path().join("files");
    let info = temp.path().join("info");
    fs::create_dir(&files).unwrap();
    fs::create_dir(&info).unwrap();
    fs::write(files.join("item"), "data").unwrap();
    fs::write(info.join("item.trashinfo"), "metadata").unwrap();
    let plan = Plan::scan(vec![files.clone(), info.clone()]).unwrap();
    fs::rename(&files, temp.path().join("saved-files")).unwrap();
    fs::create_dir(&files).unwrap();
    assert!(plan.execute().is_err());
    assert!(info.join("item.trashinfo").exists());
}
