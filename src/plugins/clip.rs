//! Historial del portapapeles: `cb` lista lo último que copiaste, `cb texto`
//! filtra. Enter lo vuelve a copiar; Shift+Supr lo quita del historial.

use super::{Action, Fuzzy, Glyph, Icon, Item, KeroCmd, Mode, Plugin, Query};
use crate::clipboard;

pub struct Clip;

pub const KEYWORD: &str = "cb";

/// `Some(filtro)` si la consulta pide el historial ("cb", "cb texto").
pub fn filter(text: &str) -> Option<&str> {
    let (head, rest) = text.split_once(' ').unwrap_or((text, ""));
    head.eq_ignore_ascii_case(KEYWORD).then(|| rest.trim())
}

impl Plugin for Clip {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::Clipboard {
            return;
        }
        let needle = q.text;
        if !q.cfg.clipboard.enabled {
            out.push(Item::new(
                "El historial del portapapeles está desactivado",
                "Actívalo en config.toml: [clipboard] enabled = true",
                Icon::Glyph(Glyph::Clipboard),
                Action::Kero(KeroCmd::OpenConfig),
                30_000,
            ));
            return;
        }
        let mut fz = (!needle.is_empty()).then(|| Fuzzy::new(needle));
        let n = clipboard::with_items(|items| {
            for (i, c) in items.iter().enumerate() {
                let title = clipboard::preview(&c.text, 120);
                let score = match &mut fz {
                    None => 30_000 - i as i32,
                    // Busca en todo el texto, no sólo en la primera línea.
                    Some(f) => match f.score(&c.text) {
                        Some(s) => 20_000 + s as i32 - i as i32,
                        None => continue,
                    },
                };
                let lines = c.text.lines().count();
                let mut sub = clipboard::ago(c.time);
                if lines > 1 {
                    sub.push_str(&format!(" · {lines} líneas"));
                }
                let mut it = Item::new(title, sub, Icon::Glyph(Glyph::Clipboard), Action::Clip(c.text.clone()), score);
                if let Some(f) = &mut fz {
                    it.highlight = f.indices(&it.title.clone());
                }
                out.push(it);
            }
            items.len()
        });
        if n == 0 {
            out.push(Item::new(
                "El historial está vacío",
                "Lo que copies a partir de ahora aparecerá aquí",
                Icon::Glyph(Glyph::Clipboard),
                Action::None,
                30_000,
            ));
        } else if needle.is_empty() {
            out.push(Item::new(
                "Borrar historial del portapapeles",
                format!("{n} entradas"),
                Icon::Glyph(Glyph::Trash),
                Action::Kero(KeroCmd::ClearClipboard),
                0,
            ));
        }
    }
}
