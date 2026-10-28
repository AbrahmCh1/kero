//! Helpers finos sobre Win32.

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{CloseHandle, COLORREF, HANDLE, HWND, POINT, RECT, SIZE};
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
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_DWORD, RegCloseKey, RegDeleteValueW,
    RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, GetCurrentProcess, INFINITE, OpenEventW, OpenMutexW,
    SYNCHRONIZATION_SYNCHRONIZE, SetEvent, WaitForSingleObject,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{
    FileOpenDialog, FOS_PICKFOLDERS, IFileOpenDialog, SIGDN_FILESYSPATH, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST, SW_HIDE,
    SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_NOSIZE, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    ULW_ALPHA, UpdateLayeredWindow, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
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

pub fn is_foreground(hwnd: Hwnd) -> bool {
    unsafe { GetForegroundWindow() == hwnd.h() }
}

/// Tamaño de icono pequeño (bandeja) según el DPI del sistema.
pub fn small_icon_size() -> (u32, u32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON, SM_CYSMICON};
    unsafe { (GetSystemMetrics(SM_CXSMICON) as u32, GetSystemMetrics(SM_CYSMICON) as u32) }
}

pub fn dpi_scale(hwnd: Hwnd) -> f32 {
    unsafe { GetDpiForWindow(hwnd.h()) as f32 / 96.0 }
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

pub fn reveal_in_explorer(path: &Path) {
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
                let already = windows::Win32::Foundation::GetLastError()
                    == windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
                // HANDLE no se cierra al salir de ámbito: el mutex vive lo que el proceso.
                !already
            }
            Err(_) => true,
        }
    }
}

const QUIT_EVENT: PCWSTR = w!("Local\\KeroLauncher.Quit");
const INSTANCE_MUTEX: PCWSTR = w!("Local\\KeroLauncher.SingleInstance");

/// Hilo que espera la orden de cierre de otro proceso (instalador/desinstalador).
pub fn listen_for_quit(on_quit: impl FnOnce() + Send + 'static) {
    let Ok(ev) = (unsafe { CreateEventW(None, true, false, QUIT_EVENT) }) else { return };
    let ev = ev.0 as isize;
    std::thread::spawn(move || {
        unsafe { WaitForSingleObject(HANDLE(ev as *mut _), INFINITE) };
        on_quit();
    });
}

/// Pide a la instancia abierta que se cierre y espera (máx. 3 s) a que lo haga.
pub fn quit_running_instance() {
    unsafe {
        if let Ok(ev) = OpenEventW(EVENT_MODIFY_STATE, false, QUIT_EVENT) {
            let _ = SetEvent(ev);
            let _ = CloseHandle(ev);
        }
        for _ in 0..30 {
            match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, INSTANCE_MUTEX) {
                Ok(m) => {
                    let _ = CloseHandle(m);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(_) => return,
            }
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

pub fn lock_workstation() {
    unsafe {
        let _ = windows::Win32::System::Shutdown::LockWorkStation();
    }
}

pub fn suspend(hibernate: bool) {
    unsafe {
        let _ = windows::Win32::System::Power::SetSuspendState(hibernate, false, false);
    }
}

pub fn empty_recycle_bin() {
    unsafe {
        // Sin SHERB_NOCONFIRMATION: Windows pide confirmación.
        let _ = windows::Win32::UI::Shell::SHEmptyRecycleBinW(None, PCWSTR::null(), 0);
    }
}
