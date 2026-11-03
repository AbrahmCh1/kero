//! Horas y fechas: `hora en tokio`, `15:00 ny a cdmx`, `hoy + 90 días`,
//! `días hasta 24 dic`, `1730592000` → fecha. Todo se calcula al escribir.

use super::{Action, Glyph, Icon, Item, Mode, Plugin, Query};
use crate::tz::{self, civil_from_days, days_from_civil, days_in_month, iso_week, weekday};

pub struct Time;

const SCORE: i32 = 11_000;

struct City {
    names: &'static [&'static str],
    label: &'static str,
    zone: &'static str,
}

macro_rules! c {
    ($label:expr, $zone:expr, [$($n:expr),*]) => {
        City { names: &[$($n),*], label: $label, zone: $zone }
    };
}

/// Nombres sin acentos y en minúsculas (así se comparan).
const CITIES: &[City] = &[
    c!(
        "Ciudad de México",
        "America/Mexico_City",
        [
            "cdmx",
            "mexico",
            "ciudad de mexico",
            "df",
            "guadalajara",
            "gdl",
            "monterrey",
            "mty",
            "puebla",
            "queretaro",
            "leon",
            "merida"
        ]
    ),
    c!("Cancún", "America/Cancun", ["cancun", "quintana roo", "playa del carmen"]),
    c!("Tijuana", "America/Tijuana", ["tijuana", "tj", "mexicali", "baja california"]),
    c!("Hermosillo", "America/Hermosillo", ["hermosillo", "sonora"]),
    c!("Chihuahua", "America/Chihuahua", ["chihuahua"]),
    c!("Mazatlán", "America/Mazatlan", ["mazatlan", "culiacan", "sinaloa", "la paz bcs"]),
    c!(
        "Nueva York",
        "America/New_York",
        [
            "ny",
            "nyc",
            "nueva york",
            "new york",
            "miami",
            "boston",
            "washington",
            "atlanta",
            "filadelfia",
            "philadelphia",
            "detroit",
            "orlando",
            "est",
            "edt"
        ]
    ),
    c!(
        "Chicago",
        "America/Chicago",
        ["chicago", "texas", "houston", "dallas", "austin", "san antonio", "nueva orleans", "cst", "cdt"]
    ),
    c!("Denver", "America/Denver", ["denver", "salt lake city", "mst", "mdt"]),
    c!("Phoenix", "America/Phoenix", ["phoenix", "arizona"]),
    c!(
        "Los Ángeles",
        "America/Los_Angeles",
        [
            "la",
            "los angeles",
            "sf",
            "san francisco",
            "seattle",
            "california",
            "las vegas",
            "san diego",
            "portland",
            "pst",
            "pdt"
        ]
    ),
    c!("Anchorage", "America/Anchorage", ["anchorage", "alaska"]),
    c!("Honolulu", "Pacific/Honolulu", ["honolulu", "hawaii"]),
    c!("Toronto", "America/Toronto", ["toronto", "canada", "montreal", "ottawa"]),
    c!("Vancouver", "America/Vancouver", ["vancouver"]),
    c!("Guatemala", "America/Guatemala", ["guatemala"]),
    c!("San Salvador", "America/El_Salvador", ["san salvador", "el salvador"]),
    c!("Tegucigalpa", "America/Tegucigalpa", ["tegucigalpa", "honduras"]),
    c!("Managua", "America/Managua", ["managua", "nicaragua"]),
    c!("San José", "America/Costa_Rica", ["costa rica", "san jose"]),
    c!("Panamá", "America/Panama", ["panama"]),
    c!("La Habana", "America/Havana", ["la habana", "habana", "cuba"]),
    c!("Santo Domingo", "America/Santo_Domingo", ["santo domingo", "republica dominicana"]),
    c!("San Juan", "America/Puerto_Rico", ["puerto rico", "san juan"]),
    c!("Bogotá", "America/Bogota", ["bogota", "colombia", "medellin", "cali"]),
    c!("Lima", "America/Lima", ["lima", "peru"]),
    c!("Quito", "America/Guayaquil", ["quito", "ecuador", "guayaquil"]),
    c!("Caracas", "America/Caracas", ["caracas", "venezuela"]),
    c!("La Paz", "America/La_Paz", ["la paz", "bolivia"]),
    c!("Santiago", "America/Santiago", ["santiago", "chile"]),
    c!("Buenos Aires", "America/Argentina/Buenos_Aires", ["buenos aires", "argentina", "cordoba"]),
    c!("Montevideo", "America/Montevideo", ["montevideo", "uruguay"]),
    c!("Asunción", "America/Asuncion", ["asuncion", "paraguay"]),
    c!("São Paulo", "America/Sao_Paulo", ["sao paulo", "brasil", "brazil", "rio", "rio de janeiro"]),
    c!("Londres", "Europe/London", ["londres", "london", "reino unido", "uk", "inglaterra"]),
    c!("Dublín", "Europe/Dublin", ["dublin", "irlanda"]),
    c!("Lisboa", "Europe/Lisbon", ["lisboa", "portugal"]),
    c!("Madrid", "Europe/Madrid", ["madrid", "espana", "barcelona", "valencia", "sevilla"]),
    c!("París", "Europe/Paris", ["paris", "francia"]),
    c!("Berlín", "Europe/Berlin", ["berlin", "alemania", "munich", "frankfurt"]),
    c!("Ámsterdam", "Europe/Amsterdam", ["amsterdam", "holanda", "paises bajos"]),
    c!("Bruselas", "Europe/Brussels", ["bruselas", "belgica"]),
    c!("Zúrich", "Europe/Zurich", ["zurich", "suiza", "ginebra"]),
    c!("Viena", "Europe/Vienna", ["viena", "austria"]),
    c!("Roma", "Europe/Rome", ["roma", "italia", "milan"]),
    c!("Estocolmo", "Europe/Stockholm", ["estocolmo", "suecia"]),
    c!("Oslo", "Europe/Oslo", ["oslo", "noruega"]),
    c!("Copenhague", "Europe/Copenhagen", ["copenhague", "dinamarca"]),
    c!("Helsinki", "Europe/Helsinki", ["helsinki", "finlandia"]),
    c!("Varsovia", "Europe/Warsaw", ["varsovia", "polonia"]),
    c!("Praga", "Europe/Prague", ["praga", "chequia"]),
    c!("Atenas", "Europe/Athens", ["atenas", "grecia"]),
    c!("Kiev", "Europe/Kyiv", ["kiev", "kyiv", "ucrania"]),
    c!("Moscú", "Europe/Moscow", ["moscu", "rusia"]),
    c!("Estambul", "Europe/Istanbul", ["estambul", "turquia"]),
    c!("El Cairo", "Africa/Cairo", ["el cairo", "cairo", "egipto"]),
    c!("Casablanca", "Africa/Casablanca", ["casablanca", "marruecos"]),
    c!("Lagos", "Africa/Lagos", ["lagos", "nigeria"]),
    c!("Nairobi", "Africa/Nairobi", ["nairobi", "kenia"]),
    c!("Johannesburgo", "Africa/Johannesburg", ["johannesburgo", "sudafrica", "ciudad del cabo"]),
    c!("Tel Aviv", "Asia/Jerusalem", ["tel aviv", "jerusalen", "israel"]),
    c!("Riad", "Asia/Riyadh", ["riad", "arabia saudita"]),
    c!("Dubái", "Asia/Dubai", ["dubai", "emiratos", "abu dabi"]),
    c!("Karachi", "Asia/Karachi", ["karachi", "pakistan"]),
    c!("Nueva Delhi", "Asia/Kolkata", ["delhi", "nueva delhi", "india", "mumbai", "bombay", "bangalore"]),
    c!("Bangkok", "Asia/Bangkok", ["bangkok", "tailandia"]),
    c!("Yakarta", "Asia/Jakarta", ["yakarta", "indonesia"]),
    c!("Singapur", "Asia/Singapore", ["singapur", "singapore"]),
    c!("Kuala Lumpur", "Asia/Kuala_Lumpur", ["kuala lumpur", "malasia"]),
    c!("Manila", "Asia/Manila", ["manila", "filipinas"]),
    c!("Hong Kong", "Asia/Hong_Kong", ["hong kong"]),
    c!("Taipéi", "Asia/Taipei", ["taipei", "taiwan"]),
    c!("Pekín", "Asia/Shanghai", ["pekin", "beijing", "china", "shanghai", "shenzhen"]),
    c!("Seúl", "Asia/Seoul", ["seul", "corea", "corea del sur"]),
    c!("Tokio", "Asia/Tokyo", ["tokio", "tokyo", "japon", "osaka"]),
    c!("Sídney", "Australia/Sydney", ["sidney", "sydney", "australia"]),
    c!("Melbourne", "Australia/Melbourne", ["melbourne"]),
    c!("Perth", "Australia/Perth", ["perth"]),
    c!("Auckland", "Pacific/Auckland", ["auckland", "nueva zelanda"]),
    c!("UTC", "UTC", ["utc", "gmt", "zulu"]),
];

