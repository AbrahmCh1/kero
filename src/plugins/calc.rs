//! Calculadora, unidades y divisas (fend): "2^10", "sqrt(2)*pi", "5 km to mi",
//! "0xff to decimal", "100 usd a mxn", "50 euros"… Enter copia el resultado.

use super::{Action, Glyph, Icon, Item, Mode, Plugin, Query};
use crate::rates;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

pub struct Calc {
    state: Mutex<State>,
}

struct State {
    ctx: fend_core::Context,
    /// Versión de las tasas con que se definieron las unidades cripto.
    rates_version: u64,
    /// Moneda local ya resuelta ("auto" → "MXN").
    local: String,
    local_cfg: String,
}

struct Deadline(Instant);

impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() > self.0
    }
}

/// fend pregunta cuánto vale cada moneda; respondemos con las tasas guardadas
/// y, si faltan o son viejas, pedimos otras en segundo plano.
struct RateHandler;

impl fend_core::ExchangeRateFnV2 for RateHandler {
    fn relative_to_base_currency(
        &self,
        currency: &str,
        _: &fend_core::ExchangeRateFnV2Options,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync + 'static>> {
        rates::refresh(false, false);
        rates::current().and_then(|r| r.per_usd(currency)).ok_or_else(|| "sin tipos de cambio".into())
    }
}

/// Cripto que fend no conoce: se definen como unidades a partir de su precio en USD.
const CRYPTO: &[(&str, &str, &str)] = &[
    ("btc", "bitcoin", "bitcoins"),
    ("eth", "ether", "ethers"),
    ("sol", "solana", "solanas"),
    ("xrp", "ripple", "ripples"),
    ("doge", "dogecoin", "dogecoins"),
    ("ltc", "litecoin", "litecoins"),
    ("ada", "cardano", "cardanos"),
    ("usdt", "tether", "tethers"),
    ("usdc", "usdc", "usdc"),
];

/// Nombres en español (singular, plural, código). "peso" es la moneda local si usa pesos.
const NAMES: &[(&str, &str, &str)] = &[
    ("dolar", "dolares", "USD"),
    ("dólar", "dólares", "USD"),
    ("libra", "libras", "GBP"),
    ("yen", "yenes", "JPY"),
    ("yuan", "yuanes", "CNY"),
    ("real", "reales", "BRL"),
    ("franco", "francos", "CHF"),
];
const PESO_COUNTRIES: &[&str] = &["MXN", "ARS", "CLP", "COP", "UYU", "DOP", "CUP", "PHP"];

/// Códigos ISO 4217 que entiende fend (para reconocer una consulta de divisas).
const ISO: &str = "aed afn all amd ang aoa ars aud awg azn bam bbd bdt bgn bhd bif bmd bnd bob brl bsd btn bwp \
byn bzd cad cdf chf clp cny cop crc cuc cup cve czk djf dkk dop dzd egp ern etb eur fjd fkp gbp gel ghs gip gmd \
gnf gtq gyd hkd hnl htg huf idr ils inr iqd irr isk jmd jod jpy kes kgs khr kmf kpw krw kwd kyd kzt lak lbp lkr \
lrd lsl lyd mad mdl mga mkd mmk mnt mop mru mur mvr mwk mxn myr mzn nad ngn nio nok npr nzd omr pab pen pgk php \
pkr pln pyg qar ron rsd rub rwf sar sbd scr sdg sek sgd shp sle sll sos srd ssp stn svc syp szl thb tjs tmt tnd \
top try ttd twd tzs uah ugx usd uyu uzs ves vnd vuv wst xaf xag xau xcd xof xpd xpf xpt yer zar zmw";

fn currency_code(word: &str) -> Option<String> {
    let w = word.trim_matches(|c: char| !c.is_alphanumeric() && !"$€£¥".contains(c)).to_lowercase();
    match w.as_str() {
        "$" => return Some("USD".into()),
        "€" | "euro" | "euros" => return Some("EUR".into()),
        "£" => return Some("GBP".into()),
        "¥" => return Some("JPY".into()),
        "peso" | "pesos" => return Some("PESO".into()),
        _ => {}
    }
    if w.len() == 3 && ISO.split(' ').any(|c| c == w) {
        return Some(w.to_uppercase());
    }
    if let Some((code, ..)) = CRYPTO.iter().find(|(c, s, p)| w == *c || w == *s || w == *p) {
        return Some(code.to_uppercase());
    }
    NAMES.iter().find(|(s, p, _)| w == *s || w == *p).map(|(.., code)| code.to_string())
}

fn mentions_currency(s: &str) -> bool {
    s.split(|c: char| c.is_whitespace() || c.is_ascii_digit()).any(|w| currency_code(w).is_some())
        || s.contains(['$', '€', '£', '¥'])
}

