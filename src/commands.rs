//! Nombres de los subcomandos de terminal. Lo comparten `kero` y la consola
//! `kero.com` de Windows (que sólo espera la respuesta si es un subcomando).

/// Nombre canónico primero, luego alias.
pub const COMMANDS: &[&[&str]] = &[
    &["buscar", "search", "s"],
    &["abrir", "open", "o"],
    &["calc", "c", "="],
    &["clip", "cb"],
    &["snip", "sn", "snippets"],
    &["copiar", "copy"],
    &["pegar", "paste"],
    &["tasas", "rates"],
    &["reindexar", "reindex"],
    &["mostrar", "toggle", "--toggle"],
    &["salir", "quit", "--quit"],
    &["config"],
    &["version", "--version", "-v", "-V"],
    &["ayuda", "help", "--help", "-h", "/?"],
];

/// Nombre canónico del subcomando, si `word` es uno.
pub fn command(word: &str) -> Option<&'static str> {
    let w = word.to_lowercase();
    COMMANDS.iter().find(|names| names.contains(&w.as_str())).map(|names| names[0])
}
