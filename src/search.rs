//! Hilo de búsqueda: recibe consultas, descarta las obsoletas y devuelve
//! resultados ordenados. La UI nunca se bloquea esperando.

use crate::config::Config;
use crate::index::SharedIndex;
use crate::plugins::{self, Item, Mode, Plugin, Query};
use crate::usage::Usage;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

pub struct Request {
    pub generation: u64,
    pub text: String,
}

pub struct Response {
    pub generation: u64,
    pub items: Vec<Item>,
    pub micros: u64,
}

pub struct Engine {
    pub index: SharedIndex,
    pub usage: Arc<RwLock<Usage>>,
    pub cfg: Arc<RwLock<Config>>,
    plugins: Vec<Box<dyn Plugin>>,
}

pub fn parse_mode(raw: &str) -> (Mode, &str) {
    let t = raw.trim_start();
    if let Some(r) = t.strip_prefix('>') {
        (Mode::Shell, r.trim())
    } else if let Some(r) = t.strip_prefix('=') {
        (Mode::Calc, r.trim())
    } else if let Some(r) = t.strip_prefix('/') {
        (Mode::Files, r.trim())
    } else {
        (Mode::All, t.trim_end())
    }
}

impl Engine {
    pub fn new(index: SharedIndex, usage: Arc<RwLock<Usage>>, cfg: Arc<RwLock<Config>>) -> Self {
        Self { index, usage, cfg, plugins: plugins::all() }
    }

    pub fn search(&self, raw: &str) -> Vec<Item> {
        let (mode, text) = parse_mode(raw);
        if text.is_empty() && mode == Mode::All {
            return Vec::new();
        }
        let index = self.index.read().clone();
        let usage = self.usage.read();
        let cfg = self.cfg.read();
        let q = Query { text, mode, index: &index, usage: &usage, cfg: &cfg };
        let mut out = Vec::with_capacity(64);
        for p in &self.plugins {
            p.query(&q, &mut out);
        }
        out.sort_by(|a, b| b.score.cmp(&a.score));
        // Si hay suficientes coincidencias claras, las dispersas sólo meten ruido.
        if out.iter().filter(|i| !i.weak).count() >= 6 {
            out.retain(|i| !i.weak);
        }
        // Mismo destino desde dos fuentes (p.ej. un .lnk en Escritorio y la app): conserva el primero.
        let mut seen = std::collections::HashSet::new();
        out.retain(|it| match &it.key {
            Some(k) => seen.insert(k.clone()),
            None => true,
        });
        out.truncate(cfg.max_results.max(5));
        out
    }

    pub fn spawn(self, on_done: impl Fn() + Send + 'static) -> (Sender<Request>, Receiver<Response>) {
        let (qtx, qrx) = crossbeam_channel::unbounded::<Request>();
        let (rtx, rrx) = crossbeam_channel::unbounded::<Response>();
        std::thread::Builder::new()
            .name("kero-search".into())
            .spawn(move || {
                while let Ok(mut req) = qrx.recv() {
                    // Sólo importa la última tecla pulsada.
                    while let Ok(newer) = qrx.try_recv() {
                        req = newer;
                    }
                    let t = Instant::now();
                    let items = self.search(&req.text);
                    let micros = t.elapsed().as_micros() as u64;
                    if rtx.send(Response { generation: req.generation, items, micros }).is_err() {
                        return;
                    }
                    on_done();
                }
            })
            .expect("search thread");
        (qtx, rrx)
    }
}
