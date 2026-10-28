pub mod apps;
pub mod files;

use crate::config::{self, Config};
use crossbeam_channel::{Receiver, Sender};
use notify::{EventKind, RecursiveMode, Watcher, event::ModifyKind};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct RootIndex {
    pub root: PathBuf,
    pub files: files::FileIndex,
}

#[derive(Default)]
pub struct Index {
    pub apps: Vec<apps::App>,
    pub roots: Vec<Arc<RootIndex>>,
}

impl Index {
    pub fn file_count(&self) -> usize {
        self.roots.iter().map(|r| r.files.len()).sum()
    }
}

pub type SharedIndex = Arc<RwLock<Arc<Index>>>;

#[derive(Default)]
pub struct IndexStatus {
    pub busy: AtomicBool,
    pub files: AtomicU64,
    pub last_ms: AtomicU64,
}

pub enum IndexCmd {
    RebuildAll,
    Reconfigure(Config),
}

pub fn spawn(
    cfg: Config,
    shared: SharedIndex,
    status: Arc<IndexStatus>,
    on_change: impl Fn() + Send + 'static,
) -> Sender<IndexCmd> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let (dirty_tx, dirty_rx) = crossbeam_channel::unbounded::<PathBuf>();
    std::thread::Builder::new()
        .name("kero-indexer".into())
        .spawn(move || Indexer { cfg, shared, status, on_change, dirty_tx, watcher: None }.run(rx, dirty_rx))
        .expect("indexer thread");
    tx
}

struct Indexer<F: Fn()> {
    cfg: Config,
    shared: SharedIndex,
    status: Arc<IndexStatus>,
    on_change: F,
    dirty_tx: Sender<PathBuf>,
    watcher: Option<notify::RecommendedWatcher>,
}

fn cache_file(root: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    root.to_string_lossy().to_lowercase().hash(&mut h);
    let dir = config::data_dir().join("index");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{:016x}.bin", h.finish()))
}

fn apps_cache() -> PathBuf {
    config::data_dir().join("apps.tsv")
}

impl<F: Fn()> Indexer<F> {
    fn opts(&self) -> files::WalkOptions {
        files::WalkOptions {
            exclude: self.cfg.index.exclude.clone(),
            exclude_ext: self.cfg.index.exclude_ext.clone(),
            skip_hidden: self.cfg.index.skip_hidden,
            max_depth: self.cfg.index.max_depth,
        }
    }

    fn publish(&self, f: impl FnOnce(&mut Index)) {
        let mut guard = self.shared.write();
        let mut next = Index { apps: guard.apps.clone(), roots: guard.roots.clone() };
        f(&mut next);
        self.status.files.store(next.file_count() as u64, Ordering::Relaxed);
        *guard = Arc::new(next);
        drop(guard);
        (self.on_change)();
    }

    fn run(mut self, rx: Receiver<IndexCmd>, dirty_rx: Receiver<PathBuf>) {
        // 1) Arranque instantáneo desde caché.
        let roots = self.cfg.resolved_roots();
        let cached_apps = apps::load(&apps_cache());
        let cached: Vec<Arc<RootIndex>> = roots
            .iter()
            .filter_map(|r| files::load(&cache_file(r)).map(|files| Arc::new(RootIndex { root: r.clone(), files })))
            .collect();
        self.publish(|idx| {
            idx.apps = cached_apps;
            idx.roots = cached;
        });

        // 2) Escaneo fresco en segundo plano.
        self.rebuild_all();
        self.rewatch();

        let mut dirty: HashSet<PathBuf> = HashSet::new();
        let mut last_event = Instant::now();
        loop {
            crossbeam_channel::select! {
                recv(rx) -> cmd => match cmd {
                    Ok(IndexCmd::RebuildAll) => self.rebuild_all(),
                    Ok(IndexCmd::Reconfigure(cfg)) => {
                        self.cfg = cfg;
                        self.rebuild_all();
                        self.rewatch();
                    }
                    Err(_) => return,
                },
                recv(dirty_rx) -> p => if let Ok(p) = p {
                    dirty.insert(p);
                    last_event = Instant::now();
                },
                default(Duration::from_millis(500)) => {}
            }
            // Debounce: espera 2 s sin eventos antes de reindexar la raíz afectada.
            if !dirty.is_empty() && last_event.elapsed() > Duration::from_secs(2) {
                let roots: Vec<PathBuf> = dirty.drain().collect();
                for r in roots {
                    self.rebuild_root(&r);
                }
            }
        }
    }

    fn rebuild_all(&mut self) {
        self.status.busy.store(true, Ordering::Relaxed);
        (self.on_change)();
        let t = Instant::now();

        let apps = apps::enumerate();
        if !apps.is_empty() {
            apps::save(&apps, &apps_cache());
            self.publish(|idx| idx.apps = apps);
        }

        let roots = self.cfg.resolved_roots();
        let opts = self.opts();
        let mut fresh = Vec::with_capacity(roots.len());
        for r in &roots {
            let files = files::build(std::slice::from_ref(r), &opts);
            let _ = files::save(&files, &cache_file(r));
            fresh.push(Arc::new(RootIndex { root: r.clone(), files }));
        }
        self.publish(|idx| idx.roots = fresh);

        self.status.last_ms.store(t.elapsed().as_millis() as u64, Ordering::Relaxed);
        self.status.busy.store(false, Ordering::Relaxed);
        (self.on_change)();
    }

    fn rebuild_root(&mut self, root: &Path) {
        let files = files::build(&[root.to_path_buf()], &self.opts());
        let _ = files::save(&files, &cache_file(root));
        let ri = Arc::new(RootIndex { root: root.to_path_buf(), files });
        self.publish(|idx| {
            if let Some(slot) = idx.roots.iter_mut().find(|r| r.root == root) {
                *slot = ri;
            } else {
                idx.roots.push(ri);
            }
        });
    }

    fn rewatch(&mut self) {
        self.watcher = None;
        if !self.cfg.index.watch {
            return;
        }
        let roots = self.cfg.resolved_roots();
        let exclude: Vec<String> = self.cfg.index.exclude.iter().map(|e| e.to_lowercase()).collect();
        let tx = self.dirty_tx.clone();
        let roots_for_cb = roots.clone();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            // Sólo nos importan altas, bajas y renombres (no cambios de contenido).
            let relevant =
                matches!(ev.kind, EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_)));
            if !relevant {
                return;
            }
            for p in &ev.paths {
                let excluded = p.components().any(|c| {
                    let s = c.as_os_str().to_string_lossy().to_lowercase();
                    exclude.contains(&s)
                });
                if excluded {
                    continue;
                }
                if let Some(root) = roots_for_cb.iter().find(|r| p.starts_with(r)) {
                    let _ = tx.send(root.clone());
                }
            }
        });
        if let Ok(mut w) = watcher {
            for r in &roots {
                let _ = w.watch(r, RecursiveMode::Recursive);
            }
            self.watcher = Some(w);
        }
    }
}
