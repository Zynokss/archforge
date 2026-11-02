use crate::{button, column, label, row, wrapped};
use gtk::{glib, prelude::*};
use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Duration};

pub(crate) fn image_from_bytes(bytes: &[u8]) -> Option<gtk::Picture> {
    image_sized(bytes, 800)
}
fn image_sized(bytes: &[u8], width: i32) -> Option<gtk::Picture> {
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    loader.connect_size_prepared(move |loader, w, h| {
        if w > width {
            loader.set_size(width, (h as i64 * width as i64 / w as i64).max(1) as i32);
        }
    });
    loader.write(bytes).ok()?;
    loader.close().ok()?;
    let pixbuf = loader.pixbuf()?;
    #[allow(deprecated)] // GTK handles RGB/RGBA and the final pixbuf row padding correctly.
    let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
    let picture = gtk::Picture::for_paintable(&texture);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_size_request(1, 1);
    Some(picture)
}

fn screenshot_gallery(images: &[(String, Vec<u8>)]) -> gtk::Box {
    let panel = column(10);
    let stage = gtk::Stack::new();
    stage.set_hexpand(true);
    let thumbs = row(8);
    thumbs.set_halign(gtk::Align::Center);
    for (index, (url, bytes)) in images.iter().enumerate() {
        if let Some(picture) = image_from_bytes(bytes) {
            picture.set_size_request(1, 360);
            picture.set_tooltip_text(Some(url));
            stage.add_named(&picture, Some(&index.to_string()));
            let thumb = gtk::Button::new();
            thumb.add_css_class("ricing-thumbnail");
            if let Some(image) = image_sized(bytes, 96) {
                image.set_size_request(96, 54);
                thumb.set_child(Some(&image));
            }
            let target = stage.clone();
            thumb.connect_clicked(move |_| target.set_visible_child_name(&index.to_string()));
            thumbs.append(&thumb);
        }
    }
    let controls = row(8);
    controls.set_halign(gtk::Align::Center);
    for (caption, step) in [("‹", -1_i32), ("›", 1)] {
        let nav = button(caption, false);
        let target = stage.clone();
        let count = images.len() as i32;
        nav.connect_clicked(move |_| {
            if count > 0 {
                let index = target
                    .visible_child_name()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                target.set_visible_child_name(&(index + step).rem_euclid(count).to_string());
            }
        });
        controls.append(&nav);
    }
    panel.append(&stage);
    panel.append(&thumbs);
    panel.append(&controls);
    panel
}

fn load_preview(
    project: crate::ricing::Project,
    target: &gtk::Box,
    readme: Option<gtk::Box>,
    gallery: bool,
) {
    while let Some(child) = target.first_child() {
        target.remove(&child);
    }
    static REQUEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let request = format!(
        "ricing-preview-{}",
        REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    target.set_widget_name(&request);
    target.set_size_request(-1, -1);
    target.append(&label("Loading screenshots…", "tiny"));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::ricing::fetch(project));
    });
    let target = target.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if target.widget_name() != request {
            return glib::ControlFlow::Break;
        }
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("Preview request stopped. Retry to reconnect.".into()),
        };
        while let Some(child) = target.first_child() {
            target.remove(&child);
        }
        let result = result.and_then(|details| {
            if !details
                .images
                .iter()
                .any(|(_, bytes)| image_from_bytes(bytes).is_some())
            {
                return Err("Image format could not be decoded.".into());
            }
            if gallery {
                target.append(&screenshot_gallery(&details.images));
            } else {
                let image = details
                    .images
                    .iter()
                    .find_map(|(_, bytes)| image_from_bytes(bytes))
                    .ok_or("Image format could not be decoded.")?;
                let frame = gtk::AspectFrame::new(0.5, 0.5, 16.0 / 9.0, false);
                frame.set_size_request(
                    1,
                    if target.has_css_class("ricing-card-art") {
                        210
                    } else {
                        180
                    },
                );
                frame.set_hexpand(true);
                frame.set_child(Some(&image));
                target.append(&frame);
            }
            if let Some(readme_view) = &readme {
                crate::ricing_document::render(readme_view, &details.readme, project);
                if details.readme.starts_with("README unavailable") {
                    let retry = button("Retry README", false);
                    retry.set_halign(gtk::Align::Start);
                    let retry_target = target.clone();
                    let readme = readme.clone();
                    retry.connect_clicked(move |_| {
                        load_preview(project, &retry_target, readme.clone(), gallery)
                    });
                    target.append(&retry);
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            target.append(&wrapped(&error, "tiny"));
            if let Some(readme) = &readme {
                crate::ricing_document::render(
                    readme,
                    "README preview unavailable. Use **View repository** for upstream documentation.",
                    project,
                );
            }
            let retry = button("Retry", false);
            retry.set_halign(gtk::Align::Start);
            let retry_target = target.clone();
            let readme = readme.clone();
            retry.connect_clicked(move |_| {
                load_preview(project, &retry_target, readme.clone(), gallery)
            });
            target.append(&retry);
        }
        glib::ControlFlow::Break
    });
}

