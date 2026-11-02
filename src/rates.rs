//! Tipos de cambio para la calculadora ("100 usd to mxn", "50 eur en pesos").
//!
//! Fuente: las tasas diarias de fawazahmed0/exchange-api (gratis, sin clave,
//! ~200 monedas y cripto), con una copia de respaldo. Se guardan en `rates.tsv`
//! y sólo se descargan cuando se usan y tienen más de 12 h.

use crate::{config, platform};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCES: &[&str] = &[
    "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.min.json",
    "https://latest.currency-api.pages.dev/v1/currencies/usd.min.json",
];
pub const SOURCE_NAME: &str = "exchange-api";
const MAX_AGE: u64 = 12 * 3600;
/// Tras un fallo (sin red), no reintentar en cada tecla.
const RETRY_AFTER: u64 = 5 * 60;

pub struct Rates {
    /// Fecha de publicación ("2026-11-01").
    pub date: String,
    /// Cuándo se descargaron (segundos Unix).
    pub fetched: u64,
    /// Unidades de cada moneda por 1 USD; código en minúsculas.
    per_usd: HashMap<String, f64>,
}

impl Rates {
    pub fn per_usd(&self, code: &str) -> Option<f64> {
        self.per_usd.get(&code.to_ascii_lowercase()).copied()
    }
}

static CURRENT: RwLock<Option<Arc<Rates>>> = RwLock::new(None);
static LOADED: AtomicBool = AtomicBool::new(false);
static FETCHING: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
static LAST_ATTEMPT: AtomicU64 = AtomicU64::new(0);
/// Sube con cada descarga: la calculadora rehace sus unidades cripto.
static VERSION: AtomicU64 = AtomicU64::new(0);
type Callback = Box<dyn Fn() + Send + Sync>;
static ON_UPDATE: OnceLock<Callback> = OnceLock::new();

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn path() -> std::path::PathBuf {
    config::data_dir().join("rates.tsv")
}

/// Se llama al terminar una descarga (bien o mal) para repetir la búsqueda visible.
pub fn on_update(f: impl Fn() + Send + Sync + 'static) {
    let _ = ON_UPDATE.set(Box::new(f));
}

pub fn current() -> Option<Arc<Rates>> {
    if !LOADED.swap(true, Ordering::AcqRel)
        && let Some(r) = load()
    {
        *CURRENT.write() = Some(Arc::new(r));
        VERSION.fetch_add(1, Ordering::Relaxed);
    }
    CURRENT.read().clone()
}

pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

pub fn is_fetching() -> bool {
    FETCHING.load(Ordering::Relaxed)
}

pub fn failed() -> bool {
    FAILED.load(Ordering::Relaxed)
}

/// Descarga tasas nuevas si las guardadas faltan o son viejas. En segundo plano,
/// salvo `blocking` (línea de comandos). `force` ignora la antigüedad.
pub fn refresh(blocking: bool, force: bool) {
    let t = now();
    let stale = force || current().is_none_or(|r| t.saturating_sub(r.fetched) > MAX_AGE);
    if !stale || (!blocking && t.saturating_sub(LAST_ATTEMPT.load(Ordering::Relaxed)) < RETRY_AFTER) {
        return;
    }
    if FETCHING.swap(true, Ordering::AcqRel) {
        return;
    }
    LAST_ATTEMPT.store(t, Ordering::Relaxed);
    if blocking {
        fetch();
    } else {
        std::thread::Builder::new().name("kero-rates".into()).spawn(fetch).ok();
    }
}

fn fetch() {
    let rates = SOURCES.iter().find_map(|u| platform::http_get(u)).and_then(|b| parse(&String::from_utf8_lossy(&b)));
    match rates {
        Some(mut r) => {
            r.fetched = now();
            save(&r);
            *CURRENT.write() = Some(Arc::new(r));
            VERSION.fetch_add(1, Ordering::Relaxed);
            FAILED.store(false, Ordering::Relaxed);
        }
        None => FAILED.store(true, Ordering::Relaxed),
    }
    FETCHING.store(false, Ordering::Release);
    if let Some(cb) = ON_UPDATE.get() {
        cb();
    }
}

/// `{"date":"2026-11-01","usd":{"aed":3.67,"mxn":18.7,…}}`: un objeto plano, sin
/// necesidad de un parser JSON completo.
fn parse(json: &str) -> Option<Rates> {
    let date = json.split("\"date\":\"").nth(1)?.split('"').next()?.to_string();
    let body = json.split("\"usd\":{").nth(1)?;
    let body = &body[..body.find('}')?];
    let mut per_usd = HashMap::new();
    for pair in body.split(',') {
        let Some((k, v)) = pair.split_once(':') else { continue };
        let k = k.trim().trim_matches('"').to_ascii_lowercase();
        if let Ok(v) = v.trim().parse::<f64>()
            && v.is_finite()
            && v > 0.0
        {
            per_usd.insert(k, v);
        }
    }
    (per_usd.len() >= 20).then_some(Rates { date, fetched: 0, per_usd })
}

fn save(r: &Rates) {
    let mut out = format!("#date\t{}\n#fetched\t{}\n", r.date, r.fetched);
    let mut codes: Vec<_> = r.per_usd.iter().collect();
    codes.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in codes {
        out.push_str(&format!("{k}\t{v}\n"));
    }
    let tmp = path().with_extension("tmp");
    if std::fs::write(&tmp, out).is_ok() {
        let _ = std::fs::rename(tmp, path());
    }
}

fn load() -> Option<Rates> {
    let text = std::fs::read_to_string(path()).ok()?;
    let mut r = Rates { date: String::new(), fetched: 0, per_usd: HashMap::new() };
    for line in text.lines() {
        let Some((k, v)) = line.split_once('\t') else { continue };
        match k {
            "#date" => r.date = v.to_string(),
            "#fetched" => r.fetched = v.parse().unwrap_or(0),
            _ => {
                if let Ok(x) = v.parse() {
                    r.per_usd.insert(k.to_string(), x);
                }
            }
        }
    }
    (!r.per_usd.is_empty()).then_some(r)
}

/// Tasas fijas para pruebas.
#[cfg(test)]
pub fn set_for_test(pairs: &[(&str, f64)]) {
    LOADED.store(true, Ordering::Relaxed);
    let per_usd = pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    *CURRENT.write() = Some(Arc::new(Rates { date: "2026-11-01".into(), fetched: now(), per_usd }));
    VERSION.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_api_payload() {
        let mut json =
            String::from(r#"{"date":"2026-11-01","usd":{"1inch":12.3,"mxn":18.72,"eur":0.89,"btc":0.0000091"#);
        for i in 0..30 {
            json.push_str(&format!(r#","x{i}":1.5"#));
        }
        json.push_str("}}");
        let r = parse(&json).unwrap();
        assert_eq!(r.date, "2026-11-01");
        assert_eq!(r.per_usd("MXN"), Some(18.72));
        assert_eq!(r.per_usd("btc"), Some(0.0000091));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("<html>error</html>").is_none());
        assert!(parse(r#"{"date":"x","usd":{}}"#).is_none());
    }
}
