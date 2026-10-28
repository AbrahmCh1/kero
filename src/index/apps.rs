//! Enumera todas las apps (Win32 + Microsoft Store) desde `shell:AppsFolder`,
//! la misma fuente que usa el menú Inicio.

use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, SHGetKnownFolderItem,
    SHGetKnownFolderPath, KF_FLAG_DEFAULT, SIGDN, SIGDN_NORMALDISPLAY,
    SIGDN_PARENTRELATIVEPARSING,
};
use windows::core::GUID;

#[derive(Clone, Debug)]
pub struct App {
    pub name: String,
    /// Nombre de parseo dentro de AppsFolder (AUMID o ruta con known-folder).
    pub id: String,
    /// Ruta del ejecutable si es una app de escritorio.
    pub path: Option<String>,
}

const NOISE_EXT: [&str; 6] = [".chm", ".txt", ".pdf", ".html", ".htm", ".rtf"];

pub fn enumerate() -> Vec<App> {
    let mut out = Vec::with_capacity(256);
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if let Ok(folder) =
            SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
        {
            if let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) {
                loop {
                    let mut slot = [None];
                    let mut fetched = 0u32;
                    if items.Next(&mut slot, Some(&mut fetched)).is_err() || fetched == 0 {
                        break;
                    }
                    let Some(item) = slot[0].take() else { break };
                    let (Some(name), Some(id)) = (
                        display_name(&item, SIGDN_NORMALDISPLAY),
                        display_name(&item, SIGDN_PARENTRELATIVEPARSING),
                    ) else {
                        continue;
                    };
                    if is_noise(&name, &id) {
                        continue;
                    }
                    let path = resolve_path(&id);
                    out.push(App { name, id, path });
                }
            }
        }
        if com.is_ok() {
            CoUninitialize();
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.name == b.name && a.id == b.id);
    out
}

fn is_noise(name: &str, id: &str) -> bool {
    let n = name.to_lowercase();
    let i = id.to_lowercase();
    n.starts_with("uninstall")
        || n.starts_with("desinstalar")
        || n.contains("readme")
        || NOISE_EXT.iter().any(|e| i.ends_with(e))
}

unsafe fn display_name(item: &IShellItem, kind: SIGDN) -> Option<String> {
    unsafe {
        let p = item.GetDisplayName(kind).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        s.filter(|s| !s.is_empty())
    }
}

/// "{GUID}\carpeta\app.exe" → "C:\Program Files\carpeta\app.exe"
fn resolve_path(id: &str) -> Option<String> {
    if id.len() > 3 && id.as_bytes()[1] == b':' {
        return Some(id.to_string());
    }
    let rest = id.strip_prefix('{')?;
    let (guid, tail) = rest.split_once('}')?;
    let guid = GUID::try_from(guid).ok()?;
    unsafe {
        let p = SHGetKnownFolderPath(&guid, KF_FLAG_DEFAULT, None).ok()?;
        let base = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as _));
        Some(format!("{}{}", base?, tail))
    }
}

// Caché en texto plano: nombre \t id \t ruta
pub fn save(apps: &[App], path: &std::path::Path) {
    let mut s = String::with_capacity(apps.len() * 96);
    for a in apps {
        s.push_str(&a.name);
        s.push('\t');
        s.push_str(&a.id);
        s.push('\t');
        s.push_str(a.path.as_deref().unwrap_or(""));
        s.push('\n');
    }
    let _ = std::fs::write(path, s);
}

pub fn load(path: &std::path::Path) -> Vec<App> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let name = it.next()?.to_string();
            let id = it.next()?.to_string();
            let path = it.next().filter(|p| !p.is_empty()).map(String::from);
            Some(App { name, id, path })
        })
        .collect()
}
