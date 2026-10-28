#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod host;
mod hotkey;
mod icons;
mod index;
mod platform;
mod plugins;
mod raster;
mod search;
mod ui;
mod usage;

use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use parking_lot::RwLock;
use platform::Ipc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use ui::Signals;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--bench") {
        bench(args.get(2).map(String::as_str).unwrap_or("informe"));
        return;
    }
    if has("--version") {
        println!("kero {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if has("--quit") {
        if platform::ipc_send(Ipc::Quit) {
            platform::wait_until_closed();
        }
        return;
    }
    if !platform::acquire_single_instance() {
        // Ya hay una instancia: un segundo arranque (o `--toggle`) la muestra u oculta.
        if !has("--hidden") {
            platform::ipc_send(Ipc::Toggle);
        }
        return;
    }
    let start_hidden = has("--hidden");
    let initial_query = args.iter().position(|a| a == "--query").and_then(|i| args.get(i + 1)).cloned();
    let pinned = has("--pin");

    // Pocos hilos bastan: la búsqueda es corta y no queremos picos de memoria.
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let _ = rayon::ThreadPoolBuilder::new().num_threads(threads).build_global();

    let cfg = config::Config::load();
    if cfg.autostart {
        platform::set_autostart(true);
    }
    let hotkey = hotkey::Hotkey::new(&cfg.hotkey);

    let ctx = egui::Context::default();
    host::run(
        ctx,
        Box::new(move |ctx, scale| {
            let signals = Arc::new(Signals::default());

            let shared_index: index::SharedIndex = Arc::new(RwLock::new(Arc::new(index::Index::default())));
            let status = Arc::new(index::IndexStatus::default());
            let usage = Arc::new(RwLock::new(usage::Usage::load(config::data_dir().join("usage.tsv"))));
            let cfg = Arc::new(RwLock::new(cfg));

            let indexer = index::spawn(cfg.read().clone(), shared_index.clone(), status.clone(), crate::host::wake);
            let engine = search::Engine::new(shared_index.clone(), usage.clone(), cfg.clone());
            let (req_tx, res_rx) = engine.spawn(crate::host::wake);

            {
                let signals = signals.clone();
                GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                    if e.state == HotKeyState::Pressed {
                        signals.toggle.store(true, Ordering::Relaxed);
                        crate::host::wake();
                    }
                }));
            }
            {
                let signals = signals.clone();
                let usage = usage.clone();
                platform::ipc_listen(move |msg| match msg {
                    Ipc::Quit => {
                        usage.read().save();
                        std::process::exit(0);
                    }
                    Ipc::Toggle => {
                        signals.toggle.store(true, Ordering::Relaxed);
                        crate::host::wake();
                    }
                });
            }

            let tray = tray::build(&signals);
            let shared = ui::Shared { status, usage, cfg, indexer, signals, req_tx, res_rx };
            let mut app = ui::KeroApp::new(ctx, shared, scale, hotkey, tray, !start_hidden);
            app.initial_query = initial_query;
            app.pinned = pinned;
            app
        }),
    );
}

/// Icono en la bandeja (Windows) o en la barra de menús (macOS). En Linux no
/// hay: evitaría depender de GTK; se usa `kero --quit` o "Kero: Salir".
#[cfg(any(windows, target_os = "macos"))]
mod tray {
    use crate::ui::{Signals, TrayCmd};
    use std::sync::Arc;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    pub fn build(signals: &Arc<Signals>) -> Option<Box<dyn std::any::Any>> {
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
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Some((_, cmd)) = ids.iter().find(|(id, _)| *id == e.id) {
                s.tray.lock().push(*cmd);
                crate::host::wake();
            }
        }));
        let s = signals.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                s.tray.lock().push(TrayCmd::Show);
                crate::host::wake();
            }
        }));

        let builder = TrayIconBuilder::new().with_tooltip("Kero").with_menu(Box::new(menu));
        #[cfg(windows)]
        let builder = builder
            .with_menu_on_left_click(false)
            .with_icon(tray_icon::Icon::from_resource(1, Some(crate::platform::small_icon_size())).ok()?);
        #[cfg(target_os = "macos")]
        let builder = {
            let (rgba, n) = lens_template();
            builder.with_icon_templated(tray_icon::Icon::from_rgba(rgba, n, n).ok()?)
        };
        let tray = builder.build().ok()?;
        Some(Box::new(tray))
    }

    /// Lupa monocroma para la barra de menús; macOS la tiñe según el tema.
    #[cfg(target_os = "macos")]
    fn lens_template() -> (Vec<u8>, u32) {
        const N: u32 = 36;
        let mut px = vec![0u8; (N * N * 4) as usize];
        let s = N as f32 / 32.0;
        let cov = |d: f32| (0.5 - d * s).clamp(0.0, 1.0);
        for y in 0..N {
            for x in 0..N {
                let (fx, fy) = ((x as f32 + 0.5) / s, (y as f32 + 0.5) / s);
                let ring = (((fx - 14.0).powi(2) + (fy - 14.0).powi(2)).sqrt() - 8.0).abs() - 2.0;
                let (ax, ay, bx, by) = (20.0f32, 20.0f32, 27.0f32, 27.0f32);
                let t = (((fx - ax) * (bx - ax) + (fy - ay) * (by - ay)) / ((bx - ax).powi(2) + (by - ay).powi(2)))
                    .clamp(0.0, 1.0);
                let handle = ((fx - (ax + t * (bx - ax))).powi(2) + (fy - (ay + t * (by - ay))).powi(2)).sqrt() - 2.3;
                let a = cov(ring).max(cov(handle));
                let i = ((y * N + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&[0, 0, 0, (a * 255.0) as u8]);
            }
        }
        (px, N)
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod tray {
    pub fn build(_signals: &std::sync::Arc<crate::ui::Signals>) -> Option<Box<dyn std::any::Any>> {
        None
    }
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
