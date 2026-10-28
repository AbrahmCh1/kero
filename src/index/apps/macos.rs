//! Apps de macOS: los bundles `.app` de las carpetas de Aplicaciones.

use super::App;
use std::path::Path;

pub fn enumerate() -> Vec<App> {
    let home = crate::config::home_dir();
    let roots = [
        Path::new("/Applications").to_path_buf(),
        Path::new("/Applications/Utilities").to_path_buf(),
        Path::new("/System/Applications").to_path_buf(),
        Path::new("/System/Applications/Utilities").to_path_buf(),
        home.join("Applications"),
    ];
    let mut out = Vec::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().is_some_and(|x| x == "app") {
                let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let p = path.to_string_lossy().into_owned();
                out.push(App { name, id: p.clone(), path: Some(p) });
            }
        }
    }
    super::finish(out)
}
