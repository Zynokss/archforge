use crate::{button, column, jobs::Job, label, row, symbol, wrapped};
use gtk::{glib, prelude::*};
use std::{cell::Cell, rc::Rc, sync::mpsc, time::Duration};

fn line_icon(kind: &'static str, size: i32) -> gtk::Widget {
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_valign(gtk::Align::Center);
    area.add_css_class("line-icon");
    area.set_accessible_role(gtk::AccessibleRole::Presentation);
    area.set_draw_func(move |area, ctx, width, height| {
        let color = area.color();
        ctx.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            color.alpha() as f64,
        );
        ctx.scale(width as f64 / 24.0, height as f64 / 24.0);
        ctx.set_line_width(1.7);
        ctx.set_line_cap(gtk::cairo::LineCap::Round);
        ctx.set_line_join(gtk::cairo::LineJoin::Round);
        if kind == "package" {
            ctx.move_to(12.0, 2.5);
            for (x, y) in [
                (21.0, 7.5),
                (21.0, 17.0),
                (12.0, 22.0),
                (3.0, 17.0),
                (3.0, 7.5),
            ] {
                ctx.line_to(x, y);
            }
            ctx.close_path();
            ctx.move_to(3.0, 7.5);
            ctx.line_to(12.0, 12.5);
            ctx.line_to(21.0, 7.5);
            ctx.move_to(12.0, 12.5);
            ctx.line_to(12.0, 22.0);
            ctx.move_to(7.5, 5.0);
            ctx.line_to(16.5, 10.0);
            ctx.line_to(16.5, 14.0);
        } else if kind == "storage" {
            ctx.move_to(4.0, 5.0);
            ctx.curve_to(4.0, 1.0, 20.0, 1.0, 20.0, 5.0);
            ctx.curve_to(20.0, 9.0, 4.0, 9.0, 4.0, 5.0);
            ctx.move_to(4.0, 5.0);
            ctx.line_to(4.0, 18.5);
            ctx.curve_to(4.0, 22.5, 20.0, 22.5, 20.0, 18.5);
            ctx.line_to(20.0, 5.0);
            ctx.move_to(4.0, 11.5);
            ctx.curve_to(4.0, 15.5, 20.0, 15.5, 20.0, 11.5);
        }
        if kind == "terminal" {
            ctx.new_path();
            ctx.move_to(4.0, 6.0);
            ctx.line_to(9.5, 12.0);
            ctx.line_to(4.0, 18.0);
            ctx.move_to(12.0, 18.0);
            ctx.line_to(20.0, 18.0);
        }
        let _ = ctx.stroke();
    });
    area.upcast()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Busy,
    Ready,
    Success,
    Warning,
    Error,
}

#[derive(Clone)]
pub(crate) struct Status {
    pub title: gtk::Label,
    pub detail: gtk::Label,
    pub spinner: gtk::Spinner,
    emblem: gtk::Box,
    image: gtk::Image,
    footer_icon: gtk::Image,
    footer_spinner: gtk::Spinner,
    compact: Rc<Cell<bool>>,
    tone: Rc<Cell<Tone>>,
    footer: gtk::Label,
}
impl Status {
    pub fn set(&self, tone: Tone, title: &str, detail: &str, footer: &str) {
        let (class, icon) = match tone {
            Tone::Busy => ("busy", "view-refresh-symbolic"),
            Tone::Ready => ("ready", "dialog-information-symbolic"),
            Tone::Success => ("success", "object-select-symbolic"),
            Tone::Warning => ("warning", "dialog-warning-symbolic"),
            Tone::Error => ("error", "dialog-error-symbolic"),
        };
        for class in ["busy", "ready", "success", "warning", "error"] {
            self.emblem.remove_css_class(class);
        }
        self.emblem.add_css_class(class);
        for name in ["error", "warning"] {
            self.title.remove_css_class(name);
            self.footer_icon.remove_css_class(name);
        }
        if matches!(tone, Tone::Error | Tone::Warning) {
            self.title.add_css_class(class);
            self.footer_icon.add_css_class(class);
        }
        for name in ["busy", "ready", "success", "warning", "error"] {
            self.footer_icon.remove_css_class(name);
        }
        self.footer_icon.add_css_class(class);
        self.tone.set(tone);
        self.refresh_emblem();
        let busy = matches!(tone, Tone::Busy);
        self.image.set_icon_name(Some(icon));
        self.image.set_visible(!busy);
        self.spinner.set_visible(busy);
        self.spinner.set_spinning(busy);
        self.footer_icon.set_icon_name(Some(icon));
        self.title.set_text(title);
        self.detail.set_text(detail);
        self.footer.set_text(footer);
    }
    fn set_compact(&self, compact: bool) {
        self.compact.set(compact);
        self.refresh_emblem();
    }
    fn refresh_emblem(&self) {
        let busy = self.tone.get() == Tone::Busy;
        self.emblem
            .set_visible(!self.compact.get() && self.tone.get() != Tone::Ready);
        self.footer_spinner.set_visible(self.compact.get() && busy);
        self.footer_spinner.set_spinning(self.compact.get() && busy);
        self.footer_icon.set_visible(!(self.compact.get() && busy));
    }
}

