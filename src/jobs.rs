#[derive(Clone, Copy)]
pub enum Job {
    CacheSize,
    CacheSizeAdmin,
    CachePreview,
    CacheClean,
    UninstalledPreview,
    UninstalledClean,
    JournalSize,
    JournalClean,
    Orphans,
    Foreign,
    Failed,
    Boot,
    Vulkan,
    GameMode,
    Updates,
    Report,
}

impl Job {
    pub fn title(self) -> &'static str {
        match self {
            Self::CacheSize => "Package cache usage",
            Self::CacheSizeAdmin => "Full package cache usage",
            Self::CachePreview => "Package cache preview",
            Self::CacheClean => "Clean older package versions",
            Self::UninstalledPreview => "Uninstalled package cache preview",
            Self::UninstalledClean => "Remove uninstalled package cache",
            Self::JournalSize => "Journal disk usage",
            Self::JournalClean => "Keep 7 days of archived logs",
            Self::Orphans => "Inspect orphan packages",
            Self::Foreign => "Foreign packages",
            Self::Failed => "Failed system services",
            Self::Boot => "Boot performance",
            Self::Vulkan => "Vulkan diagnostics",
            Self::GameMode => "GameMode self-test",
            Self::Updates => "Available repository updates",
            Self::Report => "System report",
        }
    }
    pub fn command(self) -> Result<(&'static str, Vec<String>), String> {
        if matches!(self, Self::CacheSize | Self::CacheSizeAdmin) {
            let mut args = vec!["-sch".to_string(), "--".into()];
            args.extend(crate::package_cache::directories()?);
            if matches!(self, Self::CacheSizeAdmin) {
                args.insert(0, "/usr/bin/du".into());
            }
            return Ok((
                if self.privileged() {
                    "/usr/bin/pkexec"
                } else {
                    "/usr/bin/du"
                },
                args,
            ));
        }
        let (program, args) = match self {
            Self::CacheSize => ("/usr/bin/du", vec!["-sh", "/var/cache/pacman/pkg"]),
            Self::CacheSizeAdmin => (
                "/usr/bin/pkexec",
                vec!["/usr/bin/du", "-sh", "/var/cache/pacman/pkg"],
            ),
            Self::CachePreview => ("/usr/bin/paccache", vec!["-d", "-v", "-k", "2"]),
            Self::CacheClean => (
                "/usr/bin/pkexec",
                vec!["/usr/bin/paccache", "-r", "-k", "2"],
            ),
            Self::UninstalledPreview => ("/usr/bin/paccache", vec!["-d", "-v", "-u", "-k", "0"]),
            Self::UninstalledClean => (
                "/usr/bin/pkexec",
                vec!["/usr/bin/paccache", "-r", "-u", "-k", "0"],
            ),
            Self::JournalSize => ("/usr/bin/journalctl", vec!["--disk-usage"]),
            Self::JournalClean => (
                "/usr/bin/pkexec",
                vec!["/usr/bin/journalctl", "--vacuum-time=7d"],
            ),
            Self::Orphans => ("/usr/bin/pacman", vec!["-Qdtq"]),
            Self::Foreign => ("/usr/bin/pacman", vec!["-Qm"]),
            Self::Failed => (
                "/usr/bin/systemctl",
                vec!["--failed", "--no-pager", "--plain"],
            ),
            Self::Boot => ("/usr/bin/systemd-analyze", vec!["blame", "--no-pager"]),
            Self::Vulkan => ("/usr/bin/vulkaninfo", vec!["--summary"]),
            Self::GameMode => ("/usr/bin/gamemoded", vec!["-t"]),
            Self::Updates => ("/usr/bin/checkupdates", vec!["--nocolor"]),
            Self::Report => ("/usr/bin/uname", vec!["-a"]),
        };
        Ok((program, args.into_iter().map(str::to_owned).collect()))
    }
    pub fn privileged(self) -> bool {
        self.destructive() || matches!(self, Self::CacheSizeAdmin)
    }
    pub fn destructive(self) -> bool {
        matches!(
            self,
            Self::CacheClean | Self::UninstalledClean | Self::JournalClean
        )
    }
    pub fn confirmation(self) -> &'static str {
        if self.destructive() {
            "This permanently removes data. Review the preview first. Your desktop will request administrator authentication."
        } else {
            "Administrator authentication is needed to include pacman's protected download folders. This only measures disk usage; it does not change or delete files."
        }
    }
    pub fn preview(self) -> Option<Self> {
        match self {
            Self::CacheClean => Some(Self::CachePreview),
            Self::UninstalledClean => Some(Self::UninstalledPreview),
            Self::JournalClean => Some(Self::JournalSize),
            _ => None,
        }
    }
    pub fn cache_permission_error(self, message: &str) -> bool {
        matches!(self, Self::CacheSize)
            && message.contains("du:")
            && message.contains("Permission denied")
    }
    pub fn display_command(self) -> String {
        if matches!(self, Self::Report) {
            return "/usr/bin/uname -a\n/usr/bin/pacman -Q\n/usr/bin/lspci\n/usr/bin/systemctl --failed --no-pager --plain".into();
        }
        match self.command() {
            Ok((p, a)) => format!(
                "{} {}",
                p,
                a.iter()
                    .map(|s| {
                        if s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
                        {
                            s.clone()
                        } else {
                            crate::package_cache::quote(s)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Err(_) => "Cannot read pacman cache configuration".into(),
        }
    }
    fn empty_query_message(self, error: &str) -> Option<&'static str> {
        match (self, error.trim()) {
            (Self::Orphans, "Exit 1") => Some("No orphan packages found."),
            (Self::Foreign, "Exit 1") => Some("No foreign packages found."),
            (Self::Updates, "Exit 2") => Some("No repository updates available."),
            _ => None,
        }
    }
    pub fn execute(self) -> Result<String, String> {
        let (p, a) = self.command()?;
        let args: Vec<&str> = a.iter().map(String::as_str).collect();
        let result = crate::system::run(p, &args, 180);
        if let Err(error) = &result
            && let Some(message) = self.empty_query_message(error)
        {
            return Ok(message.into());
        }
        // pacman returns 1 for an empty query; checkupdates returns 2 when up to date.
        match (&self, &result) {
            (Self::Report, Ok(kernel)) => {
                let inventory = crate::system::Inventory::load();
                let metrics = crate::system::Sampler::default().sample();
                Ok(format!(
                    "Arch Forge system report\n\nKernel: {}CPU: {}\nGPU: {}\nSession: {}\nMemory: {} / {}\nRoot storage: {} / {}\nInstalled packages: {}\nPackage query: {}\n\nFailed services\n{}",
                    kernel,
                    inventory.cpu,
                    inventory.gpu,
                    inventory.desktop,
                    crate::system::bytes(metrics.used_memory),
                    crate::system::bytes(metrics.total_memory),
                    crate::system::bytes(metrics.used_disk),
                    crate::system::bytes(metrics.total_disk),
                    inventory.packages.len(),
                    inventory.error.unwrap_or_else(|| "OK".into()),
                    crate::system::run(
                        "/usr/bin/systemctl",
                        &["--failed", "--no-pager", "--plain"],
                        10
                    )
                    .unwrap_or_else(|e| e)
                ))
            }
            _ => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_queries_are_successful_without_hiding_errors() {
        assert_eq!(
            Job::Foreign.empty_query_message("Exit 1\n"),
            Some("No foreign packages found.")
        );
        assert_eq!(
            Job::Orphans.empty_query_message("Exit 1\n"),
            Some("No orphan packages found.")
        );
        assert_eq!(
            Job::Updates.empty_query_message("Exit 2\n"),
            Some("No repository updates available.")
        );
        assert_eq!(
            Job::Foreign.empty_query_message("Exit 1\nerror: database unavailable"),
            None
        );
        assert_eq!(Job::Vulkan.empty_query_message("Exit 1\n"), None);
    }

    #[test]
    #[ignore = "Runs installed read-only tools and may access repository mirrors"]
    fn local_read_only_jobs() {
        let mut report = String::new();
        for job in [
            Job::CacheSize,
            Job::CachePreview,
            Job::JournalSize,
            Job::Orphans,
            Job::Foreign,
            Job::Failed,
            Job::Boot,
            Job::Vulkan,
            Job::Updates,
            Job::Report,
        ] {
            let result = job.execute();
            let state = match result {
                Ok(output) => format!("PASS ({} output bytes)", output.len()),
                Err(error) if job.cache_permission_error(&error) => {
                    "EXPECTED: protected cache folders; authentication offered".into()
                }
                Err(error) if !std::path::Path::new(job.command().unwrap().0).exists() => {
                    format!("UNAVAILABLE: optional tool ({error})")
                }
                Err(error) => panic!("{} failed: {error}", job.title()),
            };
            report.push_str(&format!("{}: {state}\n", job.title()));
        }
        std::fs::create_dir_all("test-results").unwrap();
        std::fs::write("test-results/runner-jobs.txt", &report).unwrap();
        println!("{report}");
    }
    #[test]
    fn protected_cache_is_identified_without_hiding_other_errors() {
        let message = "Exit 1\n14G /var/cache/pacman/pkg\ndu: cannot read directory '/var/cache/pacman/pkg/download-abc': Permission denied";
        assert!(Job::CacheSize.cache_permission_error(message));
        assert!(!Job::CacheSize.cache_permission_error("du: No such file or directory"));
        assert!(!Job::CacheClean.cache_permission_error(message));
    }
    #[test]
    fn authenticated_measurement_is_read_only_and_gated() {
        assert!(Job::CacheSizeAdmin.privileged());
        assert!(!Job::CacheSizeAdmin.destructive());
        assert!(
            Job::CacheSizeAdmin
                .confirmation()
                .contains("does not change or delete files")
        );
        let (program, args) = Job::CacheSizeAdmin.command().unwrap();
        assert_eq!(program, "/usr/bin/pkexec");
        assert_eq!(&args[..3], &["/usr/bin/du", "-sch", "--"]);
        assert_eq!(&args[3..], crate::package_cache::directories().unwrap());
        assert!(Job::CacheClean.destructive());
        assert!(Job::JournalClean.destructive());
    }
}
