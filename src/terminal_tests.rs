use super::*;
use gtk::{glib, prelude::*};
use std::sync::Mutex;

pub(super) static OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

#[test]
fn terminal_detection_and_argument_boundaries() {
    let folder = tempfile::tempdir().unwrap();
    for name in ["kitty", "xdg-terminal-exec"] {
        let path = folder.path().join(name);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = folder.path().as_os_str();
    assert_eq!(
        select(None, path).unwrap().file_name().unwrap(),
        "xdg-terminal-exec"
    );
    assert_eq!(
        select(Some(OsStr::new("kitty")), path)
            .unwrap()
            .file_name()
            .unwrap(),
        "kitty"
    );
    assert!(select(Some(OsStr::new("kitty -e evil")), path).is_err());
    assert!(select(None, OsStr::new("/nonexistent")).is_err());
    let proposed = "printf '%s' \"a ; $(not-executed)\"";
    for name in TERMINALS {
        let terminal = Path::new("/usr/bin").join(name);
        let cmd = command(&terminal, proposed);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args.last().copied(), Some(OsStr::new(proposed)));
        let bash = args
            .iter()
            .position(|arg| *arg == OsStr::new("/usr/bin/bash"))
            .unwrap();
        assert_eq!(args[bash + 4], PROMPT);
        assert_eq!(args[bash + 5], "archforge");
    }
    assert!(open("gamemoderun %command%").is_err());
    assert!(open("echo first\necho second").is_err());
}

fn walk(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = vec![widget.clone()];
    let mut child = widget.first_child();
    while let Some(w) = child {
        result.extend(walk(&w));
        child = w.next_sibling();
    }
    result
}
fn named(root: &gtk::Widget, name: &str) -> Option<gtk::Button> {
    walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some(name))
}
fn dialog(name: &str) -> gtk::Window {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some(name))
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
        assert!(Instant::now() < deadline, "Terminal UI did not complete");
        thread::sleep(Duration::from_millis(20));
    }
}
fn has(root: &gtk::Widget, text: &str) -> bool {
    walk(root).iter().any(|w| {
        w.downcast_ref::<gtk::Label>()
            .is_some_and(|l| l.text().contains(text))
    })
}

pub(super) fn ui(parent: &gtk::ApplicationWindow) {
    let folder = tempfile::tempdir().unwrap();
    let mock = folder.path().join("kitty");
    let log = folder.path().join("args.json");
    let script = format!(
        "#!/usr/bin/python\nimport sys,json\nopen({},'w').write(json.dumps(sys.argv[1:]))\n",
        serde_json::to_string(&log.to_string_lossy()).unwrap()
    );
    std::fs::write(&mock, script).unwrap();
    std::fs::set_permissions(&mock, std::fs::Permissions::from_mode(0o755)).unwrap();
    *OVERRIDE.lock().unwrap() = Some(mock.clone());
    crate::show_setup(parent, "Steam", "steam", false);
    let setup = dialog("Install Steam");
    let open = named(setup.upcast_ref(), "Open terminal").unwrap();
    open.emit_clicked();
    assert!(!open.is_sensitive());
    wait(|| !setup.is_visible());
    assert!(!setup.is_visible());
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(&log).unwrap()).unwrap();
    assert_eq!(args.last().unwrap(), "sudo pacman -Syu --needed steam");
    setup.close();
    for (name, package) in [
        ("GameMode", "gamemode"),
        ("MangoHud", "mangohud"),
        ("Gamescope", "gamescope"),
    ] {
        crate::show_setup(parent, name, package, true);
        let setup = dialog(&format!("Configure {name}"));
        assert!(named(setup.upcast_ref(), "Open terminal").is_none());
        assert!(named(setup.upcast_ref(), "Copy launch option").is_some());
        setup.close();
    }
    std::fs::write(&mock, "#!/bin/sh\nexit 7\n").unwrap();
    crate::show_setup(parent, "Vulkan tools", "vulkan-tools", true);
    let setup = dialog("Configure Vulkan tools");
    let open = named(setup.upcast_ref(), "Open terminal").unwrap();
    open.emit_clicked();
    wait(|| open.is_sensitive());
    assert!(has(setup.upcast_ref(), "could not open a terminal"));
    assert!(
        named(setup.upcast_ref(), "Copy command")
            .unwrap()
            .is_sensitive()
    );
    setup.close();
    *OVERRIDE.lock().unwrap() = None;
    println!(
        "Terminal UI passed: install command forwarded once, Steam options stay copy-only, startup errors visible, copy fallback retained. No live terminal launched."
    );
}
