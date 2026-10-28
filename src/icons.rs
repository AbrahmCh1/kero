//! Carga perezosa de iconos del Shell en un hilo COM propio.
//! Sólo se piden los iconos de las filas visibles, y se cachean por clave
//! (por extensión para archivos comunes, por ruta para .exe/.lnk/.ico).

use crate::plugins::Icon;
use crossbeam_channel::Sender;
use egui::{ColorImage, TextureHandle, TextureOptions};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, GetObjectW,
    HBITMAP, HGDIOBJ, ReleaseDC,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
use windows::core::{HSTRING, Interface};

enum Slot {
    Pending,
    Ready(TextureHandle),
    Failed,
}

#[derive(Clone)]
pub struct Icons {
    map: Arc<Mutex<HashMap<String, Slot>>>,
    tx: Sender<(String, String)>,
}

/// Extensiones cuyo icono depende del archivo concreto.
const PER_FILE: [&str; 6] = ["exe", "lnk", "ico", "url", "appref-ms", "msc"];

pub fn key_and_target(icon: &Icon) -> Option<(String, String)> {
    match icon {
        Icon::App(id) => Some((format!("app:{id}"), format!("shell:AppsFolder\\{id}"))),
        Icon::Path(p, is_dir) => {
            let target = p.to_string_lossy().into_owned();
            if *is_dir {
                // Unidades (C:\) tienen icono propio; el resto de carpetas comparten.
                if target.len() <= 3 {
                    return Some((format!("drive:{target}"), target));
                }
                return Some(("folder".into(), target));
            }
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext.is_empty() || PER_FILE.contains(&ext.as_str()) {
                Some((format!("path:{}", target.to_lowercase()), target))
            } else {
                Some((format!("ext:{ext}"), target))
            }
        }
        _ => None,
    }
}

impl Icons {
    pub fn new(ctx: egui::Context, size_px: u32) -> Self {
        let map: Arc<Mutex<HashMap<String, Slot>>> = Arc::default();
        let (tx, rx) = crossbeam_channel::unbounded::<(String, String)>();
        let m = map.clone();
        std::thread::Builder::new()
            .name("kero-icons".into())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                }
                while let Ok((key, target)) = rx.recv() {
                    let slot = match load_icon(&target, size_px) {
                        Some(img) => Slot::Ready(ctx.load_texture(&key, img, TextureOptions::LINEAR)),
                        None => Slot::Failed,
                    };
                    m.lock().insert(key, slot);
                    ctx.request_repaint();
                }
            })
            .expect("icon thread");
        Self { map, tx }
    }

    pub fn get(&self, icon: &Icon) -> Option<TextureHandle> {
        let (key, target) = key_and_target(icon)?;
        let mut map = self.map.lock();
        match map.get(&key) {
            Some(Slot::Ready(t)) => Some(t.clone()),
            Some(Slot::Pending | Slot::Failed) => None,
            None => {
                map.insert(key.clone(), Slot::Pending);
                let _ = self.tx.send((key, target));
                None
            }
        }
    }
}

fn load_icon(target: &str, size: u32) -> Option<ColorImage> {
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(target), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let sz = SIZE { cx: size as i32, cy: size as i32 };
        let hbm = factory.GetImage(sz, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK).ok()?;
        let img = bitmap_to_image(hbm);
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        img
    }
}

unsafe fn bitmap_to_image(hbm: HBITMAP) -> Option<ColorImage> {
    unsafe {
        let mut bmp = BITMAP::default();
        if GetObjectW(HGDIOBJ(hbm.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bmp as *mut _ as *mut _)) == 0 {
            return None;
        }
        let (w, h) = (bmp.bmWidth, bmp.bmHeight.abs());
        if w <= 0 || h <= 0 {
            return None;
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let hdc = GetDC(None);
        let lines = GetDIBits(hdc, hbm, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
        ReleaseDC(None, hdc);
        if lines == 0 {
            return None;
        }
        let has_alpha = buf.chunks_exact(4).any(|p| p[3] != 0);
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2); // BGRA → RGBA
            if !has_alpha {
                px[3] = 255;
            }
        }
        Some(ColorImage::from_rgba_premultiplied([w as usize, h as usize], &buf))
    }
}
