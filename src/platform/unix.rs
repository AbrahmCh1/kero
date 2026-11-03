//! Implementación macOS y Linux de la capa de plataforma.
//! Se apoya en las herramientas estándar de cada sistema (`open`, `xdg-open`,
//! `osascript`, `systemctl`, …) en lugar de enlazar frameworks nativos.

use super::Ipc;
use crate::plugins::SystemCmd;
use parking_lot::Mutex;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const MACOS: bool = cfg!(target_os = "macos");

/// Lanza un proceso desacoplado; un hilo lo recoge al terminar (sin zombis).
fn spawn(program: &str, args: &[&str]) -> bool {
    match Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?.to_str()?.split(':').map(|d| Path::new(d).join(name)).find(|p| p.is_file())
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/".into())
}

pub fn open(target: &str) -> bool {
    spawn(if MACOS { "open" } else { "xdg-open" }, &[target])
}

pub fn reveal(path: &Path) {
    if MACOS {
        spawn("open", &["-R", &path.to_string_lossy()]);
    } else if let Some(parent) = path.parent() {
        open(&parent.to_string_lossy());
    }
}

/// Ejecuta un programa. `admin` no aplica fuera de Windows.
pub fn run(exe: &str, args: &str, _admin: bool) {
    let args: Vec<&str> = args.split_whitespace().collect();
    spawn(exe, &args);
}

pub fn launch_app(id: &str, _path: Option<&str>, _admin: bool) {
    // macOS: `id` es la ruta del .app.
    #[cfg(target_os = "macos")]
    spawn("open", &[id]);
    // Linux: `id` es la ruta del .desktop.
    #[cfg(target_os = "linux")]
    if let Some((exec, terminal)) = crate::index::apps::desktop_exec(Path::new(id)) {
        if terminal {
            run_in_terminal(&exec, false);
        } else {
            spawn("sh", &["-c", &exec]);
        }
    }
}

pub fn run_in_terminal(cmd: &str, _admin: bool) {
    if MACOS {
        let escaped = cmd.replace('\\', "\\\\").replace('"', "\\\"");
        spawn(
            "osascript",
            &[
                "-e",
                &format!("tell application \"Terminal\" to do script \"{escaped}\""),
                "-e",
                "tell application \"Terminal\" to activate",
            ],
        );
        return;
    }
    // Deja la terminal abierta después del comando.
    let script = format!("{cmd}; exec \"${{SHELL:-sh}}\"");
    let mut candidates: Vec<String> = std::env::var("TERMINAL").into_iter().collect();
    candidates.extend(
        [
            "x-terminal-emulator",
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "kitty",
            "alacritty",
            "wezterm",
            "xterm",
        ]
        .map(String::from),
    );
    for term in candidates {
        if find_in_path(&term).is_none() {
            continue;
        }
        let ok = match term.as_str() {
            "gnome-terminal" => spawn(&term, &["--", "sh", "-c", &script]),
            "wezterm" => spawn(&term, &["start", "--", "sh", "-c", &script]),
            _ => spawn(&term, &["-e", "sh", "-c", &script]),
        };
        if ok {
            return;
        }
    }
}

pub fn run_system(cmd: SystemCmd) {
    if MACOS {
        let tell = |what: &str| {
            spawn("osascript", &["-e", &format!("tell application \"System Events\" to {what}")]);
        };
        match cmd {
            SystemCmd::Lock => {
                spawn("pmset", &["displaysleepnow"]);
            }
            SystemCmd::Sleep | SystemCmd::Hibernate => {
                spawn("pmset", &["sleepnow"]);
            }
            SystemCmd::Shutdown => tell("shut down"),
            SystemCmd::Restart => tell("restart"),
            SystemCmd::SignOut => tell("log out"),
            SystemCmd::EmptyTrash => {
                spawn("osascript", &["-e", "tell application \"Finder\" to empty trash"]);
            }
        }
        return;
    }
    match cmd {
        SystemCmd::Lock => {
            spawn("loginctl", &["lock-session"]);
        }
        SystemCmd::Sleep => {
            spawn("systemctl", &["suspend"]);
        }
        SystemCmd::Hibernate => {
            spawn("systemctl", &["hibernate"]);
        }
        SystemCmd::Shutdown => {
            spawn("systemctl", &["poweroff"]);
        }
        SystemCmd::Restart => {
            spawn("systemctl", &["reboot"]);
        }
        SystemCmd::SignOut => match std::env::var("XDG_SESSION_ID") {
            Ok(id) => {
                spawn("loginctl", &["terminate-session", &id]);
            }
            Err(_) => {
                spawn("gnome-session-quit", &["--logout", "--no-prompt"]);
            }
        },
        SystemCmd::EmptyTrash => {
            spawn("gio", &["trash", "--empty"]);
        }
    }
}