const DAYS: [&str; 7] = ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"];
const MONTHS: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.trim().chars().flat_map(char::to_lowercase) {
        out.push(match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            '¿' | '?' => ' ',
            c => c,
        });
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Lugares

#[derive(Clone)]
struct Place {
    label: String,
    /// `None` = la zona local.
    zone: Option<&'static str>,
    /// "utc+3": desfase fijo en segundos.
    fixed: Option<i32>,
}

impl Place {
    fn local() -> Place {
        let label = tz::local_zone()
            .and_then(|z| {
                CITIES
                    .iter()
                    .find(|c| c.zone == z)
                    .map(|c| c.label.to_string())
                    .or_else(|| z.rsplit('/').next().filter(|n| !n.is_empty()).map(|n| n.replace('_', " ")))
            })
            .unwrap_or_else(|| "tu zona".into());
        Place { label, zone: None, fixed: None }
    }

    fn offset(&self, utc: i64) -> i32 {
        match (self.fixed, self.zone) {
            (Some(f), _) => f,
            (_, Some(z)) => tz::offset(z, utc).unwrap_or(0),
            _ => tz::local_offset(utc),
        }
    }
}

fn place(s: &str) -> Option<Place> {
    let s = s.trim().trim_start_matches("el ").trim_start_matches("la ciudad de ");
    if let Some(c) = CITIES.iter().find(|c| c.names.contains(&s)) {
        return Some(Place { label: c.label.into(), zone: Some(c.zone), fixed: None });
    }
    // utc+3, gmt-5, utc+5:30
    for p in ["utc", "gmt"] {
        if let Some(rest) = s.strip_prefix(p) {
            let rest = rest.trim();
            let (sign, rest) = match rest.as_bytes().first()? {
                b'+' => (1, &rest[1..]),
                b'-' => (-1, &rest[1..]),
                _ => return None,
            };
            let (h, m) = rest.split_once(':').unwrap_or((rest, "0"));
            let (h, m): (i32, i32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
            if h > 14 || m >= 60 {
                return None;
            }
            return Some(Place {
                label: format!("UTC{}", s[3..].trim().to_uppercase()),
                zone: None,
                fixed: Some(sign * (h * 3600 + m * 60)),
            });
        }
    }
    if s == "aqui" || s == "local" || s == "mi hora" {
        return Some(Place::local());
    }
    None
}

// ---------------------------------------------------------------------------
// Formatos

fn fmt_long(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{} {d} de {} de {y}", DAYS[weekday(days) as usize], MONTHS[m as usize - 1])
}

fn fmt_short(days: i64) -> String {
    let (_, m, d) = civil_from_days(days);
    let day: String = DAYS[weekday(days) as usize].chars().take(3).collect();
    format!("{day} {d} {}", &MONTHS[m as usize - 1][..3])
}

fn fmt_iso(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn fmt_hm(local: i64) -> String {
    let s = local.rem_euclid(86_400);
    format!("{:02}:{:02}", s / 3600, s % 3600 / 60)
}

/// "+15 h", "−1 h 30 min", "misma hora".
fn fmt_diff(secs: i32) -> String {
    if secs == 0 {
        return "misma hora que tú".into();
    }
    let a = secs.abs();
    let (h, m) = (a / 3600, a % 3600 / 60);
    let amount = match (h, m) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    };
    format!("{amount} {} de ti", if secs > 0 { "adelante" } else { "atrás" })
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n.abs() == 1 { one } else { many })
}

