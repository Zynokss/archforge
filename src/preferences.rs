use serde_json::{Value, json};
fn path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| gtk::glib::home_dir().join(".config"));
    base.join("archforge/preferences.json")
}
fn read() -> Value {
    std::fs::read(path())
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}
pub(crate) fn automatic_updates() -> bool {
    read()["automatic_updates"].as_bool().unwrap_or(true)
}
pub(crate) fn reduced_motion() -> bool {
    read()["reduced_motion"].as_bool().unwrap_or(false)
}
pub(crate) fn save(key: &str, value: bool) -> Result<(), String> {
    let mut settings = read();
    settings[key] = json!(value);
    let path = path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, serde_json::to_vec_pretty(&settings).unwrap()).map_err(|e| e.to_string())
}