/// En X11 el contenido vive mientras viva el `Clipboard`: lo mantenemos.
fn clipboard<R>(f: impl FnOnce(&mut arboard::Clipboard) -> Option<R>) -> Option<R> {
    static CLIP: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();
    let mut guard = CLIP.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok())).lock();
    guard.as_mut().and_then(f)
}

pub fn copy_to_clipboard(text: &str) -> bool {
    clipboard(|c| c.set_text(text.to_string()).ok()).is_some()
}

/// Copia marcándolo como privado (nspasteboard.org en macOS, la pista de KDE en
/// Linux) para que los gestores de portapapeles no lo guarden.
pub fn copy_secret(text: &str) -> bool {
    #[cfg(target_os = "macos")]
    use arboard::SetExtApple;
    #[cfg(target_os = "linux")]
    use arboard::SetExtLinux;
    clipboard(|c| c.set().exclude_from_history().text(text.to_string()).ok()).is_some()
}

/// Copia archivos para pegarlos en Finder, el gestor de archivos, un correo…
pub fn copy_files(paths: &[PathBuf]) -> bool {
    clipboard(|c| c.set().file_list(paths).ok()).is_some()
}

/// Abre una terminal en `dir`.
pub fn open_terminal(dir: &Path) {
    let d = dir.to_string_lossy();
    if MACOS {
        spawn("open", &["-a", "Terminal", &d]);
        return;
    }
    let mut candidates: Vec<String> = std::env::var("TERMINAL").into_iter().collect();
    candidates.extend(
        [
            "x-terminal-emulator",
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "kitty",
            "alacritty",
            "wezterm",
            "xterm",
        ]
        .map(String::from),
    );
    for term in candidates {
        if find_in_path(&term).is_some()
            && let Ok(mut child) = Command::new(&term)
                .current_dir(dir)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return;
        }
    }
}

/// "Abrir con…" no tiene un diálogo común fuera de Windows: se muestra en su carpeta.
pub fn open_with(path: &Path) {
    reveal(path);
}

/// Copiar desde la terminal, en un proceso que termina enseguida. En Linux lo
/// copiado muere con quien lo copió: se le pide a la instancia abierta o a
/// `wl-copy`/`xclip`/`xsel`. En macOS el portapapeles lo guarda el sistema.
pub fn copy_from_cli(text: &str) -> bool {
    if MACOS {
        return copy_to_clipboard(text);
    }
    if ipc_send(Ipc::Copy(text.to_string())) {
        return true;
    }
    let tools: &[(&str, &[&str])] = &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"]), ("xsel", &["-ib"])];
    tools.iter().any(|(tool, args)| {
        Command::new(tool)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
            .and_then(|mut child| {
                child.stdin.take()?.write_all(text.as_bytes()).ok()?;
                child.wait().ok()
            })
            .is_some_and(|st| st.success())
    })
}

pub fn read_clipboard() -> Option<String> {
    clipboard(|c| c.get_text().ok())
}

/// Avisa con el texto cuando cambia el portapapeles.
///
/// macOS no avisa de cambios, pero lleva un contador (`changeCount`) que cuesta
/// casi nada consultar: sólo se lee el texto cuando sube, y se descarta lo que los
/// gestores de contraseñas marcan como oculto o temporal (nspasteboard.org).
#[cfg(target_os = "macos")]
pub fn watch_clipboard(on_text: impl Fn(String) + Send + Sync + 'static) {
    use objc2_app_kit::NSPasteboard;
    const PRIVATE: &[&str] = &["org.nspasteboard.ConcealedType", "org.nspasteboard.TransientType"];
    std::thread::Builder::new()
        .name("kero-clipboard".into())
        .spawn(move || {
            let count = || objc2::rc::autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount());
            let mut last = count();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let now = count();
                if now == last {
                    continue;
                }
                last = now;
                let private = objc2::rc::autoreleasepool(|_| {
                    NSPasteboard::generalPasteboard()
                        .types()
                        .is_some_and(|types| types.iter().any(|t| PRIVATE.contains(&t.to_string().as_str())))
                });
                if !private && let Some(text) = objc2::rc::autoreleasepool(|_| read_clipboard()) {
                    on_text(text);
                }
            }
        })
        .expect("clipboard thread");
}