// ---------------------------------------------------------------------------
// Horas

/// "15:00", "15h", "3pm", "3:30 pm", "9 am" → (hora, minuto) y lo que sigue.
fn take_clock(s: &str) -> Option<((u32, u32), &str)> {
    let digits = s.bytes().take_while(|b| b.is_ascii_digit() || *b == b':').count();
    if digits == 0 {
        return None;
    }
    let (num, mut rest) = s.split_at(digits);
    let (h, m) = match num.split_once(':') {
        Some((h, m)) if m.len() == 2 => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        Some(_) => return None,
        None => (num.parse::<u32>().ok()?, 0),
    };
    let mut marked = num.contains(':');
    let trimmed = rest.trim_start();
    let mut h = h;
    for (suffix, pm) in [("p.m.", true), ("a.m.", false), ("pm", true), ("am", false), ("hrs", false), ("h", false)] {
        if let Some(r) = trimmed.strip_prefix(suffix)
            && (r.is_empty() || r.starts_with(' '))
        {
            if suffix.contains('m') {
                if h == 0 || h > 12 {
                    return None;
                }
                h = match (pm, h) {
                    (true, 12) => 12,
                    (true, h) => h + 12,
                    (false, 12) => 0,
                    (false, h) => h,
                };
            }
            marked = true;
            rest = r;
            break;
        }
    }
    (marked && h < 24 && m < 60).then_some(((h, m), rest.trim_start()))
}

