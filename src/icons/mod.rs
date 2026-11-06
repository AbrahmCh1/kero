//! Iconos reales de apps y archivos. En Windows salen del Shell, en macOS de
//! NSWorkspace; en Linux todavía no: la UI usa glifos e iniciales.

#[cfg(windows)]
mod shell;
#[cfg(windows)]
pub use shell::Icons;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Icons;

#[cfg(not(any(windows, target_os = "macos")))]
#[derive(Clone)]
pub struct Icons;

#[cfg(not(any(windows, target_os = "macos")))]
impl Icons {
    pub fn new(_ctx: egui::Context, _size_px: u32) -> Self {
        Icons
    }

    pub fn get(&self, _icon: &crate::plugins::Icon) -> Option<egui::TextureHandle> {
        None
    }
}
