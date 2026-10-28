//! "#ff8800", "#f80" o "rgb(255,136,0)" → vista previa y conversiones copiables.

use super::{Action, Icon, Item, Mode, Plugin, Query};

pub struct ColorPlugin;

fn parse(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        let hex: String = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 | 8 => hex[..6].to_string(),
            _ => return None,
        };
        let v = u32::from_str_radix(&hex, 16).ok()?;
        return Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]);
    }
    let inner = s.strip_prefix("rgb(").or_else(|| s.strip_prefix("rgba("))?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|p| p.trim().parse::<u8>().ok());
    Some([it.next()??, it.next()??, it.next()??])
}

fn hsl([r, g, b]: [u8; 3]) -> (u32, u32, u32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0, 0, (l * 100.0).round() as u32);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h.round() as u32, (s * 100.0).round() as u32, (l * 100.0).round() as u32)
}

impl Plugin for ColorPlugin {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::All {
            return;
        }
        let Some(c) = parse(q.text) else { return };
        let [r, g, b] = c;
        let (h, s, l) = hsl(c);
        let formats = [
            (format!("#{r:02X}{g:02X}{b:02X}"), "HEX"),
            (format!("rgb({r}, {g}, {b})"), "RGB"),
            (format!("hsl({h}, {s}%, {l}%)"), "HSL"),
        ];
        for (i, (text, label)) in formats.into_iter().enumerate() {
            out.push(Item::new(
                text.clone(),
                format!("{label} · Enter para copiar"),
                Icon::Color(c),
                Action::Copy(text),
                9_500 - i as i32,
            ));
        }
    }
}