/// Instante UTC de la hora `h:m` de hoy (en la fecha actual de `p`).
fn utc_of(p: &Place, (h, m): (u32, u32), now: i64) -> i64 {
    let today = (now + p.offset(now) as i64).div_euclid(86_400);
    let local = today * 86_400 + h as i64 * 3600 + m as i64 * 60;
    // Dos pasadas por si el desfase cambia ese día (horario de verano).
    let guess = local - p.offset(local) as i64;
    local - p.offset(guess) as i64
}

fn time_items(q: &str, now: i64, out: &mut Vec<Item>) -> bool {
    // "hora", "hora en tokio", "que hora es en madrid", "time in tokyo"
    for prefix in ["que hora es en ", "que hora es ", "hora en ", "hora de ", "time in ", "hora ", "time "] {
        if let Some(rest) = q.strip_prefix(prefix)
            && let Some(p) = place(rest)
        {
            now_in(&p, now, out);
            return true;
        }
    }
    if matches!(q, "hora" | "que hora es" | "time") {
        now_in(&Place::local(), now, out);
        return true;
    }

    // "15:00 ny a cdmx", "3pm tokio", "9am a madrid"
    let Some((hm, rest)) = take_clock(q) else { return false };
    if rest.is_empty() {
        return false;
    }
    let (from, to) = if let Some(r) = rest.strip_prefix("a ").or_else(|| rest.strip_prefix("to ")) {
        (Place::local(), place(r))
    } else {
        let r = rest.strip_prefix("en ").or_else(|| rest.strip_prefix("in ")).unwrap_or(rest);
        match place(r) {
            Some(p) => (p, Some(Place::local())),
            None => {
                let split = [" a ", " to ", " en ", " in "].iter().find_map(|sep| {
                    let (a, b) = r.split_once(sep)?;
                    Some((place(a)?, place(b)?))
                });
                match split {
                    Some((a, b)) => (a, Some(b)),
                    None => return false,
                }
            }
        }
    };
    let Some(to) = to else { return false };
    let utc = utc_of(&from, hm, now);
    let src = utc + from.offset(utc) as i64;
    let dst = utc + to.offset(utc) as i64;
    let title = format!("{} en {}", fmt_hm(dst), to.label);
    let mut sub = format!("{} en {} ({})", fmt_hm(src), from.label, fmt_short(src.div_euclid(86_400)));
    if dst.div_euclid(86_400) != src.div_euclid(86_400) {
        sub.push_str(&format!(" → {}", fmt_short(dst.div_euclid(86_400))));
    }
    sub.push_str(" · Enter copia");
    out.push(Item::new(title, sub, Icon::Glyph(Glyph::Clock), Action::Copy(fmt_hm(dst)), SCORE));
    true
}

fn now_in(p: &Place, now: i64, out: &mut Vec<Item>) {
    let off = p.offset(now);
    let local = now + off as i64;
    let mut sub = fmt_long(local.div_euclid(86_400));
    if p.zone.is_some() || p.fixed.is_some() {
        sub.push_str(" · ");
        sub.push_str(&fmt_diff(off - tz::local_offset(now)));
    }
    sub.push_str(" · Enter copia");
    out.push(Item::new(
        format!("{} en {}", fmt_hm(local), p.label),
        sub,
        Icon::Glyph(Glyph::Clock),
        Action::Copy(fmt_hm(local)),
        SCORE,
    ));
}

