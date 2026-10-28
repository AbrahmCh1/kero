//! ">comando" ejecuta en una terminal. Shift+Enter lo lanza como administrador.

use super::{Action, Glyph, Icon, Item, Mode, Plugin, Query};

pub struct Shell;

impl Plugin for Shell {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::Shell {
            return;
        }
        let cmd = q.text.trim();
        if cmd.is_empty() {
            out.push(Item::new(
                "Escribe un comando…",
                "Se ejecuta en una terminal nueva",
                Icon::Glyph(Glyph::Terminal),
                Action::None,
                0,
            ));
            return;
        }
        out.push(Item::new(
            cmd,
            "Ejecutar en terminal",
            Icon::Glyph(Glyph::Terminal),
            Action::Shell(cmd.to_string()),
            10_000,
        ));
    }
}
