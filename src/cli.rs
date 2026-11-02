//! Kero desde la terminal: `kero buscar informe`, `kero calc 100 usd a mxn`,
//! `kero clip`, `echo hola | kero copiar`… Responde y termina sin abrir ventana.

use crate::commands::command;
use crate::config::{self, Config};
use crate::platform::{self, Ipc};
use crate::plugins::{Action, Item};
use crate::{clipboard, index, rates, search, usage};
use parking_lot::RwLock;
use std::io::{IsTerminal, Read, Write};
use std::sync::Arc;

/// Como `print!`/`println!`, pero sin abortar si quien lee cierra la tubería antes
/// (`kero buscar pdf | Select -First 3`).
macro_rules! out {
    ($($t:tt)*) => {{
        let _ = write!(std::io::stdout(), $($t)*);
    }};
}
macro_rules! outln {
    () => {{
        let _ = writeln!(std::io::stdout());
    }};
    ($($t:tt)*) => {{
        let _ = writeln!(std::io::stdout(), $($t)*);
    }};
}

const HELP: &str = "\
Kero · lanzador rápido

Uso:
  kero                       abre Kero (si ya está abierto, lo muestra u oculta)
  kero <texto>               abre Kero con esa búsqueda

  kero buscar <texto> [-n N] lista resultados (apps, archivos, comandos)
  kero abrir <texto>         abre el primer resultado
  kero calc <expresión>      calcula: 2^10, 5 km to mi, 100 usd a mxn
  kero tasas                 actualiza y muestra los tipos de cambio
  kero clip                  historial del portapapeles (numerado)
  kero clip <n>              vuelve a copiar la entrada n
  kero clip <texto>          filtra el historial
  kero clip borrar           vacía el historial
  kero copiar [texto]        copia el texto (o lo que llegue por la tubería)
  kero pegar                 escribe lo que hay en el portapapeles
  kero reindexar             vuelve a indexar apps y archivos
  kero mostrar | salir       muestra/oculta o cierra la instancia abierta
  kero config                abre config.toml
  kero version | ayuda

Alias en inglés: search, open, rates, copy, paste, reindex, toggle, quit, help.
Si la salida va a una tubería, buscar escribe sólo rutas: kero buscar pdf | fzf
";