fn detected_desktop() -> String {
    std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("XDG_SESSION_DESKTOP"))
        .unwrap_or_else(|_| "Unknown desktop".into())
        .split(':')
        .next()
        .unwrap_or("Unknown desktop")
        .to_string()
}

fn status_strip() -> gtk::Box {
    let strip = row(16);
    strip.add_css_class("ricing-system-strip");
    strip.append(&crate::tool_icons::icon(
        crate::tool_icons::Glyph::Check,
        18,
    ));
    strip.append(&label(
        if std::path::Path::new("/etc/arch-release").is_file() {
            "Arch Linux detected"
        } else {
            "Arch Linux required"
        },
        "scan-installed",
    ));
    strip.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    strip.append(&label(&format!("{} session", detected_desktop()), "muted"));
    strip.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    let backup_count = crate::ricing::backup_files().len();
    strip.append(&crate::tool_icons::icon(
        crate::tool_icons::Glyph::Cache,
        18,
    ));
    strip.append(&label(
        &format!(
            "{} configuration backup{}",
            if backup_count == 0 { "No" } else { "Local" },
            if backup_count == 1 { "" } else { "s" }
        ),
        "muted",
    ));
    strip
}

#[derive(Clone)]
struct DetailRefs {
    title: gtk::Label,
    identity: gtk::Label,
    overview: gtk::Label,
    requirements: gtk::Label,
    impact: gtk::Label,
    repo_link: gtk::LinkButton,
    install_text: gtk::Box,
    status: gtk::Label,
}

