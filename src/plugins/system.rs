//! Comandos del sistema, páginas de Configuración de Windows y comandos de Kero.

use super::{Action, Fuzzy, Icon, Item, KeroCmd, Mode, Plugin, Query, SystemCmd, glyph, quality};

pub struct System;

struct Entry {
    title: &'static str,
    subtitle: &'static str,
    /// Sinónimos extra para buscar (español + inglés).
    aliases: &'static str,
    icon: char,
    action: fn() -> Action,
}

macro_rules! e {
    ($t:expr, $s:expr, $a:expr, $i:expr, $act:expr) => {
        Entry { title: $t, subtitle: $s, aliases: $a, icon: $i, action: || $act }
    };
}

fn url(s: &str) -> Action {
    Action::Url(s.to_string())
}

const ENTRIES: &[Entry] = &[
    e!("Bloquear equipo", "Sistema", "lock", glyph::LOCK, Action::System(SystemCmd::Lock)),
    e!("Suspender", "Sistema", "sleep dormir", glyph::SLEEP, Action::System(SystemCmd::Sleep)),
    e!("Hibernar", "Sistema", "hibernate", glyph::SLEEP, Action::System(SystemCmd::Hibernate)),
    e!("Apagar", "Sistema", "shutdown power off", glyph::POWER, Action::System(SystemCmd::Shutdown)),
    e!("Reiniciar", "Sistema", "restart reboot", glyph::RESTART, Action::System(SystemCmd::Restart)),
    e!("Cerrar sesión", "Sistema", "sign out logoff log out", glyph::SIGNOUT, Action::System(SystemCmd::SignOut)),
    e!("Vaciar papelera", "Sistema", "empty recycle bin trash", glyph::TRASH, Action::System(SystemCmd::EmptyRecycleBin)),
    e!("Administrador de tareas", "Sistema", "task manager taskmgr", glyph::TASKS, Action::Run("taskmgr.exe".into(), String::new())),
    e!("Panel de control", "Sistema", "control panel", glyph::SETTINGS, Action::Run("control.exe".into(), String::new())),
    // Configuración de Windows
    e!("Bluetooth", "Configuración", "dispositivos devices", glyph::SETTINGS, url("ms-settings:bluetooth")),
    e!("Wi-Fi", "Configuración", "wifi red network internet", glyph::SETTINGS, url("ms-settings:network-wifi")),
    e!("Pantalla", "Configuración", "display monitor resolución brillo brightness", glyph::SETTINGS, url("ms-settings:display")),
    e!("Sonido", "Configuración", "sound audio volumen", glyph::SETTINGS, url("ms-settings:sound")),
    e!("Aplicaciones instaladas", "Configuración", "apps desinstalar uninstall programs", glyph::SETTINGS, url("ms-settings:appsfeatures")),
    e!("Aplicaciones de inicio", "Configuración", "startup apps arranque", glyph::SETTINGS, url("ms-settings:startupapps")),
    e!("Windows Update", "Configuración", "actualizaciones updates", glyph::SETTINGS, url("ms-settings:windowsupdate")),
    e!("Fondo de pantalla", "Configuración", "wallpaper background", glyph::SETTINGS, url("ms-settings:personalization-background")),
    e!("Colores y modo oscuro", "Configuración", "dark mode tema theme colors", glyph::SETTINGS, url("ms-settings:colors")),
    e!("Almacenamiento", "Configuración", "storage disco disk espacio", glyph::SETTINGS, url("ms-settings:storagesense")),
    e!("Energía y batería", "Configuración", "power battery bateria", glyph::SETTINGS, url("ms-settings:powersleep")),
    e!("Mouse", "Configuración", "ratón mouse touchpad", glyph::SETTINGS, url("ms-settings:mousetouchpad")),
    e!("Idioma y región", "Configuración", "language teclado keyboard region", glyph::SETTINGS, url("ms-settings:regionlanguage")),
    e!("Impresoras", "Configuración", "printers escáner scanner", glyph::SETTINGS, url("ms-settings:printers")),
    e!("VPN", "Configuración", "vpn red", glyph::SETTINGS, url("ms-settings:network-vpn")),
    e!("Notificaciones", "Configuración", "notifications", glyph::SETTINGS, url("ms-settings:notifications")),
    e!("Fecha y hora", "Configuración", "date time reloj clock", glyph::SETTINGS, url("ms-settings:dateandtime")),
    e!("Variables de entorno", "Sistema", "environment variables path", glyph::SETTINGS, Action::Run("rundll32.exe".into(), "sysdm.cpl,EditEnvironmentVariables".into())),
    e!("Configuración de Windows", "Configuración", "settings ajustes", glyph::SETTINGS, url("ms-settings:")),
    // Kero
    e!("Kero: Reindexar", "Vuelve a escanear apps y archivos", "reindex refresh actualizar", glyph::REFRESH, Action::Kero(KeroCmd::Reindex)),
    e!("Kero: Agregar carpeta al índice", "Elige una carpeta para indexar", "add folder index indexar", glyph::ADD, Action::Kero(KeroCmd::AddFolder)),
    e!("Kero: Configuración", "Abre config.toml", "settings config ajustes preferencias", glyph::SETTINGS, Action::Kero(KeroCmd::OpenConfig)),
    e!("Kero: Iniciar con Windows", "Activa o desactiva el arranque automático", "autostart startup inicio", glyph::STARTUP, Action::Kero(KeroCmd::ToggleAutostart)),
    e!("Kero: Salir", "Cierra Kero", "quit exit cerrar", glyph::EXIT, Action::Kero(KeroCmd::Quit)),
];

const HELP: &[(&str, &str)] = &[
    ("texto", "Apps, archivos, carpetas y comandos"),
    ("/texto", "Sólo archivos y carpetas"),
    ("=2^10 · 5 km to mi", "Calculadora y unidades (Enter copia)"),
    (">comando", "Ejecutar en terminal"),
    ("g gatos · yt música", "Búsqueda web por palabra clave"),
    ("C:\\ruta\\", "Navegar carpetas (Tab autocompleta)"),
    ("#ff8800", "Convertir colores"),
];

impl Plugin for System {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::All || q.text.is_empty() {
            return;
        }
        if q.text == "?" {
            for (i, (k, d)) in HELP.iter().enumerate() {
                out.push(Item::new(*k, *d, Icon::Glyph(glyph::INFO), Action::None, 20_000 - i as i32));
            }
            return;
        }
        let mut fz = Fuzzy::new(q.text);
        for e in ENTRIES {
            let title_score = fz.score(e.title);
            let alias_score = fz.score(e.aliases).map(|s| s.saturating_sub(20));
            let Some(s) = title_score.max(alias_score) else { continue };
            let lower = q.text.to_lowercase();
            let (bt, st) = quality(e.title, &lower);
            let (ba, sa) = quality(e.aliases, &lower);
            let bonus = if st { bt + 60 } else if sa { ba + 40 } else { -20 };
            let mut it = Item::new(e.title, e.subtitle, Icon::Glyph(e.icon), (e.action)(), s as i32 + bonus);
            if title_score.is_some() {
                it.highlight = fz.indices(e.title);
            }
            it.weak = !st && !sa;
            let key = format!("sys:{}", e.title);
            it.score += q.usage.bonus(&key, &q.text.to_lowercase());
            it.key = Some(key);
            out.push(it);
        }
    }
}