impl Calc {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                ctx: fend_core::Context::new(),
                rates_version: u64::MAX,
                local: String::new(),
                local_cfg: String::new(),
            }),
        }
    }
}

/// Moneda local de la configuración ("auto" → la de la región del sistema).
pub fn resolve_local(cfg: &str) -> String {
    let c = cfg.trim();
    if c.is_empty() || c.eq_ignore_ascii_case("auto") { crate::platform::local_currency() } else { c.to_uppercase() }
}

fn new_context(local: &str) -> fend_core::Context {
    use fend_core::CustomUnitAttribute::None as Plain;
    let mut ctx = fend_core::Context::new();
    ctx.set_exchange_rate_handler_v2(RateHandler);
    let peso = if PESO_COUNTRIES.contains(&local) { local } else { "MXN" };
    ctx.define_custom_unit_v1("peso", "pesos", peso, &Plain);
    for (s, p, code) in NAMES {
        ctx.define_custom_unit_v1(s, p, code, &Plain);
    }
    if let Some(r) = rates::current() {
        for (code, s, p) in CRYPTO {
            if let Some(per_usd) = r.per_usd(code) {
                let def = format!("{} USD", 1.0 / per_usd);
                let upper = code.to_uppercase();
                ctx.define_custom_unit_v1(&upper, &upper, &def, &Plain);
                ctx.define_custom_unit_v1(code, code, &upper, &Plain);
                ctx.define_custom_unit_v1(s, p, &upper, &Plain);
            }
        }
    }
    ctx
}

fn looks_like_math(s: &str) -> bool {
    let has_digit = s.bytes().any(|b| b.is_ascii_digit());
    let lower = s.to_ascii_lowercase();
    let has_fn = ["sqrt", "pi", "sin", "cos", "tan", "log", "ln", "abs", "exp"].iter().any(|f| lower.contains(f));
    has_digit || has_fn
}

/// "3,5 * 2" → "3.5 * 2" cuando la coma se usa como decimal; "100 usd a mxn" → "… to mxn".
fn normalize(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.char_indices() {
        if c == ','
            && i > 0
            && i + 1 < b.len()
            && b[i - 1].is_ascii_digit()
            && b[i + 1].is_ascii_digit()
            && !s[i + 1..].chars().take_while(|c| c.is_ascii_digit()).count().eq(&3)
        {
            out.push('.');
        } else if c == '×' {
            out.push('*');
        } else if c == '÷' {
            out.push('/');
        } else {
            out.push(c);
        }
    }
    let words: Vec<&str> = out.split_whitespace().collect();
    if words.len() >= 3 && matches!(words[words.len() - 2].to_lowercase().as_str(), "a" | "en") {
        let n = words.len();
        return format!("{} to {}", words[..n - 2].join(" "), words[n - 1]);
    }
    out
}

fn has_conversion(s: &str) -> bool {
    let l = format!(" {} ", s.to_lowercase());
    [" to ", " in ", " as ", "->", "→"].iter().any(|k| l.contains(k))
}

/// "approx. 1871.9700128 MXN" → (1871.97…, "MXN") si el resultado es dinero.
fn money(result: &str) -> Option<(f64, String)> {
    let r = result.trim().trim_start_matches("approx.").trim();
    let (num, unit) = r.rsplit_once(' ')?;
    let code = currency_code(unit)?;
    if code == "PESO" {
        return None;
    }
    let value: f64 = num.replace(',', "").parse().ok()?;
    Some((value, code))
}

/// 1871.97 → "1,871.97"; montos pequeños (cripto) con más decimales.
fn fmt_money(v: f64) -> (String, String) {
    let decimals =
        if v.abs() >= 1.0 || v == 0.0 { 2 } else { (2 - v.abs().log10().floor() as i32 + 2).clamp(2, 10) as usize };
    let mut plain = format!("{v:.decimals$}");
    // Cripto: 0.00012300 → 0.000123 (siempre al menos 2 decimales).
    while decimals > 2 && plain.ends_with('0') && plain.len() - plain.find('.').unwrap_or(0) > 3 {
        plain.pop();
    }
    let (int, frac) = plain.split_once('.').unwrap_or((&plain, ""));
    let (sign, digits) = int.strip_prefix('-').map(|d| ("-", d)).unwrap_or(("", int));
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let pretty = if frac.is_empty() { format!("{sign}{grouped}") } else { format!("{sign}{grouped}.{frac}") };
    (pretty, plain)
}

impl Calc {
    fn eval(st: &State, input: &str) -> String {
        let deadline = Deadline(Instant::now() + Duration::from_millis(60));
        let res = fend_core::evaluate_preview_with_interrupt(input, &st.ctx, &deadline);
        res.get_main_result().trim().to_string()
    }