pub(crate) struct Surface {
    pub content: gtk::Box,
    workspace: gtk::Box,
    pub actions: gtk::Box,
    pub status: Status,
}
impl Surface {
    pub fn new(
        parent: &gtk::ApplicationWindow,
        title: &str,
        subtitle: &str,
        glyph: &str,
        icon: &str,
    ) -> (gtk::Window, Self) {
        let window = gtk::Window::builder()
            .title(title)
            .transient_for(parent)
            .modal(true)
            .destroy_with_parent(true)
            .default_width(980)
            .default_height(590)
            .build();
        window.add_css_class("forge-dialog");
        if let Some(surface) = parent.surface()
            && let Some(monitor) =
                gtk::prelude::WidgetExt::display(parent).monitor_at_surface(&surface)
        {
            let area = monitor.geometry();
            window.set_default_size(980.min(area.width() - 48), 590.min(area.height() - 64));
        }
        let header = row(16);
        header.add_css_class("dialog-header");
        let badge = row(0);
        badge.add_css_class("dialog-badge");
        badge.set_valign(gtk::Align::Start);
        let image = if icon == "package-x-generic-symbolic" {
            line_icon("package", 28)
        } else {
            symbol(glyph, icon)
        };
        image.set_halign(gtk::Align::Center);
        image.set_valign(gtk::Align::Center);
        badge.append(&image);
        header.append(&badge);
        let text = column(6);
        text.set_hexpand(true);
        text.append(&wrapped(title, "dialog-title"));
        text.append(&wrapped(subtitle, "dialog-subtitle"));
        header.append(&text);
        let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
        dismiss.add_css_class("dialog-dismiss");
        dismiss.set_valign(gtk::Align::Start);
        dismiss.set_tooltip_text(Some("Close"));
        dismiss.update_property(&[gtk::accessible::Property::Label("Close dialog")]);
        let weak = window.downgrade();
        dismiss.connect_clicked(move |_| {
            if let Some(w) = weak.upgrade() {
                w.close();
            }
        });
        header.append(&dismiss);
        let handle = gtk::WindowHandle::new();
        handle.set_child(Some(&header));
        window.set_titlebar(Some(&handle));
        let content = column(20);
        content.add_css_class("dialog-content");
        let hero = column(0);
        hero.add_css_class("dialog-hero");
        let emblem = column(0);
        emblem.add_css_class("dialog-emblem");
        emblem.set_halign(gtk::Align::Center);
        emblem.set_vexpand(false);
        let image = gtk::Image::from_icon_name("dialog-information-symbolic");
        image.set_pixel_size(30);
        image.set_halign(gtk::Align::Center);
        image.set_vexpand(false);
        emblem.append(&image);
        let spinner = gtk::Spinner::new();
        spinner.set_size_request(28, 28);
        spinner.set_halign(gtk::Align::Center);
        spinner.set_valign(gtk::Align::Center);
        spinner.set_vexpand(false);
        emblem.append(&spinner);
        hero.append(&emblem);
        let headline = wrapped("Preparing…", "dialog-headline");
        headline.set_xalign(0.5);
        headline.set_justify(gtk::Justification::Center);
        hero.append(&headline);
        let description = wrapped("", "dialog-description");
        description.set_xalign(0.5);
        description.set_justify(gtk::Justification::Center);
        hero.append(&description);
        content.append(&hero);
        let footer = row(14);
        footer.add_css_class("dialog-footer");
        let hint = row(9);
        hint.set_hexpand(true);
        let footer_spinner = gtk::Spinner::new();
        footer_spinner.set_size_request(16, 16);
        hint.append(&footer_spinner);
        let footer_icon = gtk::Image::from_icon_name("dialog-information-symbolic");
        footer_icon.set_pixel_size(16);
        hint.append(&footer_icon);
        let footer_text = label("No changes made", "dialog-footer-hint");
        footer_text.set_wrap(false);
        footer_text.set_hexpand(true);
        footer_text.set_ellipsize(gtk::pango::EllipsizeMode::End);
        footer_text.set_max_width_chars(26);
        hint.append(&footer_text);
        footer.append(&hint);
        let actions = row(10);
        actions.set_valign(gtk::Align::Center);
        footer.append(&actions);
        let workspace = row(20);
        workspace.add_css_class("dialog-workspace");
        workspace.set_vexpand(true);
        content.set_hexpand(true);
        workspace.append(&content);
        let root = column(0);
        root.append(&workspace);
        root.append(&footer);
        window.set_child(Some(&root));
        let keys = gtk::EventControllerKey::new();
        let weak = window.downgrade();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                if let Some(w) = weak.upgrade() {
                    w.close();
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(keys);
        let status = Status {
            title: headline,
            detail: description,
            spinner,
            emblem,
            image,
            footer_icon,
            footer_spinner,
            compact: Rc::new(Cell::new(false)),
            tone: Rc::new(Cell::new(Tone::Busy)),
            footer: footer_text,
        };
        status.set(
            Tone::Busy,
            "Preparing…",
            "Reading the information for this action.",
            "No changes made",
        );
        (
            window,
            Self {
                content,
                workspace,
                actions,
                status,
            },
        )
    }
    pub fn stats(&self, first: (&str, &str, &str), second: (&str, &str, &str)) -> Stats {
        let box_ = row(0);
        box_.add_css_class("dialog-stats");
        let mut values = Vec::new();
        let mut captions = Vec::new();
        for (i, (caption, value, icon)) in [first, second].into_iter().enumerate() {
            if i > 0 {
                let divider = gtk::Separator::new(gtk::Orientation::Vertical);
                divider.set_margin_top(0);
                divider.set_margin_bottom(0);
                box_.append(&divider);
            }
            let cell = row(14);
            cell.add_css_class("dialog-stat");
            cell.set_hexpand(true);
            cell.set_homogeneous(false);
            let image: gtk::Widget = if icon == "package-x-generic-symbolic" {
                line_icon("package", 22)
            } else if icon == "drive-harddisk-symbolic" {
                line_icon("storage", 22)
            } else {
                let image = gtk::Image::from_icon_name(icon);
                image.set_pixel_size(22);
                image.upcast()
            };
            cell.append(&image);
            let text = column(3);
            text.set_hexpand(true);
            let caption = label(caption, "dialog-stat-caption");
            text.append(&caption);
            captions.push(caption);
            let value = wrapped(value, "dialog-stat-value");
            value.set_max_width_chars(24);
            text.append(&value);
            cell.append(&text);
            box_.append(&cell);
            values.push(value);
        }
        self.content.append(&box_);
        Stats {
            values: values.try_into().expect("Two summary values"),
            captions: captions.try_into().expect("Two summary captions"),
        }
    }
    pub fn details(&self, title: &str, subtitle: &str, command: Option<&str>) -> Details {
        self.workspace.set_orientation(gtk::Orientation::Vertical);
        self.workspace.add_css_class("output-workspace");
        self.workspace.set_spacing(12);
        self.content.set_orientation(gtk::Orientation::Vertical);
        let summary = row(16);
        summary.add_css_class("output-summary");
        let mut widget = self.content.first_child();
        while let Some(child) = widget {
            widget = child.next_sibling();
            if child.has_css_class("dialog-hero") || child.has_css_class("dialog-stats") {
                self.content.remove(&child);
                summary.append(&child);
            }
        }
        self.content.prepend(&summary);
        self.content.set_hexpand(true);
        self.content.set_size_request(-1, -1);
        self.status.set_compact(true);
        self.status.title.set_xalign(0.0);
        self.status.detail.set_xalign(0.0);
        self.status.detail.set_justify(gtk::Justification::Left);
        self.status.title.set_max_width_chars(25);
        self.status.detail.set_max_width_chars(55);
        let expander = gtk::Expander::new(None);
        expander.add_css_class("dialog-details");
        let status = self.status.clone();
        expander.connect_expanded_notify(move |e| {
            if !e.is_expanded() {
                e.set_expanded(true);
            }
            status.set_compact(true);
        });
        expander.set_expanded(true);
        expander.set_hexpand(true);
        expander.set_vexpand(true);
        let heading = row(12);
        heading.set_hexpand(true);
        heading.append(&line_icon("terminal", 22));
        let text = column(3);
        text.set_hexpand(true);
        let heading_title = label(title, "row-title");
        let heading_subtitle = wrapped(subtitle, "muted");
        text.append(&heading_title);
        text.append(&heading_subtitle);
        heading.append(&text);
        heading.add_css_class("console-heading");
        let child = column(12);
        child.add_css_class("dialog-details-content");
        heading.set_visible(false);
        child.append(&heading);
        let form = column(8);
        form.set_visible(false);
        child.append(&form);
        let output = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(12)
            .right_margin(12)
            .top_margin(12)
            .bottom_margin(12)
            .build();
        output.add_css_class("console");
        let viewer = crate::command_output::pane(&output, command, true);
        child.append(&viewer);
        let tools = row(10);
        tools.connect_map(|tools| {
            let mut child = tools.first_child();
            while let Some(widget) = child {
                if let Some(b) = widget.downcast_ref::<gtk::Button>()
                    && b.label().is_some_and(|s| {
                        matches!(s.as_str(), "Copy output" | "Copy command" | "Save output…")
                    })
                {
                    b.set_visible(false);
                }
                child = widget.next_sibling();
            }
        });
        child.append(&tools);
        expander.set_child(Some(&child));
        // Keep the native container for existing observers, with its disclosure
        // header hidden: the console is always visible and scrolls independently.
        if let Some(title) = expander.first_child().and_then(|b| b.first_child()) {
            title.set_visible(false);
        }
        self.workspace.append(&expander);
        let buffer = output.buffer();
        style_output(&buffer);
        Details {
            form,
            expander,
            output,
            tools,
            title: heading_title,
            subtitle: heading_subtitle,
        }
    }
    pub fn close_button(&self, window: &gtk::Window, text: &str, primary: bool) -> gtk::Button {
        let close = button(text, primary);
        let weak = window.downgrade();
        close.connect_clicked(move |_| {
            if let Some(w) = weak.upgrade() {
                w.close();
            }
        });
        self.actions.append(&close);
        window.set_default_widget(Some(&close));
        gtk::prelude::GtkWindowExt::set_focus(window, Some(&close));
        close
    }
}
pub(crate) fn style_output(buffer: &gtk::TextBuffer) {
    let table = buffer.tag_table();
    for (name, color) in [
        ("log-good", "#86d694"),
        ("log-warn", "#f4c16f"),
        ("log-error", "#ff858b"),
        ("log-heading", "#edbd65"),
    ] {
        let tag = gtk::TextTag::builder().name(name).foreground(color).build();
        table.add(&tag);
    }
    buffer.connect_changed(|buffer| {
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            let lower = line.to_lowercase();
            let tag = if lower.trim() == "failed services"
                || line.starts_with("$ ")
                || line.starts_with("/usr/bin/")
                || line.starts_with("Arch Forge system report")
            {
                Some("log-heading")
            } else if lower.contains("0 loaded units listed")
                || lower.contains("no failed")
                || lower.contains("0 failed")
            {
                Some("log-good")
            } else if lower.contains("error")
                || lower.contains("failed")
                || lower.contains("permission denied")
            {
                Some("log-error")
            } else if lower.contains("warning") || lower.contains("partial") {
                Some("log-warn")
            } else if lower.contains("success")
                || lower.contains("finished")
                || lower.contains("no candidate")
                || lower.contains("passed")
            {
                Some("log-good")
            } else if line.starts_with("==")
                || line.starts_with("::")
                || line.starts_with('#')
                || line.trim() == "VULKANINFO"
                || line.contains("Extensions:")
                || line.contains("Layers:")
                || line.starts_with("GPU")
            {
                Some("log-heading")
            } else {
                None
            };
            let end = offset + line.chars().count() as i32;
            if let Some(tag) = tag {
                buffer.apply_tag_by_name(
                    tag,
                    &buffer.iter_at_offset(offset),
                    &buffer.iter_at_offset(end),
                );
            }
            offset = end;
        }
    });
}

