use super::*;
fn pump() {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_millis(350) {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn contents(view: &View) -> String {
    let b = view.details.output.buffer();
    b.text(&b.start_iter(), &b.end_iter(), false).to_string()
}
pub(super) fn ui(parent: &gtk::ApplicationWindow) {
    let (window, view) = create(parent);
    let drive = app_storage::Drive {
        target: "/mnt/test-apps".into(),
        source: "/dev/fixture".into(),
        uuid: "1111-aaaa".into(),
        fs: "ext4".into(),
        options: "rw".into(),
        available: Some(100 * 1024 * 1024 * 1024),
        same_root: false,
    };
    let install = app_storage::Installation {
        id: "extra".into(),
        mount: drive.target.clone(),
        uuid: drive.uuid.clone(),
        path: "/mnt/test-apps/archforge-apps-extra".into(),
    };
    *view.inventory.borrow_mut() = Some(Inventory {
        drives: vec![drive],
        installations: vec![install],
        root_free: Some(10 * 1024 * 1024 * 1024),
        usr_mount: "/ /dev/root ext4".into(),
        flatpak: true,
    });
    view.drives
        .set_model(Some(&gtk::StringList::new(&["Fixture drive"])));
    view.installed
        .set_model(Some(&gtk::StringList::new(&["extra"])));
    view.drives.set_selected(0);
    view.installed.set_selected(0);
    view.render();
    window.present();
    pump();
    assert!(view.action.is_sensitive());
    assert!(contents(&view).contains("--add-app-drive"));
    let monitor = gtk::prelude::WidgetExt::display(&window)
        .monitor_at_surface(&window.surface().unwrap())
        .unwrap();
    assert!(
        window.height() <= monitor.geometry().height() - 48,
        "The expanded output and form must fit the monitor"
    );
    view.name.set_text("extra;bad");
    assert!(!view.action.is_sensitive());
    view.name.set_text("extra");
    assert!(view.action.is_sensitive());
    view.mode.set_selected(1);
    assert!(!view.action.is_sensitive());
    view.app.set_text("org.mozilla.firefox");
    assert!(view.action.is_sensitive());
    assert!(contents(&view).contains("--installation='archforge-extra'"));
    view.inventory.borrow_mut().as_mut().unwrap().drives[0].uuid = "wrong-drive".into();
    view.render();
    assert!(!view.action.is_sensitive());
    assert!(contents(&view).contains("different drive"));
    view.mode.set_selected(2);
    assert_eq!(view.action.label().as_deref(), Some("View drives"));
    assert!(view.command.borrow().is_none());
    assert!(contents(&view).contains("does not move system directories"));
    crate::dialogs::snapshot(&window, "pacman-system-storage-fixture");
    view.inventory.borrow_mut().as_mut().unwrap().flatpak = false;
    view.mode.set_selected(0);
    assert_eq!(view.action.label().as_deref(), Some("Install Flatpak…"));
    assert_eq!(
        view.command.borrow().as_deref(),
        Some("sudo pacman -Syu --needed flatpak")
    );
    window.close();
    let (window, view) = create(parent);
    view.reload();
    window.present();
    let start = std::time::Instant::now();
    while view.loading.get() {
        pump();
        assert!(start.elapsed() < Duration::from_secs(20));
    }
    assert!(view.inventory.borrow().is_some());
    crate::dialogs::snapshot(&window, "app-install-locations");
    window.close();
}
