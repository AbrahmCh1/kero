//! Implementación Windows de la capa de plataforma (Win32).

use super::Ipc;
use crate::plugins::SystemCmd;

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{COLORREF, CloseHandle, HANDLE, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, HBITMAP, HDC, HGDIOBJ,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, ReleaseDC, SelectObject,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_DWORD, RegCloseKey, RegDeleteValueW, RegGetValueW,
    RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, GetCurrentProcess, INFINITE, OpenEventW, OpenMutexW,
    SYNCHRONIZATION_SYNCHRONIZE, SetEvent, WaitForMultipleObjects,
};
use windows::Win32::UI::Shell::{FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetCursorPos, GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST, SW_HIDE, SW_SHOWNORMAL, SWP_NOACTIVATE,
    SWP_NOSIZE, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW,
};
use windows::core::{HSTRING, PCWSTR, w};

#[derive(Clone, Copy)]
pub struct Hwnd(pub isize);

impl Hwnd {
    fn h(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

/// Coloca la ventana centrada horizontalmente en el monitor del cursor, a ~18% de altura.
pub fn position_on_cursor_monitor(hwnd: Hwnd) {
    unsafe {
        let h = hwnd.h();
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(mon, &mut mi);
        let work = mi.rcWork;
        // Primer movimiento: dispara el cambio de DPI si cambiamos de monitor.
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), work.left + 10, work.top + 10, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
        let mut r = RECT::default();
        let _ = GetWindowRect(h, &mut r);
        let w = r.right - r.left;
        let x = work.left + ((work.right - work.left) - w) / 2;
        let y = work.top + ((work.bottom - work.top) as f32 * 0.18) as i32;
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

pub fn show_and_focus(hwnd: Hwnd) {
    unsafe {
        let _ = ShowWindow(hwnd.h(), SW_SHOWNORMAL);
    }
    focus(hwnd);
}

/// Convierte la ventana en "layered": alfa por píxel real (esquinas y sombra),
/// los píxeles transparentes dejan pasar los clics y no aparece en Alt+Tab.
pub fn make_layered(hwnd: Hwnd) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd.h(), GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd.h(), GWL_EXSTYLE, ex | (WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0) as isize);
    }
}

/// DIB de 32 bits persistente en el que pinta el rasterizador.
pub struct LayeredSurface {
    pub w: usize,
    pub h: usize,
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
}

impl LayeredSurface {
    pub fn new(w: usize, h: usize) -> Self {
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            ReleaseDC(None, screen);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w as i32,
                    biHeight: -(h as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).expect("DIB section");
            let old = SelectObject(dc, HGDIOBJ(bmp.0));
            Self { w, h, dc, bmp, old, bits: bits as *mut u32 }
        }
    }

    pub fn pixels(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, self.w * self.h) }
    }

    pub fn present(&self, hwnd: Hwnd) {
        unsafe {
            let size = SIZE { cx: self.w as i32, cy: self.h as i32 };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                hwnd.h(),
                None,
                None,
                Some(&size),
                Some(self.dc),
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
        }
    }
}

