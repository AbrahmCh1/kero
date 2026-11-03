//! Historial del portapapeles: sólo texto. Vive en memoria y en `clipboard.txt`
//! (una entrada por línea, escapada) para que `kero clip` también lo vea.
//! Lo que los gestores de contraseñas marcan como privado no entra (Windows).

use crate::config;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub text: String,
    /// Segundos Unix.
    pub time: u64,
}

/// Textos más grandes (un archivo entero, un log) no se guardan.
const MAX_BYTES: usize = 32 * 1024;

struct State {
    items: Vec<Clip>,
    /// Para notar cambios hechos por otro proceso (`kero clip borrar`).
    mtime: Option<SystemTime>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn path() -> PathBuf {
    config::data_dir().join("clipboard.txt")
}

fn mtime() -> Option<SystemTime> {
    std::fs::metadata(path()).and_then(|m| m.modified()).ok()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock();
    let disk = mtime();
    if guard.as_ref().is_none_or(|s| s.mtime != disk) {
        *guard = Some(State { items: load(), mtime: disk });
    }
    f(guard.as_mut().expect("estado cargado"))
}

/// Recorre el historial (más reciente primero) sin copiarlo.
pub fn with_items<R>(f: impl FnOnce(&[Clip]) -> R) -> R {
    with_state(|s| f(&s.items))
}

/// Huella de algo que Kero copió en privado (una contraseña generada): si el
/// vigilante lo ve llegar, no lo guarda. En Linux no hay otra forma de saberlo.
static PRIVATE: Mutex<Option<u64>> = Mutex::new(None);

fn fingerprint(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

pub fn ignore(text: &str) {
    *PRIVATE.lock() = Some(fingerprint(text));
}

/// Añade un texto copiado. `false` si no cambió nada (repetido, vacío o enorme).
pub fn push(text: String, max_items: usize) -> bool {
    if text.trim().is_empty() || text.len() > MAX_BYTES {
        return false;
    }
    if PRIVATE.lock().is_some_and(|fp| fp == fingerprint(&text)) {
        return false;
    }
    with_state(|s| {
        if s.items.first().is_some_and(|c| c.text == text) {
            return false;
        }
        s.items.retain(|c| c.text != text);
        s.items.insert(0, Clip { text, time: now() });
        s.items.truncate(max_items.max(1));
        save(s);
        true
    })
}

pub fn remove(text: &str) {
    with_state(|s| {
        s.items.retain(|c| c.text != text);
        save(s);
    });
}

pub fn clear() {
    with_state(|s| {
        s.items.clear();
        save(s);
    });
}

fn save(s: &mut State) {
    let mut out = String::new();
    for c in &s.items {
        out.push_str(&c.time.to_string());
        out.push('\t');
        out.push_str(&escape(&c.text));
        out.push('\n');
    }
    let tmp = path().with_extension("tmp");
    if std::fs::write(&tmp, out).is_ok() {
        let _ = std::fs::rename(tmp, path());
    }
    s.mtime = mtime();
}

fn load() -> Vec<Clip> {
    let Ok(text) = std::fs::read_to_string(path()) else { return Vec::new() };
    text.lines()
        .filter_map(|l| {
            let (t, body) = l.split_once('\t')?;
            Some(Clip { time: t.parse().ok()?, text: unescape(body) })
        })
        .collect()
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

/// Primera línea con contenido, con espacios colapsados y recortada a `max` caracteres.
pub fn preview(text: &str, max: usize) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let mut out: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > max {
        out = out.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
    }
    out
}

/// "hace 5 min", "hace 2 h", "ayer"…
pub fn ago(time: u64) -> String {
    let s = now().saturating_sub(time);
    match s {
        0..60 => "ahora".into(),
        60..3600 => format!("hace {} min", s / 60),
        3600..86400 => format!("hace {} h", s / 3600),
        86400..172800 => "ayer".into(),
        _ => format!("hace {} días", s / 86400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_roundtrip() {
        for s in ["hola", "a\tb\nc\\d\r", "\\n literal", "fin\\"] {
            assert_eq!(unescape(&escape(s)), s);
            assert!(!escape(s).contains('\n'));
        }
    }

    #[test]
    fn preview_first_line() {
        assert_eq!(preview("\n  hola   mundo \nsegunda", 80), "hola mundo");
        assert_eq!(preview("abcdefghij", 5), "abcd…");
    }
}