/// Ejecuta un subcomando y devuelve el código de salida, o `None` si los
/// argumentos son para la interfaz (`kero`, `kero texto`, `--hidden`…).
pub fn run(args: &[String]) -> Option<i32> {
    let name = command(args.first()?)?;
    platform::attach_console();
    let rest = &args[1..];
    let code = match name {
        "buscar" => search_cmd(rest, false),
        "abrir" => search_cmd(rest, true),
        "calc" => calc(rest),
        "clip" => clip(rest),
        "copiar" => copy(rest),
        "pegar" => paste(),
        "tasas" => show_rates(),
        "reindexar" => reindex(),
        "mostrar" => toggle(),
        "salir" => quit(),
        "config" => {
            let p = config::config_path();
            if !p.exists() {
                Config::load();
            }
            outln!("{}", p.display());
            platform::open(&p.to_string_lossy());
            0
        }
        "version" => {
            outln!("kero {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            out!("{HELP}");
            0
        }
    };
    let _ = std::io::stdout().flush();
    Some(code)
}

/// `kero texto libre` → búsqueda con la que abrir la interfaz.
pub fn gui_query(args: &[String]) -> Option<String> {
    let first = args.first()?;
    if first.starts_with("--") {
        return None;
    }
    let q = args.join(" ");
    (!q.trim().is_empty()).then_some(q)
}

fn fail(msg: &str) -> i32 {
    eprintln!("kero: {msg}");
    1
}

fn piped() -> bool {
    !std::io::stdout().is_terminal()
}

/// Recorta a `w` columnas (aprox. por caracteres) y rellena con espacios.
fn cell(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n > w {
        let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
        t.push('…');
        t
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

fn engine() -> search::Engine {
    let cfg = Config::load();
    let idx: index::SharedIndex = Arc::new(RwLock::new(Arc::new(index::load_cached(&cfg))));
    let usage = Arc::new(RwLock::new(usage::Usage::load(config::data_dir().join("usage.tsv"))));
    search::Engine::new(idx, usage, Arc::new(RwLock::new(cfg)))
}

fn search_cmd(args: &[String], open: bool) -> i32 {
    let mut limit = 10;
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-n" => limit = it.next().and_then(|n| n.parse().ok()).unwrap_or(limit),
            _ => words.push(a.as_str()),
        }
    }
    let q = words.join(" ");
    if q.trim().is_empty() {
        return fail(if open { "uso: kero abrir <texto>" } else { "uso: kero buscar <texto> [-n N]" });
    }
    let results: Vec<Item> = engine().search(&q).into_iter().filter(|i| !matches!(i.action, Action::None)).collect();
    if results.is_empty() {
        return fail(&format!("sin resultados para \u{201C}{q}\u{201D}"));
    }
    if open {
        let it = &results[0];
        return if execute(&it.action) {
            outln!("{}", it.title);
            0
        } else {
            fail(&format!("\u{201C}{}\u{201D} sólo se puede ejecutar desde Kero", it.title))
        };
    }
    if piped() {
        for it in results.iter().take(limit) {
            let line = it.action.path().map(|p| p.to_string_lossy().into_owned()).or_else(|| it.action.copy_text());
            outln!("{}", line.unwrap_or_else(|| it.title.clone()));
        }
    } else {
        let w = results.iter().take(limit).map(|i| i.title.chars().count()).max().unwrap_or(0).clamp(12, 44);
        for it in results.iter().take(limit) {
            outln!("  {}  {}", cell(&it.title, w), it.subtitle);
        }
    }
    0
}

/// Acción principal de un resultado fuera de la interfaz. `false` si necesita Kero
/// (confirmaciones, comandos internos).
fn execute(a: &Action) -> bool {
    match a {
        Action::Open(p) => platform::open(&p.to_string_lossy()),
        Action::App { id, path } => {
            platform::launch_app(id, path.as_deref(), false);
            true
        }
        Action::Url(u) => platform::open(u),
        Action::Copy(t) | Action::Clip(t) => platform::copy_from_cli(t),
        Action::Shell(c) => {
            platform::run_in_terminal(c, false);
            true
        }
        Action::Run(exe, args) => {
            platform::run(exe, args, false);
            true
        }
        Action::System(c) if !a.needs_confirm() => {
            platform::run_system(*c);
            true
        }
        _ => false,
    }
}

fn calc(args: &[String]) -> i32 {
    let expr = args.join(" ");
    if expr.trim().is_empty() {
        return fail("uso: kero calc <expresión>   p. ej. kero calc 100 usd a mxn");
    }
    let cfg = Config::load();
    // En la terminal sí podemos esperar a la red.
    rates::refresh(true, false);
    match crate::plugins::calc::Calc::new().compute(&expr, &cfg.currency) {
        Some((title, copy, detail)) => {
            if piped() {
                outln!("{copy}");
            } else {
                match detail {
                    Some(d) => outln!("{title}   ({d})"),
                    None => outln!("{title}"),
                }
            }
            0
        }
        None if rates::failed() => fail("no se pudo calcular (¿sin conexión para los tipos de cambio?)"),
        None => fail(&format!("no se pudo calcular \u{201C}{expr}\u{201D}")),
    }
}

fn show_rates() -> i32 {
    rates::refresh(true, true);
    let Some(r) = rates::current() else {
        return fail("no se pudieron obtener los tipos de cambio (revisa tu conexión)");
    };
    if rates::failed() {
        eprintln!("kero: sin conexión; se muestran las tasas guardadas");
    }
    let local = crate::plugins::calc::resolve_local(&Config::load().currency);
    outln!("Tipos de cambio del {} · fuente: {}\n", r.date, rates::SOURCE_NAME);
    let local_rate = r.per_usd(&local).unwrap_or(1.0);
    for code in ["USD", "EUR", "GBP", "CAD", "JPY", "CNY", "BTC"] {
        if code == local {
            continue;
        }
        if let Some(per_usd) = r.per_usd(code) {
            let v = local_rate / per_usd;
            outln!("  1 {code} = {} {local}", if v >= 1.0 { format!("{v:.2}") } else { format!("{v:.6}") });
        }
    }
    0
}

fn clip(args: &[String]) -> i32 {
    let arg = args.join(" ");
    let arg = arg.trim();
    if matches!(arg, "borrar" | "clear" | "limpiar") {
        clipboard::clear();
        outln!("Historial del portapapeles borrado");
        return 0;
    }
    if let Ok(n) = arg.parse::<usize>() {
        let Some(text) = clipboard::with_items(|items| items.get(n.wrapping_sub(1)).map(|c| c.text.clone())) else {
            return fail(&format!("no hay entrada {n}"));
        };
        if !platform::copy_from_cli(&text) {
            return fail("no se pudo copiar");
        }
        if piped() {
            out!("{text}");
        } else {
            outln!("Copiado: {}", clipboard::preview(&text, 70));
        }
        return 0;
    }
    let needle = arg.to_lowercase();
    let shown = clipboard::with_items(|items| {
        let mut shown = 0;
        for (i, c) in items.iter().enumerate() {
            if !needle.is_empty() && !c.text.to_lowercase().contains(&needle) {
                continue;
            }
            outln!("{:>4}  {}  {}", i + 1, cell(&clipboard::preview(&c.text, 70), 70), clipboard::ago(c.time));
            shown += 1;
        }
        shown
    });
    if shown == 0 {
        if Config::load().clipboard.enabled {
            outln!("El historial está vacío");
        } else {
            outln!("El historial está desactivado ([clipboard] enabled = false en config.toml)");
        }
    }
    0
}

fn copy(args: &[String]) -> i32 {
    let text = if args.is_empty() {
        let mut s = String::new();
        if std::io::stdin().read_to_string(&mut s).is_err() {
            return fail("no se pudo leer la entrada");
        }
        // PowerShell 5.1 antepone un BOM al texto que manda por la tubería.
        if s.starts_with('\u{feff}') {
            s.remove(0);
        }
        // `echo hola |` añade un salto de línea final que no queremos copiar.
        if s.ends_with('\n') {
            s.pop();
            if s.ends_with('\r') {
                s.pop();
            }
        }
        s
    } else {
        args.join(" ")
    };
    if platform::copy_from_cli(&text) { 0 } else { fail("no se pudo copiar") }
}

fn paste() -> i32 {
    match platform::read_clipboard() {
        Some(t) => {
            out!("{t}");
            if !piped() && !t.ends_with('\n') {
                outln!();
            }
            0
        }
        None => fail("el portapapeles no tiene texto"),
    }
}

fn reindex() -> i32 {
    if platform::ipc_send(Ipc::Reindex) {
        outln!("Reindexando en la instancia abierta…");
        return 0;
    }
    let t = std::time::Instant::now();
    let idx = index::rebuild_blocking(&Config::load());
    outln!("{} apps y {} archivos en {:.1} s", idx.apps.len(), idx.file_count(), t.elapsed().as_secs_f32());
    0
}

fn toggle() -> i32 {
    if platform::ipc_send(Ipc::Toggle) {
        return 0;
    }
    // No hay instancia: arrancar Kero sin esperarlo.
    use std::process::{Command, Stdio};
    let spawn = |exe| Command::new(exe).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    match std::env::current_exe().map(spawn) {
        Ok(Ok(_)) => 0,
        _ => fail("no se pudo abrir Kero"),
    }
}

fn quit() -> i32 {
    if platform::ipc_send(Ipc::Quit) {
        platform::wait_until_closed();
        outln!("Kero cerrado");
    } else {
        outln!("Kero no estaba abierto");
    }
    0
}
