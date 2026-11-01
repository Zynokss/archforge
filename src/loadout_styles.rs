use crate::loadouts::{App, Profile, Source};
pub(crate) fn catalog() -> Vec<App> {
    include_str!("../data/loadouts/catalog.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('|').collect();
            App {
                category: fields[0].into(),
                name: fields[1].into(),
                title: fields[2].into(),
                description: fields[3].into(),
                icon: fields[4].into(),
                source: if fields[5] == "aur" {
                    Source::Aur
                } else {
                    Source::Repo
                },
            }
        })
        .collect()
}
pub(crate) struct Kit {
    pub name: &'static str,
    pub description: &'static str,
    pub packages: &'static [&'static str],
    pub config: Option<(&'static str, &'static str)>,
}
pub(crate) const KITS: &[Kit] = &[
    Kit {
        name: "Hyprland",
        description: "A Wayland starter with Lua configuration, Waybar, launcher, and terminal.",
        packages: &[
            "hyprland",
            "kitty",
            "waybar",
            "fuzzel",
            "thunar",
            "xdg-desktop-portal-hyprland",
            "xdg-desktop-portal-gtk",
            "polkit-gnome",
        ],
        config: Some((
            ".config/hypr/hyprland.lua",
            include_str!("../data/loadouts/hyprland.lua"),
        )),
    },
    Kit {
        name: "KDE Plasma",
        description: "Plasma desktop and applications, with a matching color scheme when starter dotfiles are selected.",
        packages: &[
            "plasma-meta",
            "dolphin",
            "konsole",
            "xdg-desktop-portal-kde",
        ],
        config: None,
    },
    Kit {
        name: "GNOME",
        description: "GNOME Shell, Settings, Files, Console, and its portal. Uses desktop appearance settings.",
        packages: &[
            "gnome-shell",
            "gnome-control-center",
            "nautilus",
            "gnome-console",
            "xdg-desktop-portal-gnome",
        ],
        config: None,
    },
    Kit {
        name: "Xfce",
        description: "Xfce session, panel, window manager, file manager, terminal, and settings.",
        packages: &[
            "xfce4-session",
            "xfce4-panel",
            "xfdesktop",
            "xfwm4",
            "thunar",
            "xfce4-terminal",
            "xfce4-settings",
            "xfce4-appfinder",
            "xfce4-power-manager",
            "tumbler",
            "xdg-desktop-portal-gtk",
        ],
        config: None,
    },
    Kit {
        name: "Cinnamon",
        description: "Cinnamon desktop, Nemo, and settings. Uses desktop appearance settings.",
        packages: &[
            "cinnamon",
            "nemo",
            "cinnamon-control-center",
            "xdg-desktop-portal-gtk",
        ],
        config: None,
    },
    Kit {
        name: "Sway",
        description: "A keyboard-focused Wayland starter with Waybar, launcher, and terminal.",
        packages: &[
            "sway",
            "swaybg",
            "swaylock",
            "kitty",
            "waybar",
            "fuzzel",
            "thunar",
            "xdg-desktop-portal-wlr",
            "xdg-desktop-portal-gtk",
            "polkit-gnome",
        ],
        config: Some((
            ".config/sway/config",
            include_str!("../data/loadouts/sway.conf"),
        )),
    },
];
pub(crate) fn style(kit: Option<usize>, amber: bool) -> Profile {
    let mut p = Profile {
        name: if amber {
            "Amber Night"
        } else {
            "Crimson Night"
        }
        .into(),
        ..Profile::default()
    };
    let accent = if amber { "ffb454" } else { "ff6467" };
    p.files.insert(".config/kitty/kitty.conf".into(),format!("# Arch Forge terminal colors\nfont_family monospace\nfont_size 11\nbackground #141719\nforeground #eff1f3\nselection_background #{accent}\nselection_foreground #141719\ncursor #{accent}\nwindow_padding_width 10\nconfirm_os_window_close 1\n"));
    p.files.insert(".config/gtk-3.0/settings.ini".into(),"[Settings]\ngtk-theme-name=Adwaita-dark\ngtk-icon-theme-name=Adwaita\ngtk-application-prefer-dark-theme=1\n".into());
    p.packages.insert("kitty".into(), Source::Repo);
    p.packages.insert("adwaita-icon-theme".into(), Source::Repo);
    if let Some(kit) = kit.and_then(|i| KITS.get(i)) {
        for name in kit.packages {
            p.packages.insert((*name).into(), Source::Repo);
        }
        if kit.name == "KDE Plasma" {
            let scheme_name = if amber {
                "ArchForgeAmber"
            } else {
                "ArchForgeCrimson"
            };
            let accent_rgb = if amber { "255,180,84" } else { "255,100,103" };
            let mut scheme = format!("[General]\nName={}\nColorScheme={scheme_name}\n\n", p.name);
            for (group, background, foreground) in [
                ("Window", "20,23,25", "239,241,243"),
                ("View", "17,20,22", "239,241,243"),
                ("Button", "37,42,46", "239,241,243"),
                ("Selection", accent_rgb, "20,23,25"),
                ("Tooltip", "28,32,35", "239,241,243"),
                ("Complementary", "20,23,25", "239,241,243"),
                ("Header", "28,32,35", "239,241,243"),
            ] {
                scheme.push_str(&format!("[Colors:{group}]\nBackgroundNormal={background}\nBackgroundAlternate=37,42,46\nForegroundNormal={foreground}\nForegroundInactive=180,188,196\nForegroundActive={accent_rgb}\nForegroundLink=140,187,239\nForegroundVisited=204,165,222\nForegroundNegative=255,133,139\nForegroundNeutral=244,193,111\nForegroundPositive=134,214,148\nDecorationFocus={accent_rgb}\nDecorationHover={accent_rgb}\n\n"));
            }
            p.files.insert(
                format!(".local/share/color-schemes/{scheme_name}.colors"),
                scheme.clone(),
            );
            p.files.insert(".config/kdeglobals".into(), scheme);
        }
        if let Some((path, config)) = kit.config {
            p.files
                .insert(path.into(), config.replace("@ACCENT@", accent));
            let compositor = if kit.name == "Sway" {
                "sway"
            } else {
                "hyprland"
            };
            p.files.insert(".config/waybar/config.jsonc".into(),format!("{{\n  \"layer\": \"top\", \"position\": \"top\", \"height\": 32,\n  \"modules-left\": [\"{compositor}/workspaces\"],\n  \"modules-center\": [\"clock\"],\n  \"modules-right\": [\"cpu\", \"memory\", \"network\", \"tray\"],\n  \"clock\": {{ \"format\": \"{{:%a %d %b  %H:%M}}\" }},\n  \"cpu\": {{ \"format\": \"CPU {{usage}}%\" }},\n  \"memory\": {{ \"format\": \"RAM {{percentage}}%\" }},\n  \"network\": {{ \"format-wifi\": \"{{essid}}\", \"format-ethernet\": \"Ethernet\", \"format-disconnected\": \"Offline\" }}\n}}\n"));
            p.files.insert(".config/waybar/style.css".into(),format!("* {{ font-family: monospace; font-size: 12px; min-height: 0; }}\nwindow#waybar {{ background: #141719; color: #eff1f3; border-bottom: 1px solid #343a40; }}\n#workspaces button {{ color: #b4bcc4; border: none; padding: 0 9px; }}\n#workspaces button.active, #workspaces button.focused {{ color: #{accent}; border-bottom: 2px solid #{accent}; }}\n#clock, #cpu, #memory, #network, #tray {{ padding: 0 12px; }}\n"));
        }
    }
    p
}