/// Linux no tiene un aviso común a X11 y Wayland: se compara una vez por segundo
/// (sólo una huella del texto, para no guardar una copia de algo grande).
#[cfg(not(target_os = "macos"))]
pub fn watch_clipboard(on_text: impl Fn(String) + Send + Sync + 'static) {
    use std::hash::{Hash, Hasher};
    let fingerprint = |s: &str| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    };
    std::thread::Builder::new()
        .name("kero-clipboard".into())
        .spawn(move || {
            let mut last = read_clipboard().map(|t| fingerprint(&t));
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let Some(text) = read_clipboard() else { continue };
                let fp = fingerprint(&text);
                if last != Some(fp) {
                    last = Some(fp);
                    on_text(text);
                }
            }
        })
        .expect("clipboard thread");
}

/// Proyecta un archivo en memoria, de sólo lectura y para toda la vida del proceso.
/// Las páginas son del archivo: el sistema las comparte y las descarta cuando quiere,
/// así que no cuentan como memoria propia (a diferencia de leerlo a un `Vec`).
pub fn map_file(path: &Path) -> Option<&'static [u8]> {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn mmap(
            addr: *mut core::ffi::c_void,
            len: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            off: i64,
        ) -> *mut core::ffi::c_void;
    }
    const PROT_READ: i32 = 1;
    const MAP_PRIVATE: i32 = 2;
    let file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len() as usize;
    if len == 0 {
        return None;
    }
    let ptr = unsafe { mmap(std::ptr::null_mut(), len, PROT_READ, MAP_PRIVATE, file.as_raw_fd(), 0) };
    // MAP_FAILED = -1. La proyección sigue válida aunque se cierre el archivo.
    if ptr as isize == -1 {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, len) })
}

/// GET por HTTPS con `curl` (viene con macOS y casi todas las distribuciones).
pub fn http_get(url: &str) -> Option<Vec<u8>> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "15", url])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// En macOS y Linux los programas ya escriben en la terminal que los lanzó.
pub fn attach_console() {}

/// Moneda de la región del usuario (`LANG=es_MX.UTF-8`, o el locale de macOS).
pub fn local_currency() -> String {
    let locale = ["LC_ALL", "LC_MONETARY", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| v.contains('_'))
        .or_else(|| if MACOS { output("defaults", &["read", "-g", "AppleLocale"]) } else { None })
        .unwrap_or_default();
    // "es_MX.UTF-8", "en_US@rg=mxzzzz" (región elegida aparte en macOS).
    let region = match locale.split_once("@rg=") {
        Some((_, rg)) => rg.get(..2).unwrap_or("").to_string(),
        None => locale.split(['.', '@']).next().unwrap_or("").rsplit('_').next().unwrap_or("").to_string(),
    };
    super::currency_for_region(&region).to_string()
}

pub fn system_uses_light_theme() -> bool {
    if MACOS {
        // Sin la clave AppleInterfaceStyle el sistema está en modo claro.
        return output("defaults", &["read", "-g", "AppleInterfaceStyle"]).is_none_or(|s| !s.contains("Dark"));
    }
    if let Some(s) = output("gsettings", &["get", "org.gnome.desktop.interface", "color-scheme"])
        && s.contains("dark")
    {
        return false;
    }
    !output("gsettings", &["get", "org.gnome.desktop.interface", "gtk-theme"])
        .is_some_and(|s| s.to_lowercase().contains("dark"))
}

pub fn accent_color() -> Option<[u8; 3]> {
    None
}

pub fn set_autostart(enable: bool) {
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.to_string_lossy();
    if MACOS {
        let path = home().join("Library/LaunchAgents/com.acysolutions.kero.plist");
        if enable {
            let plist = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.acysolutions.kero</string>
  <key>ProgramArguments</key><array><string>{exe}</string><string>--hidden</string></array>
  <key>RunAtLoad</key><true/>
</dict></plist>
"#
            );
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
            let _ = std::fs::write(path, plist);
        } else {
            let _ = std::fs::remove_file(path);
        }
        return;
    }
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
    let path = base.join("autostart/kero.desktop");
    if enable {
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName=Kero\nExec=\"{exe}\" --hidden\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n"
        );
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
        let _ = std::fs::write(path, entry);
    } else {
        let _ = std::fs::remove_file(path);
    }
}

// --------------------------------------------------------------------- IPC

fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(crate::config::data_dir);
    dir.join("kero.sock")
}

static LISTENER: OnceLock<Mutex<Option<UnixListener>>> = OnceLock::new();

