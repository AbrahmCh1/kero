//! Herramientas de texto: `uuid`, `contraseña 24`, `base64 hola`, `url a b`,
//! `sha256 texto`, `json`, `contar`. Sin texto, trabajan sobre el portapapeles.

use super::{Action, Glyph, Icon, Item, Mode, Plugin, Query};
use crate::digest;

pub struct TextTools;

const SCORE: i32 = 11_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Uuid,
    Password,
    Base64,
    Url,
    Md5,
    Sha1,
    Sha256,
    Hash,
    Json,
    Count,
}

/// Palabra clave → herramienta. También sirve para la ayuda.
const TOOLS: &[(&str, Tool)] = &[
    ("uuid", Tool::Uuid),
    ("guid", Tool::Uuid),
    ("contraseña", Tool::Password),
    ("contrasena", Tool::Password),
    ("password", Tool::Password),
    ("pw", Tool::Password),
    ("base64", Tool::Base64),
    ("b64", Tool::Base64),
    ("url", Tool::Url),
    ("md5", Tool::Md5),
    ("sha1", Tool::Sha1),
    ("sha256", Tool::Sha256),
    ("hash", Tool::Hash),
    ("json", Tool::Json),
    ("contar", Tool::Count),
    ("count", Tool::Count),
];

fn copy_item(title: impl Into<String>, what: &str, copy: String) -> Item {
    Item::new(title, format!("{what} · Enter copia"), Icon::Glyph(Glyph::Text), Action::Copy(copy), SCORE)
}

/// Una línea para mostrar un resultado largo.
fn shown(s: &str) -> String {
    crate::clipboard::preview(s, 90)
}

impl Plugin for TextTools {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::All {
            return;
        }
        let (head, rest) = q.text.split_once(' ').unwrap_or((q.text, ""));
        let head = head.to_lowercase();
        let Some(&(_, tool)) = TOOLS.iter().find(|(k, _)| *k == head) else { return };
        let rest = rest.trim();
        // "contraseña fiel" busca un archivo; "contraseña 24" sí es para generar.
        if matches!(tool, Tool::Uuid | Tool::Password) && !rest.is_empty() && rest.parse::<usize>().is_err() {
            return;
        }
        match tool {
            Tool::Uuid => {
                let id = uuid_v4();
                out.push(copy_item(id.clone(), "UUID v4", id.clone()));
                let mut upper = copy_item(id.to_uppercase(), "UUID v4 en mayúsculas", id.to_uppercase());
                upper.score -= 1;
                out.push(upper);
            }
            Tool::Password => {
                let len = rest.parse::<usize>().unwrap_or(20).clamp(4, 128);
                for (i, (chars, what)) in [(FULL, "con símbolos"), (ALNUM, "sólo letras y números")].iter().enumerate()
                {
                    let pw = password(len, chars);
                    out.push(Item::new(
                        pw.clone(),
                        format!("{len} caracteres {what} · Enter copia sin dejarla en el historial"),
                        Icon::Glyph(Glyph::Lock),
                        Action::Secret(pw),
                        SCORE - i as i32,
                    ));
                }
            }
            _ => {
                // El resto transforma un texto: el escrito o, si no hay, el del portapapeles.
                let (input, source) = if rest.is_empty() {
                    match crate::platform::read_clipboard() {
                        Some(t) if !t.is_empty() => (t, " (del portapapeles)"),
                        _ => {
                            out.push(Item::new(
                                format!("{head} <texto>"),
                                "Escribe el texto o copia algo antes",
                                Icon::Glyph(Glyph::Text),
                                Action::None,
                                SCORE,
                            ));
                            return;
                        }
                    }
                } else {
                    (rest.to_string(), "")
                };
                transform(tool, &input, source, out);
            }
        }
    }
}

