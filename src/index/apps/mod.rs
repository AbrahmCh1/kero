//! Apps instaladas, con una implementación por sistema operativo.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use self::linux::{desktop_exec, enumerate};
#[cfg(target_os = "macos")]
pub use self::macos::enumerate;
#[cfg(windows)]
pub use self::windows::enumerate;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn enumerate() -> Vec<App> {
    Vec::new()
}

#[derive(Clone, Debug)]
pub struct App {
    pub name: String,
    /// Identificador para lanzarla: AUMID en Windows, ruta del .app en macOS,
    /// ruta del .desktop en Linux.
    pub id: String,
    /// Ruta del ejecutable o bundle, si se conoce (para "abrir ubicación").
    pub path: Option<String>,
}

fn finish(mut apps: Vec<App>) -> Vec<App> {
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.name == b.name && a.id == b.id);
    apps
}

// Caché en texto plano: nombre \t id \t ruta
pub fn save(apps: &[App], path: &std::path::Path) {
    let mut s = String::with_capacity(apps.len() * 96);
    for a in apps {
        s.push_str(&a.name);
        s.push('\t');
        s.push_str(&a.id);
        s.push('\t');
        s.push_str(a.path.as_deref().unwrap_or(""));
        s.push('\n');
    }
    let _ = std::fs::write(path, s);
}

pub fn load(path: &std::path::Path) -> Vec<App> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let name = it.next()?.to_string();
            let id = it.next()?.to_string();
            let path = it.next().filter(|p| !p.is_empty()).map(String::from);
            Some(App { name, id, path })
        })
        .collect()
}