/// Devuelve `false` si ya hay otra instancia corriendo.
pub fn acquire_single_instance() -> bool {
    let path = socket_path();
    if UnixStream::connect(&path).is_ok() {
        return false;
    }
    // Socket huérfano de una ejecución anterior.
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).ok();
    let _ = LISTENER.set(Mutex::new(listener));
    true
}

pub fn ipc_listen(handler: impl Fn(Ipc) + Send + 'static) {
    let Some(listener) = LISTENER.get().and_then(|l| l.lock().take()) else { return };
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut text = String::new();
            if stream.read_to_string(&mut text).is_ok()
                && let Some(msg) = Ipc::decode(&text)
            {
                handler(msg);
            }
        }
    });
}

pub fn ipc_send(msg: Ipc) -> bool {
    let Ok(mut s) = UnixStream::connect(socket_path()) else { return false };
    s.write_all(msg.encode().as_bytes()).is_ok() && s.shutdown(std::net::Shutdown::Write).is_ok()
}

pub fn wait_until_closed() {
    for _ in 0..30 {
        if UnixStream::connect(socket_path()).is_err() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

// ---------------------------------------------------------------------------

/// Selector de carpeta nativo. Bloquea: llamar desde un hilo propio.
pub fn pick_folder() -> Option<PathBuf> {
    const TITLE: &str = "Kero · Elige una carpeta para indexar";
    let picked = if MACOS {
        output("osascript", &["-e", &format!("POSIX path of (choose folder with prompt \"{TITLE}\")")])
    } else {
        output("zenity", &["--file-selection", "--directory", &format!("--title={TITLE}")])
            .or_else(|| output("kdialog", &["--getexistingdirectory", &home().to_string_lossy(), "--title", TITLE]))
    };
    picked.filter(|s| !s.is_empty()).map(PathBuf::from)
}

/// Ajusta el asignador de memoria antes de arrancar la interfaz.
///
/// macOS: por defecto guarda la memoria liberada para reutilizarla rápido, y tras
/// usar Kero quedaban ~15 MB libres pero retenidos. Con `MallocSpaceEfficient` se
/// queda en la mitad (medido: 30 → 14 MB). Sólo se lee al iniciar el proceso, así
/// que Kero se vuelve a lanzar a sí mismo con la variable (mismo PID, ~ms).
///
/// Linux (glibc): una arena por hilo multiplica la memoria retenida; con 2 basta.
pub fn tune_allocator() {
    #[cfg(target_os = "macos")]
    if std::env::var_os("MallocSpaceEfficient").is_none()
        && let Ok(exe) = std::env::current_exe()
    {
        use std::os::unix::process::CommandExt;
        // `exec` sólo vuelve si falla: entonces se sigue sin el ajuste.
        let _ = Command::new(exe).args(std::env::args_os().skip(1)).env("MallocSpaceEfficient", "1").exec();
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn mallopt(param: i32, value: i32) -> i32;
        }
        const M_ARENA_MAX: i32 = -8;
        unsafe { mallopt(M_ARENA_MAX, 2) };
    }
}

/// Oculto un rato: devuelve al sistema la memoria que el asignador tiene libre pero
/// retenida (lo que en Windows hace `EmptyWorkingSet`).
pub fn trim_working_set() {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn malloc_zone_pressure_relief(zone: *mut core::ffi::c_void, goal: usize) -> usize;
        }
        // Zona nula = todas; meta 0 = todo lo posible.
        unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        unsafe { malloc_trim(0) };
    }
}

/// Fuentes del sistema: (texto, seminegrita, iconos). Los iconos usan los
/// emoji monocromos que incluye egui.
pub fn font_candidates() -> [Vec<PathBuf>; 3] {
    if MACOS {
        return [
            vec!["/System/Library/Fonts/SFNS.ttf".into(), "/System/Library/Fonts/Supplemental/Arial.ttf".into()],
            vec![],
            vec![],
        ];
    }
    let dirs = [
        "/usr/share/fonts/truetype/noto",
        "/usr/share/fonts/noto",
        "/usr/share/fonts/google-noto",
        "/usr/share/fonts/google-noto-vf",
        "/usr/share/fonts/truetype/dejavu",
        "/usr/share/fonts/TTF",
        "/usr/share/fonts/dejavu-sans-fonts",
    ];
    let each = |files: &[&str]| -> Vec<PathBuf> {
        files.iter().flat_map(|f| dirs.iter().map(move |d| Path::new(d).join(f))).collect()
    };
    [each(&["NotoSans-Regular.ttf", "DejaVuSans.ttf"]), each(&["NotoSans-SemiBold.ttf", "DejaVuSans-Bold.ttf"]), vec![]]
}
