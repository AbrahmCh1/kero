//! Enumera todas las apps (Win32 + Microsoft Store) desde `shell:AppsFolder`,
//! la misma fuente que usa el menú Inicio.

use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, KF_FLAG_DEFAULT, SHGetKnownFolderItem,
    SHGetKnownFolderPath, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
};
use windows::core::GUID;

use super::App;

const NOISE_EXT: [&str; 6] = [".chm", ".txt", ".pdf", ".html", ".htm", ".rtf"];

pub fn enumerate() -> Vec<App> {
    let mut out = Vec::with_capacity(256);
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if let Ok(folder) = SHGetKnownFolderItem::<IShellItem>(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
            && let Ok(items) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems)
        {
            loop {
                let mut slot = [None];
                let mut fetched = 0u32;
                if items.Next(&mut slot, Some(&mut fetched)).is_err() || fetched == 0 {
                    break;
                }
                let Some(item) = slot[0].take() else { break };
                let (Some(name), Some(id)) =
                    (display_name(&item, SIGDN_NORMALDISPLAY), display_name(&item, SIGDN_PARENTRELATIVEPARSING))
                else {
                    continue;
                };
                if is_noise(&name, &id) {
                    continue;
                }
                let path = resolve_path(&id);
                out.push(App { name, id, path });
            }
        }
        if com.is_ok() {
            CoUninitialize();
        }
    }
    super::finish(out)
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
