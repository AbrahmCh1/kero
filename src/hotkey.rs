use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct Hotkey {
    manager: Option<GlobalHotKeyManager>,
    current: Option<HotKey>,
    clipboard: Option<HotKey>,
    pub error: Option<String>,
}

const FALLBACK: &str = "ctrl+alt+space";

/// Id del atajo del historial del portapapeles (0 = ninguno), para distinguirlo en el evento.
static CLIPBOARD_ID: AtomicU32 = AtomicU32::new(0);

pub fn is_clipboard(id: u32) -> bool {
    id != 0 && CLIPBOARD_ID.load(Ordering::Relaxed) == id
}

fn parse(spec: &str) -> Result<HotKey, String> {
    HotKey::from_str(&spec.replace("win", "super")).map_err(|_| format!("Atajo inválido: \u{201C}{spec}\u{201D}"))
}

impl Hotkey {
    pub fn new(spec: &str) -> Self {
        let mut h = Self { manager: GlobalHotKeyManager::new().ok(), current: None, clipboard: None, error: None };
        h.set(spec);
        h
    }

    pub fn set(&mut self, spec: &str) {
        self.error = None;
        let Some(m) = &self.manager else {
            self.error = Some("No se pudo iniciar el gestor de atajos".into());
            return;
        };
        if let Some(old) = self.current.take() {
            let _ = m.unregister(old);
        }
        match parse(spec) {
            Ok(hk) if m.register(hk).is_ok() => self.current = Some(hk),
            Ok(_) => {
                let fb = HotKey::from_str(FALLBACK).expect("fallback hotkey");
                if m.register(fb).is_ok() {
                    self.current = Some(fb);
                }
                self.error = Some(format!("{spec} ocupado (¿PowerToys?) · usando Ctrl+Alt+Espacio"));
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Atajo que abre directo el historial del portapapeles. Vacío = ninguno.
    /// Devuelve un aviso si no se pudo registrar.
    pub fn set_clipboard(&mut self, spec: &str) -> Option<String> {
        let m = self.manager.as_ref()?;
        if let Some(old) = self.clipboard.take() {
            let _ = m.unregister(old);
        }
        CLIPBOARD_ID.store(0, Ordering::Relaxed);
        if spec.trim().is_empty() {
            return None;
        }
        match parse(spec) {
            Ok(hk) if m.register(hk).is_ok() => {
                CLIPBOARD_ID.store(hk.id(), Ordering::Relaxed);
                self.clipboard = Some(hk);
                None
            }
            Ok(_) => Some(format!("{spec} ya lo usa otra app · atajo del portapapeles sin activar")),
            Err(e) => Some(e),
        }
    }
}
