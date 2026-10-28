//! Comandos del sistema, páginas de configuración del sistema y comandos de Kero.

use super::{Action, Fuzzy, Glyph, Icon, Item, KeroCmd, Mode, Plugin, Query, SystemCmd, quality};

pub struct System;

struct Entry {
    title: &'static str,
    subtitle: &'static str,
    /// Sinónimos extra para buscar (español + inglés).
    aliases: &'static str,
    icon: Glyph,
    action: fn() -> Action,
}

macro_rules! e {
    ($t:expr, $s:expr, $a:expr, $i:expr, $act:expr) => {
        Entry { title: $t, subtitle: $s, aliases: $a, icon: $i, action: || $act }
    };
}

#[cfg(any(windows, target_os = "macos"))]
fn url(s: &str) -> Action {
    Action::Url(s.to_string())
}

#[cfg(target_os = "linux")]
fn settings(panel: &str) -> Action {
    Action::Run("gnome-control-center".into(), panel.into())
}

const COMMON: &[Entry] = &[
    e!("Bloquear equipo", "Sistema", "lock", Glyph::Lock, Action::System(SystemCmd::Lock)),
    e!("Suspender", "Sistema", "sleep dormir", Glyph::Sleep, Action::System(SystemCmd::Sleep)),
    e!("Apagar", "Sistema", "shutdown power off", Glyph::Power, Action::System(SystemCmd::Shutdown)),
    e!("Reiniciar", "Sistema", "restart reboot", Glyph::Restart, Action::System(SystemCmd::Restart)),
    e!("Cerrar sesión", "Sistema", "sign out logoff log out", Glyph::SignOut, Action::System(SystemCmd::SignOut)),
    e!("Vaciar papelera", "Sistema", "empty recycle bin trash", Glyph::Trash, Action::System(SystemCmd::EmptyTrash)),
    // Kero
    e!(
        "Kero: Reindexar",
        "Vuelve a escanear apps y archivos",
        "reindex refresh actualizar",
        Glyph::Refresh,
        Action::Kero(KeroCmd::Reindex)
    ),
    e!(
        "Kero: Agregar carpeta al índice",
        "Elige una carpeta para indexar",
        "add folder index indexar",
        Glyph::Add,
        Action::Kero(KeroCmd::AddFolder)
    ),
    e!(
        "Kero: Configuración",
        "Abre config.toml",
        "settings config ajustes preferencias",
        Glyph::Settings,
        Action::Kero(KeroCmd::OpenConfig)
    ),
    e!(
        "Kero: Iniciar con el sistema",
        "Activa o desactiva el arranque automático",
        "autostart startup inicio",
        Glyph::Startup,
        Action::Kero(KeroCmd::ToggleAutostart)
    ),
    e!("Kero: Salir", "Cierra Kero", "quit exit cerrar", Glyph::Exit, Action::Kero(KeroCmd::Quit)),
];

#[cfg(windows)]
const PLATFORM: &[Entry] = &[
    e!("Hibernar", "Sistema", "hibernate", Glyph::Sleep, Action::System(SystemCmd::Hibernate)),
    e!(
        "Administrador de tareas",
        "Sistema",
        "task manager taskmgr",
        Glyph::Tasks,
        Action::Run("taskmgr.exe".into(), String::new())
    ),
    e!(
        "Panel de control",
        "Sistema",
        "control panel",
        Glyph::Settings,
        Action::Run("control.exe".into(), String::new())
    ),
    e!(
        "Variables de entorno",
        "Sistema",
        "environment variables path",
        Glyph::Settings,
        Action::Run("rundll32.exe".into(), "sysdm.cpl,EditEnvironmentVariables".into())
    ),
    e!("Bluetooth", "Configuración", "dispositivos devices", Glyph::Settings, url("ms-settings:bluetooth")),
    e!("Wi-Fi", "Configuración", "wifi red network internet", Glyph::Settings, url("ms-settings:network-wifi")),
    e!(
        "Pantalla",
        "Configuración",
        "display monitor resolución brillo brightness",
        Glyph::Settings,
        url("ms-settings:display")
    ),
    e!("Sonido", "Configuración", "sound audio volumen", Glyph::Settings, url("ms-settings:sound")),
    e!(
        "Aplicaciones instaladas",
        "Configuración",
        "apps desinstalar uninstall programs",
        Glyph::Settings,
        url("ms-settings:appsfeatures")
    ),
    e!(
        "Aplicaciones de inicio",
        "Configuración",
        "startup apps arranque",
        Glyph::Settings,
        url("ms-settings:startupapps")
    ),
    e!("Windows Update", "Configuración", "actualizaciones updates", Glyph::Settings, url("ms-settings:windowsupdate")),
    e!(
        "Fondo de pantalla",
        "Configuración",
        "wallpaper background",
        Glyph::Settings,
        url("ms-settings:personalization-background")
    ),
    e!(
        "Colores y modo oscuro",
        "Configuración",
        "dark mode tema theme colors",
        Glyph::Settings,
        url("ms-settings:colors")
    ),
    e!(
        "Almacenamiento",
        "Configuración",
        "storage disco disk espacio",
        Glyph::Settings,
        url("ms-settings:storagesense")
    ),
    e!("Energía y batería", "Configuración", "power battery bateria", Glyph::Settings, url("ms-settings:powersleep")),
    e!("Mouse", "Configuración", "ratón mouse touchpad", Glyph::Settings, url("ms-settings:mousetouchpad")),
    e!(
        "Idioma y región",
        "Configuración",
        "language teclado keyboard region",
        Glyph::Settings,
        url("ms-settings:regionlanguage")
    ),
    e!("Impresoras", "Configuración", "printers escáner scanner", Glyph::Settings, url("ms-settings:printers")),
    e!("VPN", "Configuración", "vpn red", Glyph::Settings, url("ms-settings:network-vpn")),
    e!("Notificaciones", "Configuración", "notifications", Glyph::Settings, url("ms-settings:notifications")),
    e!("Fecha y hora", "Configuración", "date time reloj clock", Glyph::Settings, url("ms-settings:dateandtime")),
    e!("Configuración de Windows", "Configuración", "settings ajustes", Glyph::Settings, url("ms-settings:")),
];

