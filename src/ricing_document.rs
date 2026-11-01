//! Native, inert rendering of upstream Markdown and HTML. No scripts or embedded web content.
use crate::{column, label, wrapped};
use gtk::prelude::*;
use scraper::ElementRef;

fn inline(element: ElementRef<'_>, project: crate::ricing::Project) -> String {
    let mut text = String::new();
    for child in element.children() {
        if let Some(value) = child.value().as_text() {
            text.push_str(&gtk::glib::markup_escape_text(
                &value.replace(['\n', '\r'], " "),
            ));
        } else if let Some(e) = ElementRef::wrap(child) {
            let body = inline(e, project);
            match e.value().name() {
                "strong" | "b" => text.push_str(&format!("<b>{body}</b>")),
                "em" | "i" => text.push_str(&format!("<i>{body}</i>")),
                "code" => text.push_str(&format!("<tt>{body}</tt>")),
                "br" => text.push('\n'),
                "a" => {
                    if body.trim().is_empty() {
                        continue;
                    }
                    if let Some(href) = e
                        .attr("href")
                        .and_then(|url| crate::ricing::document_link(url, project))
                    {
                        text.push_str(&format!(
                            "<a href=\"{}\">{body}</a>",
                            gtk::glib::markup_escape_text(&href)
                        ));
                    } else {
                        text.push_str(&body);
                    }
                }
                "img" | "script" | "style" => {}
                _ => text.push_str(&body),
            }
        }
    }
    text
}

fn paragraph(element: ElementRef<'_>, project: crate::ricing::Project, class: &str) -> gtk::Label {
    let view = wrapped("", class);
    view.set_markup(
        &inline(element, project)
            .replace("[!WARNING]", "⚠ Warning")
            .replace("[!NOTE]", "ⓘ Note"),
    );
    if matches!(element.value().name(), "h1" | "h2") {
        view.set_xalign(0.5);
        view.set_justify(gtk::Justification::Center);
    }
    view.set_selectable(true);
    view.set_hexpand(true);
    view.set_max_width_chars(100);
    view
}

fn images(element: ElementRef<'_>, target: &gtk::Box, project: crate::ricing::Project) {
    let selector = scraper::Selector::parse("img").unwrap();
    for image in element.select(&selector) {
        let Some(url) = image.attr("src").map(str::to_string) else {
            continue;
        };
        let holder = column(0);
        holder.set_halign(gtk::Align::Start);
        holder.set_visible(false);
        target.append(&holder);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::ricing::document_image(&url, project));
        });
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            match rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
                Ok(Some(bytes)) => {
                    if let Some(picture) = crate::ricing_ui::image_from_bytes(&bytes) {
                        picture.set_size_request(1, 180);
                        picture.set_hexpand(true);
                        holder.set_halign(gtk::Align::Fill);
                        holder.set_visible(true);
                        holder.append(&picture);
                    }
                    gtk::glib::ControlFlow::Break
                }
                _ => gtk::glib::ControlFlow::Break,
            }
        });
    }
}

fn blocks(element: ElementRef<'_>, target: &gtk::Box, project: crate::ricing::Project) {
    for child in element.children() {
        let Some(e) = ElementRef::wrap(child) else {
            if let Some(text) = child.value().as_text()
                && !text.trim().is_empty()
            {
                target.append(&wrapped(text.trim(), "github-paragraph"));
            }
            continue;
        };
        match e.value().name() {
            "script" | "style" | "svg" | "iframe" | "video" | "source" => {}
            "h1" | "h2" | "h3" | "h4" => {
                if inline(e, project).trim().is_empty() {
                    continue;
                }
                target.append(&paragraph(
                    e,
                    project,
                    &format!("github-{}", e.value().name()),
                ));
                if matches!(e.value().name(), "h1" | "h2") {
                    target.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
                }
            }
            "p" | "li" | "summary" => {
                if !inline(e, project).trim().is_empty() {
                    target.append(&paragraph(e, project, "github-paragraph"));
                }
                images(e, target, project);
            }
            "pre" => {
                let text = e.text().collect::<String>();
                let code = wrapped(&text, "github-code");
                code.set_selectable(true);
                target.append(&code);
            }
            "blockquote" => {
                let note = column(8);
                note.add_css_class("github-warning");
                if e.attr("class").is_some_and(|c| c.contains("warning"))
                    || e.text().collect::<String>().contains("[!WARNING]")
                {
                    note.append(&label("⚠ Warning", "github-warning-title"));
                }
                blocks(e, &note, project);
                target.append(&note);
            }
            "details" => {
                let selector = scraper::Selector::parse("summary").unwrap();
                let title = e
                    .select(&selector)
                    .next()
                    .map(|s| s.text().collect::<String>())
                    .unwrap_or_else(|| "Details".into());
                let contents = column(10);
                contents.add_css_class("github-details");
                for section in e
                    .children()
                    .filter_map(ElementRef::wrap)
                    .filter(|s| s.value().name() != "summary")
                {
                    if matches!(section.value().name(), "p" | "ul" | "ol" | "div") {
                        blocks_or_paragraph(section, &contents, project);
                    } else {
                        contents.append(&paragraph(section, project, "github-paragraph"));
                    }
                }
                let expander = gtk::Expander::builder()
                    .label(title)
                    .child(&contents)
                    .expanded(e.attr("open").is_some())
                    .build();
                target.append(&expander);
            }
            "img" => {
                // Render the parent fragment so the common image path handles standalone tags.
                let html = scraper::Html::parse_fragment(&e.html());
                images(html.root_element(), target, project);
            }
            "hr" => target.append(&gtk::Separator::new(gtk::Orientation::Horizontal)),
            "table" => {
                let table = column(8);
                table.add_css_class("github-table");
                blocks(e, &table, project);
                target.append(&table);
            }
            _ => blocks(e, target, project),
        }
    }
}
fn blocks_or_paragraph(e: ElementRef<'_>, target: &gtk::Box, project: crate::ricing::Project) {
    if e.value().name() == "p" {
        target.append(&paragraph(e, project, "github-paragraph"));
        images(e, target, project);
    } else {
        blocks(e, target, project);
    }
}

pub(crate) fn render(target: &gtk::Box, markdown: &str, project: crate::ricing::Project) {
    while let Some(child) = target.first_child() {
        target.remove(&child);
    }
    target.add_css_class("github-document");
    let parser = pulldown_cmark::Parser::new_ext(markdown, pulldown_cmark::Options::all());
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    let document = scraper::Html::parse_fragment(&html);
    blocks(document.root_element(), target, project);
    if target.first_child().is_none() {
        target.append(&label("No document content available.", "muted"));
    }
}
