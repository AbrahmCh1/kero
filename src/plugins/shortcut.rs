//! Atajos propios de `config.toml`: `[[shortcut]] name = "Proyectos"` con
//! `open = "D:\\Proyectos"` (carpeta, archivo o URL) o `run = "comando"`.

use super::{Action, Fuzzy, Glyph, Icon, Item, Mode, Plugin, Query, quality};
use crate::config::{self, Shortcut};

pub struct Shortcuts;

/// Bonus como el de las apps: los creaste tú, así que van arriba.
const BONUS: i32 = 90;

fn target(s: &Shortcut) -> (Icon, Action, String) {
    if !s.run.trim().is_empty() {
        return (Icon::Glyph(Glyph::Terminal), Action::Shell(s.run.trim().to_string()), s.run.trim().to_string());
    }
    let open = s.open.trim();
    let is_uri = open.contains("://")
        || open.split_once(':').is_some_and(|(scheme, _)| scheme.len() > 1 && !scheme.contains(['\\', '/']));
    if is_uri {
        return (Icon::Glyph(Glyph::Link), Action::Url(open.to_string()), open.to_string());
    }
    let path = config::expand_path(open);
    let shown = config::display_path(&path);
    (Icon::Path(path.clone(), path.is_dir()), Action::Open(path), shown)
}

impl Plugin for Shortcuts {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::All || q.text.is_empty() || q.cfg.shortcuts.is_empty() {
            return;
        }
        let mut fz = Fuzzy::new(q.text);
        let lower = q.text.to_lowercase();
        for s in &q.cfg.shortcuts {
            if s.name.trim().is_empty() || (s.open.trim().is_empty() && s.run.trim().is_empty()) {
                continue;
            }
            let Some(score) = fz.score(&s.name) else { continue };
            let (bonus, strong) = quality(&s.name, &lower);
            let (icon, action, shown) = target(s);
            let key = format!("sc:{}", s.name);
            let mut it = Item::new(
                s.name.clone(),
                format!("Atajo · {shown}"),
                icon,
                action,
                score as i32 + bonus + if strong { BONUS } else { 0 } + q.usage.bonus(&key, &lower),
            );
            it.highlight = fz.indices(&s.name);
            it.weak = !strong;
            it.key = Some(key);
            out.push(it);
        }
    }
}