fn detail_page(
    parent: &gtk::ApplicationWindow,
    view_stack: &gtk::Stack,
    selected: &Rc<RefCell<Option<crate::ricing::Project>>>,
    readme: &gtk::Box,
    gallery: &gtk::Box,
) -> (gtk::Box, DetailRefs) {
    let page = column(14);
    page.add_css_class("ricing-detail-page");
    let back = gtk::Button::new();
    let back_contents = row(8);
    back_contents.append(&gtk::Image::from_icon_name("go-previous-symbolic"));
    back_contents.append(&label("Back to Discover", ""));
    back.set_child(Some(&back_contents));
    back.set_halign(gtk::Align::Start);
    back.add_css_class("flat");
    let view_stack = view_stack.clone();
    back.connect_clicked(move |_| view_stack.set_visible_child_name("discover"));
    page.append(&back);

    let top = row(12);
    top.add_css_class("ricing-detail-heading");
    let heading = column(4);
    heading.set_hexpand(true);
    heading.append(&label("Workspace / Ricing", "eyebrow"));
    let title = label("Choose a desktop setup", "page-title");
    title.set_widget_name("ricing-project-title");
    heading.append(&title);
    let identity = label("GitHub repository · Arch Linux", "subtitle");
    identity.set_widget_name("ricing-project-identity");
    heading.append(&identity);
    top.append(&heading);
    let repo_link = gtk::LinkButton::builder()
        .uri("https://github.com")
        .label("View repository ↗")
        .build();
    repo_link.add_css_class("secondary-action");
    repo_link.set_valign(gtk::Align::Center);
    repo_link.set_hexpand(false);
    repo_link.set_widget_name("ricing-repo-link");
    top.append(&repo_link);
    page.append(&top);

    let body = row(14);
    body.add_css_class("ricing-detail-layout");
    let article = column(10);
    article.set_hexpand(true);
    article.set_valign(gtk::Align::Start);
    article.add_css_class("loadout-panel");
    let gallery_header = row(10);
    gallery_header.append(&label("Repository screenshots", "section-title"));
    gallery_header.append(&label("From the project README", "tiny"));
    article.append(&gallery_header);
    gallery.add_css_class("ricing-gallery");
    gallery.set_homogeneous(false);
    gallery.set_vexpand(false);
    article.append(gallery);

    let readme_tabs = gtk::Stack::new();
    readme_tabs.set_vhomogeneous(false);
    readme_tabs.set_hexpand(true);
    readme_tabs.set_vexpand(false);
    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&readme_tabs));
    switcher.set_halign(gtk::Align::Start);
    article.append(&switcher);
    let overview = column(8);
    overview.append(&label("About this setup", "section-title"));
    let overview_text = wrapped(
        "Choose a project in Discover to load its current GitHub README, release activity, and screenshots.",
        "muted",
    );
    overview_text.set_widget_name("ricing-overview-text");
    overview.append(&overview_text);
    overview.append(&label("Project requirements", "row-title"));
    let requirement_text = wrapped("", "muted");
    requirement_text.set_widget_name("ricing-requirements");
    overview.append(&requirement_text);
    overview.append(&label("Setup impact", "row-title"));
    let impact_text = wrapped("", "notice");
    impact_text.set_widget_name("ricing-impact");
    overview.append(&impact_text);
    overview.add_css_class("github-document");
    let overview_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(420)
        .max_content_height(600)
        .child(&overview)
        .build();
    readme_tabs.add_titled(&overview_scroll, Some("overview"), "Overview");

    let readme_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(420)
        .max_content_height(600)
        .vexpand(true)
        .child(readme)
        .build();
    readme_tabs.add_titled(&readme_scroll, Some("readme"), "Components & README");
    let install_view = column(12);
    install_view.set_widget_name("ricing-install-command");
    let install_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(420)
        .max_content_height(600)
        .child(&install_view)
        .build();
    readme_tabs.add_titled(&install_scroll, Some("install"), "Installation");
    article.append(&readme_tabs);
    let fetch_status = label("README loads from GitHub when you choose a setup.", "tiny");
    fetch_status.set_widget_name("ricing-fetch-status");
    article.append(&fetch_status);
    body.append(&article);

    let aside = column(12);
    aside.add_css_class("loadout-panel");
    aside.set_width_request(340);
    aside.set_hexpand(false);
    aside.set_valign(gtk::Align::Start);
    let install_heading = row(10);
    install_heading.append(&label("Install this setup", "section-title"));

    aside.append(&install_heading);

    aside.append(&label("System compatibility", "row-title"));
    let arch = row(9);
    arch.append(&crate::tool_icons::icon(
        crate::tool_icons::Glyph::Check,
        18,
    ));
    arch.append(&label(
        if std::path::Path::new("/etc/arch-release").exists() {
            "Arch Linux · detected"
        } else {
            "Arch Linux · required, not detected"
        },
        "muted",
    ));
    aside.append(&arch);
    let desktop = row(9);
    desktop.append(&crate::tool_icons::icon(
        crate::tool_icons::Glyph::Monitor,
        18,
    ));
    desktop.append(&label(
        &format!("Detected · {}", detected_desktop()),
        "muted",
    ));
    aside.append(&desktop);
    aside.append(&wrapped(
        "Check the project README for compositor versions, GPU notes, and any extra dependencies.",
        "tiny",
    ));
    aside.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    aside.append(&label("Before you install", "row-title"));
    for text in [
        "Read the upstream requirements",
        "Back up affected configuration files",
        "Review every command in the terminal",
    ] {
        let r = row(8);
        r.append(&label("○", "muted"));
        r.append(&wrapped(text, "tiny"));
        aside.append(&r);
    }
    let review = button("Review & install", true);
    review.set_hexpand(true);
    let selected_for_install = selected.clone();
    let parent_for_install = parent.clone();
    review.connect_clicked(move |_| {
        if let Some(project) = *selected_for_install.borrow() {
            crate::tools::terminal_review(
                &parent_for_install,
                "Review dotfile setup",
                &format!(
                    "{}\n\n{}\n\nThe embedded terminal shows an editable command and requires review and fresh authentication before running.",
                    project.impact, project.description
                ),
                project.command,
            );
        }
    });
    aside.append(&review);
    let clone = button("Clone repository", false);
    clone.set_hexpand(true);
    let selected_for_clone = selected.clone();
    let parent_for_clone = parent.clone();
    clone.connect_clicked(move |_| {
        if let Some(project) = *selected_for_clone.borrow() {
            let path = format!("$HOME/.cache/archforge/ricing/{}", project.repository.replace('/', "-"));
            let command = format!(
                "git clone --depth=1 https://github.com/{}.git \"{}\"",
                project.repository, path
            );
            crate::tools::terminal_review(
                &parent_for_clone,
                "Clone dotfile repository",
                "This downloads the selected public repository into Arch Forge's cache folder. It does not run the project's installer.",
                &command,
            );
        }
    });
    aside.append(&clone);
    aside.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    body.append(&aside);
    page.append(&body);

    fetch_status.set_text("README loads from GitHub when you choose a setup.");
    (
        page,
        DetailRefs {
            title,
            identity,
            overview: overview_text,
            requirements: requirement_text,
            impact: impact_text,
            repo_link,
            install_text: install_view,
            status: fetch_status,
        },
    )
}

