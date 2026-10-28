//! Capa de plataforma: la misma API en Windows (Win32) y en macOS/Linux.

/// Órdenes entre procesos de Kero (`kero --quit`, `kero --toggle`, segundo arranque).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ipc {
    Quit,
    Toggle,
}

#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use win32::*;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;