pub(crate) struct Stats {
    pub values: [gtk::Label; 2],
    pub captions: [gtk::Label; 2],
}
impl std::ops::Deref for Stats {
    type Target = [gtk::Label; 2];
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

pub(crate) struct Details {
    pub form: gtk::Box,
    pub title: gtk::Label,
    pub subtitle: gtk::Label,
    pub expander: gtk::Expander,
    pub output: gtk::TextView,
    pub tools: gtk::Box,
}

#[derive(Debug, PartialEq)]
pub(crate) struct CacheSummary {
    pub count: u64,
    pub space: String,
}
pub(crate) fn cache_summary(text: &str) -> Option<CacheSummary> {
    if text.contains("no candidate packages found for pruning") {
        return Some(CacheSummary {
            count: 0,
            space: "0 B".into(),
        });
    }
    let line = text.lines().find(|l| l.contains("disk space saved:"))?;
    let count = line
        .split_once(" candidates")
        .or_else(|| line.split_once(" packages removed"))
        .and_then(|(prefix, _)| prefix.split_whitespace().last()?.parse().ok())?;
    let space = line
        .split_once("disk space saved:")?
        .1
        .split(')')
        .next()?
        .trim()
        .to_string();
    if space.is_empty() {
        return None;
    }
    Some(CacheSummary { count, space })
}

fn package_count(count: u64) -> String {
    format!(
        "{count} {}",
        if count == 1 { "package" } else { "packages" }
    )
}

struct JobView {
    job: Job,
    status: Status,
    details: Details,
    stats: Stats,
    copy: gtk::Button,
    authorize: gtk::Button,
    retry: gtk::Button,
    close: gtk::Button,
    parent: glib::WeakRef<gtk::ApplicationWindow>,
}
impl JobView {
    fn preview(&self, result: Result<String, String>) {
        match result {
            Ok(text) => {
                self.details.output.buffer().set_text(&text);
                self.copy.set_sensitive(true);
                if let Some(summary) = cache_summary(&text) {
                    self.stats[0].set_text(&package_count(summary.count));
                    self.stats[1].set_text(&summary.space);
                    if summary.count == 0 {
                        self.status.set(
                            Tone::Success,
                            if matches!(self.job, Job::UninstalledClean | Job::UninstalledPreview) { "No uninstalled archives to remove" } else { "Your package cache is already tidy" },
                            if matches!(self.job, Job::UninstalledClean | Job::UninstalledPreview) { "No cached versions of uninstalled packages are eligible for removal." } else { "No older package versions are eligible for removal." },
                            "No changes needed",
                        );
                        self.authorize.set_visible(false);
                        self.authorize.set_sensitive(false);
                        self.done();
                        return;
                    }
                } else if matches!(self.job, Job::JournalClean) {
                    self.stats[0].set_text("7 days");
                    self.stats[1].set_text(&journal_usage(&text));
                } else {
                    self.stats[0].set_text("See details");
                    self.stats[1].set_text("Not reported");
                }
                self.status.set(Tone::Ready, "Ready for your review", if matches!(self.job,Job::JournalClean) { "Only archived journal files older than seven days are removed. Active journals are kept." } else { if matches!(self.job,Job::UninstalledClean) { "Review the archives below. All cached versions of packages no longer installed are eligible for removal." } else { "Review the archives alongside before confirming. The latest two cached versions of each package are kept." } }, "Awaiting confirmation");
                if !self.job.destructive() {
                    self.status.set(Tone::Ready, "Your cache preview is ready", "This is a read-only preview. Review the listed archives and their reported size below.", "Preview only");
                }
                self.details.expander.set_expanded(true);
                self.authorize.set_sensitive(true);
            }
            Err(error) => {
                self.details.output.buffer().set_text(&error);
                self.copy.set_sensitive(true);
                self.status.set(Tone::Error,"Could not preview this cleanup", "No files were changed. Read the output for the error and try again after resolving it.", "No changes made");
                self.authorize.set_sensitive(false);
                self.authorize.set_visible(false);
                self.details.expander.set_expanded(true);
                self.done();
            }
        }
    }
    fn done(&self) {
        self.close.set_label("Done");
        self.close.add_css_class("primary");
    }
    fn run(self: &Rc<Self>) {
        if self.job.privileged() {
            if let Some(parent) = self.parent.upgrade() {
                match self.job.command() {
                    Ok((program, args)) => {
                        let command = if program == "/usr/bin/pkexec" {
                            format!(
                                "sudo {}",
                                args.iter()
                                    .map(|a| crate::package_cache::quote(a))
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            )
                        } else {
                            self.job.display_command()
                        };
                        crate::embedded_terminal::show(
                            parent.upcast_ref(),
                            self.job.title(),
                            self.job.confirmation(),
                            &command,
                        );
                        if let Some(window) = self
                            .details
                            .output
                            .root()
                            .and_then(|r| r.downcast::<gtk::Window>().ok())
                        {
                            window.close();
                        }
                    }
                    Err(error) => self.status.set(
                        Tone::Error,
                        "Could not prepare command",
                        &error,
                        "Nothing has run",
                    ),
                }
            }
            return;
        }
        self.status.set(
            Tone::Busy,
            if self.job.destructive() {
                "Cleaning up…"
            } else {
                "Running your check…"
            },
            "You can close this dialog; the operation will continue.",
            "Working…",
        );
        self.authorize.set_sensitive(false);
        self.copy.set_sensitive(false);
        self.details.output.buffer().set_text("");
        let job = self.job;
        let (tx, rx) = mpsc::channel();
        let operation = crate::Operation::new();
        std::thread::spawn(move || {
            let _operation = operation;
            let _ = tx.send(job.execute());
        });
        let view = self.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let result = match rx.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("The command worker stopped unexpectedly.".into()),
            };
            view.finished(result);
            glib::ControlFlow::Break
        });
    }
    fn finished(&self, result: Result<String, String>) {
        if let Some(parent) = self.parent.upgrade() {
            crate::update_cache_summary(&parent, self.job, &result);
        }
        self.authorize.set_visible(false);
        self.copy.set_sensitive(true);
        self.done();
        match result {
            Ok(text) => {
                if self.job.destructive() {
                    self.details.expander.set_expanded(false);
                }
                self.details
                    .output
                    .buffer()
                    .set_text(if text.trim().is_empty() {
                        "Command completed without output."
                    } else {
                        &text
                    });
                if let Some(summary) = cache_summary(&text) {
                    self.stats[0].set_text(&package_count(summary.count));
                    self.stats[1].set_text(&summary.space);
                }
                if matches!(self.job, Job::CacheClean | Job::UninstalledClean) {
                    self.stats.captions[0].set_text("Archives processed");
                    self.stats.captions[1].set_text("Space reclaimed");
                    if cache_summary(&text).is_none() {
                        self.stats[0].set_text("Not reported");
                        self.stats[1].set_text("Not reported");
                    }
                } else if matches!(self.job, Job::JournalClean) {
                    self.stats.captions[1].set_text("Outcome");
                    self.stats[1].set_text("Completed");
                }
                let (headline, detail) = if self.job.destructive() {
                    (
                        "Cleanup completed",
                        "The command finished successfully. You can review or copy the output below.",
                    )
                } else if text.starts_with("No orphan packages") {
                    self.stats[0].set_text("0 packages");
                    (
                        "No orphan packages found",
                        "No unused dependency packages were reported by pacman.",
                    )
                } else if text.starts_with("No repository updates") {
                    (
                        "Your repositories are up to date",
                        "No package updates are available from your configured repositories.",
                    )
                } else if text.starts_with("No foreign packages") {
                    (
                        "No foreign packages found",
                        "All installed packages are present in your configured repositories.",
                    )
                } else {
                    (
                        "Your results are ready",
                        "The check completed successfully. The full output is available alongside.",
                    )
                };
                self.status
                    .set(Tone::Success, headline, detail, "Completed");
                if matches!(self.job, Job::Failed) {
                    let count = text
                        .lines()
                        .filter(|line| line.split_whitespace().any(|word| word == "failed"))
                        .count();
                    self.stats[0].set_text(&format!(
                        "{count} {}",
                        if count == 1 { "unit" } else { "units" }
                    ));
                    self.status.set(if count > 0 { Tone::Warning } else { Tone::Success }, if count > 0 { "Some system units need attention" } else { "No failed system units" }, if count > 0 { "Systemd reports failed units. Review Technical details to see which units failed." } else { "Systemd did not report any failed system units in this check." }, if count > 0 { "Review failed units" } else { "Check completed" });
                } else if matches!(self.job, Job::Updates) {
                    let count = text.lines().filter(|line| line.contains(" -> ")).count();
                    if text.starts_with("No repository updates") {
                        self.stats[0].set_text("0 updates");
                    } else if count > 0 {
                        self.stats[0].set_text(&format!(
                            "{count} {}",
                            if count == 1 { "update" } else { "updates" }
                        ));
                        self.status.set(Tone::Ready, "Repository updates are available", "This is a read-only check. Read the output to review the available package versions.", "Review available updates");
                    } else {
                        self.stats[0].set_text("See details");
                    }
                }
                let explanation = match self.job {
                    Job::CacheSize | Job::CacheSizeAdmin => Some((
                        "Package cache measured",
                        "Downloaded package archives use the space shown below. This check does not remove files.",
                    )),
                    Job::JournalSize => Some((
                        "Journal usage measured",
                        "This includes active and archived journals. Cleanup only removes eligible archived logs.",
                    )),
                    Job::Boot => Some((
                        "Startup timings collected",
                        "The slowest unit is shown below. Units can start in parallel, so these timings do not add up to total boot time.",
                    )),
                    Job::Vulkan => Some((
                        "Vulkan information collected",
                        "The first reported GPU and API version are shown below. The output lists all detected devices and drivers.",
                    )),
                    Job::Report => Some((
                        "System report generated",
                        "Memory and storage usage are shown below. Review the full report before sharing it.",
                    )),
                    Job::GameMode => Some((
                        "GameMode self-test completed",
                        "The test command finished successfully. Review the individual checks in the output.",
                    )),
                    _ => None,
                };
                if let Some((title, detail)) = explanation {
                    self.status.set(Tone::Success, title, detail, "Completed");
                }
                match self.job {
                    Job::CacheSize | Job::CacheSizeAdmin => self.stats[0].set_text(
                        crate::package_cache::measured_size(&text).unwrap_or("Not reported"),
                    ),
                    Job::JournalSize => {
                        self.stats[0].set_text(&journal_usage(&text));
                    }
                    Job::Boot => {
                        self.stats.captions[0].set_text("Slowest startup unit");
                        self.stats.captions[1].set_text("Startup duration");
                        if let Some(line) = text
                            .lines()
                            .find(|line| line.split_whitespace().count() >= 2)
                            && let Some((time, unit)) = line.trim().rsplit_once(char::is_whitespace)
                        {
                            self.stats[0].set_text(unit.trim());
                            self.stats[1].set_text(time);
                        }
                    }
                    Job::Vulkan => {
                        self.stats.captions[0].set_text("Detected GPU");
                        self.stats.captions[1].set_text("Vulkan API");
                        self.stats[0].set_text(
                            &output_field(&text, "deviceName")
                                .unwrap_or_else(|| "Not reported".into()),
                        );
                        self.stats[1].set_text(
                            &output_field(&text, "apiVersion")
                                .unwrap_or_else(|| "Not reported".into()),
                        );
                    }
                    Job::Report => {
                        self.stats.captions[0].set_text("Memory in use");
                        self.stats.captions[1].set_text("Root storage");
                        self.stats[0].set_text(
                            &output_field(&text, "Memory").unwrap_or_else(|| "Not reported".into()),
                        );
                        self.stats[1].set_text(
                            &output_field(&text, "Root storage")
                                .unwrap_or_else(|| "Not reported".into()),
                        );
                    }
                    Job::GameMode => {
                        self.stats.captions[0].set_text("Self-test");
                        self.stats[0].set_text("Completed");
                    }
                    Job::CachePreview | Job::UninstalledPreview => {
                        self.preview(Ok(text));
                        self.authorize.set_visible(false);
                    }
                    Job::Orphans | Job::Foreign => {
                        if !text.starts_with("No ") {
                            self.stats[0].set_text(&package_count(
                                text.lines().filter(|s| !s.is_empty()).count() as u64,
                            ));
                        }
                    }
                    _ => {}
                }
            }
            Err(error) => {
                if matches!(
                    self.job,
                    Job::CacheClean
                        | Job::UninstalledClean
                        | Job::CachePreview
                        | Job::UninstalledPreview
                ) {
                    self.stats[0].set_text("Not reported");
                    self.stats[1].set_text("Not reported");
                }
                let partial = self.job.cache_permission_error(&error);
                self.retry.set_visible(partial);
                if partial {
                    self.close.remove_css_class("primary");
                }
                self.status.set(if partial { Tone::Warning } else { Tone::Error }, if partial { "Some folders need authentication" } else { "Could not complete this check" }, if partial { "This measurement is partial. Authenticate to include pacman's protected download folders." } else { "The command returned an error. Read the output for the full output; no success is assumed." }, if partial { "Partial result" } else { "Check the details" });
                self.details.output.buffer().set_text(&if partial {format!("PARTIAL MEASUREMENT: the size below excludes unreadable folders.\n\n{error}")} else {error});
                self.details.expander.set_expanded(true);
            }
        }
    }
}
fn output_field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (name, value) = line.trim().split_once(['=', ':'])?;
        (name.trim() == key && !value.trim().is_empty()).then(|| value.trim().to_string())
    })
}

