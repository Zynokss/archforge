//! Shared matching for packaging variants. Exact package names always win.
//! Only terminal packaging suffixes are removed; debug packages, plugins,
//! libraries, nightly builds and differently named forks remain distinct.
const SUFFIXES: &[&str] = &["-bin", "-git", "-svn", "-hg", "-bzr", "-cvs", "-darcs"];

pub(crate) fn base_name(name: &str) -> &str {
    let mut base = name;
    while let Some(next) = SUFFIXES.iter().find_map(|suffix| base.strip_suffix(suffix)) {
        if next.is_empty() {
            break;
        }
        base = next;
    }
    base
}

/// Prefer an exact name, then an unsuffixed base, then a single alternative.
/// Multiple alternatives require an exact choice instead of picking arbitrarily.
pub(crate) fn resolve<'a>(
    names: impl IntoIterator<Item = &'a str>,
    requested: &str,
) -> Option<&'a str> {
    let base = base_name(requested);
    let mut plain = None;
    let mut candidate = None;
    let mut ambiguous = false;
    for name in names {
        if name == requested {
            return Some(name);
        }
        if base_name(name) != base {
            continue;
        }
        if name == base {
            plain = Some(name);
        }
        if candidate.is_some_and(|previous| previous != name) {
            ambiguous = true;
        }
        candidate = Some(name);
    }
    plain.or_else(|| if ambiguous { None } else { candidate })
}

pub(crate) fn installed_name<'a>(
    installed: &'a std::collections::BTreeMap<String, String>,
    name: &'a str,
) -> &'a str {
    if let Some((exact, _)) = installed.get_key_value(name) {
        return exact;
    }
    resolve(installed.keys().map(String::as_str), name).unwrap_or(name)
}

/// Precompute family matches for UIs that resolve thousands of rows per frame.
/// Exact installed names still take priority at the call site.
pub(crate) fn alias_index(
    installed: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, String> {
    let mut families = std::collections::BTreeMap::<&str, Vec<&str>>::new();
    for name in installed.keys() {
        families.entry(base_name(name)).or_default().push(name);
    }
    families
        .into_iter()
        .filter_map(|(base, names)| {
            resolve(names, base).map(|actual| (base.to_owned(), actual.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_packaging_suffixes_match_in_both_directions() {
        for suffix in SUFFIXES {
            let variant = format!("example{suffix}");
            assert_eq!(
                resolve([variant.as_str()], "example"),
                Some(variant.as_str())
            );
            assert_eq!(resolve(["example"], &variant), Some("example"));
            assert_eq!(
                resolve([variant.as_str()], "example-bin-git"),
                Some(variant.as_str())
            );
        }
    }
    #[test]
    fn exact_names_win_and_ambiguous_alternatives_are_not_chosen() {
        assert_eq!(
            resolve(["example-git", "example-bin", "example"], "example-bin"),
            Some("example-bin")
        );
        assert_eq!(resolve(["example-bin", "example-git"], "example"), None);
        assert_eq!(
            resolve(["example-git", "example"], "example-bin"),
            Some("example")
        );
    }
    #[test]
    fn indexed_matches_keep_exact_and_ambiguity_rules() {
        let installed = std::collections::BTreeMap::from([
            ("example-bin".into(), "1".into()),
            ("example-git".into(), "1".into()),
            ("tool-git".into(), "1".into()),
        ]);
        let index = alias_index(&installed);
        assert!(!index.contains_key("example"));
        assert_eq!(index.get("tool").map(String::as_str), Some("tool-git"));
        for name in [
            "tool",
            "tool-bin",
            "tool-git",
            "example",
            "example-bin",
            "example-git",
            "unknown",
        ] {
            let indexed = if installed.contains_key(name) {
                name
            } else {
                index
                    .get(base_name(name))
                    .map(String::as_str)
                    .unwrap_or(name)
            };
            assert_eq!(indexed, installed_name(&installed, name));
        }
    }
    #[test]
    fn unrelated_packages_are_not_merged() {
        for name in [
            "example-debug",
            "example-bin-debug",
            "example-plugin",
            "libexample",
            "example-nightly",
            "example-git-tools",
            "example-tools-git",
        ] {
            assert_eq!(resolve([name], "example"), None, "{name}");
        }
    }
}
