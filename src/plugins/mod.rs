//! Sistema de plugins: cada plugin recibe la consulta y empuja resultados.
//! El motor (`search.rs`) los ordena por `score` y la UI sólo pinta.

pub mod calc;
pub mod color;
pub mod launcher;
pub mod path;
pub mod shell;
pub mod system;
pub mod web;

use crate::config::Config;
use crate::index::Index;
use crate::usage::Usage;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};
use std::path::PathBuf;

/// Iconos de la interfaz. En Windows usan "Segoe Fluent Icons"/"Segoe MDL2 Assets";
/// en macOS y Linux, los emoji monocromos que incluye egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Search,
    Calc,
    Globe,
    Link,
    Lock,
    Power,
    Restart,
    Sleep,
    SignOut,
    Trash,
    Settings,
    Terminal,
    Folder,
    File,
    Refresh,
    Add,
    Exit,
    Info,
    Startup,
    #[cfg_attr(not(windows), allow(dead_code))]
    Tasks,
}

impl Glyph {
    #[cfg(windows)]
    pub fn symbol(self) -> &'static str {
        match self {
            Glyph::Search => "\u{E721}",
            Glyph::Calc => "\u{E8EF}",
            Glyph::Globe => "\u{E774}",
            Glyph::Link => "\u{E71B}",
            Glyph::Lock => "\u{E72E}",
            Glyph::Power | Glyph::Exit => "\u{E7E8}",
            Glyph::Restart => "\u{E777}",
            Glyph::Sleep => "\u{E708}",
            Glyph::SignOut => "\u{F3B1}",
            Glyph::Trash => "\u{E74D}",
            Glyph::Settings => "\u{E713}",
            Glyph::Terminal => "\u{E756}",
            Glyph::Folder => "\u{E8B7}",
            Glyph::File => "\u{E7C3}",
            Glyph::Refresh => "\u{E72C}",
            Glyph::Add => "\u{E710}",
            Glyph::Info => "\u{E946}",
            Glyph::Startup => "\u{E7B5}",
            Glyph::Tasks => "\u{E9D9}",
        }
    }

    #[cfg(not(windows))]
    pub fn symbol(self) -> &'static str {
        match self {
            Glyph::Search => "🔍",
            Glyph::Calc => "🖩",
            Glyph::Globe => "🌐",
            Glyph::Link => "🔗",
            Glyph::Lock => "🔒",
            Glyph::Power => "⏹",
            Glyph::Exit => "✖",
            Glyph::Restart | Glyph::Refresh => "🔄",
            Glyph::Sleep => "🌙",
            Glyph::SignOut => "🚪",
            Glyph::Trash => "🗑",
            Glyph::Settings => "⚙",
            Glyph::Terminal => "💻",
            Glyph::Folder => "📁",
            Glyph::File => "📄",
            Glyph::Add => "➕",
            Glyph::Info => "ℹ",
            Glyph::Startup => "🚀",
            Glyph::Tasks => "📊",
        }
    }
}