// ---------------------------------------------------------------------------
// Fechas

fn month(s: &str) -> Option<u32> {
    const EN: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let s = s.trim_end_matches('.');
    if s.len() < 3 {
        return None;
    }
    (1..=12u32).find(|&m| {
        let es = MONTHS[m as usize - 1];
        es.starts_with(s) || (s.len() == 3 && EN[m as usize - 1] == s) || (s == "sept" && m == 9)
    })
}

/// Fecha escrita → días desde 1970. Sin año: este año (o el siguiente si `future` y ya pasó).
fn parse_date(s: &str, today: i64, future: bool) -> Option<i64> {
    let s = s.trim().trim_start_matches("el ").trim();
    match s {
        "hoy" | "today" => return Some(today),
        "manana" | "tomorrow" => return Some(today + 1),
        "ayer" | "yesterday" => return Some(today - 1),
        "pasado manana" => return Some(today + 2),
        _ => {}
    }
    let (ty, _, _) = civil_from_days(today);
    let build = |d: u32, m: u32, y: Option<i64>| -> Option<i64> {
        let year = y.unwrap_or(ty);
        if !(1..=12).contains(&m) || d == 0 || d > days_in_month(year, m) {
            return None;
        }
        let days = days_from_civil(year, m, d);
        Some(if y.is_none() && future && days < today {
            days_from_civil(year + 1, m, d.min(days_in_month(year + 1, m)))
        } else {
            days
        })
    };
    let year = |t: &str| -> Option<i64> {
        let y: i64 = t.parse().ok()?;
        match t.len() {
            2 => Some(2000 + y),
            4 => Some(y),
            _ => None,
        }
    };
    match s {
        "navidad" | "christmas" => return build(25, 12, None),
        "nochebuena" => return build(24, 12, None),
        "ano nuevo" | "new year" => {
            return build(1, 1, None).map(|d| if d <= today { days_from_civil(ty + 1, 1, 1) } else { d });
        }
        _ => {}
    }
    // 2026-12-24
    if let [y, m, d] = s.split('-').collect::<Vec<_>>()[..]
        && y.len() == 4
    {
        return build(d.parse().ok()?, m.parse().ok()?, Some(y.parse().ok()?));
    }
    // 24/12, 24/12/2026, 24-12-26
    let sep = if s.contains('/') { '/' } else { '-' };
    let parts: Vec<&str> = s.split(sep).collect();
    if (2..=3).contains(&parts.len()) && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) {
        let y = match parts.get(2) {
            Some(t) => Some(year(t)?),
            None => None,
        };
        return build(parts[0].parse().ok()?, parts[1].parse().ok()?, y);
    }
    // 24 dic, 24 de diciembre de 2026, dic 24 2026
    let words: Vec<&str> = s.split([' ', ',']).filter(|w| !w.is_empty() && !matches!(*w, "de" | "del")).collect();
    let (d, m, rest) = match words[..] {
        [d, m, ref rest @ ..] if d.bytes().all(|b| b.is_ascii_digit()) => (d.parse().ok()?, month(m)?, rest),
        [m, d, ref rest @ ..] if d.bytes().all(|b| b.is_ascii_digit()) => (d.parse().ok()?, month(m)?, rest),
        _ => return None,
    };
    let y = match rest {
        [] => None,
        [y] => Some(year(y)?),
        _ => return None,
    };
    build(d, m, y)
}

fn add_months(days: i64, months: i64) -> i64 {
    let (y, m, d) = civil_from_days(days);
    let total = y * 12 + m as i64 - 1 + months;
    let (ny, nm) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
    days_from_civil(ny, nm, d.min(days_in_month(ny, nm)))
}

/// "90 dias", "2 semanas", "3 meses", "1 ano" → (cantidad, unidad en días o meses).
fn take_span(s: &str) -> Option<(i64, bool)> {
    let (n, unit) = s.trim().split_once(' ').unwrap_or((s.trim(), "dias"));
    let n: i64 = n.parse().ok()?;
    match unit.trim() {
        "d" | "dia" | "dias" | "day" | "days" => Some((n, false)),
        "sem" | "semana" | "semanas" | "w" | "week" | "weeks" => Some((n * 7, false)),
        "mes" | "meses" | "month" | "months" => Some((n, true)),
        "ano" | "anos" | "year" | "years" => Some((n * 12, true)),
        _ => None,
    }
}