#[cfg(target_os = "macos")]
const PLATFORM: &[Entry] = &[
    e!(
        "Bluetooth",
        "Ajustes del Sistema",
        "dispositivos devices",
        Glyph::Settings,
        url("x-apple.systempreferences:com.apple.BluetoothSettings")
    ),
    e!(
        "Wi-Fi",
        "Ajustes del Sistema",
        "wifi red network internet",
        Glyph::Settings,
        url("x-apple.systempreferences:com.apple.wifi-settings-extension")
    ),
    e!(
        "Pantallas",
        "Ajustes del Sistema",
        "display monitor resolución brillo brightness",
        Glyph::Settings,
        url("x-apple.systempreferences:com.apple.Displays-Settings.extension")
    ),
    e!(
        "Sonido",
        "Ajustes del Sistema",
        "sound audio volumen",
        Glyph::Settings,
        url("x-apple.systempreferences:com.apple.Sound-Settings.extension")
    ),
    e!(
        "Red",
        "Ajustes del Sistema",
        "network internet vpn",
        Glyph::Settings,
        url("x-apple.systempreferences:com.apple.Network-Settings.extension")
    ),
    e!(
        "Batería",
        "Ajustes del Sistema",
        "battery energía power",
        Glyph::Settings,
        url("x-apple.systempreferences:com.apple.Battery-Settings.extension")
    ),
    e!(
        "Ítems de inicio",
        "Ajustes del Sistema",
        "login items startup arranque",
        Glyph::Settings,
        url("x-apple.systempreferences:com.apple.LoginItems-Settings.extension")
    ),
    e!(
        "Privacidad y seguridad",
        "Ajustes del Sistema",
        "privacy security permisos accesibilidad",
        Glyph::Settings,
        url("x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension")
    ),
    e!(
        "Ajustes del Sistema",
        "Ajustes del Sistema",
        "settings preferences preferencias",
        Glyph::Settings,
        url("x-apple.systempreferences:")
    ),
];

#[cfg(target_os = "linux")]
const PLATFORM: &[Entry] = &[
    e!("Hibernar", "Sistema", "hibernate", Glyph::Sleep, Action::System(SystemCmd::Hibernate)),
    e!("Bluetooth", "Configuración", "dispositivos devices", Glyph::Settings, settings("bluetooth")),
    e!("Wi-Fi", "Configuración", "wifi red network internet", Glyph::Settings, settings("wifi")),
    e!("Pantallas", "Configuración", "display monitor resolución brillo", Glyph::Settings, settings("display")),
    e!("Sonido", "Configuración", "sound audio volumen", Glyph::Settings, settings("sound")),
    e!("Energía", "Configuración", "power battery bateria", Glyph::Settings, settings("power")),
    e!(
        "Configuración",
        "Configuración",
        "settings ajustes preferencias",
        Glyph::Settings,
        Action::Run("gnome-control-center".into(), String::new())
    ),
];

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
const PLATFORM: &[Entry] = &[];

const HELP: &[(&str, &str)] = &[
    ("texto", "Apps, archivos, carpetas y comandos"),
    ("/texto", "Sólo archivos y carpetas"),
    ("=2^10 · 5 km to mi", "Calculadora y unidades (Enter copia)"),
    (">comando", "Ejecutar en terminal"),
    ("g gatos · yt música", "Búsqueda web por palabra clave"),
    (if cfg!(windows) { "C:\\ruta\\" } else { "~/ruta/" }, "Navegar carpetas (Tab autocompleta)"),
    ("#ff8800", "Convertir colores"),
];

impl Plugin for System {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if q.mode != Mode::All || q.text.is_empty() {
            return;
        }
        if q.text == "?" {
            for (i, (k, d)) in HELP.iter().enumerate() {
                out.push(Item::new(*k, *d, Icon::Glyph(Glyph::Info), Action::None, 20_000 - i as i32));
            }
            return;
        }
        let mut fz = Fuzzy::new(q.text);
        let lower = q.text.to_lowercase();
        for e in COMMON.iter().chain(PLATFORM) {
            let title_score = fz.score(e.title);
            let alias_score = fz.score(e.aliases).map(|s| s.saturating_sub(20));
            let Some(s) = title_score.max(alias_score) else { continue };
            let (bt, st) = quality(e.title, &lower);
            let (ba, sa) = quality(e.aliases, &lower);
            let bonus = if st {
                bt + 60
            } else if sa {
                ba + 40
            } else {
                -20
            };
            let mut it = Item::new(e.title, e.subtitle, Icon::Glyph(e.icon), (e.action)(), s as i32 + bonus);
            if title_score.is_some() {
                it.highlight = fz.indices(e.title);
            }
            it.weak = !st && !sa;
            let key = format!("sys:{}", e.title);
            it.score += q.usage.bonus(&key, &lower);
            it.key = Some(key);
            out.push(it);
        }
    }
}
