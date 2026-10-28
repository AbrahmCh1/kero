//! Iconos reales de apps y archivos. En Windows salen del Shell; en macOS y
//! Linux todavía no: la UI usa glifos e iniciales en su lugar.

#[cfg(windows)]
mod shell;
#[cfg(windows)]
pub use shell::Icons;

#[cfg(not(windows))]
#[derive(Clone)]
pub struct Icons;

#[cfg(not(windows))]
impl Icons {
    pub fn new(_ctx: egui::Context, _size_px: u32) -> Self {
        Icons
    }

    pub fn get(&self, _icon: &crate::plugins::Icon) -> Option<egui::TextureHandle> {
        None
    }
}
