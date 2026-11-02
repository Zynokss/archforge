use std::{fs, path::PathBuf};

pub const OPTIONS: [(&str, &str); 7] = [
    ("crimson", "Crimson"),
    ("amber", "Amber Night"),
    ("arch-night", "Arch Night"),
    ("sage", "Sage"),
    ("lavender", "Lavender"),
    ("sand", "Sand"),
    ("desktop", "Desktop colors"),
];

#[derive(Clone)]
pub struct Palette {
    pub background: String,
    pub surface: String,
    pub high: String,
    pub highest: String,
    pub foreground: String,
    pub muted: String,
    pub primary: String,
    pub on_primary: String,
    pub container: String,
    pub on_container: String,
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}
fn state_home() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
}
pub fn desktop_signature() -> String {
    fs::read_to_string(state_home().join("quickshell/user/generated/colors.json"))
        .unwrap_or_default()
}
fn preference_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("archforge/appearance")
}
pub fn selected() -> String {
    let name = std::env::var("ARCH_FORGE_THEME")
        .ok()
        .or_else(|| fs::read_to_string(preference_path()).ok())
        .or_else(|| {
            let legacy = preference_path()
                .parent()?
                .parent()?
                .join("arch-forge/appearance");
            fs::read_to_string(legacy).ok()
        })
        .unwrap_or_else(|| "crimson".into());
    if OPTIONS.iter().any(|(id, _)| *id == name.trim()) {
        name.trim().into()
    } else {
        "crimson".into()
    }
}
pub fn save(name: &str) -> Result<(), String> {
    if !OPTIONS.iter().any(|(id, _)| *id == name) {
        return Err("Unknown color palette".into());
    }
    let path = preference_path();
    fs::create_dir_all(path.parent().unwrap())
        .and_then(|_| fs::write(path, name))
        .map_err(|e| e.to_string())
}
fn hex(value: &str) -> Option<String> {
    (value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|b| b.is_ascii_hexdigit()))
    .then(|| value.to_owned())
}
fn rgb(value: &str) -> [f64; 3] {
    [1, 3, 5].map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap_or(0) as f64 / 255.0)
}
fn contrast(a: &str, b: &str) -> f64 {
    fn lum(v: &str) -> f64 {
        let c = rgb(v).map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
    }
    let (a, b) = (lum(a), lum(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn legible(color: &str, background: &str, fallback: &str) -> String {
    if contrast(color, background) >= 4.5 {
        return color.into();
    }
    let base = rgb(color);
    let dest = rgb(fallback);
    for step in 1..=20 {
        let amount = step as f64 / 20.0;
        let c = [0, 1, 2].map(|i| ((base[i] + (dest[i] - base[i]) * amount) * 255.0).round() as u8);
        let result = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        if contrast(&result, background) >= 4.5 {
            return result;
        }
    }
    fallback.into()
}

impl Palette {
    pub fn preset(name: &str) -> Self {
        let values = match name {
            "crimson" => [
                "#0b0507", "#14090d", "#231017", "#3a1823", "#f5eef1", "#c7b8bf", "#ff2c50",
                "#26020b", "#420d1b", "#ffe6ed",
            ],
            "arch-night" => [
                "#080f14", "#101a21", "#182731", "#293c48", "#edf4fa", "#b1c3d0", "#35d3fa",
                "#03202b", "#103b50", "#d7f6ff",
            ],
            "amber" => [
                "#0e0a06", "#181109", "#281c0e", "#3c2c18", "#f7f0e7", "#c9bba9", "#ffb454",
                "#251905", "#493018", "#ffe2b8",
            ],
            "lavender" => [
                "#0c0810", "#160f1d", "#23172e", "#392549", "#f2eaf8", "#c4b5d1", "#c7a0f5",
                "#29143b", "#39204f", "#f0d9ff",
            ],
            "sand" => [
                "#0d0b07", "#17130c", "#262014", "#3a3020", "#f5eee1", "#c6bda9", "#dfc48c",
                "#30240c", "#41331b", "#f8e3b5",
            ],
            _ => [
                "#070d09", "#0f1912", "#19291d", "#2b3d2c", "#edf3e9", "#b8c6b5", "#a8d59c",
                "#172a13", "#263d25", "#e1f3d8",
            ],
        };
        let [
            background,
            surface,
            high,
            highest,
            foreground,
            muted,
            primary,
            on_primary,
            container,
            on_container,
        ] = values.map(String::from);
        Self {
            background,
            surface,
            high,
            highest,
            foreground,
            muted,
            primary,
            on_primary,
            container,
            on_container,
        }
    }
    pub fn load(name: &str) -> (Self, String) {
        if name != "desktop" {
            return (
                Self::preset(name),
                if name == "arch-night" {
                    "Midnight blue surfaces, cyan edge lighting and luminous controls.".into()
                } else if name == "amber" {
                    "Amber accents on cool charcoal surfaces.".into()
                } else {
                    "Deep tinted surfaces, illuminated borders and accent-colored hover glow."
                        .into()
                },
            );
        }
        match fs::read_to_string(state_home().join("quickshell/user/generated/colors.json")) {
            Ok(contents) => match Self::from_desktop(&contents) {
                Some(p) => (p, "Following your end4 / Quickshell color palette.".into()),
                None => (
                    Self::preset("sage"),
                    "Desktop palette could not be read. Using Sage.".into(),
                ),
            },
            Err(_) => (
                Self::preset("sage"),
                "No end4 desktop palette found. Using Sage.".into(),
            ),
        }
    }
    fn from_desktop(contents: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(contents).ok()?;
        let get = |key: &str| hex(value.get(key)?.as_str()?);
        let background = get("background")?;
        let surface = get("surface_container")?;
        let high = get("surface_container_high")?;
        let foreground = get("on_surface")?;
        let foreground = legible(
            &foreground,
            &surface,
            if contrast("#ffffff", &surface) > contrast("#101210", &surface) {
                "#ffffff"
            } else {
                "#101210"
            },
        );
        let primary = legible(&get("primary")?, &surface, &foreground);
        let container = get("primary_container")?;
        let on_primary = legible(
            &get("on_primary")?,
            &primary,
            if contrast("#ffffff", &primary) > contrast("#101210", &primary) {
                "#ffffff"
            } else {
                "#101210"
            },
        );
        Some(Self {
            background,
            surface,
            high: high.clone(),
            highest: get("surface_container_highest").unwrap_or(high),
            muted: legible(
                &get("on_surface_variant")?,
                &get("surface_container_high")?,
                &foreground,
            ),
            primary,
            on_primary,
            container: container.clone(),
            on_container: legible(&get("on_primary_container")?, &container, &foreground),
            foreground,
        })
    }
    fn css(&self) -> String {
        format!(
            "@define-color forge_bg {};\n@define-color forge_surface {};\n@define-color forge_high {};\n@define-color forge_highest {};\n@define-color forge_fg {};\n@define-color forge_muted {};\n@define-color forge_primary {};\n@define-color forge_on_primary {};\n@define-color forge_container {};\n@define-color forge_on_container {};\n{}",
            self.background,
            self.surface,
            self.high,
            self.highest,
            self.foreground,
            self.muted,
            self.primary,
            self.on_primary,
            self.container,
            self.on_container,
            include_str!("style.css")
        )
    }
}

pub fn apply(provider: &gtk::CssProvider, name: &str) -> String {
    let (palette, description) = Palette::load(name);
    let mut css = palette.css();
    css.push_str(include_str!("luminous.css"));
    provider.load_from_string(&css);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    fn recolor_brand(widget: &gtk::Widget, primary: &str) {
        use gtk::prelude::*;
        if (widget.has_css_class("brand-icon") || widget.has_css_class("themed-arch-mark"))
            && let Some(image) = widget.downcast_ref::<gtk::Image>()
        {
            image.set_paintable(
                crate::arch_logo(primary, image.pixel_size().max(1))
                    .paintable()
                    .as_ref(),
            );
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            recolor_brand(&w, primary);
            child = w.next_sibling();
        }
    }
    for window in gtk::Window::list_toplevels() {
        recolor_brand(&window, &palette.primary);
    }
    description
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_have_readable_text() {
        for (name, _) in OPTIONS {
            let p = Palette::preset(name);
            assert!(contrast(&p.foreground, &p.surface) >= 7.0);
            assert!(contrast(&p.on_primary, &p.primary) >= 4.5);
            assert!(contrast(&p.muted, &p.high) >= 4.5);
        }
    }
    #[test]
    fn desktop_values_cannot_inject_css() {
        assert!(hex("#123abc").is_some());
        assert!(hex("red;}").is_none());
        assert!(Palette::from_desktop("{\"primary\": \"url(bad)\"}").is_none());
    }
    #[test]
    fn desktop_low_contrast_is_adjusted() {
        assert!(contrast(&legible("#555555", "#555555", "#ffffff"), "#555555") >= 4.5);
    }
}

/// Decorative, palette-driven header. Controls stay on top and retain their size.
pub(crate) fn banner(header: &impl gtk::prelude::IsA<gtk::Widget>) -> gtk::Overlay {
    use gtk::prelude::*;
    let banner = gtk::Overlay::new();
    banner.add_css_class("forge-banner");
    let art = gtk::DrawingArea::new();
    art.add_css_class("forge-banner-art");
    art.set_hexpand(true);
    art.set_vexpand(true);
    art.set_can_target(false);
    art.set_accessible_role(gtk::AccessibleRole::Presentation);
    let logo = std::cell::RefCell::new(None::<(String, gtk::gdk_pixbuf::Pixbuf)>);
    art.set_draw_func(move |area, cr, width, height| {
        let color = area.color();
        let (r, g, b) = (
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
        );
        let (w, h) = (width as f64, height as f64);
        let x = w * 0.62;
        let _ = cr.save();
        cr.translate(x, h * 1.05);
        cr.scale((w * 0.42).max(1.), (h * 1.25).max(1.));
        let glow = gtk::cairo::RadialGradient::new(0., 0., 0., 0., 0., 1.);
        glow.add_color_stop_rgba(0., r, g, b, 0.35);
        glow.add_color_stop_rgba(0.4, r, g, b, 0.10);
        glow.add_color_stop_rgba(1., r, g, b, 0.);
        let _ = cr.set_source(&glow);
        let _ = cr.paint();
        let _ = cr.restore();
        // Small windows keep the ambient light without placing artwork over text.
        if w < 950. {
            return;
        }
        // Reuse the same transparent vector as the sidebar and desktop icon.
        // Cache each tint until the active palette changes.
        let tint = format!(
            "#{:02x}{:02x}{:02x}",
            (r * 255.).round() as u8,
            (g * 255.).round() as u8,
            (b * 255.).round() as u8
        );
        let mut cached = logo.borrow_mut();
        if cached
            .as_ref()
            .is_none_or(|(previous, _)| previous != &tint)
        {
            let loader = gtk::gdk_pixbuf::PixbufLoader::with_type("svg").expect("SVG loader");
            loader.set_size(112, 112);
            let svg = include_str!("../data/arch-mark.svg").replace("#ff6467", &tint);
            loader.write(svg.as_bytes()).expect("Bundled Arch mark");
            loader.close().expect("Complete SVG");
            *cached = Some((tint, loader.pixbuf().expect("Decoded Arch mark")));
        }
        if let Some((_, pixbuf)) = cached.as_ref() {
            use gtk::gdk::prelude::GdkCairoContextExt;
            // Fit inside the actual allocation, including shorter page headers.
            // Keep a clear inset so neither the tip nor the base touches a clip edge.
            let size = 112_f64.min((h - 16.).max(0.));
            if size > 0. {
                let _ = cr.save();
                cr.translate(x - size / 2., (h - size) / 2.);
                cr.scale(size / pixbuf.width() as f64, size / pixbuf.height() as f64);
                cr.set_source_pixbuf(pixbuf, 0., 0.);
                let _ = cr.paint();
                let _ = cr.restore();
            }
        }
    });
    banner.set_child(Some(&art));
    banner.add_overlay(header);
    banner.set_measure_overlay(header, true);
    header.set_valign(gtk::Align::Center);
    banner
}