fn show_selection(
    panel: &gtk::Box,
    project: crate::ricing::Project,
    parent: &gtk::ApplicationWindow,
) {
    while let Some(child) = panel.first_child() {
        panel.remove(&child);
    }
    panel.append(&label(project.name, "section-title"));
    let link = gtk::LinkButton::with_label(
        &format!("https://github.com/{}", project.repository),
        &format!(
            "@{} · View on GitHub ↗",
            project.repository.split('/').next().unwrap_or_default()
        ),
    );
    link.set_halign(gtk::Align::Start);
    panel.append(&link);
    let preview = column(0);
    panel.append(&preview);
    load_preview(project, &preview, None, false);
    panel.append(&wrapped(project.description, "muted"));
    panel.append(&label("Includes", "row-title"));
    let chips = row(6);
    for name in [project.desktop, "Community dotfiles"] {
        chips.append(&label(name, "ricing-chip"));
    }
    panel.append(&chips);
    panel.append(&label("Requirements", "row-title"));
    panel.append(&wrapped(project.requirements, "muted"));
    panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    panel.append(&label("Before you install", "row-title"));
    panel.append(&wrapped(project.impact, "tiny"));
    let install = button("Review & install", true);
    let parent = parent.clone();
    install.connect_clicked(move |_| {
        crate::tools::terminal_review(
            &parent,
            "Review setup installation",
            project.impact,
            project.command,
        )
    });
    panel.append(&install);
}

