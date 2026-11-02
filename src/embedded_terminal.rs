//! Interactive PTY sessions with a separate review and fresh authentication gate.
use crate::{button, column, label, row, wrapped};
use gtk::{glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use vte4::prelude::*;

// The command is a positional argument; no user command or password is interpolated.
// The sudo probe refuses policy configurations that permit credential-free validation.
pub(crate) const SESSION: &str = r#"
unset HISTFILE BASH_ENV ENV SUDO_ASKPASS
printf '\033[1;36mArch Forge · reviewed command\033[0m\n'
printf 'This action requires fresh sudo authentication. Password input is hidden.\n\n'
if /usr/bin/sudo -n -k -v >/dev/null 2>&1; then
    /usr/bin/sudo -k
    printf '\033[1;31mBlocked: sudo accepted passwordless validation.\033[0m\nArch Forge cannot enforce a password prompt with this sudo policy. Nothing was run.\n'
    exit 77
fi
/usr/bin/sudo -k
/usr/bin/sudo -v || { printf '\033[1;31mAuthentication failed or cancelled. Nothing was run.\033[0m\n'; exit 78; }
trap '/usr/bin/sudo -k >/dev/null 2>&1' EXIT
printf '\n\033[1;33mRunning the reviewed command as your desktop account…\033[0m\n\n'
/usr/bin/bash --noprofile --norc -c "$1"
result=$?
if [[ $result -eq 0 ]]; then color=32; else color=31; fi
printf '\n\033[1;%smCommand finished (exit %s).\033[0m\n' "$color" "$result"
exit "$result"
"#;

fn session_script() -> String {
    SESSION.into()
}

pub(crate) fn purpose(command: &str) -> &'static str {
    if command.contains("pacman -R") {
        "Removes the listed packages. Pacman will show dependencies and ask you to confirm its transaction. Check the package names carefully."
    } else if command.contains("pacman -S") {
        "Updates repository packages and/or installs the listed packages. Review pacman's transaction and downloads before confirming."
    } else if command.contains("journalctl") && command.contains("--vacuum") {
        "Older archived journal logs will be permanently deleted. Active journal files are not removed. Review the retention period or size limit in the command."
    } else if command.contains("paccache")
        || command.contains("journalctl --vacuum")
        || command.contains("prune")
        || command.contains("--clean")
        || command.contains("yay -Sc")
    {
        "Runs the cleanup command shown below. It may permanently remove cached files or archived data. Read the flags and any transaction prompts before confirming."
    } else if command.contains("timeshift") {
        "Runs the Timeshift command shown below using your existing configuration. Check whether the command lists snapshots or creates a new snapshot."
    } else if command.contains("git clone")
        || command.contains("curl")
        || command.contains("setup")
        || command.contains("install.sh")
    {
        "Downloads or runs the selected setup. Upstream installers can install packages and replace desktop configuration. Review its installation instructions and back up affected files first."
    } else {
        "Runs the exact command below using Bash and your desktop account. It can change files or system configuration according to the command and its flags. Review it before proceeding."
    }
}