fn date_item(days: i64, what: String, out: &mut Vec<Item>) {
    let long = fmt_long(days);
    let (y, _, _) = civil_from_days(days);
    let doy = days - days_from_civil(y, 1, 1) + 1;
    out.push(Item::new(
        long.clone(),
        format!("{what} · {} · semana {} · día {doy} · Enter copia", fmt_iso(days), iso_week(days)),
        Icon::Glyph(Glyph::Calendar),
        Action::Copy(long),
        SCORE,
    ));
}

fn date_items(q: &str, now: i64, out: &mut Vec<Item>) -> bool {
    let today = (now + tz::local_offset(now) as i64).div_euclid(86_400);
    if matches!(q, "hoy" | "fecha" | "today" | "date" | "que dia es hoy" | "que dia es") {
        date_item(today, "hoy".into(), out);
        return true;
    }
    // "en 90 dias", "hace 2 semanas"
    for (prefix, sign) in [("en ", 1), ("dentro de ", 1), ("in ", 1), ("hace ", -1)] {
        if let Some(rest) = q.strip_prefix(prefix)
            && let Some((n, months)) = take_span(rest)
            && rest.contains(' ')
        {
            let d = if months { add_months(today, sign * n) } else { today + sign * n };
            date_item(d, q.to_string(), out);
            return true;
        }
    }
    // "hoy + 90 dias", "24 dic - 2 semanas" (con espacios: "24-12-2026" es una fecha)
    for op in ['+', '-'] {
        if let Some((base, span)) = q.rsplit_once(op)
            && (base.ends_with(' ') || span.starts_with(' '))
            && let Some(start) = parse_date(base, today, false)
            && let Some((n, months)) = take_span(span)
        {
            let n = if op == '+' { n } else { -n };
            let d = if months { add_months(start, n) } else { start + n };
            date_item(d, q.to_string(), out);
            return true;
        }
    }
    // "dias hasta 24 dic", "cuanto falta para navidad", "dias desde 1 ene"
    let until = ["dias hasta ", "dias para ", "cuanto falta para ", "faltan para ", "days until "];
    let since = ["dias desde ", "days since "];
    for (prefixes, future) in [(&until[..], true), (&since[..], false)] {
        for p in prefixes {
            if let Some(rest) = q.strip_prefix(p)
                && let Some(d) = parse_date(rest, today, future)
            {
                let n = if future { d - today } else { today - d };
                let mut sub = format!("{} el {}", if future { "hasta" } else { "desde" }, fmt_long(d));
                if n.abs() >= 7 {
                    let (w, r) = (n.abs() / 7, n.abs() % 7);
                    sub.push_str(&format!(" · {}", plural(w, "semana", "semanas")));
                    if r > 0 {
                        sub.push_str(&format!(" y {}", plural(r, "día", "días")));
                    }
                }
                sub.push_str(" · Enter copia");
                out.push(Item::new(
                    plural(n, "día", "días"),
                    sub,
                    Icon::Glyph(Glyph::Calendar),
                    Action::Copy(n.to_string()),
                    SCORE,
                ));
                return true;
            }
        }
    }
    // Fecha suelta con año o con barras: "24/12/2026", "2026-12-24", "5 may 2027" → día de la semana.
    let explicit = q.contains('/')
        || q.matches('-').count() == 2
        || q.split(' ').any(|w| w.len() == 4 && w.bytes().all(|b| b.is_ascii_digit()));
    if explicit && let Some(d) = parse_date(q, today, false) {
        let n = d - today;
        let what = match n {
            0 => "hoy".to_string(),
            n if n > 0 => format!("en {}", plural(n, "día", "días")),
            n => format!("hace {}", plural(-n, "día", "días")),
        };
        date_item(d, what, out);
        return true;
    }
    false
}

