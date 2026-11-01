use crate::{
    button,
    dialogs::{Surface, Tone},
};
use gtk::{glib, prelude::*};
use std::{
    cell::RefCell,
    ffi::{CString, OsStr, OsString},
    fs::{self, File, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Path, PathBuf},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

#[derive(Clone, Copy)]
pub enum Action {
    Trash,
    YayCache,
    PipCache,
    Thumbnails,
    Orphans,
    Aur,
}
impl Action {
    pub fn title(self) -> &'static str {
        match self {
            Self::Trash => "Empty home Trash",
            Self::YayCache => "Clear yay build cache",
            Self::PipCache => "Clear pip cache",
            Self::Thumbnails => "Clear thumbnail cache",
            Self::Orphans => "Remove orphan dependencies",
            Self::Aur => "Review AUR cache with yay",
        }
    }
    fn terminal(self) -> bool {
        matches!(self, Self::Orphans | Self::Aur)
    }
    fn paths(self) -> Result<Vec<PathBuf>, String> {
        paths_for(
            self,
            &glib::home_dir(),
            std::env::var_os("XDG_CACHE_HOME"),
            std::env::var_os("XDG_DATA_HOME"),
            std::env::var_os("PIP_CACHE_DIR"),
        )
    }
}
pub(crate) fn paths_for(
    action: Action,
    home: &Path,
    cache: Option<OsString>,
    data: Option<OsString>,
    pip: Option<OsString>,
) -> Result<Vec<PathBuf>, String> {
    if !home.is_absolute() || home == Path::new("/") {
        return Err("Cannot determine a safe home directory.".into());
    }
    let cache = xdg(cache, &home.join(".cache"));
    let data = xdg(data, &home.join(".local/share"));
    Ok(match action {
        Action::Trash => vec![data.join("Trash/files"), data.join("Trash/info")],
        Action::YayCache | Action::Aur => vec![cache.join("yay")],
        Action::PipCache => vec![xdg(pip, &cache.join("pip"))],
        Action::Thumbnails => vec![cache.join("thumbnails")],
        Action::Orphans => vec![],
    })
}

pub(crate) fn xdg(value: Option<OsString>, fallback: &Path) -> PathBuf {
    value
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p != Path::new("/"))
        .unwrap_or_else(|| fallback.into())
}