pub(crate) fn show(
    parent: &gtk::Window,
    name: &str,
    description: &str,
    proposed: &str,
) -> gtk::Window {
    let mut owner = parent.clone();
    while let Some(next) = owner.transient_for() {
        owner = next;
    }
    let window = gtk::Window::builder()
        .title(name)
        .transient_for(&owner)
        .modal(true)
        .destroy_with_parent(false)
        .default_width(1080)
        .default_height(760)
        .build();
    window.add_css_class("forge-dialog");
    window.add_css_class("embedded-terminal-window");
    if let Some(application) = owner.application() {
        application.add_window(&window);
    }
    let root = column(12);
    root.add_css_class("embedded-terminal-content");
    let header = row(12);
    let icon = gtk::Image::from_icon_name("utilities-terminal-symbolic");
    icon.set_pixel_size(30);
    header.append(&icon);
    let title = label(name, "dialog-title");
    title.set_hexpand(true);
    header.append(&title);
    let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
    dismiss.add_css_class("dialog-dismiss");
    let weak = window.downgrade();
    dismiss.connect_clicked(move |_| {
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    });
    header.append(&dismiss);
    root.append(&header);
    root.append(&wrapped(description, "subtitle"));
    let explanation = wrapped(purpose(proposed), "embedded-terminal-notice");
    root.append(&explanation);
    let editor = gtk::TextView::builder()
        .editable(true)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(12)
        .right_margin(12)
        .top_margin(10)
        .bottom_margin(10)
        .build();
    editor.add_css_class("embedded-command-editor");
    for (name, color) in [
        ("embedded-program", "#8be9df"),
        ("embedded-option", "#ffbf69"),
        ("embedded-sudo", "#cba6f7"),
    ] {
        editor
            .buffer()
            .tag_table()
            .add(&gtk::TextTag::builder().name(name).foreground(color).build());
    }
    editor.buffer().connect_changed(|buffer| {
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        let mut offset = 0;
        for word in text.split_inclusive(char::is_whitespace) {
            let token = word.trim();
            if !token.is_empty() {
                let base = token.trim_matches(['\'', '"']);
                let tag = if base == "sudo" {
                    "embedded-sudo"
                } else if base.starts_with('-') {
                    "embedded-option"
                } else {
                    "embedded-program"
                };
                buffer.apply_tag_by_name(
                    tag,
                    &buffer.iter_at_offset(offset),
                    &buffer.iter_at_offset(offset + token.chars().count() as i32),
                );
            }
            offset += word.chars().count() as i32;
        }
    });
    editor.buffer().set_text(proposed);
    let command_scroll = gtk::ScrolledWindow::builder()
        .child(&editor)
        .min_content_height(48)
        .max_content_height(100)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let command_panel = column(8);
    command_panel.add_css_class("embedded-command-panel");
    let command_heading = row(12);
    let caption = label("Command to run", "section-title");
    caption.set_hexpand(true);
    command_heading.append(&caption);
    let copy_command = button("Copy command", false);
    let buffer = editor.buffer();
    copy_command.connect_clicked(move |_| {
        crate::copy_text(&buffer.text(&buffer.start_iter(), &buffer.end_iter(), false))
    });
    command_heading.append(&copy_command);
    command_panel.append(&command_heading);
    command_panel.append(&command_scroll);
    command_panel.append(&label(
        "Administrator authentication required  ·  Bash",
        "tiny",
    ));
    root.append(&command_panel);
    let review = gtk::CheckButton::with_label("I reviewed the command");
    review.add_css_class("embedded-review-check");
    review.set_valign(gtk::Align::Center);
    review.set_tooltip_text(Some(
        "Read the command and action explanation, then check this box to enable authentication.",
    ));
    root.append(&wrapped("Every run requests fresh sudo authentication, even when the command runs as your regular account. Passwordless sudo validation is blocked. Shell commands run in Bash; your desktop account and environment are used.", "tiny"));
    let terminal = vte4::Terminal::new();
    terminal.set_hexpand(true);
    terminal.set_vexpand(true);
    terminal.set_scrollback_lines(10000);
    terminal.set_scroll_on_output(true);
    terminal.set_scroll_on_keystroke(true);
    terminal.set_allow_hyperlink(false);
    terminal.set_input_enabled(false);
    terminal.set_font(Some(&gtk::pango::FontDescription::from_string(
        "Monospace 12",
    )));
    let rgba = |hex: &str| gtk::gdk::RGBA::parse(hex).unwrap();
    let colors: Vec<_> = [
        "#15191f", "#ef717a", "#88c98b", "#edbd65", "#80b4eb", "#be9de4", "#77cbd0", "#d1dbe7",
        "#67788b", "#ff8b93", "#a3e3a1", "#ffda8a", "#a8cfff", "#d8b9ff", "#a1edf0", "#ffffff",
    ]
    .iter()
    .map(|hex| rgba(hex))
    .collect();
    terminal.set_colors(
        Some(&rgba("#d1dbe7")),
        Some(&rgba("#000000")),
        &colors.iter().collect::<Vec<_>>(),
    );
    terminal.feed(b"\x1b[1;36mArch Forge Terminal\x1b[0m\r\n\x1b[38;5;180mReview first. Authenticate each run.\x1b[0m\r\n\r\nNo process has started. Review the command above, then choose Authenticate & run.\r\n");
    terminal.set_color_cursor(Some(&rgba("#edbd65")));
    terminal.set_color_bold(Some(&rgba("#edbd65")));
    let keys = gtk::EventControllerKey::new();
    let term_keys = terminal.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if modifiers
            .contains(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK)
        {
            match key.to_unicode().map(|c| c.to_ascii_lowercase()) {
                Some('c') => {
                    term_keys.copy_clipboard_format(vte4::Format::Text);
                    return glib::Propagation::Stop;
                }
                Some('v') => {
                    if term_keys.is_input_enabled() {
                        term_keys.paste_clipboard();
                    }
                    return glib::Propagation::Stop;
                }
                _ => {}
            }
        }
        glib::Propagation::Proceed
    });
    terminal.add_controller(keys);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&terminal)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(200)
        .vexpand(true)
        .build();
    scroll.add_css_class("embedded-terminal-frame");
    let terminal_panel = column(8);
    terminal_panel.add_css_class("embedded-output-panel");
    terminal_panel.set_vexpand(true);
    let terminal_heading = row(10);
    let terminal_title = label("Terminal output", "section-title");
    terminal_title.set_hexpand(true);
    terminal_heading.append(&terminal_title);
    terminal_panel.append(&terminal_heading);
    terminal_panel.append(&scroll);
    root.append(&terminal_panel);
    let footer = row(10);
    let status = wrapped("Ready for review · nothing has run", "muted");
    status.set_hexpand(true);
    status.set_max_width_chars(28);
    footer.append(&status);
    let copy = button("Copy selection", false);
    let term = terminal.clone();
    copy.connect_clicked(move |_| term.copy_clipboard_format(vte4::Format::Text));
    terminal_heading.append(&copy);
    let cancel = button("Ctrl+C", false);
    cancel.set_tooltip_text(Some("Send an interrupt to the foreground process"));
    cancel.set_sensitive(false);
    let term = terminal.clone();
    cancel.connect_clicked(move |_| term.feed_child(b"\x03"));
    terminal_heading.append(&cancel);
    let run = button("Authenticate & run", true);
    run.set_sensitive(false);
    footer.append(&review);
    let run_group = column(4);
    run_group.append(&run);
    let run_hint = label("Confirm review to enable", "embedded-run-hint");
    run_group.append(&run_hint);
    footer.append(&run_group);
    review.connect_toggled(move |check| {
        run_hint.set_text(if check.is_active() {
            "Fresh authentication required"
        } else {
            "Confirm review to enable"
        })
    });
    let done = button("Cancel", false);
    let weak = window.downgrade();
    done.connect_clicked(move |_| {
        if let Some(w) = weak.upgrade() {
            w.close();
        }
    });
    footer.insert_child_after(&done, Some(&status));
    footer.add_css_class("embedded-review-footer");
    root.append(&footer);
    window.set_child(Some(&root));
    let running = Rc::new(Cell::new(false));
    let operation = Rc::new(RefCell::new(None::<crate::Operation>));
    let run_button = run.clone();
    let running_review = running.clone();
    review
        .connect_toggled(move |c| run_button.set_sensitive(c.is_active() && !running_review.get()));
    let checked = review.clone();
    let explanation = explanation.clone();
    editor.buffer().connect_changed(move |buffer| {
        checked.set_active(false);
        let command = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        explanation.set_text(purpose(&command));
    });
    let checked = review.clone();
    let run_button = run.clone();
    let editor_for_run = editor.clone();
    let term = terminal.clone();
    let status_for_run = status.clone();
    let running_start = running.clone();
    let operation_start = operation.clone();
    let cancel_start = cancel.clone();
    run.connect_clicked(move |_| {
        if running_start.get() || !checked.is_active() {
            return;
        }
        let buffer = editor_for_run.buffer();
        let command = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        if command.trim().is_empty()
            || command.contains(['\0', '\r'])
            || command.contains("%command%")
        {
            status_for_run.set_text("Enter a runnable command before continuing.");
            return;
        }
        running_start.set(true);
        *operation_start.borrow_mut() = Some(crate::Operation::new());
        run_button.set_sensitive(false);
        checked.set_sensitive(false);
        editor_for_run.set_editable(false);
        cancel_start.set_sensitive(true);
        term.reset(true, true);
        term.feed(b"\x1b[2J\x1b[3J\x1b[H");
        term.set_input_enabled(true);
        status_for_run.set_text("Running · enter authentication in the terminal below");
        term.grab_focus();
        let running_error = running_start.clone();
        let operation_error = operation_start.clone();
        let run_error = run_button.clone();
        let checked_error = checked.clone();
        let editor_error = editor_for_run.clone();
        let status_error = status_for_run.clone();
        let cancel_error = cancel_start.clone();
        let terminal_error = term.clone();
        let home = gtk::glib::home_dir();
        let session = session_script();
        term.spawn_async(
            vte4::PtyFlags::DEFAULT,
            home.to_str(),
            &[
                "/usr/bin/bash",
                "--noprofile",
                "--norc",
                "-p",
                "-c",
                &session,
                "archforge",
                &command,
            ],
            &[
                "TERM=xterm-256color",
                "COLORTERM=truecolor",
                "HISTFILE=/dev/null",
                "BASH_ENV=/dev/null",
                "ENV=/dev/null",
            ],
            glib::SpawnFlags::DEFAULT,
            || {},
            -1,
            None::<&gtk::gio::Cancellable>,
            move |result| {
                if let Err(error) = result {
                    terminal_error.set_input_enabled(false);
                    running_error.set(false);
                    operation_error.borrow_mut().take();
                    checked_error.set_sensitive(true);
                    checked_error.set_active(false);
                    editor_error.set_editable(true);
                    cancel_error.set_sensitive(false);
                    run_error.set_sensitive(false);
                    status_error.set_text(&format!("Could not start terminal: {error}"));
                }
            },
        );
    });
    let running_exit = running.clone();
    let operation_exit = operation.clone();
    let status_exit = status.clone();
    let checked = review.clone();
    terminal.connect_child_exited(move |terminal, raw_status| {
        terminal.set_input_enabled(false);
        running_exit.set(false);
        operation_exit.borrow_mut().take();
        checked.set_sensitive(true);
        checked.set_active(false);
        editor.set_editable(true);
        cancel.set_sensitive(false);
        let code = if libc::WIFEXITED(raw_status) {
            libc::WEXITSTATUS(raw_status)
        } else {
            128 + libc::WTERMSIG(raw_status)
        };
        status_exit.set_text(match code {
            0 => "Completed · review again to run another action",
            77 => "Blocked by passwordless sudo policy · command was not run",
            78 => "Authentication cancelled or failed · command was not run",
            _ => "Command stopped or reported an error · review the terminal output",
        });
    });
    window.connect_close_request(move |_| { if running.get() { status.set_text("A command is still running. Finish it, or use Ctrl+C and wait for it to exit before closing."); glib::Propagation::Stop } else { glib::Propagation::Proceed } });
    window.present();
    window
}
