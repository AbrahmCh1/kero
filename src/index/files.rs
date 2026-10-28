//! Índice compacto de archivos.
//!
//! Cada directorio se guarda una sola vez (`dirs`) y cada archivo es sólo
//! un nombre dentro de una arena de texto + el índice de su carpeta padre.
//! Así 300k archivos ocupan ~10 MB en vez de ~40 MB con rutas completas.

use parking_lot::Mutex;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const FLAG_DIR: u8 = 1;
pub const FLAG_ASCII: u8 = 2;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct FileEntry {
    pub name_off: u32,
    pub dir: u32,
    pub name_len: u16,
    pub flags: u8,
    pub _pad: u8,
}

#[derive(Default)]
pub struct FileIndex {
    pub dirs: Vec<Box<str>>,
    pub names: String,
    pub entries: Vec<FileEntry>,
}

impl FileIndex {
    #[inline]
    pub fn name(&self, e: &FileEntry) -> &str {
        let start = e.name_off as usize;
        &self.names[start..start + e.name_len as usize]
    }

    pub fn path(&self, e: &FileEntry) -> PathBuf {
        let dir = &self.dirs[e.dir as usize];
        let mut s = String::with_capacity(dir.len() + e.name_len as usize + 1);
        s.push_str(dir);
        if !s.ends_with(std::path::MAIN_SEPARATOR) {
            s.push(std::path::MAIN_SEPARATOR);
        }
        s.push_str(self.name(e));
        PathBuf::from(s)
    }

    pub fn dir_of(&self, e: &FileEntry) -> &str {
        &self.dirs[e.dir as usize]
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn memory_bytes(&self) -> usize {
        self.names.capacity()
            + self.entries.capacity() * std::mem::size_of::<FileEntry>()
            + self.dirs.iter().map(|d| d.len() + 16).sum::<usize>()
    }
}

pub struct WalkOptions {
    pub exclude: Vec<String>,
    pub exclude_ext: Vec<String>,
    pub skip_hidden: bool,
    pub max_depth: usize,
}

struct Chunk {
    dir: PathBuf,
    children: Vec<(String, bool)>,
}

struct Kind {
    is_dir: bool,
    hidden: bool,
    /// Enlace simbólico o junction: no se recorre.
    link: bool,
}

#[cfg(windows)]
fn classify(entry: &std::fs::DirEntry, _name: &str) -> Option<Kind> {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    const REPARSE_POINT: u32 = 0x400;
    // En Windows `DirEntry::metadata` sale de los datos de FindNextFile: no cuesta otra llamada.
    let meta = entry.metadata().ok()?;
    let attrs = meta.file_attributes();
    Some(Kind { is_dir: meta.is_dir(), hidden: attrs & (HIDDEN | SYSTEM) != 0, link: attrs & REPARSE_POINT != 0 })
}

#[cfg(not(windows))]
fn classify(entry: &std::fs::DirEntry, name: &str) -> Option<Kind> {
    // `file_type` usa d_type de readdir: tampoco requiere un stat por archivo.
    let ft = entry.file_type().ok()?;
    Some(Kind { is_dir: ft.is_dir(), hidden: name.starts_with('.'), link: ft.is_symlink() })
}

/// Recorre las raíces en paralelo (un trabajo de rayon por carpeta).
pub fn build(roots: &[PathBuf], opts: &WalkOptions) -> FileIndex {
    let chunks: Mutex<Vec<Chunk>> = Mutex::new(Vec::with_capacity(4096));
    let exclude: Vec<String> = opts.exclude.iter().map(|s| s.to_lowercase()).collect();
    let exclude_ext: Vec<String> = opts.exclude_ext.iter().map(|s| s.to_lowercase()).collect();

    rayon::scope(|s| {
        for root in roots {
            let chunks = &chunks;
            let exclude = &exclude;
            let exclude_ext = &exclude_ext;
            let root = root.clone();
            s.spawn(move |s| visit(s, root, 0, opts, exclude, exclude_ext, chunks));
        }
    });

    let mut chunks = chunks.into_inner();
    // Orden estable: facilita que el índice sea determinista entre ejecuciones.
    chunks.sort_unstable_by(|a, b| a.dir.cmp(&b.dir));

    let total: usize = chunks.iter().map(|c| c.children.len()).sum();
    let mut idx = FileIndex {
        dirs: Vec::with_capacity(chunks.len()),
        names: String::with_capacity(total * 16),
        entries: Vec::with_capacity(total + roots.len()),
    };

    for (dir_id, chunk) in chunks.into_iter().enumerate() {
        idx.dirs.push(chunk.dir.to_string_lossy().into());
        for (name, is_dir) in chunk.children {
            push_entry(&mut idx, &name, dir_id as u32, is_dir);
        }
    }
    // Las propias raíces (Descargas, Documentos…) también son buscables.
    for root in roots {
        if let (Some(parent), Some(name)) = (root.parent(), root.file_name()) {
            let dir_id = idx.dirs.len() as u32;
            idx.dirs.push(parent.to_string_lossy().into());
            push_entry(&mut idx, &name.to_string_lossy(), dir_id, true);
        }
    }
    idx.names.shrink_to_fit();
    idx.entries.shrink_to_fit();
    idx
}

fn push_entry(idx: &mut FileIndex, name: &str, dir: u32, is_dir: bool) {
    let len = name.len().min(u16::MAX as usize);
    let mut flags = if is_dir { FLAG_DIR } else { 0 };
    if name.is_ascii() {
        flags |= FLAG_ASCII;
    }
    idx.entries.push(FileEntry { name_off: idx.names.len() as u32, dir, name_len: len as u16, flags, _pad: 0 });
    idx.names.push_str(&name[..len]);
}

fn visit<'s>(
    s: &rayon::Scope<'s>,
    dir: PathBuf,
    depth: usize,
    opts: &'s WalkOptions,
    exclude: &'s [String],
    exclude_ext: &'s [String],
    chunks: &'s Mutex<Vec<Chunk>>,
) {
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut children = Vec::new();
    let mut subdirs = Vec::new();
    for entry in rd.flatten() {
        let Ok(name) = entry.file_name().into_string() else { continue };
        let Some(kind) = classify(&entry, &name) else { continue };
        if opts.skip_hidden && kind.hidden {
            continue;
        }
        let lower = name.to_lowercase();
        if exclude.contains(&lower) {
            continue;
        }
        let is_dir = kind.is_dir;
        if !is_dir
            && let Some(ext) = lower.rsplit_once('.').map(|(_, e)| e)
            && exclude_ext.iter().any(|e| e == ext)
        {
            continue;
        }
        // No seguimos junctions/symlinks de carpetas: evita ciclos y duplicados.
        if is_dir && !kind.link && depth < opts.max_depth {
            subdirs.push(entry.path());
        }
        children.push((name, is_dir));
    }
    chunks.lock().push(Chunk { dir, children });
    for sub in subdirs {
        s.spawn(move |s| visit(s, sub, depth + 1, opts, exclude, exclude_ext, chunks));
    }
}

