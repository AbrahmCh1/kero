//! Navegación directa por rutas: "D:\Pro" lista D:\ filtrando por "Pro".
//! Tab autocompleta la ruta seleccionada.

use super::{Action, Fuzzy, Icon, Item, Mode, Plugin, Query};
use crate::config::expand_path;
use std::path::PathBuf;

pub struct PathBrowse;

fn is_path_like(s: &str) -> bool {
    let b = s.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
        || s.starts_with("\\\\")
        || s.starts_with("~\\")
        || s.starts_with("~/")
        || s.starts_with('%')
}

impl Plugin for PathBrowse {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::All || !is_path_like(q.text) {
            return;
        }
        let raw = q.text.replace('/', "\\");
        let (dir, partial) = match raw.rfind('\\') {
            Some(i) => (&raw[..=i], &raw[i + 1..]),
            None => (raw.as_str(), ""),
        };
        let dir_path = expand_path(dir);
        // "C:" → "C:\"
        let dir_path = if dir_path.as_os_str().len() == 2 { PathBuf::from(format!("{}\\", dir_path.display())) } else { dir_path };
        let Ok(rd) = std::fs::read_dir(&dir_path) else { return };

        let mut fz = (!partial.is_empty()).then(|| Fuzzy::new(partial));
        let mut items = Vec::new();
        for e in rd.flatten().take(5000) {
            let name = e.file_name().to_string_lossy().into_owned();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let score = match &mut fz {
                Some(f) => match f.score(&name) {
                    Some(s) => s as i32,
                    None => continue,
                },
                None => 0,
            };
            let path = e.path();
            let mut it = Item::new(
                name.clone(),
                dir_path.display().to_string(),
                Icon::Path(path.clone(), is_dir),
                Action::Open(path),
                7_000 + score + if is_dir { 50 } else { 0 },
            );
            if let Some(f) = &mut fz {
                it.highlight = f.indices(&name);
            }
            it.complete = Some(format!("{dir}{name}{}", if is_dir { "\\" } else { "" }));
            items.push(it);
        }
        if fz.is_none() {
            items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
            // Mantiene el orden alfabético dentro del bloque de carpetas/archivos.
            let n = items.len() as i32;
            for (i, it) in items.iter_mut().enumerate() {
                it.score = 7_000 + n - i as i32;
            }
        }
        // La carpeta actual primero.
        if partial.is_empty() {
            out.push(Item::new(
                dir_path.display().to_string(),
                "Abrir carpeta",
                Icon::Path(dir_path.clone(), true),
                Action::Open(dir_path),
                20_000,
            ));
        }
        out.extend(items);
    }
}
