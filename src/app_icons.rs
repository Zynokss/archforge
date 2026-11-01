use crate::loadouts::App;
use gtk::{gdk, gdk_pixbuf, prelude::*};
use std::{cell::RefCell, collections::BTreeMap, path::PathBuf, sync::OnceLock};

const MAP: &str = include_str!("../data/app-icons/map.tsv");
const PACKAGE_SYMBOLS: &str = include_str!("../data/app-icons/package-symbols.tsv");
const CATEGORY_SYMBOLS: &str = include_str!("../data/app-icons/category-symbols.tsv");
thread_local! {
    static TEXTURES: RefCell<BTreeMap<String, Option<gdk::Texture>>> = const { RefCell::new(BTreeMap::new()) };
}

fn asset_root() -> Option<&'static PathBuf> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        if let Ok(exe) = std::env::current_exe() {
            for parent in exe.ancestors().take(6) {
                if parent.file_name().is_some_and(|name| name == "target")
                    && let Some(project) = parent.parent()
                {
                    let candidate = project.join("data/app-icons");
                    if candidate.join("map.tsv").is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        let installed = PathBuf::from("/usr/share/archforge/app-icons");
        installed.join("map.tsv").is_file().then_some(installed)
    })
    .as_ref()
}

fn lookup(table: &'static str, key: &str) -> Option<&'static str> {
    static TABLES: OnceLock<BTreeMap<&'static str, BTreeMap<&'static str, &'static str>>> =
        OnceLock::new();
    TABLES
        .get_or_init(|| {
            [MAP, PACKAGE_SYMBOLS, CATEGORY_SYMBOLS]
                .into_iter()
                .map(|source| {
                    let map = source
                        .lines()
                        .filter(|line| !line.starts_with('#'))
                        .filter_map(|line| line.split_once('|'))
                        .collect();
                    (source, map)
                })
                .collect()
        })
        .get(table)?
        .get(key)
        .copied()
}

fn asset(name: &str) -> Option<&'static str> {
    lookup(MAP, name)
}

fn icon_path(app: &App) -> &'static str {
    asset(&app.name)
        .or_else(|| lookup(PACKAGE_SYMBOLS, &app.name))
        .or_else(|| lookup(CATEGORY_SYMBOLS, &app.category))
        .unwrap_or("symbols/package.svg")
}

fn texture(path: &str) -> Option<gdk::Texture> {
    TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache
            .entry(path.to_owned())
            .or_insert_with(|| {
                let path = asset_root()?.join(path);
                let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_scale(path, 56, 56, true).ok()?;
                let pixbuf = pixbuf.add_alpha(false, 0, 0, 0).ok()?;
                Some(
                    gdk::MemoryTexture::new(
                        pixbuf.width(),
                        pixbuf.height(),
                        gdk::MemoryFormat::R8g8b8a8,
                        &pixbuf.read_pixel_bytes(),
                        pixbuf.rowstride() as usize,
                    )
                    .upcast(),
                )
            })
            .clone()
    })
}

pub(crate) fn icon(app: &App) -> gtk::Widget {
    let path = icon_path(app);
    if let Some(texture) = texture(path) {
        let image = gtk::Image::from_paintable(Some(&texture));
        image.set_pixel_size(28);
        image.set_accessible_role(gtk::AccessibleRole::Presentation);
        image.set_tooltip_text(Some(&format!(
            "{} · {}",
            app.title,
            path.rsplit('/').next().unwrap_or("icon")
        )));
        image.upcast()
    } else {
        // Bundled symbols cover every catalog category; this last-resort theme
        // icon is only used if the local installation is damaged.
        let image = gtk::Image::from_icon_name("package-x-generic-symbolic");
        image.set_pixel_size(28);
        image.set_accessible_role(gtk::AccessibleRole::Presentation);
        image.set_tooltip_text(Some(&app.title));
        image.upcast()
    }
}
