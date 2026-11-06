//! Iconos de macOS: NSWorkspace convierte la ruta de un bundle o archivo en un
//! NSImage, que se rasteriza a RGBA en un hilo propio y se cachea por clave.

use crate::plugins::Icon;
use crossbeam_channel::Sender;
use egui::{ColorImage, TextureHandle, TextureOptions};
use objc2::AnyThread;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext, NSWorkspace};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

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

fn key_and_target(icon: &Icon) -> Option<(String, String)> {
    match icon {
        Icon::App(id) => Some((format!("app:{id}"), id.clone())),
        Icon::Path(path, is_dir) => {
            let target = path.to_string_lossy().into_owned();
            if *is_dir {
                return Some(("folder".into(), target));
            }
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext.is_empty() {
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
                while let Ok((key, target)) = rx.recv() {
                    let slot = match load_icon(&target, size_px) {
                        Some(img) => Slot::Ready(ctx.load_texture(&key, img, TextureOptions::LINEAR)),
                        None => Slot::Failed,
                    };
                    m.lock().insert(key, slot);
                    crate::host::wake();
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

fn load_icon(path: &str, size: u32) -> Option<ColorImage> {
    autoreleasepool(|_| {
        let size = size.max(1);
        let workspace = NSWorkspace::sharedWorkspace();
        let image = workspace.iconForFile(&NSString::from_str(path));
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                size as isize,
                size as isize,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
        }?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        let previous = NSGraphicsContext::currentContext();
        NSGraphicsContext::setCurrentContext(Some(&context));
        image.drawInRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(size as f64, size as f64)));
        context.flushGraphics();
        NSGraphicsContext::setCurrentContext(previous.as_deref());

        let bytes_per_row = rep.bytesPerRow() as usize;
        let row_bytes = (size as usize).checked_mul(4)?;
        let data = rep.bitmapData();
        if data.is_null() || bytes_per_row < row_bytes {
            return None;
        }
        let mut pixels = Vec::with_capacity(row_bytes * size as usize);
        for y in 0..size as usize {
            let row = unsafe { std::slice::from_raw_parts(data.add(y * bytes_per_row), row_bytes) };
            pixels.extend_from_slice(row);
        }
        Some(ColorImage::from_rgba_premultiplied([size as usize, size as usize], &pixels))
    })
}
