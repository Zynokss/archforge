use super::*;
use std::time::Instant;
fn walk(w: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut result = vec![w.clone()];
    let mut child = w.first_child();
    while let Some(c) = child {
        result.extend(walk(&c));
        child = c.next_sibling();
    }
    result
}
fn wait(mut test: impl FnMut() -> bool) {
    let start = Instant::now();
    loop {
        while glib::MainContext::default().pending() {
            glib::MainContext::default().iteration(false);
        }
        if test() {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "Tools UI timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn text_view(window: &gtk::Window) -> gtk::TextView {
    walk(window.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast().ok())
        .unwrap()
}
fn text(window: &gtk::Window) -> String {
    let b = text_view(window).buffer();
    b.text(&b.start_iter(), &b.end_iter(), false).to_string()
}
fn find_window(title: &str) -> gtk::Window {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.title().as_deref() == Some(title))
        .unwrap()
}
fn click(root: &gtk::Widget, name: &str) {
    let button = walk(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| {
            b.label().as_deref() == Some(name)
                || b.tooltip_text().as_deref() == Some(name)
                || walk(b.upcast_ref())
                    .iter()
                    .filter_map(|w| w.downcast_ref::<gtk::Label>())
                    .any(|l| l.text() == name)
        })
        .unwrap();
    button.emit_clicked();
}
#[test]
fn catalog_has_unique_named_commands_and_all_categories() {
    let mut names = std::collections::HashSet::new();
    let mut icons = std::collections::HashSet::new();
    for tool in TOOLS {
        assert!(names.insert(tool.name));
        assert!(
            icons.insert(format!(
                "{:?}",
                crate::tool_icons::for_tool(tool.name).expect("Every tool needs an icon")
            )),
            "duplicate icon for {}",
            tool.name
        );
        match tool.action {
            Action::Read(p, args, _) => {
                assert!(p.starts_with("/usr/bin/"));
                assert!(args.iter().all(|a| !a.contains(['\0', '\r', '\n'])));
            }
            Action::Terminal(command) => assert!(!command.contains("--noconfirm")),
            _ => {}
        }
    }
    assert!(TOOLS.len() >= 20);
}
pub(super) fn ui(parent: &gtk::ApplicationWindow) {
    let page = page(parent);
    let root = page.upcast_ref::<gtk::Widget>();
    let window = gtk::Window::builder()
        .title("Tools catalog preview")
        .default_width(1040)
        .default_height(760)
        .build();
    let scroll = gtk::ScrolledWindow::builder().child(&page).build();
    window.set_child(Some(&scroll));
    window.present();
    let entry = walk(root)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::SearchEntry>().ok())
        .unwrap();
    let dropdown = walk(root)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::DropDown>().ok())
        .unwrap();
    let visible = || {
        walk(root)
            .into_iter()
            .filter(|w| w.has_css_class("catalog-tool") && w.get_visible())
            .count()
    };
    assert_eq!(visible(), 8);
    assert!(
        walk(root)
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::Label>())
            .any(|l| l.text() == "21 tools")
    );
    click(root, "View all package tools");
    assert_eq!(visible(), 5);
    click(root, "All tools");
    assert_eq!(visible(), 8);
    entry.set_text("file owner");
    wait(|| visible() == 1);
    entry.set_text("no-such-tool-xyz");
    wait(|| visible() == 0);
    click(root, "Clear search and filters");
    wait(|| entry.text().is_empty() && visible() == 8);
    entry.set_text("");
    click(root, "Storage");
    wait(|| visible() == TOOLS.iter().filter(|t| t.category == "Storage").count());
    dropdown.set_selected(1);
    let names: Vec<_> = walk(root)
        .iter()
        .filter(|w| w.has_css_class("row-title"))
        .filter_map(|w| w.downcast_ref::<gtk::Label>())
        .map(|l| l.text().to_string())
        .collect();
    assert!(names.windows(2).all(|w| w[0] <= w[1]));
    click(root, "Compact list");
    assert_eq!(
        visible(),
        TOOLS.iter().filter(|t| t.category == "Storage").count()
    );
    let selected = selection(root);
    let rebuilt = super::page(parent);
    restore(rebuilt.upcast_ref(), &selected);
    wait(|| selection(rebuilt.upcast_ref()).list);
    assert_eq!(selection(rebuilt.upcast_ref()).category, 2);
    assert_eq!(selection(rebuilt.upcast_ref()).sort, 1);
    click(root, "Grouped view");
    crate::dialogs::snapshot(&window, "tools-catalog-storage");
    dropdown.set_selected(0);
    entry.set_text("cache location");
    wait(|| visible() == 1);
    click(root, "Configure");
    let cache = find_window("Package cache location");
    wait(|| {
        !walk(cache.upcast_ref())
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::Label>())
            .any(|l| l.text() == "Reading…")
    });
    let path = walk(cache.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::Entry>().ok())
        .unwrap();
    path.set_text("/tmp/x;touch-malicious");
    click(cache.upcast_ref(), "Open embedded terminal");
    assert!(
        walk(cache.upcast_ref())
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::Label>())
            .any(|l| l.text() == "Check the directory path")
    );
    path.set_text("/var/cache/archforge-test");
    assert!(
        !walk(cache.upcast_ref())
            .iter()
            .filter_map(|w| w.downcast_ref::<gtk::Label>())
            .any(|l| l.text() == "Check the directory path")
    );
    click(cache.upcast_ref(), "Copy command");
    assert!(text(&cache).contains("--set-package-cache"));
    crate::dialogs::snapshot(&cache, "package-cache-location");
    cache.close();
    activate(
        parent,
        *TOOLS
            .iter()
            .find(|t| t.name == "Explicit package list")
            .unwrap(),
    );
    let output = find_window("Explicit package list");
    wait(|| !text(&output).starts_with("Waiting for"));
    assert!(!text(&output).is_empty());
    click(output.upcast_ref(), "Copy output");
    output.close();
    activate(
        parent,
        *TOOLS
            .iter()
            .find(|t| t.name == "Find a file's package")
            .unwrap(),
    );
    let lookup = find_window("Find a file's package");
    let path = walk(lookup.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::Entry>().ok())
        .unwrap();
    path.set_text("/usr/bin/pacman");
    click(lookup.upcast_ref(), "Find owner");
    let result = find_window("Find a file's package");
    wait(|| text(&result).contains("owned by pacman"));
    result.close();
    window.close();
}

#[test]
fn search_and_sort_include_tools_outside_category_previews() {
    assert_eq!(matching("SSD trim", 0, 0).len(), 1);
    assert_eq!(
        matching("FILE OWNER", 0, 0)[0].name,
        "Find a file's package"
    );
    assert!(matching("cache", 4, 0).is_empty());
    let az = matching("", 0, 1);
    let za = matching("", 0, 2);
    assert_eq!(
        az.iter().map(|t| t.name).rev().collect::<Vec<_>>(),
        za.iter().map(|t| t.name).collect::<Vec<_>>()
    );
    assert!(
        az.windows(2)
            .all(|w| w[0].name.to_lowercase() <= w[1].name.to_lowercase())
    );
    assert_eq!(matching("", 999, 0).len(), 0);
}