    /// Evalúa y devuelve (título, texto a copiar, detalle) o `None`.
    pub fn compute(&self, text: &str, local_cfg: &str) -> Option<(String, String, Option<String>)> {
        let mut st = self.state.lock();
        if st.local_cfg != local_cfg || st.local.is_empty() {
            st.local_cfg = local_cfg.to_string();
            st.local = resolve_local(local_cfg);
            st.rates_version = u64::MAX;
        }
        // Las unidades cripto dependen de las tasas: se rehacen cuando llegan nuevas.
        let currency = mentions_currency(text);
        if currency && st.rates_version != rates::version() {
            let _ = rates::current();
            st.ctx = new_context(&st.local);
            st.rates_version = rates::version();
        }
        let input = normalize(text);
        let mut result = Self::eval(&st, &input);
        let mut converted = has_conversion(&input);
        // "100 usd", "1 btc" → a la moneda local (o a USD si ya es la local). Si el
        // resultado se ve igual que la entrada, fend no devuelve nada: se convierte igual.
        if !converted && currency && (result.is_empty() || money(&result).is_some()) {
            let source = money(&result).map(|(_, c)| c).or_else(|| {
                input.split_whitespace().find_map(currency_code).map(|c| if c == "PESO" { st.local.clone() } else { c })
            });
            let target = if source.as_deref() == Some(st.local.as_str()) { "USD" } else { st.local.as_str() };
            let again = Self::eval(&st, &format!("({input}) to {target}"));
            if !again.is_empty() {
                result = again;
                converted = true;
            }
        }
        if result.is_empty() {
            return None;
        }
        if let Some((value, code)) = money(&result) {
            let (pretty, plain) = fmt_money(value);
            let date = rates::current().map(|r| r.date.clone()).unwrap_or_default();
            let detail = if converted && !date.is_empty() { Some(format!("tasas del {date}")) } else { None };
            return Some((format!("{pretty} {code}"), plain, detail));
        }
        // "2025" → "2025" no aporta nada.
        if result.replace(' ', "") == input.trim().replace(' ', "") {
            return None;
        }
        Some((result.clone(), result, None))
    }
}

impl Plugin for Calc {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if !matches!(q.mode, Mode::All | Mode::Calc) || q.text.is_empty() {
            return;
        }
        if q.mode == Mode::All && !looks_like_math(q.text) {
            return;
        }
        let Some((title, copy, detail)) = self.compute(q.text, &q.cfg.currency) else {
            // Divisas sin tasas todavía: decir qué pasa en vez de no mostrar nada.
            if mentions_currency(q.text) && rates::current().is_none() {
                let (t, s) = if rates::is_fetching() || !rates::failed() {
                    ("Obteniendo tipos de cambio…", "Un momento")
                } else {
                    ("No se pudieron obtener los tipos de cambio", "Revisa tu conexión a internet")
                };
                out.push(Item::new(t, s, Icon::Glyph(Glyph::Globe), Action::None, 10_000));
            }
            return;
        };
        let mut sub = q.text.trim().to_string();
        if let Some(d) = detail {
            sub.push_str(" · ");
            sub.push_str(&d);
        }
        sub.push_str(" · Enter copia ");
        sub.push_str(&copy);
        let mut it = Item::new(title, sub, Icon::Glyph(Glyph::Calc), Action::Copy(copy.clone()), 10_000);
        it.complete = Some(copy);
        out.push(it);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc() -> Calc {
        rates::set_for_test(&[("usd", 1.0), ("mxn", 20.0), ("eur", 0.8), ("btc", 0.00001), ("gbp", 0.75)]);
        Calc::new()
    }

    #[test]
    fn plain_math_still_works() {
        let c = calc();
        assert_eq!(c.compute("2^10", "MXN").unwrap().0, "1024");
        assert_eq!(c.compute("3,5 * 2", "MXN").unwrap().0, "7");
    }

    #[test]
    fn converts_currencies() {
        let c = calc();
        assert_eq!(c.compute("100 usd to mxn", "MXN").unwrap().0, "2,000.00 MXN");
        assert_eq!(c.compute("100 usd a mxn", "MXN").unwrap().0, "2,000.00 MXN");
        assert_eq!(c.compute("80 eur en dolares", "MXN").unwrap().0, "100.00 USD");
        assert_eq!(c.compute("2000 pesos to usd", "MXN").unwrap().1, "100.00");
    }

    #[test]
    fn converts_to_local_by_default() {
        let c = calc();
        assert_eq!(c.compute("100 usd", "MXN").unwrap().0, "2,000.00 MXN");
        assert_eq!(c.compute("100 mxn", "MXN").unwrap().0, "5.00 USD");
        assert_eq!(c.compute("1 btc", "MXN").unwrap().0, "2,000,000.00 MXN");
    }

    #[test]
    fn money_format() {
        assert_eq!(fmt_money(1234567.891).0, "1,234,567.89");
        assert_eq!(fmt_money(-12.5).0, "-12.50");
        assert_eq!(fmt_money(0.000123).0, "0.000123");
    }
}