struct Folder {
    directory: File,
    path: PathBuf,
    entries: Vec<OsString>,
    device: u64,
}
pub(crate) struct Plan {
    folders: Vec<Folder>,
}
fn fd_path(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}
impl Plan {
    pub(crate) fn scan(paths: Vec<PathBuf>) -> Result<Self, String> {
        let mut folders = vec![];
        for path in paths {
            if !path.is_absolute() || path == Path::new("/") || path == glib::home_dir() {
                return Err("Refusing to clean a root or home directory.".into());
            }
            let directory = match OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&path)
            {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(format!(
                        "Cannot review {}: {e}. Linked folders are not followed.",
                        path.display()
                    ));
                }
            };
            let metadata = directory.metadata().map_err(|e| e.to_string())?;
            if [PathBuf::from("/"), glib::home_dir()]
                .iter()
                .filter_map(|p| fs::metadata(p).ok())
                .any(|m| m.dev() == metadata.dev() && m.ino() == metadata.ino())
            {
                return Err(
                    "Refusing to clean a root or home directory, including aliased paths.".into(),
                );
            }
            // SAFETY: geteuid has no preconditions.
            if metadata.uid() != unsafe { libc::geteuid() } {
                return Err(format!(
                    "{} is not owned by your user. It will not be cleaned with administrator access.",
                    path.display()
                ));
            }
            let entries = fs::read_dir(fd_path(&directory))
                .map_err(|e| e.to_string())?
                .map(|e| e.map(|e| e.file_name()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            folders.push(Folder {
                directory,
                path,
                entries,
                device: metadata.dev(),
            });
        }
        Ok(Self { folders })
    }
    fn count(&self) -> usize {
        self.folders.iter().map(|f| f.entries.len()).sum()
    }
    fn preview(&self) -> String {
        let mut text = format!(
            "{} top-level entries selected, including their contents and hidden files.\nOnly the listed user folders are affected. Linked folders and other filesystems are not traversed.\n",
            self.count()
        );
        for folder in &self.folders {
            text += &format!(
                "\n{} — {} entries\n",
                folder.path.display(),
                folder.entries.len()
            );
            for name in folder.entries.iter().take(50) {
                text += &format!("  {}\n", name.to_string_lossy());
            }
            if folder.entries.len() > 50 {
                text += "  … more entries in this folder\n";
            }
        }
        if self.folders.is_empty() {
            text += "\nThese cache folders do not exist. Nothing to remove.";
        }
        text
    }
    pub(crate) fn execute(self) -> Result<String, String> {
        let mut count = 0;
        let mut errors = vec![];
        for folder in self.folders {
            // If Trash payload removal fails, retain its metadata for recovery.
            if !errors.is_empty() {
                break;
            }
            let original = folder.directory.metadata().map_err(|e| e.to_string())?;
            if !fs::symlink_metadata(&folder.path)
                .is_ok_and(|m| m.is_dir() && m.dev() == original.dev() && m.ino() == original.ino())
            {
                errors.push(format!(
                    "{} changed since review. It was kept; review it again.",
                    folder.path.display()
                ));
                continue;
            }
            for name in folder.entries {
                match remove_entry(&folder.directory, &name, folder.device, 0) {
                    Ok(()) => count += 1,
                    Err(error) => errors.push(format!(
                        "{}/{}: {error}",
                        folder.path.display(),
                        name.to_string_lossy()
                    )),
                }
            }
        }
        let message = format!("Removed {count} reviewed top-level entries.");
        if errors.is_empty() {
            Ok(message)
        } else {
            Err(format!(
                "{message}\nSome entries were kept:\n{}",
                errors.join("\n")
            ))
        }
    }
}
fn remove_entry(parent: &File, name: &OsStr, device: u64, depth: usize) -> Result<(), String> {
    if depth >= 256 {
        return Err("folder is too deeply nested; kept for manual review".into());
    }
    let path = fd_path(parent).join(name);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let name = CString::new(name.as_bytes()).map_err(|e| e.to_string())?;
    let flags = if metadata.is_dir() {
        // SAFETY: parent is an open directory; name is a single NUL-terminated entry name.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: openat returned a newly owned descriptor.
        let directory = unsafe { File::from_raw_fd(fd) };
        let stat = directory.metadata().map_err(|e| e.to_string())?;
        // SAFETY: geteuid has no preconditions.
        if stat.dev() != device || stat.uid() != unsafe { libc::geteuid() } {
            return Err("another filesystem or user-owned folder was kept".into());
        }
        for entry in fs::read_dir(fd_path(&directory)).map_err(|e| e.to_string())? {
            remove_entry(
                &directory,
                &entry.map_err(|e| e.to_string())?.file_name(),
                device,
                depth + 1,
            )?;
        }
        libc::AT_REMOVEDIR
    } else {
        0
    };
    // SAFETY: deletion is relative to the retained directory descriptor. Symlinks
    // are unlinked, never followed; ancestor renames cannot redirect this operation.
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), flags) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

pub(crate) fn orphan_command(output: &str) -> Result<Option<String>, String> {
    let names: Vec<_> = output.split_whitespace().collect();
    if names.is_empty() {
        return Ok(None);
    }
    if names.iter().any(|name| {
        name.starts_with('-')
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
    }) {
        return Err("Pacman returned an unexpected package name.".into());
    }
    let command = format!("sudo pacman -Rs -- {}", names.join(" "));
    if command.len() > 32_000 {
        return Err(
            "Too many orphan packages for one terminal command. Review them manually.".into(),
        );
    }
    Ok(Some(command))
}
enum Prepared {
    Files(Plan),
    Terminal {
        command: Option<String>,
        preview: String,
    },
}
fn prepare(action: Action) -> Result<Prepared, String> {
    if matches!(action, Action::Orphans) {
        let output = match crate::system::run("/usr/bin/pacman", &["-Qdtq"], 20) {
            Ok(output) => output,
            Err(error) if error.trim() == "Exit 1" => String::new(),
            Err(e) => return Err(e),
        };
        let command = orphan_command(&output)?;
        return Ok(Prepared::Terminal {
            preview: if command.is_some() {
                format!(
                    "Orphan dependency candidates:\n{output}\nPacman will show the complete removal transaction and ask for confirmation. Nothing is removed during this review."
                )
            } else {
                "No orphan dependencies found.".into()
            },
            command,
        });
    }
    let plan = Plan::scan(action.paths()?)?;
    if matches!(action, Action::Aur) {
        return Ok(Prepared::Terminal {
            command: Some("yay -Sc --aur".into()),
            preview: format!(
                "{}\n\nyay runs as your regular user and asks which AUR cache entries to remove. This preview does not invoke yay or delete files.",
                plan.preview()
            ),
        });
    }
    Ok(Prepared::Files(plan))
}

