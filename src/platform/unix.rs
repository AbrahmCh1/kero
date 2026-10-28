//! Implementación macOS y Linux de la capa de plataforma.
//! Se apoya en las herramientas estándar de cada sistema (`open`, `xdg-open`,
//! `osascript`, `systemctl`, …) en lugar de enlazar frameworks nativos.

use super::Ipc;
use crate::plugins::SystemCmd;
use parking_lot::Mutex;
use std::io::{BufRead, BufReader, Write};
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

pub fn copy_to_clipboard(text: &str) -> bool {
    // En X11 el contenido vive mientras viva el `Clipboard`: lo mantenemos.
    static CLIP: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();
    let mut guard = CLIP.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok())).lock();
    guard.as_mut().is_some_and(|c| c.set_text(text.to_string()).is_ok())
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
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            if BufReader::new(stream).read_line(&mut line).is_ok() {
                match line.trim() {
                    "quit" => handler(Ipc::Quit),
                    "toggle" => handler(Ipc::Toggle),
                    _ => {}
                }
            }
        }
    });
}

pub fn ipc_send(msg: Ipc) -> bool {
    let Ok(mut s) = UnixStream::connect(socket_path()) else { return false };
    let line = match msg {
        Ipc::Quit => "quit\n",
        Ipc::Toggle => "toggle\n",
    };
    s.write_all(line.as_bytes()).is_ok()
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

pub fn trim_working_set() {}

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
