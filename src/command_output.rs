//! Shared read-only command viewer. Display controls never change command output.
use crate::{button, column, copy_text, label, row};
use gtk::{glib, prelude::*};

pub(crate) fn pane(view: &gtk::TextView, command: Option<&str>, expandable: bool) -> gtk::Box {
    let pane = column(8);
    pane.add_css_class("command-output-pane");
    pane.set_vexpand(true);
    let toolbar = row(8);
    toolbar.add_css_class("output-toolbar");
    let title = label("Command output", "row-title");
    title.set_hexpand(true);
    toolbar.append(&title);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Find in output…")
        .width_request(180)
        .build();
    search.update_property(&[gtk::accessible::Property::Label("Find in command output")]);
    toolbar.append(&search);
    let next = button("Next", false);
    toolbar.append(&next);
    let wrap = gtk::ToggleButton::with_label("Wrap");
    wrap.set_active(true);
    toolbar.append(&wrap);
    let expand = button("Expand ↗", false);
    expand.set_visible(expandable);
    toolbar.append(&expand);
    pane.append(&toolbar);
    if let Some(command) = command {
        let line = row(8);
        line.add_css_class("output-command");
        let text = crate::wrapped(command, "console-command");
        text.set_selectable(true);
        text.set_hexpand(true);
        line.append(&text);
        let copy = button("Copy command", false);
        let command = command.to_string();
        copy.connect_clicked(move |_| copy_text(&command));
        line.append(&copy);
        pane.append(&line);
    }
    view.set_left_margin(52);
    view.set_top_margin(12);
    view.set_bottom_margin(12);
    view.set_monospace(true);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    let scroll = gtk::ScrolledWindow::builder()
        .child(view)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(130)
        .hexpand(true)
        .vexpand(true)
        .build();
    let overlay = gtk::Overlay::new();
    overlay.set_vexpand(true);
    overlay.set_child(Some(&scroll));
    let gutter = gtk::DrawingArea::new();
    gutter.set_width_request(42);
    gutter.set_halign(gtk::Align::Start);
    gutter.set_can_target(false);
    gutter.add_css_class("output-line-numbers");
    let text_view = view.downgrade();
    gutter.set_draw_func(move |_, cr, _, height| {
        let Some(view) = text_view.upgrade() else {
            return;
        };
        let buffer = view.buffer();
        let rect = view.visible_rect();
        let (mut iter, _) = view.line_at_y(rect.y());
        cr.select_font_face(
            "monospace",
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Normal,
        );
        cr.set_font_size(11.0);
        cr.set_source_rgb(0.43, 0.49, 0.56);
        loop {
            let (y, line_height) = view.line_yrange(&iter);
            let (_, y) = view.buffer_to_window_coords(gtk::TextWindowType::Widget, 0, y);
            if y > height {
                break;
            }
            let number = (iter.line() + 1).to_string();
            cr.move_to(
                (35.0 - number.len() as f64 * 6.6).max(2.0),
                y as f64 + (line_height.min(20) as f64 * 0.78),
            );
            let _ = cr.show_text(&number);
            if iter.line() + 1 >= buffer.line_count() || !iter.forward_line() {
                break;
            }
        }
    });
    overlay.add_overlay(&gutter);
    pane.append(&overlay);
    let numbers = gutter.downgrade();
    scroll.vadjustment().connect_value_changed(move |_| {
        if let Some(g) = numbers.upgrade() {
            g.queue_draw();
        }
    });
    let numbers = gutter.downgrade();
    view.buffer().connect_changed(move |_| {
        if let Some(g) = numbers.upgrade() {
            g.queue_draw();
        }
    });
    let view_for_wrap = view.clone();
    let numbers = gutter.downgrade();
    wrap.connect_toggled(move |b| {
        view_for_wrap.set_wrap_mode(if b.is_active() {
            gtk::WrapMode::WordChar
        } else {
            gtk::WrapMode::None
        });
        if let Some(g) = numbers.upgrade() {
            g.queue_draw();
        }
    });
    let footer = row(8);
    footer.add_css_class("output-toolbar");
    let state = label("Read-only output · scroll inside this panel", "tiny");
    let line_state = state.downgrade();
    view.buffer().connect_changed(move |buffer| {
        if let Some(state) = line_state.upgrade() {
            state.set_text(&format!(
                "Read-only output · {} lines · scroll to explore",
                buffer.line_count()
            ));
        }
    });
    state.set_hexpand(true);
    footer.append(&state);
    let matches = label("", "tiny");
    footer.append(&matches);
    let copy = button("Copy output", false);
    let buffer = view.buffer();
    copy.connect_clicked(move |_| {
        copy_text(&buffer.text(&buffer.start_iter(), &buffer.end_iter(), false))
    });
    footer.append(&copy);
    let save = button("Save log", false);
    let owner = view.downgrade();
    save.connect_clicked(move |_| {
        let Some(view) = owner.upgrade() else {
            return;
        };
        let Some(window) = view.root().and_then(|w| w.downcast::<gtk::Window>().ok()) else {
            return;
        };
        let buffer = view.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        glib::spawn_future_local(async move {
            let dialog = gtk::FileDialog::builder()
                .title("Save command output")
                .initial_name("archforge-output.txt")
                .build();
            if let Ok(file) = dialog.save_future(Some(&window)).await
                && let Err(error) = file.replace_contents(
                    text.as_bytes(),
                    None,
                    false,
                    gtk::gio::FileCreateFlags::REPLACE_DESTINATION,
                    None::<&gtk::gio::Cancellable>,
                )
            {
                let alert = gtk::AlertDialog::builder()
                    .message("Could not save log")
                    .detail(error.to_string())
                    .build();
                alert.show(Some(&window));
            }
        });
    });
    footer.append(&save);
    pane.append(&footer);
    let buffer = view.buffer();
    let tag = buffer
        .tag_table()
        .lookup("output-search")
        .unwrap_or_else(|| {
            let tag = gtk::TextTag::builder()
                .name("output-search")
                .background("#66502a")
                .foreground("#ffffff")
                .build();
            buffer.tag_table().add(&tag);
            tag
        });
    let find = {
        let view = view.clone();
        let search = search.clone();
        let matches = matches.clone();
        move |advance: bool| {
            let buffer = view.buffer();
            buffer.remove_tag(&tag, &buffer.start_iter(), &buffer.end_iter());
            let query = search.text();
            if query.is_empty() {
                matches.set_text("");
                return;
            }
            let flags = gtk::TextSearchFlags::CASE_INSENSITIVE | gtk::TextSearchFlags::TEXT_ONLY;
            let from = if advance {
                buffer
                    .selection_bounds()
                    .map(|(_, end)| end)
                    .unwrap_or_else(|| buffer.start_iter())
            } else {
                buffer.start_iter()
            };
            let selected = from
                .forward_search(&query, flags, None)
                .or_else(|| buffer.start_iter().forward_search(&query, flags, None));
            let mut cursor = buffer.start_iter();
            let mut count = 0;
            while let Some((start, end)) = cursor.forward_search(&query, flags, None) {
                buffer.apply_tag(&tag, &start, &end);
                cursor = end;
                count += 1;
                if count >= 2000 {
                    break;
                }
            }
            matches.set_text(&format!(
                "{count} match{}",
                if count == 1 { "" } else { "es" }
            ));
            if let Some((mut start, end)) = selected {
                buffer.select_range(&start, &end);
                view.scroll_to_iter(&mut start, 0.1, false, 0.0, 0.0);
            }
        }
    };
    let find = std::rc::Rc::new(find);
    let changed = find.clone();
    search.connect_search_changed(move |_| changed(false));
    let advance = find.clone();
    next.connect_clicked(move |_| advance(true));
    let refresh_search = find.clone();
    view.buffer()
        .connect_changed(move |_| refresh_search(false));
    search.connect_activate(move |_| find(true));
    let owner = view.downgrade();
    let command = command.map(str::to_string);
    expand.connect_clicked(move |_| {
        let Some(view) = owner.upgrade() else {
            return;
        };
        let Some(parent) = view.root().and_then(|w| w.downcast::<gtk::Window>().ok()) else {
            return;
        };
        let window = gtk::Window::builder()
            .title("Command output")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .modal(true)
            .default_width(1100)
            .default_height(720)
            .build();
        window.add_css_class("forge-dialog");
        let expanded = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .buffer(&view.buffer())
            .build();
        expanded.add_css_class("console");
        let content = pane_for_expansion(&expanded, command.as_deref());
        let done = button("Done", true);
        let weak = window.downgrade();
        done.connect_clicked(move |_| {
            if let Some(w) = weak.upgrade() {
                w.close();
            }
        });
        content.append(&done);
        window.set_child(Some(&content));
        window.present();
    });
    pane
}
fn pane_for_expansion(view: &gtk::TextView, command: Option<&str>) -> gtk::Box {
    pane(view, command, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn walk(w: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut out = vec![w.clone()];
        let mut c = w.first_child();
        while let Some(child) = c {
            out.extend(walk(&child));
            c = child.next_sibling();
        }
        out
    }
    fn pump() {
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(500) {
            while glib::MainContext::default().pending() {
                glib::MainContext::default().iteration(false);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    #[test]
    #[ignore]
    fn command_viewer_controls() {
        gtk::init().unwrap();
        let provider = gtk::CssProvider::new();
        crate::theme::apply(&provider, "amber");
        let window = gtk::Window::builder()
            .title("Output verification")
            .default_width(1160)
            .default_height(760)
            .build();
        window.add_css_class("forge-dialog");
        let view = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .build();
        view.add_css_class("console");
        crate::dialogs::style_output(&view.buffer());
        let mut output = String::from(
            "VULKANINFO\nVulkan Instance Version: 1.4.354\ndeviceName = AMD Radeon RX 6600\n",
        );
        for n in 0..2000 {
            output.push_str(&format!("Extension {n}: Unicode ✓ — read-only output\n"));
        }
        view.buffer().set_text(&output);
        let panel = pane(&view, Some("/usr/bin/vulkaninfo --summary"), true);
        window.set_child(Some(&panel));
        window.present();
        pump();
        let search = walk(panel.upcast_ref())
            .into_iter()
            .find_map(|w| w.downcast::<gtk::SearchEntry>().ok())
            .unwrap();
        search.set_text("deviceName");
        pump();
        let buffer = view.buffer();
        let (a, b) = buffer.selection_bounds().unwrap();
        assert_eq!(buffer.text(&a, &b, false), "deviceName");
        let wrap = walk(panel.upcast_ref())
            .into_iter()
            .find_map(|w| w.downcast::<gtk::ToggleButton>().ok())
            .unwrap();
        wrap.set_active(false);
        assert_eq!(view.wrap_mode(), gtk::WrapMode::None);
        wrap.set_active(true);
        for w in walk(panel.upcast_ref()) {
            if let Some(button) = w.downcast_ref::<gtk::Button>()
                && button.label().as_deref() == Some("Copy output")
            {
                button.emit_clicked();
            }
        }
        crate::dialogs::snapshot(&window, "shared-output-viewer");
        let expand = walk(panel.upcast_ref())
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Button>().ok())
            .find(|b| b.label().as_deref() == Some("Expand ↗"))
            .unwrap();
        expand.emit_clicked();
        pump();
        let expanded = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.title().as_deref() == Some("Command output"))
            .unwrap();
        let expanded_view = walk(expanded.upcast_ref())
            .into_iter()
            .find_map(|w| w.downcast::<gtk::TextView>().ok())
            .unwrap();
        assert_eq!(expanded_view.buffer(), buffer);
        expanded.close();
        buffer.set_text("No results\n");
        pump();
        assert_eq!(
            buffer.text(&buffer.start_iter(), &buffer.end_iter(), false),
            "No results\n"
        );
        window.close();
    }
}
