//! Búsquedas web por palabra clave ("g gatos", "yt lofi"), apertura de URLs
//! escritas a mano y una búsqueda web de respaldo al final de la lista.

use super::{Action, Icon, Item, Mode, Plugin, Query, glyph};

pub struct Web;

pub fn encode(q: &str) -> String {
    let mut s = String::with_capacity(q.len() * 3);
    for b in q.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => s.push(b as char),
            b' ' => s.push('+'),
            _ => s.push_str(&format!("%{b:02X}")),
        }
    }
    s
}

fn as_url(s: &str) -> Option<String> {
    let s = s.trim();
    if s.contains(char::is_whitespace) || s.len() < 4 {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(s.to_string());
    }
    if lower.starts_with("localhost:") {
        return Some(format!("http://{s}"));
    }
    if lower.starts_with("www.") {
        return Some(format!("https://{s}"));
    }
    // dominio.tld[/ruta]
    let host = lower.split('/').next()?;
    let (name, tld) = host.rsplit_once('.')?;
    let tld_ok = (2..=12).contains(&tld.len()) && tld.bytes().all(|b| b.is_ascii_alphabetic());
    let name_ok = !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        && !name.starts_with('.');
    // Evita que "informe.pdf" o "main.rs" parezcan URLs.
    const FILE_EXT: [&str; 14] =
        ["pdf", "txt", "rs", "js", "ts", "py", "md", "exe", "png", "jpg", "docx", "xlsx", "zip", "json"];
    if tld_ok && name_ok && !FILE_EXT.contains(&tld) {
        return Some(format!("https://{s}"));
    }
    None
}

impl Plugin for Web {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::All || q.text.is_empty() {
            return;
        }
        if let Some((kw, rest)) = q.text.split_once(' ') {
            let rest = rest.trim();
            if let Some(engine) = q.cfg.web_engines.iter().find(|e| e.keyword.eq_ignore_ascii_case(kw)) {
                let url = engine.url.replace("{q}", &encode(rest));
                out.push(Item::new(
                    if rest.is_empty() { format!("Buscar en {}", engine.name) } else { rest.to_string() },
                    format!("Buscar en {}", engine.name),
                    Icon::Glyph(glyph::GLOBE),
                    Action::Url(url),
                    9_000,
                ));
            }
        }

        if let Some(url) = as_url(q.text) {
            out.push(Item::new(q.text.trim(), "Abrir en el navegador", Icon::Glyph(glyph::LINK), Action::Url(url), 8_000));
        }

        // Respaldo: siempre al final.
        if let Some(engine) = q.cfg.web_engines.first() {
            if q.text.chars().count() >= 2 {
                out.push(Item::new(
                    format!("Buscar \u{201C}{}\u{201D}", q.text.trim()),
                    format!("en {}", engine.name),
                    Icon::Glyph(glyph::SEARCH),
                    Action::Url(engine.url.replace("{q}", &encode(q.text.trim()))),
                    i32::MIN / 2,
                ));
            }
        }
    }
}
