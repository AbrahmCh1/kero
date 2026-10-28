//! Apps de Linux: archivos `.desktop` (XDG), incluidos Flatpak y Snap.

use super::App;
use std::path::{Path, PathBuf};

fn app_dirs() -> Vec<PathBuf> {
    let home = crate::config::home_dir();
    let data_home = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let mut dirs = vec![data_home.join("applications")];
    dirs.extend(data_dirs.split(':').filter(|d| !d.is_empty()).map(|d| Path::new(d).join("applications")));
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    dirs.push("/var/lib/snapd/desktop/applications".into());
    dirs
}

/// Campos de la sección `[Desktop Entry]`.
fn entry_fields(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines()
        .skip_while(|l| l.trim() != "[Desktop Entry]")
        .skip(1)
        .take_while(|l| !l.starts_with('['))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
}

pub fn enumerate() -> Vec<App> {
    let mut out = Vec::new();
    // El primer directorio gana: ~/.local/share sobrescribe a /usr/share.
    let mut seen = std::collections::HashSet::new();
    for dir in app_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "desktop") {
                continue;
            }
            if !seen.insert(e.file_name()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let mut name = None;
            let mut visible = true;
            let mut is_app = true;
            for (k, v) in entry_fields(&text) {
                match k {
                    "Name" => name = Some(v.to_string()),
                    "NoDisplay" | "Hidden" if v == "true" => visible = false,
                    "Type" if v != "Application" => is_app = false,
                    _ => {}
                }
            }
            if let (Some(name), true, true) = (name, visible, is_app) {
                out.push(App { name, id: path.to_string_lossy().into_owned(), path: None });
            }
        }
    }
    super::finish(out)
}

/// Línea de comandos de un `.desktop` sin los códigos de campo (%f, %U…), y si pide terminal.
pub fn desktop_exec(path: &Path) -> Option<(String, bool)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut exec = None;
    let mut terminal = false;
    for (k, v) in entry_fields(&text) {
        match k {
            "Exec" => exec = Some(v.to_string()),
            "Terminal" => terminal = v == "true",
            _ => {}
        }
    }
    let exec = exec?;
    let mut cmd = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            if chars.next() == Some('%') {
                cmd.push('%');
            }
        } else {
            cmd.push(c);
        }
    }
    Some((cmd.trim().to_string(), terminal))
}
