//! Consola de Kero para Windows. Se instala como `kero.com` junto a `kero.exe`:
//! la terminal elige `.com` antes que `.exe`, así que `kero buscar …` pasa por
//! aquí, que sí es un programa de consola: la terminal espera la respuesta y
//! muestra la salida. Para abrir la ventana (`kero`, `kero texto`) no espera.
//! En macOS y Linux no hace falta: el ejecutable principal ya es de terminal.

#[cfg(windows)]
#[path = "../commands.rs"]
mod commands;

#[cfg(windows)]
fn main() {
    use std::process::{Command, Stdio, exit};
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(exe) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("kero.exe"))) else { exit(1) };
    let mut cmd = Command::new(exe);
    cmd.args(&args);
    if args.first().and_then(|a| commands::command(a)).is_some() {
        let code = cmd.status().ok().and_then(|s| s.code()).unwrap_or(1);
        exit(code);
    }
    let started = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok();
    exit(if started { 0 } else { 1 });
}

#[cfg(not(windows))]
fn main() {
    eprintln!("kero-console sólo se usa en Windows; ejecuta `kero` directamente.");
    std::process::exit(1);
}