/// "1730592000" (segundos) o "1730592000000" (milisegundos) → fecha.
fn unix_items(q: &str, now: i64, out: &mut Vec<Item>) -> bool {
    if matches!(q, "unix" | "timestamp" | "epoch") {
        out.push(Item::new(
            now.to_string(),
            "Segundos Unix de ahora · Enter copia",
            Icon::Glyph(Glyph::Clock),
            Action::Copy(now.to_string()),
            SCORE,
        ));
        return true;
    }
    if !q.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let secs = match q.len() {
        9 | 10 => q.parse::<i64>().ok(),
        13 => q.parse::<i64>().ok().map(|ms| ms / 1000),
        _ => None,
    };
    // 1973–2100: fuera de eso es más probable que sea otro número (un teléfono).
    let Some(secs) = secs.filter(|s| (100_000_000..=4_102_444_800).contains(s)) else { return false };
    let local = secs + tz::local_offset(secs) as i64;
    let utc_iso = format!("{}T{}:{:02}Z", fmt_iso(secs.div_euclid(86_400)), fmt_hm(secs), secs.rem_euclid(60));
    let title = format!("{}, {}", fmt_long(local.div_euclid(86_400)), fmt_hm(local));
    out.push(Item::new(
        title.clone(),
        format!("Fecha Unix · UTC {utc_iso} · Enter copia"),
        Icon::Glyph(Glyph::Calendar),
        Action::Copy(title),
        SCORE,
    ));
    out.push(Item::new(
        utc_iso.clone(),
        "ISO 8601 (UTC) · Enter copia",
        Icon::Glyph(Glyph::Calendar),
        Action::Copy(utc_iso),
        SCORE - 1,
    ));
    true
}

impl Plugin for Time {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::All || q.text.len() < 3 {
            return;
        }
        let text = fold(q.text);
        let now = tz::now();
        let _ = unix_items(&text, now, out) || time_items(&text, now, out) || date_items(&text, now, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks() {
        assert_eq!(take_clock("15:00 ny"), Some(((15, 0), "ny")));
        assert_eq!(take_clock("3pm tokio"), Some(((15, 0), "tokio")));
        assert_eq!(take_clock("3:30 pm"), Some(((15, 30), "")));
        assert_eq!(take_clock("12 am"), Some(((0, 0), "")));
        assert_eq!(take_clock("9h madrid"), Some(((9, 0), "madrid")));
        assert_eq!(take_clock("15 tokio"), None);
        assert_eq!(take_clock("25:00"), None);
    }

    #[test]
    fn dates() {
        let today = days_from_civil(2026, 11, 3);
        assert_eq!(parse_date("24 dic", today, true), Some(days_from_civil(2026, 12, 24)));
        assert_eq!(parse_date("24 de diciembre de 2027", today, true), Some(days_from_civil(2027, 12, 24)));
        assert_eq!(parse_date("1 ene", today, true), Some(days_from_civil(2027, 1, 1)));
        assert_eq!(parse_date("1 ene", today, false), Some(days_from_civil(2026, 1, 1)));
        assert_eq!(parse_date("24/12/26", today, false), Some(days_from_civil(2026, 12, 24)));
        assert_eq!(parse_date("2026-02-30", today, false), None);
        assert_eq!(parse_date("navidad", today, true), Some(days_from_civil(2026, 12, 25)));
        assert_eq!(parse_date("dec 25", today, true), Some(days_from_civil(2026, 12, 25)));
        assert_eq!(add_months(days_from_civil(2026, 1, 31), 1), days_from_civil(2026, 2, 28));
        assert_eq!(take_span(" 90 dias"), Some((90, false)));
        assert_eq!(take_span("2 semanas"), Some((14, false)));
    }

    #[test]
    fn places() {
        assert_eq!(place("tokio").map(|p| p.zone), Some(Some("Asia/Tokyo")));
        assert_eq!(place("ciudad de mexico").map(|p| p.label), Some("Ciudad de México".to_string()));
        assert_eq!(place("utc+5:30").and_then(|p| p.fixed), Some(5 * 3600 + 1800));
        assert!(place("narnia").is_none());
        assert_eq!(fold("¿Qué hora es en Tokio?"), "que hora es en tokio");
    }

    #[test]
    fn conversion() {
        let now = days_from_civil(2026, 7, 1) * 86_400 + 12 * 3600;
        let mut out = Vec::new();
        assert!(time_items("15:00 ny a tokio", now, &mut out));
        assert_eq!(out[0].title, "04:00 en Tokio");
        let mut out = Vec::new();
        assert!(date_items("dias hasta 24 dic", days_from_civil(2026, 11, 3) * 86_400 + 43_200, &mut out));
    }
}