/// Tecla modificadora principal: ⌘ en macOS, Ctrl en el resto.
pub const MOD_ENTER: &str = if cfg!(target_os = "macos") { "⌘ Enter" } else { "Ctrl Enter" };
pub const MOD_C: &str = if cfg!(target_os = "macos") { "⌘ C" } else { "Ctrl C" };
/// Elevar con Shift+Enter sólo existe en Windows (UAC).
const ADMIN: bool = cfg!(windows);

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Icon {
    None,
    Glyph(Glyph),
    /// Icono de app (en Windows, de `shell:AppsFolder`).
    App(String),
    /// Icono de archivo/carpeta del Shell.
    Path(PathBuf, bool),
    Color([u8; 3]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemCmd {
    Lock,
    Sleep,
    /// macOS no distingue hibernar de suspender.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Hibernate,
    Shutdown,
    Restart,
    SignOut,
    EmptyTrash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeroCmd {
    Reindex,
    OpenConfig,
    AddFolder,
    ToggleAutostart,
    Quit,
}

#[derive(Clone, Debug)]
pub enum Action {
    None,
    Open(PathBuf),
    App {
        id: String,
        path: Option<String>,
    },
    /// URL o URI (https:, ms-settings:, …)
    Url(String),
    Copy(String),
    Shell(String),
    /// Programa + argumentos (en macOS todo se abre por URL o como app).
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Run(String, String),
    System(SystemCmd),
    Kero(KeroCmd),
}

impl Action {
    pub fn needs_confirm(&self) -> bool {
        matches!(self, Action::System(SystemCmd::Shutdown | SystemCmd::Restart | SystemCmd::SignOut))
    }

    /// Ruta asociada (para "abrir ubicación" y "copiar ruta").
    pub fn path(&self) -> Option<PathBuf> {
        match self {
            Action::Open(p) => Some(p.clone()),
            Action::App { path: Some(p), .. } => Some(PathBuf::from(p)),
            _ => None,
        }
    }

    /// Texto que copia Ctrl+C.
    pub fn copy_text(&self) -> Option<String> {
        match self {
            Action::Copy(s) | Action::Url(s) | Action::Shell(s) => Some(s.clone()),
            _ => self.path().map(|p| p.to_string_lossy().into_owned()),
        }
    }

    pub fn hints(&self) -> Vec<(&'static str, &'static str)> {
        let mut h = vec![("Enter", self.primary_label())];
        match self {
            Action::Open(_) => h.extend([(MOD_ENTER, "Carpeta"), (MOD_C, "Copiar ruta")]),
            Action::App { path: Some(_), .. } => {
                if ADMIN {
                    h.push(("Shift Enter", "Admin"));
                }
                h.push((MOD_ENTER, "Ubicación"));
            }
            Action::Url(_) => h.push((MOD_C, "Copiar")),
            Action::Shell(_) if ADMIN => h.push(("Shift Enter", "Admin")),
            _ => {}
        }
        h
    }

    pub fn primary_label(&self) -> &'static str {
        match self {
            Action::Open(_) | Action::App { .. } | Action::Url(_) => "Abrir",
            Action::Copy(_) => "Copiar",
            _ => "Ejecutar",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub title: String,
    pub subtitle: String,
    pub icon: Icon,
    pub action: Action,
    pub score: i32,
    /// Índices (en chars) del título que coinciden con la búsqueda.
    pub highlight: Vec<u32>,
    /// Clave para frecuencia de uso.
    pub key: Option<String>,
    /// Texto a poner en la barra al pulsar Tab.
    pub complete: Option<String>,
    /// Coincidencia sólo dispersa: se descarta si hay suficientes fuertes.
    pub weak: bool,
}

impl Item {
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>, icon: Icon, action: Action, score: i32) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            icon,
            action,
            score,
            highlight: Vec::new(),
            key: None,
            complete: None,
            weak: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    All,
    /// `/texto` — sólo archivos y carpetas
    Files,
    /// `>comando`
    Shell,
    /// `=expresión`
    Calc,
}

pub struct Query<'a> {
    pub text: &'a str,
    pub mode: Mode,
    pub index: &'a Index,
    pub usage: &'a Usage,
    pub cfg: &'a Config,
}

pub trait Plugin: Send + Sync {
    fn query(&self, q: &Query, out: &mut Vec<Item>);
}

pub fn all() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(calc::Calc::new()),
        Box::new(color::ColorPlugin),
        Box::new(web::Web),
        Box::new(shell::Shell),
        Box::new(path::PathBrowse),
        Box::new(system::System),
        Box::new(launcher::Launcher),
    ]
}

// ---------------------------------------------------------------------------

/// Calidad de la coincidencia más allá del score fuzzy: (bonus, fuerte).
/// "Fuerte" = cada palabra de la búsqueda aparece como prefijo, subcadena o
/// siglas; las coincidencias sólo dispersas (p‑d‑f en "Preferencias de…") no.
pub fn quality(name: &str, query_lower: &str) -> (i32, bool) {
    let n = name.to_lowercase();
    let mut total = 0;
    let mut atoms = 0;
    let mut strong = true;
    for q in query_lower.split_whitespace() {
        atoms += 1;
        let words = || n.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty());
        let b = if n == q || n.rsplit_once('.').is_some_and(|(stem, _)| stem == q) {
            120
        } else if n.starts_with(q) {
            80
        } else if words().any(|w| w.starts_with(q)) {
            60
        } else if q.len() >= 2 && words().filter_map(|w| w.chars().next()).collect::<String>().starts_with(q) {
            50
        } else if n.contains(q) {
            35
        } else {
            strong = false;
            0
        };
        total += b;
    }
    if atoms == 0 {
        return (0, false);
    }
    (total / atoms, strong)
}

/// Envoltura cómoda sobre nucleo para plugins con listas pequeñas.
pub struct Fuzzy {
    pub pattern: Pattern,
    matcher: Matcher,
    buf: Vec<char>,
}

impl Fuzzy {
    pub fn new(query: &str) -> Self {
        let mut cfg = nucleo_matcher::Config::DEFAULT;
        cfg.prefer_prefix = true;
        Self {
            pattern: Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart),
            matcher: Matcher::new(cfg),
            buf: Vec::new(),
        }
    }

    pub fn score(&mut self, s: &str) -> Option<u32> {
        self.pattern.score(Utf32Str::new(s, &mut self.buf), &mut self.matcher)
    }

    pub fn indices(&mut self, s: &str) -> Vec<u32> {
        let mut idx = Vec::new();
        self.pattern.indices(Utf32Str::new(s, &mut self.buf), &mut self.matcher, &mut idx);
        idx.sort_unstable();
        idx.dedup();
        idx
    }
}