fn transform(tool: Tool, input: &str, source: &str, out: &mut Vec<Item>) {
    let mut push = |title: String, what: String, copy: String| {
        let mut it = copy_item(shown(&title), &what, copy);
        it.score -= out.len() as i32;
        out.push(it);
    };
    match tool {
        Tool::Base64 => {
            let decoded = base64_decode(input.trim()).and_then(|b| String::from_utf8(b).ok());
            let enc = base64_encode(input.as_bytes());
            // Si ya parece base64, lo útil suele ser decodificarlo.
            if let Some(d) = decoded.filter(|d| !d.trim().is_empty()) {
                push(d.clone(), format!("Base64 decodificado{source}"), d);
            }
            push(enc.clone(), format!("Base64{source}"), enc);
        }
        Tool::Url => {
            let decoded = url_decode(input);
            if decoded != input {
                push(decoded.clone(), format!("URL decodificada{source}"), decoded);
            }
            let enc = url_encode(input);
            push(enc.clone(), format!("URL codificada{source}"), enc);
        }
        Tool::Md5 | Tool::Sha1 | Tool::Sha256 | Tool::Hash => {
            type Digest = fn(&[u8]) -> String;
            let algos: &[(&str, Digest)] =
                &[("SHA-256", digest::sha256), ("SHA-1", digest::sha1), ("MD5", digest::md5)];
            for (name, f) in algos {
                let wanted = match tool {
                    Tool::Md5 => *name == "MD5",
                    Tool::Sha1 => *name == "SHA-1",
                    Tool::Sha256 => *name == "SHA-256",
                    _ => true,
                };
                if wanted {
                    let h = f(input.as_bytes());
                    push(h.clone(), format!("{name}{source}"), h);
                }
            }
        }
        Tool::Json => match (json_format(input, true), json_format(input, false)) {
            (Some(pretty), Some(min)) => {
                let lines = pretty.lines().count();
                push(min.clone(), format!("JSON formateado ({lines} líneas){source}"), pretty);
                push(min.clone(), format!("JSON minificado{source}"), min);
            }
            _ => push("No es JSON válido".into(), format!("JSON{source}"), String::new()),
        },
        Tool::Count => {
            let chars = input.chars().count();
            let words = input.split_whitespace().count();
            let lines = input.lines().count();
            push(
                format!("{chars} caracteres · {words} palabras · {lines} líneas"),
                format!("{} bytes{source}", input.len()),
                chars.to_string(),
            );
        }
        Tool::Uuid | Tool::Password => {}
    }
    // Un JSON inválido no tiene nada que copiar.
    if let Some(it) = out.last_mut()
        && matches!(&it.action, Action::Copy(s) if s.is_empty())
    {
        it.action = Action::None;
        it.subtitle = it.subtitle.replace(" · Enter copia", "");
    }
}

// ---------------------------------------------------------------------------

/// Bytes aleatorios del sistema operativo.
fn random(buf: &mut [u8]) {
    if getrandom::fill(buf).is_err() {
        // No debería pasar. `RandomState` también se siembra con azar del sistema.
        use std::hash::BuildHasher;
        let s = std::collections::hash_map::RandomState::new();
        for (i, chunk) in buf.chunks_mut(8).enumerate() {
            chunk.copy_from_slice(&s.hash_one(i).to_le_bytes()[..chunk.len()]);
        }
    }
}

