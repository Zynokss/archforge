use std::{fs, path::PathBuf};

pub const OPTIONS: [(&str, &str); 5] = [
    ("crimson", "Crimson"),
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
        .join("arch-forge/appearance")
}
pub fn selected() -> String {
    let name = std::env::var("ARCH_FORGE_THEME")
        .ok()
        .or_else(|| fs::read_to_string(preference_path()).ok())
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
                "#101214", "#181b1e", "#21252a", "#30353c", "#f0f1f3", "#aeb7c3", "#ff6467",
                "#26090c", "#452327", "#ffdadc",
            ],
            "lavender" => [
                "#17141b", "#201c25", "#2b2531", "#39303f", "#eee6f2", "#c4b8c9", "#d5b7ee",
                "#392448", "#493653", "#f0d9ff",
            ],
            "sand" => [
                "#191714", "#23201b", "#2e2a23", "#3b352b", "#efe8dc", "#c6beb0", "#dfc48c",
                "#3c2e0d", "#4c4026", "#f8e3b5",
            ],
            _ => [
                "#141714", "#1c211c", "#252c25", "#343d32", "#e5e9df", "#b7c1b2", "#c1d0b5",
                "#293622", "#3d4b35", "#dfecd2",
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
                "Material colors for your workbench.".into(),
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
    provider.load_from_string(&palette.css());
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    description
}
