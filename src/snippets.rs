//! Textos guardados ("snippets"): `snippets.txt` en la carpeta de configuración,
//! editable a mano. Cada uno empieza con una línea `## nombre`; lo que sigue,
//! hasta el próximo `## `, es el texto. Lo de antes del primero se ignora.

use crate::config;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    pub name: String,
    pub text: String,
}

struct State {
    items: Vec<Snippet>,
    mtime: Option<SystemTime>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

const HEADER: &str = "# Textos guardados de Kero. Escribe \"sn\" en Kero para verlos; Enter copia.\n\
                      # Cada uno empieza con \"## nombre\" y sigue con el texto (puede tener varias líneas).\n\
                      # También puedes guardar uno desde el historial del portapapeles (cb) con Ctrl+S.\n\n";

pub fn path() -> PathBuf {
    config::config_dir().join("snippets.txt")
}

fn mtime() -> Option<SystemTime> {
    std::fs::metadata(path()).and_then(|m| m.modified()).ok()
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock();
    let disk = mtime();
    if guard.as_ref().is_none_or(|s| s.mtime != disk) {
        let items = std::fs::read_to_string(path()).map(|t| parse(&t)).unwrap_or_default();
        *guard = Some(State { items, mtime: disk });
    }
    f(guard.as_mut().expect("estado cargado"))
}

pub fn with_items<R>(f: impl FnOnce(&[Snippet]) -> R) -> R {
    with_state(|s| f(&s.items))
}

/// Guarda un texto con un nombre sacado de su primera línea. Devuelve el nombre.
pub fn add(text: &str) -> String {
    with_state(|s| {
        let base = crate::clipboard::preview(text, 40);
        let mut name = base.clone();
        let mut n = 2;
        while s.items.iter().any(|i| i.name == name) {
            name = format!("{base} ({n})");
            n += 1;
        }
        s.items.push(Snippet { name: name.clone(), text: text.to_string() });
        save(s);
        name
    })
}

pub fn remove(name: &str) {
    with_state(|s| {
        s.items.retain(|i| i.name != name);
        save(s);
    });
}

/// Crea el archivo con su explicación si todavía no existe (para editarlo a mano).
pub fn ensure_file() -> PathBuf {
    if !path().exists() {
        with_state(save);
    }
    path()
}

fn save(s: &mut State) {
    let mut out = String::from(HEADER);
    for i in &s.items {
        out.push_str("## ");
        out.push_str(&i.name);
        out.push('\n');
        out.push_str(&i.text);
        out.push_str("\n\n");
    }
    let tmp = path().with_extension("tmp");
    if std::fs::write(&tmp, out).is_ok() {
        let _ = std::fs::rename(tmp, path());
    }
    s.mtime = mtime();
}

fn parse(text: &str) -> Vec<Snippet> {
    let mut items: Vec<Snippet> = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    let flush = |cur: Option<(String, Vec<&str>)>, items: &mut Vec<Snippet>| {
        if let Some((name, lines)) = cur {
            let text = lines.join("\n").trim_end().to_string();
            if !name.is_empty() && !text.is_empty() {
                items.push(Snippet { name, text });
            }
        }
    };
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            flush(current.take(), &mut items);
            current = Some((name.trim().to_string(), Vec::new()));
        } else if let Some((_, lines)) = &mut current {
            lines.push(line);
        }
    }
    flush(current, &mut items);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file() {
        let t = "# comentario\nignorado\n## firma\nAbraham\nACY Solutions\n\n## vacío\n\n## cuenta\n  0123 4567\n";
        let items = parse(t);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], Snippet { name: "firma".into(), text: "Abraham\nACY Solutions".into() });
        assert_eq!(items[1].text, "  0123 4567");
    }
}
