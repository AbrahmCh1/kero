//! Apps + archivos indexados. Es la ruta caliente: el recorrido de archivos
//! corre en paralelo con un `Matcher` por hilo y un top-K por bloque.

use super::{Action, Fuzzy, Icon, Item, Mode, Plugin, Query, quality};
use crate::config::display_path;
use crate::index::files::{FLAG_DIR, FileEntry, FileIndex};
use nucleo_matcher::pattern::Pattern;
use nucleo_matcher::{Matcher, Utf32Str};
use rayon::prelude::*;
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub struct Launcher;

const APP_BONUS: i32 = 90;
const TOP_K: usize = 120;
const CHUNK: usize = 8192;

thread_local! {
    static MATCHER: RefCell<(Matcher, Vec<char>)> = RefCell::new({
        let mut cfg = nucleo_matcher::Config::DEFAULT;
        cfg.prefer_prefix = true;
        (Matcher::new(cfg), Vec::new())
    });
}

impl Plugin for Launcher {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if !matches!(q.mode, Mode::All | Mode::Files) || q.text.is_empty() {
            return;
        }
        let mut fz = Fuzzy::new(q.text);
        let lower = q.text.to_lowercase();

        if q.mode == Mode::All {
            for app in &q.index.apps {
                let Some(s) = fz.score(&app.name) else { continue };
                let (bonus, strong) = quality(&app.name, &lower);
                let mut score = s as i32 + bonus + if strong { APP_BONUS } else { 10 };
                let key = format!("app:{}", app.id);
                score += q.usage.bonus(&key, &lower);
                let mut it = Item::new(
                    app.name.clone(),
                    "Aplicación",
                    Icon::App(app.id.clone()),
                    Action::App { id: app.id.clone(), path: app.path.clone() },
                    score,
                );
                it.highlight = fz.indices(&app.name);
                it.key = Some(key);
                it.weak = !strong;
                out.push(it);
            }
        }

        // Una o dos letras sobre 300k archivos sólo produce ruido.
        if q.text.chars().count() < 2 && q.mode == Mode::All {
            return;
        }

        let hits = search_files(q, &fz.pattern);
        for (score, ri, ei) in hits {
            let root = &q.index.roots[ri];
            let e = &root.files.entries[ei];
            let name = root.files.name(e);
            let path = root.files.path(e);
            let is_dir = e.flags & FLAG_DIR != 0;
            let key = format!("file:{}", path.to_string_lossy().to_lowercase());
            let mut s = score as i32;
            s += q.usage.bonus(&key, &lower);
            if is_dir {
                s += 6;
            }
            // Ligera penalización por profundidad: lo cercano a la raíz suele ser lo buscado.
            s -= (root.files.dir_of(e).matches(std::path::MAIN_SEPARATOR).count() as i32).min(12);
            let (bonus, strong) = quality(name, &lower);
            s += if strong { bonus } else { -30 };
            let mut it = Item::new(
                name,
                display_path(std::path::Path::new(root.files.dir_of(e))),
                Icon::Path(path.clone(), is_dir),
                Action::Open(path),
                s,
            );
            it.highlight = fz.indices(name);
            it.key = Some(key);
            it.weak = !strong;
            out.push(it);
        }
    }
}

/// Devuelve (score, índice de raíz, índice de entrada) de los mejores K.
fn search_files(q: &Query, pattern: &Pattern) -> Vec<(u32, usize, usize)> {
    let mut all: Vec<(u32, usize, usize)> = q
        .index
        .roots
        .iter()
        .enumerate()
        .flat_map(|(ri, root)| {
            let files: &FileIndex = &root.files;
            files
                .entries
                .par_chunks(CHUNK)
                .enumerate()
                .map(|(ci, chunk)| top_k_chunk(files, chunk, ci * CHUNK, pattern))
                .flatten_iter()
                .map(move |(s, ei)| (s, ri, ei))
                .collect::<Vec<_>>()
        })
        .collect();
    all.sort_unstable_by_key(|a| std::cmp::Reverse(a.0));
    all.truncate(TOP_K);
    all
}

fn top_k_chunk(files: &FileIndex, chunk: &[FileEntry], base: usize, pattern: &Pattern) -> Vec<(u32, usize)> {
    MATCHER.with(|cell| {
        let (matcher, buf) = &mut *cell.borrow_mut();
        let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::with_capacity(TOP_K + 1);
        for (i, e) in chunk.iter().enumerate() {
            let name = files.name(e);
            let hay = Utf32Str::new(name, buf);
            if let Some(s) = pattern.score(hay, matcher) {
                if heap.len() < TOP_K {
                    heap.push(Reverse((s, base + i)));
                } else if let Some(Reverse((min, _))) = heap.peek()
                    && s > *min
                {
                    heap.pop();
                    heap.push(Reverse((s, base + i)));
                }
            }
        }
        heap.into_iter().map(|Reverse(x)| x).collect()
    })
}
