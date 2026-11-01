use crate::loadouts::App;
use gtk::{gdk, gdk_pixbuf, prelude::*};
use std::{cell::RefCell, collections::BTreeMap, path::PathBuf, sync::OnceLock};

const MAP: &str = include_str!("../data/app-icons/map.tsv");
thread_local! {
    static TEXTURES: RefCell<BTreeMap<String, Option<gdk::Texture>>> = const { RefCell::new(BTreeMap::new()) };
}
fn asset_root() -> Option<&'static PathBuf> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        // Development executables and test binaries both live below target/.
        // Resolve relative to the executable so no build-machine paths are embedded.
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
fn asset(name: &str) -> Option<&'static str> {
    static FILES: OnceLock<BTreeMap<&'static str, &'static str>> = OnceLock::new();
    FILES
        .get_or_init(|| {
            MAP.lines()
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| l.split_once('|'))
                .collect()
        })
        .get(name)
        .copied()
}
fn texture(name: &str) -> Option<gdk::Texture> {
    TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache
            .entry(name.to_owned())
            .or_insert_with(|| {
                let path = asset_root()?.join(asset(name)?);
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
fn mark(name: &str, title: &str) -> String {
    // ASCII marks avoid missing-font boxes, including for imported package names.
    match name {
        "openai-codex" => return "CX".into(),
        "ripgrep" => return "RG".into(),
        "fd" => return "FD".into(),
        "fzf" => return "FZF".into(),
        "jq" => return "JQ".into(),
        "eza" => return "EZA".into(),
        "curl" => return "CURL".into(),
        "wget" => return "GET".into(),
        "openssh" => return "SSH".into(),
        "base-devel" => return "DEV".into(),
        _ => {}
    }
    let short = name.strip_prefix("python-").unwrap_or(name);
    if short.len() <= 4 && short.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return short.to_ascii_uppercase();
    }
    let words: Vec<_> = title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let initials: String = words
        .iter()
        .take(3)
        .filter_map(|w| w.chars().next())
        .collect();
    if words.len() > 1 {
        return initials.to_ascii_uppercase();
    }
    let text: String = short
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(3)
        .collect();
    if text.is_empty() {
        "PKG".into()
    } else {
        text.to_ascii_uppercase()
    }
}
fn identity_color(name: &str) -> (f64, f64, f64) {
    let hash = name.bytes().fold(0x811c9dc5u32, |n, b| {
        (n ^ b as u32).wrapping_mul(0x01000193)
    });
    const COLORS: [(f64, f64, f64); 10] = [
        (0.40, 0.79, 0.96),
        (0.53, 0.83, 0.69),
        (0.95, 0.70, 0.40),
        (0.76, 0.67, 0.95),
        (0.96, 0.57, 0.62),
        (0.48, 0.84, 0.81),
        (0.72, 0.81, 0.49),
        (0.55, 0.70, 0.97),
        (0.91, 0.65, 0.85),
        (0.88, 0.78, 0.58),
    ];
    COLORS[hash as usize % COLORS.len()]
}
fn fallback(name: &str, title: &str) -> gtk::DrawingArea {
    let drawing = gtk::DrawingArea::new();
    drawing.set_content_width(28);
    drawing.set_content_height(28);
    drawing.set_halign(gtk::Align::Center);
    drawing.set_valign(gtk::Align::Center);
    drawing.set_accessible_role(gtk::AccessibleRole::Presentation);
    drawing.set_tooltip_text(Some(&format!("{title} · package mark")));
    let text = mark(name, title);
    let (r, g, b) = identity_color(name);
    drawing.set_draw_func(move |_, c, width, height| {
        let scale = f64::from(width.min(height)) / 32.;
        let _ = c.save();
        c.translate(
            (f64::from(width) - 32. * scale) / 2.,
            (f64::from(height) - 32. * scale) / 2.,
        );
        c.scale(scale, scale);
        let radius = 7.;
        for (x, y, start) in [
            (25., 7., -std::f64::consts::FRAC_PI_2),
            (25., 25., 0.),
            (7., 25., std::f64::consts::FRAC_PI_2),
            (7., 7., std::f64::consts::PI),
        ] {
            c.arc(x, y, radius, start, start + std::f64::consts::FRAC_PI_2);
        }
        c.close_path();
        c.set_source_rgba(r, g, b, 0.13);
        let _ = c.fill_preserve();
        c.set_line_width(1.);
        c.set_source_rgba(r, g, b, 0.5);
        let _ = c.stroke();
        c.select_font_face(
            "Sans",
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Bold,
        );
        c.set_font_size(match text.len() {
            4.. => 8.5,
            3 => 10.,
            _ => 12.,
        });
        if let Ok(extents) = c.text_extents(&text) {
            c.move_to(
                16. - extents.width() / 2. - extents.x_bearing(),
                16. - extents.height() / 2. - extents.y_bearing(),
            );
            c.set_source_rgb(r, g, b);
            let _ = c.show_text(&text);
        }
        let _ = c.restore();
    });
    drawing
}
pub(crate) fn icon(app: &App) -> gtk::Widget {
    if let Some(texture) = texture(&app.name) {
        let image = gtk::Image::from_paintable(Some(&texture));
        image.set_pixel_size(28);
        image.set_accessible_role(gtk::AccessibleRole::Presentation);
        image.set_tooltip_text(Some(&app.title));
        image.upcast()
    } else {
        fallback(&app.name, &app.title).upcast()
    }
}
