use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Atajo global, p.ej. "alt+space", "ctrl+space", "win+alt+k".
    pub hotkey: String,
    /// Iniciar Kero con Windows.
    pub autostart: bool,
    /// Tema: "auto" (sigue a Windows), "dark" o "light".
    pub theme: String,
    /// Número máximo de resultados.
    pub max_results: usize,
    pub index: IndexConfig,
    #[serde(rename = "web")]
    pub web_engines: Vec<WebEngine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexConfig {
    /// Carpetas que se indexan (recursivo). Admite %VARIABLES% y "~".
    pub roots: Vec<String>,
    /// Nombres de carpeta o archivo que se ignoran (comparación exacta, sin mayúsculas).
    pub exclude: Vec<String>,
    /// Extensiones que se ignoran (sin punto).
    pub exclude_ext: Vec<String>,
    pub skip_hidden: bool,
    pub max_depth: usize,
    /// Vigilar cambios en disco y reindexar automáticamente.
    pub watch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebEngine {
    pub keyword: String,
    pub name: String,
    /// URL con `{q}` donde va la búsqueda.
    pub url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "alt+space".into(),
            autostart: false,
            theme: "auto".into(),
            max_results: 40,
            index: IndexConfig::default(),
            web_engines: vec![
                engine("g", "Google", "https://www.google.com/search?q={q}"),
                engine("yt", "YouTube", "https://www.youtube.com/results?search_query={q}"),
                engine("gh", "GitHub", "https://github.com/search?q={q}"),
                engine("w", "Wikipedia", "https://es.wikipedia.org/w/index.php?search={q}"),
                engine("maps", "Google Maps", "https://www.google.com/maps/search/{q}"),
                engine("tr", "Google Translate", "https://translate.google.com/?sl=auto&tl=es&text={q}"),
                engine("crates", "crates.io", "https://crates.io/search?q={q}"),
            ],
        }
    }
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            roots: vec![
                "~\\Desktop".into(),
                "~\\Documents".into(),
                "~\\Downloads".into(),
                "~\\Pictures".into(),
                "~\\Videos".into(),
                "~\\Music".into(),
                "~\\OneDrive".into(),
            ],
            exclude: [
                "node_modules", ".git", ".svn", ".hg", "target", "__pycache__", ".venv", "venv",
                ".cache", ".gradle", ".idea", ".vs", "obj", "$RECYCLE.BIN",
                "System Volume Information", "AppData",
            ]
            .map(String::from)
            .to_vec(),
            exclude_ext: ["tmp", "crdownload", "part", "lock"].map(String::from).to_vec(),
            skip_hidden: true,
            max_depth: 14,
            watch: true,
        }
    }
}

fn engine(k: &str, n: &str, u: &str) -> WebEngine {
    WebEngine { keyword: k.into(), name: n.into(), url: u.into() }
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| ".".into());
    let dir = base.join("Kero");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| ".".into());
    let dir = base.join("Kero");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                eprintln!("config.toml inválido: {e}");
                Config::default()
            }),
            Err(_) => {
                let cfg = Config::default();
                cfg.save();
                cfg
            }
        }
    }

    pub fn save(&self) {
        let header = "# Configuración de Kero. Los cambios se aplican al guardar.\n\
                      # roots: carpetas a indexar · exclude: carpetas/archivos a ignorar\n\
                      # [[web]]: atajos de búsqueda web, p.ej. \"g gatos\"\n\n";
        if let Ok(body) = toml::to_string_pretty(self) {
            let _ = std::fs::write(config_path(), format!("{header}{body}"));
        }
    }

    pub fn resolved_roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = self
            .index
            .roots
            .iter()
            .map(|r| expand_path(r))
            .filter(|p| p.is_dir())
            .collect();
        roots.sort();
        // Quita raíces anidadas dentro de otra raíz para no indexar dos veces.
        let mut out: Vec<PathBuf> = Vec::new();
        for r in roots {
            if !out.iter().any(|o| r.starts_with(o)) {
                out.push(r);
            }
        }
        out
    }
}

pub fn expand_path(raw: &str) -> PathBuf {
    let mut s = raw.trim().to_string();
    if let Some(rest) = s.strip_prefix('~') {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        s = format!("{home}{rest}");
    }
    // Expande %VAR%
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    PathBuf::from(out)
}

pub fn display_path(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if let Ok(home) = std::env::var("USERPROFILE") {
        if let Some(rest) = s.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    s
}
