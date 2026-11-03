//! Textos guardados: `sn` los lista, `sn texto` filtra y, en la búsqueda normal,
//! aparecen por su nombre. Enter copia; Shift+Supr lo borra.

use super::{Action, Fuzzy, Glyph, Icon, Item, KeroCmd, Mode, Plugin, Query, quality};
use crate::{clipboard, snippets};

pub struct Snip;

pub const KEYWORD: &str = "sn";

/// `Some(filtro)` si la consulta pide los textos guardados ("sn", "sn texto").
pub fn filter(text: &str) -> Option<&str> {
    let (head, rest) = text.split_once(' ').unwrap_or((text, ""));
    head.eq_ignore_ascii_case(KEYWORD).then(|| rest.trim())
}

fn item(s: &snippets::Snippet, score: i32) -> Item {
    let lines = s.text.lines().count();
    let mut sub = clipboard::preview(&s.text, 80);
    if lines > 1 {
        sub.push_str(&format!(" · {lines} líneas"));
    }
    Item::new(
        s.name.clone(),
        sub,
        Icon::Glyph(Glyph::Snippet),
        Action::Snippet { name: s.name.clone(), text: s.text.clone() },
        score,
    )
}

impl Plugin for Snip {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        match q.mode {
            Mode::Snippets => list(q.text, out),
            // En la búsqueda normal sólo si el nombre coincide de verdad.
            Mode::All if q.text.chars().count() >= 2 => {
                let mut fz = Fuzzy::new(q.text);
                let lower = q.text.to_lowercase();
                snippets::with_items(|items| {
                    for s in items {
                        let Some(score) = fz.score(&s.name) else { continue };
                        let (bonus, strong) = quality(&s.name, &lower);
                        if !strong {
                            continue;
                        }
                        let key = format!("sn:{}", s.name);
                        let mut it = item(s, score as i32 + bonus + 40 + q.usage.bonus(&key, &lower));
                        it.highlight = fz.indices(&s.name);
                        it.key = Some(key);
                        out.push(it);
                    }
                });
            }
            _ => {}
        }
    }
}

fn list(needle: &str, out: &mut Vec<Item>) {
    let mut fz = (!needle.is_empty()).then(|| Fuzzy::new(needle));
    let n = snippets::with_items(|items| {
        for (i, s) in items.iter().enumerate() {
            let score = match &mut fz {
                None => 30_000 - i as i32,
                Some(f) => match f.score(&s.name).map(|v| v + 40).max(f.score(&s.text)) {
                    Some(v) => 20_000 + v as i32,
                    None => continue,
                },
            };
            let mut it = item(s, score);
            if let Some(f) = &mut fz {
                it.highlight = f.indices(&s.name);
            }
            out.push(it);
        }
        items.len()
    });
    if n == 0 {
        out.push(Item::new(
            "Todavía no tienes textos guardados",
            "Enter abre snippets.txt · o guarda uno desde el historial (cb) con Ctrl+S",
            Icon::Glyph(Glyph::Snippet),
            Action::Kero(KeroCmd::EditSnippets),
            30_000,
        ));
    } else if needle.is_empty() {
        out.push(Item::new(
            "Editar textos guardados",
            "Abre snippets.txt",
            Icon::Glyph(Glyph::Settings),
            Action::Kero(KeroCmd::EditSnippets),
            0,
        ));
    }
}
