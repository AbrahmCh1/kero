//! Capa de plataforma: la misma API en Windows (Win32) y en macOS/Linux.

/// Órdenes entre procesos de Kero (`kero --quit`, `kero toggle`, `kero "texto"`, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ipc {
    Quit,
    Toggle,
    /// Mostrar Kero con esta búsqueda escrita.
    Show(String),
    Reindex,
    /// Copiar texto desde la instancia abierta (en Linux lo copiado muere con el proceso que copia).
    Copy(String),
}

impl Ipc {
    /// Mensajes con datos: primera línea = tipo, el resto = contenido.
    fn encode(&self) -> String {
        match self {
            Ipc::Quit => "quit\n".into(),
            Ipc::Toggle => "toggle\n".into(),
            Ipc::Show(q) => format!("show\n{q}"),
            Ipc::Reindex => "reindex\n".into(),
            Ipc::Copy(t) => format!("copy\n{t}"),
        }
    }

    fn decode(s: &str) -> Option<Ipc> {
        let (kind, rest) = s.split_once('\n').unwrap_or((s, ""));
        Some(match kind.trim() {
            "quit" => Ipc::Quit,
            "toggle" => Ipc::Toggle,
            "show" => Ipc::Show(rest.to_string()),
            "reindex" => Ipc::Reindex,
            "copy" => Ipc::Copy(rest.to_string()),
            _ => return None,
        })
    }
}

/// Moneda de una región ISO 3166 ("MX" → "MXN"). Sólo las más comunes; el resto, USD.
#[cfg_attr(windows, allow(dead_code))]
fn currency_for_region(region: &str) -> &'static str {
    const EURO: &[&str] = &[
        "AT", "BE", "CY", "DE", "EE", "ES", "FI", "FR", "GR", "HR", "IE", "IT", "LT", "LU", "LV", "MT", "NL", "PT",
        "SI", "SK", "AD", "MC", "SM", "VA", "ME", "XK",
    ];
    let r = region.to_ascii_uppercase();
    if EURO.contains(&r.as_str()) {
        return "EUR";
    }
    match r.as_str() {
        "MX" => "MXN",
        "AR" => "ARS",
        "BO" => "BOB",
        "BR" => "BRL",
        "CA" => "CAD",
        "CL" => "CLP",
        "CO" => "COP",
        "CR" => "CRC",
        "DO" => "DOP",
        "GT" => "GTQ",
        "HN" => "HNL",
        "NI" => "NIO",
        "PE" => "PEN",
        "PY" => "PYG",
        "UY" => "UYU",
        "VE" => "VES",
        "GB" => "GBP",
        "CH" => "CHF",
        "SE" => "SEK",
        "NO" => "NOK",
        "DK" => "DKK",
        "PL" => "PLN",
        "CZ" => "CZK",
        "HU" => "HUF",
        "RO" => "RON",
        "TR" => "TRY",
        "RU" => "RUB",
        "UA" => "UAH",
        "JP" => "JPY",
        "CN" => "CNY",
        "HK" => "HKD",
        "TW" => "TWD",
        "KR" => "KRW",
        "IN" => "INR",
        "ID" => "IDR",
        "SG" => "SGD",
        "TH" => "THB",
        "PH" => "PHP",
        "AU" => "AUD",
        "NZ" => "NZD",
        "ZA" => "ZAR",
        "IL" => "ILS",
        "AE" => "AED",
        "SA" => "SAR",
        "EG" => "EGP",
        "NG" => "NGN",
        _ => "USD",
    }
}

#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use win32::*;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;