impl Drop for LayeredSurface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(HGDIOBJ(self.bmp.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

pub fn focus(hwnd: Hwnd) {
    unsafe {
        // Al pulsar el atajo global Windows nos concede el primer plano;
        // no hace falta (ni conviene) simular teclas.
        let _ = SetForegroundWindow(hwnd.h());
    }
}

pub fn hide(hwnd: Hwnd) {
    unsafe {
        let _ = ShowWindow(hwnd.h(), SW_HIDE);
    }
}

/// Tamaño de icono pequeño (bandeja) según el DPI del sistema.
pub fn small_icon_size() -> (u32, u32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON, SM_CYSMICON};
    unsafe { (GetSystemMetrics(SM_CXSMICON) as u32, GetSystemMetrics(SM_CYSMICON) as u32) }
}

pub fn shell_execute(verb: Option<&str>, file: &str, args: &str, dir: Option<&Path>) -> bool {
    let verb = verb.map(HSTRING::from);
    let file = HSTRING::from(file);
    let args = HSTRING::from(args);
    let dir = dir.map(|d| HSTRING::from(d.as_os_str()));
    unsafe {
        let r = ShellExecuteW(
            None,
            verb.as_ref().map(|v| PCWSTR(v.as_ptr())).unwrap_or(PCWSTR::null()),
            &file,
            if args.is_empty() { PCWSTR::null() } else { PCWSTR(args.as_ptr()) },
            dir.as_ref().map(|d| PCWSTR(d.as_ptr())).unwrap_or(PCWSTR::null()),
            SW_SHOWNORMAL,
        );
        r.0 as isize > 32
    }
}

pub fn open(target: &str) -> bool {
    shell_execute(None, target, "", None)
}

pub fn reveal(path: &Path) {
    let arg = format!("/select,\"{}\"", path.display());
    shell_execute(None, "explorer.exe", &arg, None);
}

pub fn copy_to_clipboard(text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let ok = (|| {
            let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).ok()?;
            let ptr = GlobalLock(mem) as *mut u16;
            if ptr.is_null() {
                return None;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            let _ = GlobalUnlock(mem);
            SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(mem.0))).ok()
        })()
        .is_some();
        let _ = CloseClipboard();
        ok
    }
}

const PERSONALIZE: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

fn reg_dword(subkey: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = 4u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    Some(data)
}

pub fn system_uses_light_theme() -> bool {
    reg_dword(PERSONALIZE, w!("AppsUseLightTheme")).unwrap_or(0) == 1
}

/// Color de acento de Windows como [r, g, b].
pub fn accent_color() -> Option<[u8; 3]> {
    // AccentColor está en formato ABGR.
    let v = reg_dword(w!("Software\\Microsoft\\Windows\\DWM"), w!("AccentColor"))?;
    Some([(v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, ((v >> 16) & 0xFF) as u8])
}

pub fn set_autostart(enable: bool) {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, KEY_SET_VALUE, &mut key).is_err() {
            return;
        }
        if enable {
            // La copia instalada (scripts\install.ps1) tiene prioridad sobre la que
            // se esté ejecutando, p.ej. target\release.
            let installed = std::env::var_os("LOCALAPPDATA")
                .map(|d| PathBuf::from(d).join("Programs\\Kero\\kero.exe"))
                .filter(|p| p.is_file());
            let exe = installed.or_else(|| std::env::current_exe().ok());
            if let Some(exe) = exe {
                let cmd = format!("\"{}\" --hidden", exe.display());
                let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
                let _ = RegSetValueExW(key, w!("Kero"), None, REG_SZ, Some(bytes));
            }
        } else {
            let _ = RegDeleteValueW(key, w!("Kero"));
        }
        let _ = RegCloseKey(key);
    }
}

/// Devuelve `false` si ya hay otra instancia corriendo.
pub fn acquire_single_instance() -> bool {
    unsafe {
        match CreateMutexW(None, true, INSTANCE_MUTEX) {
            Ok(_h) => {
                let already =
                    windows::Win32::Foundation::GetLastError() == windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
                // HANDLE no se cierra al salir de ámbito: el mutex vive lo que el proceso.
                !already
            }
            Err(_) => true,
        }
    }
}

const INSTANCE_MUTEX: PCWSTR = w!("Local\\KeroLauncher.SingleInstance");
const QUIT_EVENT: PCWSTR = w!("Local\\KeroLauncher.Quit");
const TOGGLE_EVENT: PCWSTR = w!("Local\\KeroLauncher.Toggle");
/// Mensajes con datos: el contenido va en `ipc.msg` y este evento avisa.
const MESSAGE_EVENT: PCWSTR = w!("Local\\KeroLauncher.Message");

fn mailbox() -> PathBuf {
    crate::config::data_dir().join("ipc.msg")
}