pub fn show(parent: &gtk::ApplicationWindow, action: Action) {
    let (dialog, surface) = Surface::new(
        parent,
        action.title(),
        "Preview your selected files before making changes.",
        if matches!(action, Action::Orphans) {
            "\u{e8cb}"
        } else {
            "\u{e872}"
        },
        if matches!(action, Action::Orphans) {
            "package-x-generic-symbolic"
        } else {
            "user-trash-symbolic"
        },
    );
    let status = surface.status.clone();
    status.set(
        Tone::Busy,
        "Reading cleanup candidates…",
        "This is a read-only preview. Nothing is removed before you review and confirm.",
        "No changes made",
    );
    let stats = surface.stats(
        (
            if matches!(action, Action::Orphans) {
                "Orphan packages"
            } else {
                "Selected entries"
            },
            "—",
            "folder-symbolic",
        ),
        (
            if action.terminal() {
                "Cleanup method"
            } else {
                "Cleanup scope"
            },
            if matches!(action, Action::Orphans) {
                "pacman"
            } else if matches!(action, Action::Aur) {
                "yay (user)"
            } else {
                "Your account"
            },
            "avatar-default-symbolic",
        ),
    );
    let details = surface.details(
        "Review selected items",
        "Folders, candidates, and cleanup details",
        None,
    );
    let buffer = details.output.buffer();
    let controls = surface.actions.clone();
    let close = surface.close_button(&dialog, "Close", false);
    let confirm = button(
        if action.terminal() {
            "Open terminal"
        } else {
            "Permanently clean"
        },
        false,
    );
    confirm.add_css_class(if action.terminal() {
        "primary"
    } else {
        "destructive"
    });
    confirm.set_sensitive(false);
    controls.append(&confirm);
    let copy = button("Copy command", false);
    copy.set_visible(action.terminal());
    copy.set_sensitive(false);
    details.tools.append(&copy);
    let proposed = Rc::new(RefCell::new(None::<String>));
    let command = proposed.clone();
    copy.connect_clicked(move |_| {
        if let Some(command) = command.borrow().as_ref() {
            crate::copy_text(command);
        }
    });
    let slot = Rc::new(RefCell::new(None));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(prepare(action));
    });
    let state = slot.clone();
    let preview_status = status.clone();
    let preview_buffer = buffer.clone();
    let enable = confirm.downgrade();
    let preview_close = close.clone();
    let disclosure = details.expander.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("Review stopped unexpectedly.".into()),
        };
        match result {
            Ok(prepared) => {
                let (text, can_clean) = match &prepared {
                    Prepared::Files(plan) => (plan.preview(), plan.count() > 0),
                    Prepared::Terminal { command, preview } => (
                        format!("{preview}\n\n{}", command.as_deref().unwrap_or("")),
                        command.is_some(),
                    ),
                };
                if let Prepared::Terminal { command, .. } = &prepared {
                    *proposed.borrow_mut() = command.clone();
                    copy.set_sensitive(command.is_some());
                }
                preview_buffer.set_text(&text);
                let count = match &prepared {
                    Prepared::Files(plan) => Some(plan.count()),
                    Prepared::Terminal { command, .. } if matches!(action, Action::Orphans) => {
                        command
                            .as_ref()
                            .map(|c| {
                                c.split(" -- ")
                                    .nth(1)
                                    .map(|s| s.split_whitespace().count())
                                    .unwrap_or(0)
                            })
                            .or(Some(0))
                    }
                    _ => None,
                };
                stats[0].set_text(
                    &count
                        .map(|c| {
                            format!(
                                "{c} {}",
                                if matches!(action, Action::Orphans) {
                                    if c == 1 { "package" } else { "packages" }
                                } else if c == 1 {
                                    "entry"
                                } else {
                                    "entries"
                                }
                            )
                        })
                        .unwrap_or_else(|| "Review with yay".into()),
                );
                if can_clean {
                    preview_status.set(Tone::Ready, "Ready for your review", if action.terminal() { "Review the candidates below. The terminal command waits for Enter and asks for transaction confirmation." } else if matches!(action,Action::Trash) { "Emptying Trash is permanent. Review the selected files and metadata before confirming." } else { "Close apps using this cache first. The reviewed entries will be permanently removed; cache folders are kept." }, "Awaiting confirmation");
                    disclosure.set_expanded(true);
                } else {
                    preview_status.set(
                        Tone::Success,
                        "Nothing needs cleaning",
                        "No eligible entries were found in the selected locations.",
                        "No changes needed",
                    );
                    preview_close.set_label("Done");
                    preview_close.add_css_class("primary");
                }
                if let Some(enable) = enable.upgrade() {
                    enable.set_sensitive(can_clean);
                    enable.set_visible(can_clean);
                }
                *state.borrow_mut() = Some(prepared);
            }
            Err(error) => {
                preview_status.set(
                    Tone::Error,
                    "Could not review these locations",
                    "No files were changed. Open the details below to see what needs attention.",
                    "No changes made",
                );
                disclosure.set_expanded(true);
                preview_close.set_label("Done");
                preview_close.add_css_class("primary");
                if let Some(enable) = enable.upgrade() {
                    enable.set_visible(false);
                }
                preview_buffer.set_text(&error);
            }
        }
        glib::ControlFlow::Break
    });
    let weak = dialog.downgrade();
    confirm.connect_clicked(move |button| {
        let Some(prepared) = slot.borrow_mut().take() else {
            return;
        };
        button.set_sensitive(false);
        status.set(Tone::Busy, if action.terminal() { "Opening your terminal…" } else { "Cleaning selected entries…" }, "You can close this dialog; the operation will continue.", "Working…");
        let terminal = matches!(&prepared, Prepared::Terminal { .. });
        let (tx, rx) = mpsc::channel();
        let operation = crate::Operation::new();
        std::thread::spawn(move || {
            let _operation = operation;
            let result = match prepared {
                Prepared::Files(plan) => plan.execute(),
                Prepared::Terminal { command, .. } => {
                    crate::terminal::open(command.as_deref().unwrap_or(""))
                        .map(|()| "Terminal opened.".into())
                }
            };
            let _ = tx.send(result);
        });
        let status = status.clone();
        let buffer = buffer.clone();
        let weak = weak.clone();
        let close = close.clone();
        let confirm = button.downgrade();
        let disclosure = details.expander.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let result = match rx.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Cleanup worker stopped unexpectedly.".into()),
            };
            match result {
                Ok(message) if terminal => {
                    if let Some(dialog) = weak.upgrade() {
                        dialog.close();
                    }
                    let _ = message;
                }
                Ok(message) => {
                    status.set(Tone::Success, "Cleanup completed", "The selected cleanup finished. Your cache root folders have been kept.", "Completed");
                    close.set_label("Done"); close.add_css_class("primary");
                    if let Some(confirm) = confirm.upgrade() { confirm.set_visible(false); }
                    disclosure.set_expanded(false);
                    buffer.set_text(&message);
                }
                Err(error) => {
                    status.set(if terminal { Tone::Error } else { Tone::Warning }, if terminal { "Could not open your terminal" } else { "Some entries were kept" }, if terminal { "No cleanup command has run. Copy the command from the details or review again to retry." } else { "Open the details below to review the entries that could not be removed. Review again before retrying." }, "Check the details");
                    close.set_label("Done"); close.add_css_class("primary");
                    if let Some(confirm) = confirm.upgrade() { confirm.set_visible(false); }
                    disclosure.set_expanded(true);
                    buffer.set_text(&error);
                }
            }
            glib::ControlFlow::Break
        });
    });
    dialog.present();
}
