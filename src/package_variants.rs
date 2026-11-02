//! Reviewed package alternatives shared by all interfaces.
pub(crate) const GROUPS: &[&[&str]] = &[
    &["vesktop-bin", "vesktop", "vesktop-git"],
    &["lutris", "lutris-git"],
    &["adw-gtk-theme", "adw-gtk-theme-git"],
];

pub(crate) fn installed_name<'a>(
    installed: &'a std::collections::BTreeMap<String, String>,
    name: &'a str,
) -> &'a str {
    if installed.contains_key(name) {
        return name;
    }
    GROUPS
        .iter()
        .find(|group| group.contains(&name))
        .and_then(|group| {
            group
                .iter()
                .find(|candidate| installed.contains_key(**candidate))
        })
        .copied()
        .unwrap_or(name)
}