// ---------------------------------------------------------------------------
// Persistencia binaria simple (sin dependencias).

const MAGIC: &[u8; 8] = b"KEROIDX2";

pub fn save(idx: &FileIndex, path: &Path) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let f = std::fs::File::create(&tmp)?;
        let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
        w.write_all(MAGIC)?;
        w.write_all(&(idx.dirs.len() as u32).to_le_bytes())?;
        for d in &idx.dirs {
            w.write_all(&(d.len() as u32).to_le_bytes())?;
            w.write_all(d.as_bytes())?;
        }
        w.write_all(&(idx.names.len() as u32).to_le_bytes())?;
        w.write_all(idx.names.as_bytes())?;
        w.write_all(&(idx.entries.len() as u32).to_le_bytes())?;
        for e in &idx.entries {
            w.write_all(&e.name_off.to_le_bytes())?;
            w.write_all(&e.dir.to_le_bytes())?;
            w.write_all(&e.name_len.to_le_bytes())?;
            w.write_all(&[e.flags])?;
        }
        w.flush()?;
    }
    std::fs::rename(tmp, path)
}

pub fn load(path: &Path) -> Option<FileIndex> {
    let mut buf = Vec::new();
    std::fs::File::open(path).ok()?.read_to_end(&mut buf).ok()?;
    let mut r = Reader { buf: &buf, pos: 0 };
    if r.take(8)? != MAGIC {
        return None;
    }
    let ndirs = r.u32()? as usize;
    let mut dirs = Vec::with_capacity(ndirs);
    for _ in 0..ndirs {
        let len = r.u32()? as usize;
        dirs.push(std::str::from_utf8(r.take(len)?).ok()?.into());
    }
    let nlen = r.u32()? as usize;
    let names = std::str::from_utf8(r.take(nlen)?).ok()?.to_string();
    let n = r.u32()? as usize;
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let name_off = r.u32()?;
        let dir = r.u32()?;
        let name_len = u16::from_le_bytes(r.take(2)?.try_into().ok()?);
        let flags = r.take(1)?[0];
        if (name_off as usize + name_len as usize) > names.len() || dir as usize >= dirs.len() {
            return None;
        }
        entries.push(FileEntry { name_off, dir, name_len, flags, _pad: 0 });
    }
    Some(FileIndex { dirs, names, entries })
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.buf.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(s)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}