fn project_card(project: crate::ricing::Project, open: impl Fn() + 'static) -> gtk::Box {
    let card = column(7);
    card.add_css_class("ricing-project-card");
    card.set_valign(gtk::Align::Start);
    let artwork = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork.add_css_class("ricing-card-art");
    card.append(&artwork);
    load_preview(project, &artwork, None, false);
    let title = label(project.name, "row-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    card.append(&title);
    card.append(&wrapped(project.description, "tiny"));
    let chips = row(6);
    chips.append(&label(project.desktop, "ricing-chip"));
    chips.append(&label(project.style, "ricing-chip"));
    card.append(&chips);
    let action = button("View setup →", false);
    action.add_css_class("ricing-view-setup");
    action.set_hexpand(true);
    action.connect_clicked(move |_| open());
    card.append(&action);
    card
}

fn backups_page() -> gtk::Box {
    let page = column(12);
    page.add_css_class("loadout-panel");
    page.append(&label("Configuration backups", "section-title"));
    page.append(&wrapped(
        "Arch Forge saves private backups before it replaces configuration files. This list reads local backup metadata only; it does not upload backup contents.",
        "muted",
    ));
    let files = crate::ricing::backup_files();
    if files.is_empty() {
        page.append(&wrapped(
            "No Arch Forge dotfile backups are present yet. A backup is created when you apply reviewed dotfiles that replace existing files.",
            "tiny",
        ));
    }
    for (name, size, _) in files {
        let line = row(12);
        line.add_css_class("ricing-backup-row");
        line.append(&crate::tool_icons::icon(
            crate::tool_icons::Glyph::Cache,
            20,
        ));
        let text = column(2);
        text.set_hexpand(true);
        text.append(&label(&name, "row-title"));
        text.append(&label(
            &format!("{:.1} KiB · local backup", size as f64 / 1024.),
            "tiny",
        ));
        line.append(&text);
        page.append(&line);
    }
    page
}

fn setups_page(setups: gtk::Widget) -> gtk::Box {
    let page = column(10);
    page.add_css_class("loadout-panel");
    page.append(&label("Arch Forge starter kits", "section-title"));
    page.append(&wrapped(
        "Small, reviewable desktop starters for Hyprland, Sway, KDE Plasma, GNOME, Xfce, and Cinnamon. Add packages and starter files to your loadout, then review dotfiles before applying them.",
        "muted",
    ));
    page.append(&setups);
    page
}

pub(crate) fn page(parent: &gtk::ApplicationWindow, setups: gtk::Widget) -> gtk::Box {
    let page = column(14);
    page.add_css_class("content");
    page.add_css_class("ricing-page");
    page.set_vexpand(true);
    let heading = row(18);
    heading.add_css_class("page-header");
    let title = column(4);
    title.set_hexpand(true);
    title.append(&label("Workspace / Ricing", "eyebrow"));
    title.append(&label("Make Arch feel like yours.", "page-title"));
    title.append(&wrapped(
        "Discover community desktop setups, inspect their files and screenshots, then decide what to bring into your Arch loadout.",
        "subtitle",
    ));
    heading.append(&title);
    let backup_button = button("Backups", false);
    let add_repo = gtk::LinkButton::builder()
        .uri("https://github.com/topics/hyprland-rice")
        .label("Browse GitHub ↗")
        .build();
    add_repo.set_valign(gtk::Align::Center);
    add_repo.set_hexpand(false);
    heading.append(&add_repo);
    heading.append(&backup_button);
    page.append(&crate::theme::banner(&heading));

    let sections = gtk::Stack::new();
    sections.set_vhomogeneous(false);
    sections.set_hexpand(true);
    sections.set_vexpand(true);
    let tabs = gtk::StackSwitcher::new();
    tabs.set_stack(Some(&sections));
    tabs.set_halign(gtk::Align::Start);
    page.append(&tabs);
    let system_strip = status_strip();
    page.append(&system_strip);

    let discovery = column(10);
    let discover_view = gtk::Stack::new();
    discover_view.set_vhomogeneous(false);
    discover_view.set_hexpand(true);
    discover_view.set_vexpand(true);
    let selected = Rc::new(RefCell::new(None::<crate::ricing::Project>));
    let readme = column(12);
    let gallery = column(8);
    gallery.append(&wrapped(
        "Screenshots linked from the project's README appear here after you open a setup.",
        "tiny",
    ));
    let (detail, detail_refs) = detail_page(parent, &discover_view, &selected, &readme, &gallery);
    let detail_status = detail_refs.status.clone();
    discover_view.add_named(&discovery, Some("discover"));
    discover_view.add_named(&detail, Some("detail"));
    discover_view.set_visible_child_name("discover");
    let discover_heading = heading.clone();
    let discover_tabs = tabs.clone();
    discover_view.connect_visible_child_name_notify(move |stack| {
        let visible = stack.visible_child_name().as_deref() == Some("discover");
        discover_heading.set_visible(visible);
        discover_tabs.set_visible(visible);
        system_strip.set_visible(visible);
    });

    let filters = row(9);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search setups, creators, or desktops…")
        .hexpand(true)
        .build();
    let desktop_filter = gtk::DropDown::from_strings(&["All desktops", "Hyprland"]);
    let style_filter = gtk::DropDown::from_strings(&["All styles", "Full setups", "Shell add-ons"]);
    filters.append(&search);
    filters.append(&desktop_filter);
    filters.append(&style_filter);
    discovery.append(&filters);
    let ranking = row(8);
    ranking.append(&crate::tool_icons::icon(
        crate::tool_icons::Glyph::Network,
        18,
    ));
    ranking.append(&wrapped(
        "Community desktop setups · real repository screenshots",
        "tiny",
    ));
    discovery.append(&ranking);

    let split = row(12);
    split.add_css_class("ricing-discover-layout");
    let cards_panel = column(8);
    cards_panel.add_css_class("ricing-card-grid-panel");
    cards_panel.set_hexpand(true);
    let cards = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(2)
        .column_spacing(10)
        .row_spacing(10)
        .homogeneous(true)
        .build();
    cards.set_widget_name("ricing-project-cards");
    cards_panel.append(&cards);
    split.append(&cards_panel);
    let selected_panel = column(10);
    selected_panel.add_css_class("loadout-panel");
    selected_panel.set_width_request(320);
    selected_panel.set_valign(gtk::Align::Start);
    show_selection(&selected_panel, crate::ricing::PROJECTS[0], parent);
    let selected_scroll = gtk::ScrolledWindow::builder()
        .child(&selected_panel)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(320)
        .max_content_width(320)
        .propagate_natural_width(true)
        .min_content_height(500)
        .max_content_height(780)
        .propagate_natural_height(true)
        .valign(gtk::Align::Start)
        .build();
    split.append(&selected_scroll);
    discovery.append(&split);

    let rerender = {
        let cards = cards.clone();
        let search = search.clone();
        let desktop_filter = desktop_filter.clone();
        let style_filter = style_filter.clone();
        let selected = selected.clone();
        let discover_view = discover_view.clone();
        let detail_status = detail_status.clone();
        let readme = readme.clone();
        let gallery = gallery.clone();
        let parent = parent.clone();
        Rc::new(move || {
            while let Some(child) = cards.first_child() {
                cards.remove(&child);
            }
            let needle = search.text().to_lowercase();
            for project in crate::ricing::PROJECTS {
                if desktop_filter.selected() > 0 && project.desktop != "Hyprland" {
                    continue;
                }
                let style_idx = style_filter.selected();
                if style_idx > 0
                    && match style_idx {
                        1 => !project.style.to_lowercase().contains("full"),
                        2 => !project.style.to_lowercase().contains("shell"),
                        _ => false,
                    }
                {
                    continue;
                }
                let searchable = format!(
                    "{} {} {} {} {}",
                    project.name,
                    project.repository,
                    project.desktop,
                    project.style,
                    project.description
                )
                .to_lowercase();
                if !searchable.contains(&needle) {
                    continue;
                }
                let project = *project;
                let selected = selected.clone();
                let discover_view = discover_view.clone();
                let detail_status = detail_status.clone();
                let readme = readme.clone();
                let gallery = gallery.clone();
                let detail_refs = detail_refs.clone();
                let card = project_card(project, move || {
                    *selected.borrow_mut() = Some(project);

                    discover_view.set_visible_child_name("detail");
                    detail_status.set_text(
                        "Fetching repository details, README, and screenshots from GitHub…",
                    );

                    crate::ricing_document::render(
                        &readme,
                        "Loading the upstream README…",
                        project,
                    );
                    detail_refs.title.set_text(project.name);
                    detail_refs.identity.set_text(&format!(
                        "@{} · {} · {}",
                        project.repository.split('/').next().unwrap_or_default(),
                        project.desktop,
                        project.style
                    ));
                    detail_refs.overview.set_text(project.description);
                    detail_refs.requirements.set_text(project.requirements);
                    detail_refs.impact.set_text(project.impact);
                    detail_refs
                        .repo_link
                        .set_uri(&format!("https://github.com/{}", project.repository));
                    crate::ricing_document::render(
                        &detail_refs.install_text,
                        &crate::ricing::installation_document(project),
                        project,
                    );
                    detail_status
                        .set_text("Repository previews · successful downloads are cached locally");
                    load_preview(project, &gallery, Some(readme.clone()), true);
                });
                let pick = gtk::GestureClick::new();
                let selection_panel = selected_panel.clone();
                let parent = parent.clone();
                pick.connect_released(move |_, _, _, _| {
                    show_selection(&selection_panel, project, &parent)
                });
                card.add_controller(pick);
                cards.insert(&card, -1);
            }
            if cards.first_child().is_none() {
                cards.insert(
                    &wrapped(
                        "No setups match these filters. Try All desktops or clear the search.",
                        "muted",
                    ),
                    -1,
                );
            }
        })
    };
    rerender();
    for widget in [
        search.clone().upcast::<gtk::Widget>(),
        desktop_filter.clone().upcast(),
        style_filter.clone().upcast(),
    ] {
        let render = rerender.clone();
        if let Some(entry) = widget.downcast_ref::<gtk::SearchEntry>() {
            entry.connect_search_changed(move |_| render());
        } else if let Some(dropdown) = widget.downcast_ref::<gtk::DropDown>() {
            dropdown.connect_selected_notify(move |_| render());
        }
    }
    sections.add_titled(&discover_view, Some("discover"), "Discover");
    sections.add_titled(&setups_page(setups), Some("my-setups"), "My setups");
    sections.add_titled(&backups_page(), Some("backups"), "Backups");
    let sections_for_button = sections.clone();
    backup_button.connect_clicked(move |_| sections_for_button.set_visible_child_name("backups"));
    page.append(&sections);
    page
}

#[cfg(test)]
mod gallery_checks {
    use super::*;
    fn walk(w: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut out = vec![w.clone()];
        let mut child = w.first_child();
        while let Some(c) = child {
            out.extend(walk(&c));
            child = c.next_sibling();
        }
        out
    }
    fn wait(mut condition: impl FnMut() -> bool) {
        let start = std::time::Instant::now();
        loop {
            while glib::MainContext::default().pending() {
                glib::MainContext::default().iteration(false);
            }
            if condition() {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(80),
                "Ricing UI did not settle"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    #[test]
    #[ignore]
    fn ricing_gallery_states() {
        gtk::init().unwrap();
        let provider = gtk::CssProvider::new();
        crate::theme::apply(&provider, "amber");
        let app = gtk::Application::builder()
            .application_id("io.github.zynokss.RicingCheck")
            .build();
        app.register(None::<&gtk::gio::Cancellable>).unwrap();
        let window = gtk::ApplicationWindow::builder()
            .application(&app)
            .title("Ricing verification")
            .default_width(1586)
            .default_height(992)
            .build();
        let shell = row(0);
        let sidebar = column(12);
        sidebar.add_css_class("sidebar");
        sidebar.set_width_request(220);
        sidebar.append(&label("Arch Forge", "section-title"));
        for name in [
            "Overview",
            "Tools",
            "Gaming",
            "Cleanup",
            "Packages",
            "Diagnostics",
            "Loadouts",
            "Ricing",
        ] {
            sidebar.append(&button(name, name == "Ricing"));
        }
        shell.append(&sidebar);
        let page = page(&window, column(0).upcast());
        let scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .child(&page)
            .build();
        shell.append(&scroll);
        window.set_child(Some(&shell));
        window.present();
        wait(|| window.width() > 0);
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-discover-loading");
        wait(|| {
            walk(page.upcast_ref())
                .iter()
                .filter(|w| w.is_visible())
                .filter_map(|w| w.downcast_ref::<gtk::Label>())
                .all(|l| l.text() != "Loading screenshots…")
        });
        assert!(
            walk(page.upcast_ref())
                .iter()
                .any(|w| w.is::<gtk::Picture>())
        );
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-discover-success");
        let view = walk(page.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.label().as_deref() == Some("View setup →"))
            .unwrap();
        view.emit_clicked();
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-detail-loading");
        wait(|| {
            walk(page.upcast_ref())
                .iter()
                .any(|w| w.has_css_class("ricing-thumbnail"))
        });
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-detail-success");
        let documents = walk(page.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Stack>().ok())
            .find(|stack| stack.child_by_name("readme").is_some())
            .unwrap();
        documents.set_visible_child_name("readme");
        wait(|| {
            walk(page.upcast_ref())
                .iter()
                .any(|w| w.is::<gtk::Expander>())
        });
        assert!(
            !walk(page.upcast_ref())
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Label>())
                .any(|l| l.text().contains("<h1>"))
        );
        scroll.vadjustment().set_value(600.0);
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-readme-github");
        if let Some(expander) = walk(page.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Expander>().ok())
            .next()
        {
            expander.set_expanded(true);
        }
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-readme-expanded");
        documents.set_visible_child_name("install");
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-installation-github");
        documents.set_visible_child_name("overview");
        let next = walk(page.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.label().as_deref() == Some("›"))
            .unwrap();
        next.emit_clicked();
        let failed = crate::ricing::Project {
            repository: "archforge-test/no-such-repository",
            ..crate::ricing::PROJECTS[0]
        };
        let gallery = walk(page.upcast_ref())
            .into_iter()
            .find(|w| w.has_css_class("ricing-gallery"))
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        load_preview(failed, &gallery, None, true);
        wait(|| {
            walk(gallery.upcast_ref())
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Button>())
                .any(|b| b.label().as_deref() == Some("Retry"))
        });
        assert!(
            !walk(gallery.upcast_ref())
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Label>())
                .any(|l| l.text().contains("Loading"))
        );
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-detail-failure");
        let back = walk(page.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| {
                walk(b.upcast_ref())
                    .iter()
                    .filter_map(|w| w.downcast_ref::<gtk::Label>())
                    .any(|l| l.text() == "Back to Discover")
            })
            .unwrap();
        back.emit_clicked();
        let art = walk(page.upcast_ref())
            .into_iter()
            .find(|w| w.has_css_class("ricing-card-art"))
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        load_preview(failed, &art, None, false);
        wait(|| {
            walk(art.upcast_ref())
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::Button>())
                .any(|b| b.label().as_deref() == Some("Retry"))
        });
        crate::dialogs::snapshot(window.upcast_ref(), "ricing-discover-failure");
        window.close();
    }
}