fn journal_usage(text: &str) -> String {
    text.split("take up ")
        .nth(1)
        .and_then(|s| s.split(" in ").next())
        .map(str::to_string)
        .unwrap_or_else(|| "See details".into())
}

fn create_job(parent: &gtk::ApplicationWindow, job: Job) -> (gtk::Window, Rc<JobView>) {
    let (glyph, icon, subtitle) = match job {
        Job::CacheClean | Job::UninstalledClean | Job::CachePreview | Job::UninstalledPreview => (
            "\u{e8cb}",
            "package-x-generic-symbolic",
            "Preview your package cache before making changes.",
        ),
        Job::CacheSize | Job::CacheSizeAdmin => (
            "\u{e2c7}",
            "folder-symbolic",
            "Understand the storage used by downloaded packages.",
        ),
        Job::JournalClean | Job::JournalSize => (
            "\u{e873}",
            "text-x-generic-symbolic",
            "Review journal usage and archived log retention.",
        ),
        Job::Failed => (
            "\u{e8b8}",
            "view-list-symbolic",
            "Inspect services that failed in the current boot.",
        ),
        Job::Boot => (
            "\u{e425}",
            "appointment-soon-symbolic",
            "Review the time your services take to start.",
        ),
        Job::Vulkan => (
            "\u{e322}",
            "video-display-symbolic",
            "Inspect your graphics implementation and detected devices.",
        ),
        Job::GameMode => (
            "\u{ea28}",
            "applications-games-symbolic",
            "Check that GameMode is ready for your games.",
        ),
        Job::Updates => (
            "\u{e863}",
            "software-update-available-symbolic",
            "Check the package updates available from your repositories.",
        ),
        Job::Report => (
            "\u{e873}",
            "text-x-generic-symbolic",
            "Collect useful system details and review them before sharing.",
        ),
        _ => (
            "\u{e8cb}",
            "package-x-generic-symbolic",
            "Inspect your locally installed packages.",
        ),
    };
    let (window, surface) = Surface::new(parent, job.title(), subtitle, glyph, icon);
    let (first, second) = match job {
        Job::CacheClean | Job::CachePreview | Job::UninstalledClean | Job::UninstalledPreview => {
            ("Cleanup candidates", "Space to reclaim")
        }
        Job::JournalClean => ("Journal retention", "Current journal usage"),
        Job::JournalSize => ("Journal disk usage", "Runs as"),
        Job::Boot => ("Slowest startup unit", "Startup duration"),
        Job::Vulkan => ("Detected GPU", "Vulkan API"),
        Job::Report => ("Memory in use", "Root storage"),
        Job::GameMode => ("Self-test", "Runs as"),
        Job::CacheSize | Job::CacheSizeAdmin => ("Cache size", "Measurement access"),
        Job::Orphans | Job::Foreign => ("Packages found", "Package database"),
        Job::Failed => ("Failed units", "Scope"),
        Job::Updates => ("Available updates", "Repository check"),
    };
    let stats = surface.stats(
        (first, "—", icon),
        (
            second,
            if matches!(job, Job::Failed) {
                "System manager"
            } else if matches!(job, Job::Updates) {
                "Configured repos"
            } else if matches!(job, Job::Orphans | Job::Foreign) {
                "pacman"
            } else if job.privileged() {
                "Administrator"
            } else {
                "Your account"
            },
            "drive-harddisk-symbolic",
        ),
    );
    if matches!(
        job,
        Job::CacheClean
            | Job::CachePreview
            | Job::UninstalledClean
            | Job::UninstalledPreview
            | Job::JournalClean
    ) {
        stats[1].set_text("—");
    }
    if matches!(job, Job::Boot | Job::Vulkan | Job::Report) {
        stats[1].set_text("—");
    }
    let details = surface.details(
        "Command output",
        "Full log · scroll here to review",
        Some(&job.display_command()),
    );
    let copy = button("Copy output", false);
    copy.set_sensitive(false);
    details.tools.append(&copy);
    let command = button("Copy command", false);
    command.connect_clicked(move |_| crate::copy_text(&job.display_command()));
    details.tools.append(&command);
    let retry = button("Measure with authentication", true);
    retry.set_visible(false);
    surface.actions.append(&retry);
    let close = surface.close_button(&window, "Close", false);
    let authorize = button(
        if job.destructive() {
            "Authenticate & clean"
        } else {
            "Authenticate & measure"
        },
        true,
    );
    if job.destructive() {
        authorize.add_css_class("destructive");
    }
    authorize.set_visible(job.privileged());
    surface.actions.append(&authorize);
    let view = Rc::new(JobView {
        job,
        status: surface.status,
        details,
        stats,
        copy,
        authorize,
        retry,
        close,
        parent: parent.downgrade(),
    });
    if !job.destructive() && !job.privileged() && job.preview().is_none() {
        let rerun = button("Run again", false);
        rerun.set_sensitive(false);
        surface.actions.prepend(&rerun);
        let weak = Rc::downgrade(&view);
        rerun.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade()
                && view.status.tone.get() != Tone::Busy
            {
                view.run();
            }
        });
        let weak = Rc::downgrade(&view);
        let window = window.downgrade();
        let rerun = rerun.downgrade();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let (Some(view), Some(window), Some(button)) =
                (weak.upgrade(), window.upgrade(), rerun.upgrade())
            else {
                return glib::ControlFlow::Break;
            };
            if !window.is_visible() {
                return glib::ControlFlow::Break;
            }
            button.set_sensitive(view.status.tone.get() != Tone::Busy);
            glib::ControlFlow::Continue
        });
    }
    let weak = Rc::downgrade(&view);
    view.authorize.connect_clicked(move |_| {
        if let Some(view) = weak.upgrade() {
            view.run();
        }
    });
    let weak = Rc::downgrade(&view);
    view.copy.connect_clicked(move |_| {
        if let Some(view) = weak.upgrade() {
            let b = view.details.output.buffer();
            crate::copy_text(&b.text(&b.start_iter(), &b.end_iter(), false));
        }
    });
    let p = parent.downgrade();
    let w = window.downgrade();
    view.retry.connect_clicked(move |_| {
        if let Some(w) = w.upgrade() {
            w.close();
        }
        if let Some(p) = p.upgrade() {
            show_job(&p, Job::CacheSizeAdmin);
        }
    });
    let owned = view.clone();
    window.connect_destroy(move |_| {
        let _ = &owned;
    });
    (window, view)
}

pub(crate) fn size_output(window: &gtk::Window, parent: &gtk::ApplicationWindow) {
    if let Some(surface) = parent.surface()
        && let Some(monitor) = gtk::prelude::WidgetExt::display(parent).monitor_at_surface(&surface)
    {
        let bounds = monitor.geometry();
        window.set_default_size(1160.min(bounds.width() - 48), 760.min(bounds.height() - 64));
    }
}
pub fn show_job(parent: &gtk::ApplicationWindow, job: Job) {
    let (window, view) = create_job(parent, job);
    size_output(&window, parent);
    if let Some(preview) = job.preview() {
        view.authorize.set_sensitive(false);
        view.status.set(
            Tone::Busy,
            "Reading cleanup candidates…",
            "This is a read-only preview. No files have been changed.",
            "No changes made",
        );
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(preview.execute());
        });
        let view = view.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let result = match rx.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => Err("The preview worker stopped unexpectedly.".into()),
            };
            view.preview(result);
            glib::ControlFlow::Break
        });
    } else if job.privileged() {
        view.status.set(
            Tone::Ready,
            "Ready to measure the full cache",
            job.confirmation(),
            "Authentication required",
        );
    } else {
        view.run();
    }
    window.present();
}
