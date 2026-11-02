use super::*;
#[test]
fn cache_paths_reject_injection_and_system_trees() {
    for value in [
        "/",
        "relative",
        "~/cache",
        "/tmp/../cache",
        "/tmp/./cache",
        "/tmp/a b",
        "/tmp/a\nCacheDir=x",
        "/tmp/a;touch",
        "/tmp/$(id)",
        "/etc/cache",
        "/usr/cache",
        "/proc/test",
        "/var/lib/pacman",
    ] {
        assert!(validate_path(value).is_err(), "{value}");
    }
    for value in [
        "/var/cache/pacman/pkg",
        "/mnt/nvme/package-cache",
        "/media/disk/cache.v2",
    ] {
        assert!(validate_path(value).is_ok(), "{value}");
    }
}
#[test]
fn cache_rewrite_is_reversible_and_keeps_original_fallbacks() {
    let original = "# hello\n[options]\n#CacheDir = ignored\nCacheDir = /old\nInclude = /etc/extra-options\n[core]\nInclude = /etc/pacman.d/mirrorlist\n";
    let first = rewrite(original, Some("/mnt/data/cache")).unwrap();
    assert!(
        first
            .contains("[options]\n# Arch Forge package cache: begin\nCacheDir = /mnt/data/cache\n")
    );
    assert!(first.contains("CacheDir = /old\nInclude = /etc/extra-options"));
    assert_eq!(rewrite(&first, None).unwrap(), original);
    let second = rewrite(&first, Some("/var/cache/other")).unwrap();
    assert_eq!(second.matches(START).count(), 1);
    assert!(!second.contains("/mnt/data/cache"));
    assert_eq!(rewrite(&second, None).unwrap(), original);
    assert_eq!(rewrite(&second, Some("/var/cache/other")).unwrap(), second);
}
#[test]
fn malformed_blocks_are_never_silently_discarded() {
    for value in [
        format!("[options]\n{START}\nCacheDir=/x"),
        format!("[options]\n{END}\n"),
        format!("[options]\n{START}\n{START}\n{END}\n"),
    ] {
        assert!(rewrite(&value, Some("/var/cache/test")).is_err());
    }
    assert!(rewrite("[core]\n", Some("/var/cache/test")).is_err());
    assert!(rewrite("[options]\n[options]\n", Some("/var/cache/test")).is_err());
}
#[test]
fn measurement_prefers_combined_total_for_multiple_caches() {
    assert_eq!(
        measured_size("2G\t/mnt/data/cache\n4G\t/var/cache/pacman/pkg\n6G\ttotal\n"),
        Some("6G")
    );
    assert_eq!(measured_size("14G /var/cache/pacman/pkg\n"), Some("14G"));
    assert_eq!(measured_size("Permission denied"), None);
}
#[test]
fn pacman_accepts_staged_cache_config_without_touching_system_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("pacman.conf");
    std::fs::write(
        &file,
        rewrite("[options]\nCacheDir = /old\n", Some("/var/cache/proposed")).unwrap(),
    )
    .unwrap();
    let result = crate::system::run(
        "/usr/bin/pacman-conf",
        &["--config", file.to_str().unwrap(), "CacheDir"],
        5,
    )
    .unwrap();
    assert_eq!(result.lines().next(), Some("/var/cache/proposed"));
    assert!(result.contains("/old"));
}
