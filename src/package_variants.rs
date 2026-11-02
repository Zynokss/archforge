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
