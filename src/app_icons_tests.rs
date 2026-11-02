use super::*;

#[test]
fn resolver_uses_logo_then_package_then_category_then_generic() {
    let catalog = crate::loadout_styles::catalog();
    let app = |name: &str| catalog.iter().find(|app| app.name == name).unwrap();
    assert_eq!(icon_path(app("git")), asset("git").unwrap());
    assert_eq!(icon_path(app("less")), "symbols/document.svg");
    assert_eq!(icon_path(app("unzip")), "symbols/archive.svg");
    assert_eq!(icon_path(app("man-db")), "symbols/book.svg");
    assert_eq!(icon_path(app("sudo")), "symbols/shield.svg");
    assert_eq!(icon_path(app("openssh")), "symbols/ssh.svg");
    assert_eq!(icon_path(app("xdg-utils")), "symbols/desktop.svg");
    assert_eq!(icon_path(app("curl")), asset("curl").unwrap());

    let mut unknown = App {
        name: "locally-added-tool".into(),
        title: "Locally added tool".into(),
        category: "Network & security".into(),
        description: String::new(),
        source: crate::loadouts::Source::Foreign,
        priority: 2,
        popularity: None,
    };
    assert_eq!(icon_path(&unknown), "symbols/network.svg");
    unknown.category = "Unlisted category".into();
    assert_eq!(icon_path(&unknown), "symbols/package.svg");
}

#[test]
fn packaging_variants_keep_logos_and_functional_symbols() {
    let catalog = crate::loadout_styles::catalog();
    for name in ["firefox", "obs-studio", "lutris", "vesktop-bin", "less"] {
        let app = catalog.iter().find(|a| a.name == name).unwrap();
        for suffix in ["-bin", "-git", "-hg"] {
            let mut variant = app.clone();
            variant.name = format!("{}{suffix}", crate::package_variants::base_name(name));
            assert_eq!(icon_path(&variant), icon_path(app), "{}", variant.name);
        }
    }
}

#[test]
fn icon_tables_are_unique_and_point_to_bundled_svg_files() {
    let root = asset_root().expect("Development artwork directory");
    for table in [MAP, PACKAGE_SYMBOLS, CATEGORY_SYMBOLS] {
        let mut seen = std::collections::BTreeSet::new();
        for line in table.lines().filter(|line| !line.starts_with('#')) {
            let (name, file) = line.split_once('|').unwrap();
            assert!(seen.insert(name), "duplicate resolver entry: {name}");
            assert!(file.starts_with("svg/") || file.starts_with("symbols/"));
            assert!(file.ends_with(".svg") && !file.contains(".."));
            let path = root.join(file);
            assert!(path.is_file() && !path.is_symlink(), "missing icon: {file}");
        }
    }
    let names: std::collections::BTreeSet<_> = crate::loadout_styles::catalog()
        .into_iter()
        .map(|app| app.name)
        .collect();
    for line in MAP.lines().filter(|line| !line.starts_with('#')) {
        let name = line.split_once('|').unwrap().0;
        assert!(
            crate::package_variants::resolve(names.iter().map(String::as_str), name).is_some(),
            "stale application logo mapping: {name}"
        );
    }
    assert!(asset("definitely-not-in-the-catalog").is_none());
}

pub(super) fn ui(parent: &gtk::ApplicationWindow) {
    let root = asset_root().unwrap();
    let paths: std::collections::BTreeSet<_> = [MAP, PACKAGE_SYMBOLS, CATEGORY_SYMBOLS]
        .into_iter()
        .flat_map(|table| table.lines())
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('|').map(|(_, file)| file))
        .chain(std::iter::once("symbols/package.svg"))
        .collect();
    for file in &paths {
        let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_scale(root.join(file), 56, 56, true)
            .unwrap_or_else(|error| panic!("{file}: {error}"));
        let bytes = pixbuf.read_pixel_bytes();
        let bytes = bytes.as_ref();
        let stride = pixbuf.rowstride() as usize;
        let channels = pixbuf.n_channels() as usize;
        let mut visible = 0;
        for y in 0..pixbuf.height() as usize {
            for x in 0..pixbuf.width() as usize {
                let i = y * stride + x * channels;
                if channels != 4 || bytes[i + 3] > 30 {
                    visible += 1;
                }
            }
        }
        assert!(visible > 10, "{file} must not render blank");
    }
    for app in crate::loadout_styles::catalog() {
        assert!(
            texture(icon_path(&app)).is_some(),
            "Every catalog app should resolve to a readable local SVG: {}",
            app.name
        );
    }

    let names = [
        "git",
        "curl",
        "less",
        "unzip",
        "man-db",
        "sudo",
        "openssh",
        "xdg-utils",
        "firefox",
        "steam",
        "python",
        "ollama",
        "gimp",
        "virt-manager",
        "orca",
    ];
    let gallery = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(7)
        .max_children_per_line(7)
        .column_spacing(12)
        .row_spacing(12)
        .build();
    let apps = crate::loadout_styles::catalog();
    for name in names {
        let app = apps.iter().find(|app| app.name == name).unwrap();
        let cell = crate::column(6);
        cell.set_size_request(115, 72);
        let image = icon(app);
        image.set_halign(gtk::Align::Center);
        cell.append(&image);
        let label = crate::label(&app.title, "tiny");
        label.set_xalign(0.5);
        label.set_max_width_chars(18);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        cell.append(&label);
        gallery.insert(&cell, -1);
    }
    let window = gtk::Window::builder()
        .title("Package icon resolver check")
        .transient_for(parent)
        .default_width(920)
        .default_height(560)
        .build();
    let scroll = gtk::ScrolledWindow::builder().child(&gallery).build();
    window.set_child(Some(&scroll));
    window.present();
    crate::dialogs::snapshot(&window, "app-icon-gallery");
    window.close();
}
