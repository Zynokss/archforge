//! Dispatch reviewed commands to the GTK embedded terminal, including worker callers.
pub fn open(proposed: &str) -> Result<(), String> {
    if proposed.trim().is_empty()
        || proposed.contains("%command%")
        || proposed.contains(['\r', '\0'])
    {
        return Err("Enter a runnable command. Steam launch options belong in Steam's Launch Options field.".into());
    }
    let command = proposed.to_string();
    gtk::glib::idle_add_once(move || {
        use gtk::prelude::*;
        let parent = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.is::<gtk::ApplicationWindow>() && w.is_visible());
        if let Some(parent) = parent {
            crate::embedded_terminal::show(
                &parent,
                "Review command",
                crate::embedded_terminal::purpose(&command),
                &command,
            );
        }
    });
    Ok(())
}
