use crate::{
    app_storage::{self, Inventory},
    button, column,
    dialogs::{Details, Stats, Status, Surface, Tone},
    label, row, wrapped,
};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};
struct View {
    window: glib::WeakRef<gtk::Window>,
    parent: glib::WeakRef<gtk::ApplicationWindow>,
    status: Status,
    stats: Stats,
    details: Details,
    mode: gtk::DropDown,
    drives: gtk::DropDown,
    installed: gtk::DropDown,
    name: gtk::Entry,
    app: gtk::Entry,
    setup_fields: gtk::Box,
    install_fields: gtk::Box,
    action: gtk::Button,
    inventory: RefCell<Option<Inventory>>,
    command: RefCell<Option<String>>,
    loading: Cell<bool>,
    generation: Cell<u64>,
    updating: Cell<bool>,
}
impl View {
    fn render(&self) {
        if self.updating.get() {
            return;
        }
        let mode = self.mode.selected();
        self.setup_fields.set_visible(mode == 0);
        self.install_fields.set_visible(mode == 1);
        self.command.borrow_mut().take();
        self.action.set_sensitive(false);
        if self.loading.get() {
            self.status.set(
                Tone::Busy,
                "Reading storage locations…",
                "Checking mounted filesystems and registered app drives.",
                "Read-only inspection",
            );
            return;
        }
        let binding = self.inventory.borrow();
        let Some(inv) = binding.as_ref() else {
            return;
        };
        self.stats[0].set_text(
            &inv.root_free
                .map(crate::system::bytes)
                .unwrap_or_else(|| "Unavailable".into()),
        );
        self.stats.captions[1].set_text("Selected drive free");
        self.stats[1].set_text("—");
        if mode == 2 {
            self.stats.captions[1].set_text("Package install target");
            self.stats[1].set_text("Arch system");
            self.status.set(Tone::Ready,"Pacman uses system storage","Package files go to their defined paths in your Arch installation. There is no per-package drive selector.","System storage inspection");
            self.details.output.buffer().set_text(&format!("Where /usr is stored:\n{}\nPacman files normally live under /usr, /etc, and /var.\n\nTo gain space for pacman packages:\n• Expand or migrate the root filesystem, or plan a separate system mount with a backup and boot recovery available.\n• A bind mount for a complete application directory requires a package-specific plan; moving individual files or symlinking /usr is not a general solution.\n• Cache relocation frees archive storage, but does not change where installed package files go.\n\nArch Forge does not move system directories or edit fstab. Use View drives to inspect the layout before planning a migration.\n\nMounted data filesystems:\n{}",inv.usr_mount,inv.drives.iter().map(|d|format!("{} — {} ({})",d.target,d.source,d.fs)).collect::<Vec<_>>().join("\n")));
            self.action.set_label("View drives");
            self.action.set_sensitive(true);
            return;
        }
        if !inv.flatpak {
            self.status.set(Tone::Ready,"Flatpak is required","Install Flatpak first, then refresh locations. Pacman remains your Arch system package manager.","Optional desktop-app support");
            self.details.output.buffer().set_text("sudo pacman -Syu --needed flatpak\n\nReview pacman's transaction in the embedded terminal. After installation, return here and refresh locations.");
            *self.command.borrow_mut() = Some("sudo pacman -Syu --needed flatpak".into());
            self.action.set_label("Install Flatpak…");
            self.action.set_sensitive(true);
            return;
        }
        self.action.set_label("Open embedded terminal");
        let result = if mode == 0 {
            let candidates: Vec<_> = inv
                .drives
                .iter()
                .filter(|d| app_storage::eligibility(d).is_ok())
                .collect();
            if let Some(drive) = candidates.get(self.drives.selected() as usize) {
                self.stats[1].set_text(
                    &drive
                        .available
                        .map(crate::system::bytes)
                        .unwrap_or_else(|| "Unavailable".into()),
                );
                app_storage::setup_command(&self.name.text(),&drive.target).map(|command|(command,format!("Destination: {}\nFilesystem: {}\n\nThe command registers a new named Flatpak installation and adds Flathub to it. It creates a dedicated app folder and a new configuration file; it does not replace existing installations or move installed apps.\n\nThe mount must be persistent, protected, and root-owned. Only ext4, Btrfs, XFS, and F2FS drives are offered. Keep the drive mounted at the same path. Sign out and back in if new shortcuts do not appear.",app_storage::location(&self.name.text(),&drive.target).unwrap_or_default(),drive.fs)))
            } else {
                Err("No eligible data filesystem is mounted. Mount a writable Linux data filesystem at a persistent location under /mnt, /media, or /home, then refresh. The root filesystem and shared Btrfs storage are excluded because they would not free root space.".into())
            }
        } else if let Some(installation) = inv.installations.get(self.installed.selected() as usize)
        {
            match app_storage::verify(installation, &inv.drives) {
                Ok(drive) => {
                    self.stats[1].set_text(
                        &drive
                            .available
                            .map(crate::system::bytes)
                            .unwrap_or_else(|| "Unavailable".into()),
                    );
                    app_storage::install_command(&installation.id,&self.app.text()).map(|command|(command,format!("App files and runtimes will be installed in:\n{}\n\nInstallation: archforge-{}\nSource: Flathub\n\nThe command verifies that the configured drive is mounted before installing. Flatpak shows the app, runtimes, permissions, and download size for you to review.\n\nApp data stays in your home directory; temporary files and some metadata can still use system storage. Existing copies of the app are not moved. Keep this drive mounted to run its apps.",installation.path,installation.id)))
                }
                Err(error) => Err(error),
            }
        } else {
            Err("No app drives are registered yet. Choose Flatpak: add a drive first, complete its terminal command, then refresh locations.".into())
        };
        match result {
            Ok((command, explanation)) => {
                self.status.set(Tone::Ready,if mode==0{"Add an app installation drive"}else{"Install this app on the selected drive"},"Desktop apps available as Flatpaks can use this location. Review the destination and command before running.","Nothing has been installed");
                self.details
                    .output
                    .buffer()
                    .set_text(&format!("{command}\n\n{explanation}"));
                *self.command.borrow_mut() = Some(command);
                self.action.set_sensitive(true);
            }
            Err(error) => {
                self.status.set(
                    Tone::Warning,
                    "Check the installation choices",
                    &error,
                    "No changes made",
                );
                self.details.output.buffer().set_text(&error);
            }
        }
    }
    fn reload(self: &Rc<Self>) {
        if self.loading.replace(true) {
            return;
        }
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        self.render();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(Inventory::load());
        });
        let view = self.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            if view.window.upgrade().is_none() {
                return glib::ControlFlow::Break;
            }
            let result = match rx.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("Storage inspection worker stopped.".into()),
            };
            if view.generation.get() != generation {
                return glib::ControlFlow::Break;
            }
            view.loading.set(false);
            match result {
                Ok(mut inv) => {
                    let old_path = {
                        let old = view.inventory.borrow();
                        old.as_ref()
                            .and_then(|i| {
                                i.drives
                                    .iter()
                                    .filter(|d| app_storage::eligibility(d).is_ok())
                                    .nth(view.drives.selected() as usize)
                            })
                            .map(|d| d.target.clone())
                    };
                    let old_install = {
                        let old = view.inventory.borrow();
                        old.as_ref()
                            .and_then(|i| i.installations.get(view.installed.selected() as usize))
                            .map(|i| i.id.clone())
                    };
                    inv.drives.sort_by(|a, b| b.available.cmp(&a.available));
                    let candidates: Vec<_> = inv
                        .drives
                        .iter()
                        .filter(|d| app_storage::eligibility(d).is_ok())
                        .collect();
                    let names: Vec<_> = candidates
                        .iter()
                        .map(|d| {
                            format!(
                                "{} · {} free · {}",
                                d.target,
                                d.available
                                    .map(crate::system::bytes)
                                    .unwrap_or_else(|| "unknown space".into()),
                                d.fs
                            )
                        })
                        .collect();
                    let refs: Vec<_> = names.iter().map(String::as_str).collect();
                    view.updating.set(true);
                    view.drives.set_model(Some(&gtk::StringList::new(&refs)));
                    if let Some(index) = candidates
                        .iter()
                        .position(|d| Some(&d.target) == old_path.as_ref())
                    {
                        view.drives.set_selected(index as u32);
                    } else if !candidates.is_empty() {
                        view.drives.set_selected(0);
                    }
                    let installs: Vec<_> = inv
                        .installations
                        .iter()
                        .map(|i| format!("{} · {}", i.id, i.path))
                        .collect();
                    let refs: Vec<_> = installs.iter().map(String::as_str).collect();
                    view.installed.set_model(Some(&gtk::StringList::new(&refs)));
                    if let Some(index) = inv
                        .installations
                        .iter()
                        .position(|i| Some(&i.id) == old_install.as_ref())
                    {
                        view.installed.set_selected(index as u32);
                    } else if !installs.is_empty() {
                        view.installed.set_selected(0);
                    }
                    *view.inventory.borrow_mut() = Some(inv);
                    view.updating.set(false);
                    view.render();
                }
                Err(error) => {
                    view.status.set(
                        Tone::Error,
                        "Could not inspect storage",
                        &error,
                        "No changes made",
                    );
                    view.details.output.buffer().set_text(&error);
                    view.action.set_sensitive(false);
                }
            }
            glib::ControlFlow::Break
        });
    }
}
fn create(parent: &gtk::ApplicationWindow) -> (gtk::Window, Rc<View>) {
    let (window, surface) = Surface::new(
        parent,
        "App install locations",
        "Desktop-app drives and Arch system package storage.",
        "\u{e2c7}",
        "drive-removable-media-symbolic",
    );
    surface.content.set_spacing(12);
    let stats = surface.stats(
        (
            "Root filesystem free",
            "Reading…",
            "drive-harddisk-symbolic",
        ),
        ("Selected drive free", "—", "drive-removable-media-symbolic"),
    );
    surface.content.append(&wrapped("Flatpak supports app-drive selection. Pacman packages use the storage layout of the Arch system.","muted"));
    let details = surface.details(
        "Installation review",
        "Choose a location · review the command · run it in the embedded terminal",
        None,
    );
    details.form.set_visible(true);
    let mode = gtk::DropDown::from_strings(&[
        "Flatpak: add a drive",
        "Flatpak: install an app",
        "Pacman: system package storage",
    ]);
    mode.update_property(&[gtk::accessible::Property::Label("App location mode")]);
    details.form.append(&mode);
    let setup_fields = column(8);
    let drives = gtk::DropDown::from_strings(&["Reading mounted drives…"]);
    drives.update_property(&[gtk::accessible::Property::Label("Destination filesystem")]);
    let destination = row(10);
    destination.append(&label("Destination", "tiny"));
    drives.set_hexpand(true);
    destination.append(&drives);
    setup_fields.append(&destination);
    let name = gtk::Entry::builder()
        .text("extra")
        .max_length(32)
        .hexpand(true)
        .placeholder_text("Drive name, e.g. extra")
        .build();
    name.update_property(&[gtk::accessible::Property::Label("App drive name")]);
    let naming = row(10);
    naming.append(&label("Drive name", "tiny"));
    naming.append(&name);
    setup_fields.append(&naming);
    details.form.append(&setup_fields);
    let install_fields = column(8);
    let installed = gtk::DropDown::from_strings(&["Refresh to load registered app drives"]);
    installed.update_property(&[gtk::accessible::Property::Label("Registered app drive")]);
    let destination = row(10);
    destination.append(&label("App drive", "tiny"));
    installed.set_hexpand(true);
    destination.append(&installed);
    install_fields.append(&destination);
    let app = gtk::Entry::builder()
        .placeholder_text("Application ID, e.g. org.mozilla.firefox")
        .build();
    app.update_property(&[gtk::accessible::Property::Label("Flatpak application ID")]);
    let application = row(10);
    application.append(&label("App ID", "tiny"));
    app.set_hexpand(true);
    application.append(&app);
    application.append(&gtk::LinkButton::with_label(
        "https://flathub.org/apps",
        "Find apps",
    ));
    install_fields.append(&application);
    details.form.append(&install_fields);
    let refresh = button("Refresh locations", false);
    details.tools.append(&refresh);
    let copy = button("Copy review", false);
    details.tools.append(&copy);
    let docs = gtk::LinkButton::with_label(
        "https://docs.flatpak.org/en/latest/tips-and-tricks.html#adding-a-custom-installation",
        "Location guide",
    );
    details.tools.append(&docs);
    surface.close_button(&window, "Close", false);
    let action = button("Open embedded terminal", true);
    surface.actions.append(&action);
    let view = Rc::new(View {
        window: window.downgrade(),
        parent: parent.downgrade(),
        status: surface.status,
        stats,
        details,
        mode: mode.clone(),
        drives: drives.clone(),
        installed: installed.clone(),
        name: name.clone(),
        app: app.clone(),
        setup_fields,
        install_fields,
        action: action.clone(),
        inventory: RefCell::new(None),
        command: RefCell::new(None),
        loading: Cell::new(false),
        generation: Cell::new(0),
        updating: Cell::new(false),
    });
    for dropdown in [mode, drives, installed] {
        let weak = Rc::downgrade(&view);
        dropdown.connect_selected_notify(move |_| {
            if let Some(v) = weak.upgrade() {
                v.render();
            }
        });
    }
    for entry in [name, app] {
        let weak = Rc::downgrade(&view);
        entry.connect_changed(move |_| {
            if let Some(v) = weak.upgrade() {
                v.render();
            }
        });
    }
    let weak = Rc::downgrade(&view);
    refresh.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            v.reload();
        }
    });
    let weak = Rc::downgrade(&view);
    copy.connect_clicked(move |_| {
        if let Some(v) = weak.upgrade() {
            let buffer = v.details.output.buffer();
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            if !text.is_empty() {
                crate::copy_text(&text);
            }
        }
    });
    let weak = Rc::downgrade(&view);
    action.connect_clicked(move |button| {
        if let Some(v) = weak.upgrade() {
            if v.mode.selected() == 2 {
                if let Some(parent) = v.parent.upgrade() {
                    crate::tools::inspect_mounts(&parent);
                }
            } else if let (Some(window), Some(command)) =
                (v.window.upgrade(), v.command.borrow().clone())
            {
                crate::tools::launch_terminal(&window, command, v.status.clone(), button);
            }
        }
    });
    let keep = view.clone();
    window.connect_destroy(move |_| {
        let _ = &keep;
    });
    (window, view)
}
pub(crate) fn show(parent: &gtk::ApplicationWindow) {
    let (window, view) = create(parent);
    view.reload();
    window.present();
}