fn uuid_v4() -> String {
    let mut b = [0u8; 16];
    random(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

const ALNUM: &str = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const FULL: &str = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#$%&*-_=+?";

/// Contraseña con al menos una minúscula, mayúscula, dígito (y símbolo si hay).
/// Sin caracteres que se confunden (l, 1, I, O, 0). Sin sesgo de módulo.
fn password(len: usize, alphabet: &str) -> String {
    let chars: Vec<char> = alphabet.chars().collect();
    let classes: [fn(char) -> bool; 4] = [
        |c| c.is_ascii_lowercase(),
        |c| c.is_ascii_uppercase(),
        |c| c.is_ascii_digit(),
        |c| !c.is_ascii_alphanumeric(),
    ];
    let limit = 256 - 256 % chars.len();
    loop {
        let mut out = String::with_capacity(len);
        let mut buf = [0u8; 64];
        'fill: while out.len() < len {
            random(&mut buf);
            for &b in &buf {
                if (b as usize) < limit {
                    out.push(chars[b as usize % chars.len()]);
                    if out.len() == len {
                        break 'fill;
                    }
                }
            }
        }
        let ok = classes.iter().all(|f| !chars.iter().any(|&c| f(c)) || out.chars().any(f));
        if ok {
            return out;
        }
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, &b)| acc | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Acepta el alfabeto normal y el de URLs, con o sin relleno.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    if s.len() < 2 || s.len() % 4 == 1 {
        return None;
    }
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace()) {
        acc = (acc << 6) | val(c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        match b[i] {
            b'%' if i + 2 < b.len() => match (hex(b[i + 1]), hex(b[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Reindenta (o compacta) un JSON sin construir el árbol. `None` si no lo parece.
pub fn json_format(s: &str, pretty: bool) -> Option<String> {
    let s = s.trim();
    if !(s.starts_with('{') || s.starts_with('[')) {
        return None;
    }
    let mut out = String::with_capacity(s.len() * 2);
    let mut stack = Vec::new();
    let mut in_str = false;
    let mut escaped = false;
    let newline = |out: &mut String, depth: usize| {
        if pretty {
            out.push('\n');
            out.extend(std::iter::repeat_n("  ", depth));
        }
    };
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '{' | '[' => {
                stack.push(if c == '{' { '}' } else { ']' });
                out.push(c);
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
                if chars.peek() == stack.last() {
                    out.push(chars.next()?);
                    stack.pop();
                } else {
                    newline(&mut out, stack.len());
                }
            }
            '}' | ']' => {
                if stack.pop() != Some(c) {
                    return None;
                }
                newline(&mut out, stack.len());
                out.push(c);
            }
            ',' => {
                out.push(c);
                newline(&mut out, stack.len());
            }
            ':' => out.push_str(if pretty { ": " } else { ":" }),
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
        if stack.is_empty() {
            // Lo que quede después del valor raíz tiene que ser espacio.
            return chars.all(char::is_whitespace).then_some(out);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        for s in ["", "a", "ab", "abc", "hola mundo", "ñandú ✓"] {
            assert_eq!(base64_decode(&base64_encode(s.as_bytes())).unwrap_or_default(), s.as_bytes());
        }
        assert_eq!(base64_encode(b"hola"), "aG9sYQ==");
        assert_eq!(base64_decode("aG9sYQ").unwrap(), b"hola");
        assert!(base64_decode("no es base64!").is_none());
    }

    #[test]
    fn url_roundtrip() {
        assert_eq!(url_encode("a b&c=ñ"), "a%20b%26c%3D%C3%B1");
        assert_eq!(url_decode("a%20b%26c%3D%C3%B1"), "a b&c=ñ");
        assert_eq!(url_decode("100%"), "100%");
        assert_eq!(url_decode("a+b"), "a b");
    }

    #[test]
    fn json() {
        let j = r#"{"a":[1, 2,{}],"b":{"c":"x, \"y\" {"} ,"d":[]}"#;
        assert_eq!(
            json_format(j, true).unwrap(),
            "{\n  \"a\": [\n    1,\n    2,\n    {}\n  ],\n  \"b\": {\n    \"c\": \"x, \\\"y\\\" {\"\n  },\n  \"d\": []\n}"
        );
        assert_eq!(json_format(j, false).unwrap(), r#"{"a":[1,2,{}],"b":{"c":"x, \"y\" {"},"d":[]}"#);
        assert!(json_format("{\"a\": [1}", true).is_none());
        assert!(json_format("hola", true).is_none());
        assert!(json_format("{} extra", true).is_none());
    }

    #[test]
    fn generators() {
        let id = uuid_v4();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        let pw = password(20, FULL);
        assert_eq!(pw.chars().count(), 20);
        assert!(pw.chars().any(|c| c.is_ascii_digit()) && pw.chars().any(|c| !c.is_ascii_alphanumeric()));
        assert!(password(12, ALNUM).chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
