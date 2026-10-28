use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;
use std::str::FromStr;

pub struct Hotkey {
    manager: Option<GlobalHotKeyManager>,
    current: Option<HotKey>,
    pub error: Option<String>,
}

const FALLBACK: &str = "ctrl+alt+space";

impl Hotkey {
    pub fn new(spec: &str) -> Self {
        let mut h = Self { manager: GlobalHotKeyManager::new().ok(), current: None, error: None };
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
        let parsed = HotKey::from_str(&spec.replace("win", "super"));
        match parsed {
            Ok(hk) if m.register(hk).is_ok() => self.current = Some(hk),
            Ok(_) => {
                let fb = HotKey::from_str(FALLBACK).expect("fallback hotkey");
                if m.register(fb).is_ok() {
                    self.current = Some(fb);
                }
                self.error = Some(format!(
                    "{spec} ocupado (¿PowerToys?) · usando Ctrl+Alt+Espacio"
                ));
            }
            Err(_) => self.error = Some(format!("Atajo inválido: \u{201C}{spec}\u{201D}")),
        }
    }
}