/// Escucha órdenes de otros procesos de Kero (`--quit`, `toggle`, `kero "texto"`, …).
pub fn ipc_listen(handler: impl Fn(Ipc) + Send + 'static) {
    let events = [QUIT_EVENT, TOGGLE_EVENT, MESSAGE_EVENT].map(|n| unsafe { CreateEventW(None, false, false, n) });
    let [Ok(q), Ok(t), Ok(m)] = events else { return };
    let raw = [q.0 as isize, t.0 as isize, m.0 as isize];
    std::thread::spawn(move || {
        let handles = raw.map(|h| HANDLE(h as *mut _));
        loop {
            let r = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
            match r.0 {
                0 => handler(Ipc::Quit),
                1 => handler(Ipc::Toggle),
                2 => {
                    let text = std::fs::read_to_string(mailbox()).unwrap_or_default();
                    let _ = std::fs::remove_file(mailbox());
                    if let Some(msg) = Ipc::decode(&text) {
                        handler(msg);
                    }
                }
                _ => return,
            }
        }
    });
}

/// Envía una orden a la instancia abierta. `false` si no hay ninguna escuchando.
pub fn ipc_send(msg: Ipc) -> bool {
    let name = match msg {
        Ipc::Quit => QUIT_EVENT,
        Ipc::Toggle => TOGGLE_EVENT,
        _ => MESSAGE_EVENT,
    };
    unsafe {
        let Ok(ev) = OpenEventW(EVENT_MODIFY_STATE, false, name) else { return false };
        let ok = !matches!(msg, Ipc::Show(_) | Ipc::Reindex | Ipc::Copy(_))
            || std::fs::write(mailbox(), msg.encode()).is_ok();
        if ok {
            let _ = SetEvent(ev);
        }
        let _ = CloseHandle(ev);
        ok
    }
}

/// Espera (máx. 3 s) a que la instancia abierta termine.
pub fn wait_until_closed() {
    for _ in 0..30 {
        match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, INSTANCE_MUTEX) } {
            Ok(m) => {
                let _ = unsafe { CloseHandle(m) };
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(_) => return,
        }
    }
}

/// Selector de carpeta nativo. Bloquea: llamar desde un hilo propio.
pub fn pick_folder() -> Option<PathBuf> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dlg: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let opts = dlg.GetOptions().ok()?;
        dlg.SetOptions(opts | FOS_PICKFOLDERS).ok()?;
        let _ = dlg.SetTitle(w!("Kero · Elige una carpeta para indexar"));
        dlg.Show(None).ok()?;
        let item = dlg.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s.map(PathBuf::from)
    }
}

/// Devuelve al sistema las páginas que no usamos mientras estamos ocultos.
pub fn trim_working_set() {
    unsafe {
        let _ = windows::Win32::System::ProcessStatus::EmptyWorkingSet(GetCurrentProcess());
    }
}

/// Ejecuta un programa; `admin` lo eleva con UAC.
pub fn run(exe: &str, args: &str, admin: bool) {
    shell_execute(admin.then_some("runas"), exe, args, None);
}

pub fn launch_app(id: &str, path: Option<&str>, admin: bool) {
    match (admin, path) {
        (true, Some(p)) => run(p, "", true),
        _ => {
            open(&format!("shell:AppsFolder\\{id}"));
        }
    }
}

pub fn run_in_terminal(cmd: &str, admin: bool) {
    let home = std::env::var("USERPROFILE").ok().map(PathBuf::from);
    shell_execute(admin.then_some("runas"), "cmd.exe", &format!("/k {cmd}"), home.as_deref());
}

