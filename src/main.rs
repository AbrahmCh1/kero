#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod host;
mod hotkey;
mod icons;
mod index;
mod plugins;
mod raster;
mod search;
mod ui;
mod usage;
mod win;

use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use ui::{Signals, TrayCmd};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--bench") {
        bench(args.get(2).map(String::as_str).unwrap_or("informe"));
        return;
    }
    if !win::acquire_single_instance() {
        return;
    }
    let start_hidden = args.iter().any(|a| a == "--hidden");
    let initial_query = args.iter().position(|a| a == "--query").and_then(|i| args.get(i + 1)).cloned();
    let pinned = args.iter().any(|a| a == "--pin");

    // Pocos hilos bastan: la búsqueda es corta y no queremos picos de memoria.
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let _ = rayon::ThreadPoolBuilder::new().num_threads(threads).build_global();

    let cfg = config::Config::load();
    if cfg.autostart {
        win::set_autostart(true);
    }
    let hotkey = hotkey::Hotkey::new(&cfg.hotkey);

    let ctx = egui::Context::default();
    host::run(
        ctx,
        Box::new(move |ctx, hwnd| {
            let signals = Arc::new(Signals::default());

            let shared_index: index::SharedIndex = Arc::new(RwLock::new(Arc::new(index::Index::default())));
            let status = Arc::new(index::IndexStatus::default());
            let usage = Arc::new(RwLock::new(usage::Usage::load(config::data_dir().join("usage.tsv"))));
            let cfg = Arc::new(RwLock::new(cfg));

            let c = ctx.clone();
            let indexer = index::spawn(cfg.read().clone(), shared_index.clone(), status.clone(), move || {
                c.request_repaint()
            });
            let engine = search::Engine::new(shared_index.clone(), usage.clone(), cfg.clone());
            let c = ctx.clone();
            let (req_tx, res_rx) = engine.spawn(move || c.request_repaint());

            {
                let signals = signals.clone();
                let c = ctx.clone();
                GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                    if e.state == HotKeyState::Pressed {
                        signals.toggle.store(true, Ordering::Relaxed);
                        c.request_repaint();
                    }
                }));
            }

            let tray = build_tray(ctx, &signals);
            let shared = ui::Shared { status, usage, cfg, indexer, signals, req_tx, res_rx };
            let mut app = ui::KeroApp::new(ctx, shared, Some(hwnd), hotkey, tray, !start_hidden);
            app.initial_query = initial_query;
            app.pinned = pinned;
            app
        }),
    );
}

fn build_tray(ctx: &egui::Context, signals: &Arc<Signals>) -> Option<tray_icon::TrayIcon> {
    let menu = Menu::new();
    let show = MenuItem::new("Abrir Kero", true, None);
    let reindex = MenuItem::new("Reindexar", true, None);
    let settings = MenuItem::new("Configuración", true, None);
    let quit = MenuItem::new("Salir", true, None);
    let _ = menu.append_items(&[&show, &reindex, &settings, &PredefinedMenuItem::separator(), &quit]);
    let ids = [
        (show.id().clone(), TrayCmd::Show),
        (reindex.id().clone(), TrayCmd::Reindex),
        (settings.id().clone(), TrayCmd::Settings),
        (quit.id().clone(), TrayCmd::Quit),
    ];

    let s = signals.clone();
    let c = ctx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if let Some((_, cmd)) = ids.iter().find(|(id, _)| *id == e.id) {
            s.tray.lock().push(*cmd);
            c.request_repaint();
        }
    }));
    let s = signals.clone();
    let c = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            s.tray.lock().push(TrayCmd::Show);
            c.request_repaint();
        }
    }));

    let (rgba, size) = tray_pixels();
    TrayIconBuilder::new()
        .with_tooltip("Kero")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_icon(tray_icon::Icon::from_rgba(rgba, size, size).ok()?)
        .build()
        .ok()
}

/// Icono de bandeja dibujado por código: cuadrado redondeado con una lupa.
fn tray_pixels() -> (Vec<u8>, u32) {
    const N: u32 = 32;
    let accent = win::accent_color().unwrap_or([0, 120, 212]);
    let mut px = vec![0u8; (N * N * 4) as usize];
    let cov = |d: f32| (0.5 - d).clamp(0.0, 1.0);
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Cuadrado redondeado
            let (hw, r) = (15.0, 7.0);
            let qx = (fx - 16.0).abs() - (hw - r);
            let qy = (fy - 16.0).abs() - (hw - r);
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
            let bg = cov(outside);
            // Lupa: anillo + mango
            let ring = (((fx - 14.0).powi(2) + (fy - 14.0).powi(2)).sqrt() - 6.5).abs() - 1.6;
            let (ax, ay, bx, by) = (18.8, 18.8, 24.0, 24.0);
            let t = (((fx - ax) * (bx - ax) + (fy - ay) * (by - ay)) / ((bx - ax).powi(2) + (by - ay).powi(2))).clamp(0.0, 1.0);
            let handle = ((fx - (ax + t * (bx - ax))).powi(2) + (fy - (ay + t * (by - ay))).powi(2)).sqrt() - 1.9;
            let fg = cov(ring).max(cov(handle));
            let i = ((y * N + x) * 4) as usize;
            for c in 0..3 {
                px[i + c] = (accent[c] as f32 * (1.0 - fg) + 255.0 * fg) as u8;
            }
            px[i + 3] = (bg * 255.0) as u8;
        }
    }
    (px, N)
}

/// `kero --bench <consulta>`: mide indexado y búsqueda sin UI.
fn bench(q: &str) {
    use std::time::Instant;
    let cfg = config::Config::load();
    let roots = cfg.resolved_roots();
    let opts = index::files::WalkOptions {
        exclude: cfg.index.exclude.clone(),
        exclude_ext: cfg.index.exclude_ext.clone(),
        skip_hidden: cfg.index.skip_hidden,
        max_depth: cfg.index.max_depth,
    };
    let t = Instant::now();
    let apps = index::apps::enumerate();
    println!("apps: {} en {:?}", apps.len(), t.elapsed());
    let t = Instant::now();
    let mut idx = index::Index { apps, roots: Vec::new() };
    for r in &roots {
        let files = index::files::build(std::slice::from_ref(r), &opts);
        println!("  {}: {} entradas, {} KB", r.display(), files.len(), files.memory_bytes() / 1024);
        idx.roots.push(Arc::new(index::RootIndex { root: r.clone(), files }));
    }
    println!("archivos: {} en {:?}", idx.file_count(), t.elapsed());
    let shared: index::SharedIndex = Arc::new(RwLock::new(Arc::new(idx)));
    let usage = Arc::new(RwLock::new(usage::Usage::default()));
    let engine = search::Engine::new(shared, usage, Arc::new(RwLock::new(cfg)));
    for query in [q, "chrome", "pdf", "2^10 + sqrt(16)", "5 km to mi", "#ff8800", "g gatos", "apagar"] {
        let t = Instant::now();
        let res = engine.search(query);
        let el = t.elapsed();
        println!("\n\"{query}\" → {} resultados en {:?}", res.len(), el);
        for it in res.iter().take(4) {
            println!("   [{:>6}] {} — {}", it.score, it.title, it.subtitle);
        }
    }
}