pub fn run_system(cmd: SystemCmd) {
    match cmd {
        SystemCmd::Lock => unsafe {
            let _ = windows::Win32::System::Shutdown::LockWorkStation();
        },
        SystemCmd::Sleep | SystemCmd::Hibernate => unsafe {
            let _ = windows::Win32::System::Power::SetSuspendState(cmd == SystemCmd::Hibernate, false, false);
        },
        SystemCmd::Shutdown => run("shutdown.exe", "/s /t 0", false),
        SystemCmd::Restart => run("shutdown.exe", "/r /t 0", false),
        SystemCmd::SignOut => run("shutdown.exe", "/l", false),
        // Sin SHERB_NOCONFIRMATION: Windows pide confirmación.
        SystemCmd::EmptyTrash => unsafe {
            let _ = windows::Win32::UI::Shell::SHEmptyRecycleBinW(None, PCWSTR::null(), 0);
        },
    }
}

/// Fuentes del sistema: (texto, seminegrita, iconos). Rutas absolutas, en orden de preferencia.
pub fn font_candidates() -> [Vec<PathBuf>; 3] {
    let dir = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    [
        vec![dir.join("segoeui.ttf")],
        vec![dir.join("seguisb.ttf")],
        vec![dir.join("SegoeIcons.ttf"), dir.join("segmdl2.ttf")],
    ]
}

// ---------------------------------------------------------------------------
// Red, portapapeles y consola.

/// GET por HTTPS con WinHTTP (usa el proxy del sistema). `None` si falla o no responde 200.
pub fn http_get(url: &str) -> Option<Vec<u8>> {
    use windows::Win32::Networking::WinHttp::*;
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((rest, "/".into()));
    let close = |h: *mut core::ffi::c_void| unsafe {
        if !h.is_null() {
            let _ = WinHttpCloseHandle(h);
        }
    };
    unsafe {
        let session = WinHttpOpen(w!("Kero"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0);
        if session.is_null() {
            return None;
        }
        let _ = WinHttpSetTimeouts(session, 5000, 5000, 5000, 10000);
        let connect = WinHttpConnect(session, &HSTRING::from(host), INTERNET_DEFAULT_HTTPS_PORT, 0);
        let request = if connect.is_null() {
            std::ptr::null_mut()
        } else {
            WinHttpOpenRequest(
                connect,
                w!("GET"),
                &HSTRING::from(path),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            )
        };
        let body = (|| {
            if request.is_null() {
                return None;
            }
            WinHttpSendRequest(request, None, None, 0, 0, 0).ok()?;
            WinHttpReceiveResponse(request, std::ptr::null_mut()).ok()?;
            let mut status = 0u32;
            let mut len = 4u32;
            WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut status as *mut u32 as *mut _),
                &mut len,
                std::ptr::null_mut(),
            )
            .ok()?;
            if status != 200 {
                return None;
            }
            let mut out = Vec::new();
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                let mut read = 0u32;
                WinHttpReadData(request, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut read).ok()?;
                if read == 0 {
                    break;
                }
                out.extend_from_slice(&buf[..read as usize]);
            }
            Some(out)
        })();
        close(request);
        close(connect);
        close(session);
        body
    }
}

/// Proyecta un archivo en memoria, de sólo lectura y para toda la vida del proceso.
/// Las páginas son del archivo (compartidas, descartables): no cuentan como memoria
/// privada, a diferencia de leerlo a un `Vec`.
pub fn map_file(path: &Path) -> Option<&'static [u8]> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::System::Memory::{CreateFileMappingW, FILE_MAP_READ, MapViewOfFile, PAGE_READONLY};
    let file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len() as usize;
    if len == 0 {
        return None;
    }
    unsafe {
        let mapping =
            CreateFileMappingW(HANDLE(file.as_raw_handle()), None, PAGE_READONLY, 0, 0, PCWSTR::null()).ok()?;
        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        // La vista mantiene viva la proyección; el handle ya no hace falta.
        let _ = CloseHandle(mapping);
        if view.Value.is_null() {
            return None;
        }
        Some(std::slice::from_raw_parts(view.Value as *const u8, len))
    }
}

/// Desde la terminal: en Windows lo copiado lo guarda el sistema.
pub fn copy_from_cli(text: &str) -> bool {
    copy_to_clipboard(text)
}

/// Abre el portapapeles reintentando: otra app puede tenerlo abierto un instante.
fn open_clipboard() -> bool {
    for _ in 0..10 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    false
}

pub fn read_clipboard() -> Option<String> {
    use windows::Win32::System::DataExchange::GetClipboardData;
    if !open_clipboard() {
        return None;
    }
    let text = unsafe {
        (|| {
            let h = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
            let mem = windows::Win32::Foundation::HGLOBAL(h.0);
            let ptr = GlobalLock(mem) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(mem);
            Some(s)
        })()
    };
    let _ = unsafe { CloseClipboard() };
    text
}

/// Los gestores de contraseñas marcan lo que copian para que no quede en historiales.
fn clipboard_is_private() -> bool {
    use windows::Win32::System::DataExchange::{
        GetClipboardData, IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };
    unsafe {
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        let ignore = RegisterClipboardFormatW(w!("Clipboard Viewer Ignore"));
        if IsClipboardFormatAvailable(exclude).is_ok() || IsClipboardFormatAvailable(ignore).is_ok() {
            return true;
        }
        let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        if IsClipboardFormatAvailable(history).is_err() || !open_clipboard() {
            return false;
        }
        let allowed = (|| {
            let h = GetClipboardData(history).ok()?;
            let mem = windows::Win32::Foundation::HGLOBAL(h.0);
            let ptr = GlobalLock(mem) as *const u32;
            if ptr.is_null() {
                return None;
            }
            let v = *ptr;
            let _ = GlobalUnlock(mem);
            Some(v != 0)
        })();
        let _ = CloseClipboard();
        allowed == Some(false)
    }
}

type ClipboardCallback = Box<dyn Fn() + Send + Sync>;
static CLIPBOARD_CHANGED: std::sync::OnceLock<ClipboardCallback> = std::sync::OnceLock::new();

unsafe extern "system" fn clipboard_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcW, WM_CLIPBOARDUPDATE};
    if msg == WM_CLIPBOARDUPDATE {
        if let Some(cb) = CLIPBOARD_CHANGED.get() {
            cb();
        }
        return windows::Win32::Foundation::LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Avisa con el texto cada vez que cambia el portapapeles. Usa una ventana de
/// mensajes con `AddClipboardFormatListener`: no consume CPU mientras no copias.
pub fn watch_clipboard(on_text: impl Fn(String) + Send + Sync + 'static) {
    use windows::Win32::System::DataExchange::AddClipboardFormatListener;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG, RegisterClassW, WINDOW_EX_STYLE,
        WINDOW_STYLE, WNDCLASSW,
    };
    let cb = move || {
        if clipboard_is_private() {
            return;
        }
        if let Some(text) = read_clipboard() {
            on_text(text);
        }
    };
    if CLIPBOARD_CHANGED.set(Box::new(cb)).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("kero-clipboard".into())
        .spawn(|| unsafe {
            let Ok(module) = GetModuleHandleW(None) else { return };
            let class = w!("KeroClipboardListener");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(clipboard_wndproc),
                hInstance: module.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            let Ok(hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(module.into()),
                None,
            ) else {
                return;
            };
            if AddClipboardFormatListener(hwnd).is_err() {
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
        })
        .expect("clipboard thread");
}

/// Conecta la salida a la consola de quien lanzó Kero (el ejecutable es de ventana).
pub fn attach_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

/// Moneda de la configuración regional de Windows ("MXN", "EUR", …).
pub fn local_currency() -> String {
    use windows::Win32::Globalization::{GetLocaleInfoEx, LOCALE_SINTLSYMBOL};
    let mut buf = [0u16; 16];
    let n = unsafe { GetLocaleInfoEx(PCWSTR::null(), LOCALE_SINTLSYMBOL, Some(&mut buf)) };
    let code = String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize]);
    if code.len() == 3 { code } else { "USD".into() }
}
