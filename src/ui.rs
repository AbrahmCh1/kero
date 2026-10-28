use crate::config::{self, Config};
use crate::icons::Icons;
use crate::index::{IndexCmd, IndexStatus};
use crate::platform;
use crate::plugins::{Action, Glyph, Icon, Item, KeroCmd};
use crate::search::{Request, Response};
use crate::usage::Usage;
use crossbeam_channel::{Receiver, Sender};
use egui::{
    self, Align2, Color32, CornerRadius, Event, FontFamily, FontId, Id, Key, Pos2, Rect, Sense, Shadow, Stroke,
    StrokeKind, Vec2, pos2, text::LayoutJob, vec2,
};
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

// Medidas en puntos lógicos.
/// En Windows la ventana es "layered" (alfa real): panel con sombra y esquinas
/// dentro de una ventana fija. En macOS y Linux es un panel opaco y la ventana
/// se ajusta a su altura.
pub const LAYERED: bool = cfg!(windows);
pub const PANEL_W: f32 = 712.0;
const MARGIN: f32 = if LAYERED { 24.0 } else { 0.0 };
const TOP: f32 = if LAYERED { 8.0 } else { 0.0 };
pub const WIN_W: f32 = PANEL_W + 2.0 * MARGIN;
pub const WIN_H: f32 = 600.0;
pub const INPUT_H: f32 = 60.0;
const ROW_H: f32 = 50.0;
const MAX_ROWS: usize = 8;
const FOOTER_H: f32 = 34.0;
const RADIUS: u8 = if LAYERED { 14 } else { 0 };
const DEFAULT_ACCENT: [u8; 3] = [0, 120, 212];
/// Tras este tiempo oculto se devuelve la memoria no usada al sistema.
const TRIM_AFTER: Duration = Duration::from_secs(10);

/// Órdenes del menú de la bandeja (no hay bandeja en Linux).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub enum TrayCmd {
    Show,
    Reindex,
    Settings,
    Quit,
}

#[derive(Default)]
pub struct Signals {
    pub toggle: AtomicBool,
    pub tray: Mutex<Vec<TrayCmd>>,
}

#[derive(Clone, Copy)]
struct Theme {
    bg: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    faint: Color32,
    sel: Color32,
    accent: Color32,
    shadow: Color32,
    danger: Color32,
}

impl Theme {
    fn new(light: bool, accent: [u8; 3]) -> Self {
        let accent = Color32::from_rgb(accent[0], accent[1], accent[2]);
        if light {
            Theme {
                bg: Color32::from_rgb(250, 250, 251),
                border: Color32::from_black_alpha(28),
                text: Color32::from_rgb(24, 24, 27),
                muted: Color32::from_rgb(110, 110, 118),
                faint: Color32::from_black_alpha(14),
                sel: Color32::from_black_alpha(13),
                accent,
                shadow: Color32::from_black_alpha(60),
                danger: Color32::from_rgb(196, 43, 28),
            }
        } else {
            Theme {
                bg: Color32::from_rgb(28, 28, 32),
                border: Color32::from_white_alpha(22),
                text: Color32::from_rgb(236, 236, 240),
                muted: Color32::from_rgb(140, 140, 150),
                faint: Color32::from_white_alpha(14),
                sel: Color32::from_white_alpha(16),
                accent,
                shadow: Color32::from_black_alpha(120),
                danger: Color32::from_rgb(255, 110, 100),
            }
        }
    }
}

pub struct Shared {
    pub status: Arc<IndexStatus>,
    pub usage: Arc<RwLock<Usage>>,
    pub cfg: Arc<RwLock<Config>>,
    pub indexer: Sender<IndexCmd>,
    pub signals: Arc<Signals>,
    pub req_tx: Sender<Request>,
    pub res_rx: Receiver<Response>,
}

pub struct KeroApp {
    s: Shared,
    icons: Icons,
    theme: Theme,
    query: String,
    items: Vec<Item>,
    selected: usize,
    scroll: usize,
    generation: u64,
    search_micros: u64,
    visible: bool,
    shown_at: Instant,
    hidden_at: Instant,
    trimmed: bool,
    focus_input: bool,
    confirm: Option<usize>,
    toast: Option<(String, Instant)>,
    hide_at: Option<Instant>,
    cfg_mtime: Option<SystemTime>,
    last_pointer: Option<Pos2>,
    pub hotkey: crate::hotkey::Hotkey,
    /// Icono de bandeja (sólo debe seguir vivo).
    pub _tray: Option<Box<dyn std::any::Any>>,
    /// `--query "texto"`: abre con una búsqueda precargada (pruebas y capturas).
    pub initial_query: Option<String>,
    /// `--pin`: no se oculta al perder el foco (pruebas).
    pub pinned: bool,
    needs_show: bool,
    needs_hide: bool,
    had_focus: bool,
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let [regular, semi, icons] = platform::font_candidates();
    // Carga la primera fuente existente de cada lista.
    let mut load = |name: &str, paths: &[std::path::PathBuf]| -> bool {
        match paths.iter().find_map(|p| std::fs::read(p).ok()) {
            Some(bytes) => {
                fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
                true
            }
            None => false,
        }
    };
    let has_ui = load("system", &regular);
    let has_sb = load("system-sb", &semi);
    let has_icons = load("icons", &icons);

    let defaults = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = Vec::new();
    if has_ui {
        prop.push("system".to_string());
    }
    prop.extend(defaults.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, prop.clone());

    let mut sb = Vec::new();
    if has_sb {
        sb.push("system-sb".to_string());
    }
    sb.extend(prop.iter().cloned());
    fonts.families.insert(FontFamily::Name("semibold".into()), sb);

    let mut ic = Vec::new();
    if has_icons {
        ic.push("icons".to_string());
    }
    ic.extend(prop);
    fonts.families.insert(FontFamily::Name("icons".into()), ic);
    ctx.set_fonts(fonts);
}

fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

fn wants_light(cfg: &Config) -> bool {
    match cfg.theme.as_str() {
        "light" => true,
        "dark" => false,
        _ => platform::system_uses_light_theme(),
    }
}

impl KeroApp {
    pub fn new(
        ctx: &egui::Context,
        s: Shared,
        scale: f32,
        hotkey: crate::hotkey::Hotkey,
        tray: Option<Box<dyn std::any::Any>>,
        start_visible: bool,
    ) -> Self {
        setup_fonts(ctx);
        let light = wants_light(&s.cfg.read());
        let theme = Theme::new(light, platform::accent_color().unwrap_or(DEFAULT_ACCENT));
        apply_style(ctx, &theme, light);
        let icons = Icons::new(ctx.clone(), (28.0 * scale).round() as u32);
        if start_visible {
            s.signals.toggle.store(true, Ordering::Relaxed);
        }
        let cfg_mtime = std::fs::metadata(config::config_path()).and_then(|m| m.modified()).ok();
        let mut app = Self {
            s,
            icons,
            theme,
            query: String::new(),
            items: Vec::new(),
            selected: 0,
            scroll: 0,
            generation: 0,
            search_micros: 0,
            visible: false,
            shown_at: Instant::now(),
            hidden_at: Instant::now(),
            trimmed: false,
            focus_input: true,
            confirm: None,
            toast: None,
            hide_at: None,
            cfg_mtime,
            last_pointer: None,
            hotkey,
            _tray: tray,
            initial_query: None,
            pinned: false,
            needs_show: false,
            needs_hide: false,
            had_focus: false,
        };
        if let Some(err) = app.hotkey.error.clone() {
            app.toast = Some((err, Instant::now() + Duration::from_secs(8)));
        }
        app
    }

    fn send_query(&mut self) {
        self.generation += 1;
        self.confirm = None;
        let _ = self.s.req_tx.send(Request { generation: self.generation, text: self.query.clone() });
    }

    fn poll_results(&mut self) {
        while let Ok(r) = self.s.res_rx.try_recv() {
            if r.generation == self.generation {
                self.items = r.items;
                self.selected = 0;
                self.scroll = 0;
                self.search_micros = r.micros;
            }
        }
    }

    fn show(&mut self, ctx: &egui::Context) {
        if self.hotkey.error.is_some() {
            // El atajo pudo estar ocupado al arrancar (p. ej. por PowerToys) y liberarse después.
            let spec = self.s.cfg.read().hotkey.clone();
            self.hotkey.set(&spec);
        }
        self.reload_config_if_changed(ctx);
        self.query.clear();
        self.items.clear();
        self.selected = 0;
        self.scroll = 0;
        self.confirm = None;
        self.hide_at = None;
        self.visible = true;
        self.trimmed = false;
        self.shown_at = Instant::now();
        self.focus_input = true;
        self.needs_show = true;
        self.needs_hide = false;
        self.had_focus = false;
        if let Some(q) = self.initial_query.take() {
            self.query = q;
            self.send_query();
        }
        ctx.request_repaint();
    }

    fn hide(&mut self, ctx: &egui::Context) {
        self.needs_hide = true;
        self.needs_show = false;
        self.visible = false;
        self.query.clear();
        self.items.clear();
        self.generation += 1;
        self.hidden_at = Instant::now();
        self.hide_at = None;
        self.trimmed = false;
        let _ = ctx;
    }

    fn reload_config_if_changed(&mut self, ctx: &egui::Context) {
        let mtime = std::fs::metadata(config::config_path()).and_then(|m| m.modified()).ok();
        if mtime == self.cfg_mtime {
            return;
        }
        self.cfg_mtime = mtime;
        let new_cfg = Config::load();
        let old = self.s.cfg.read().clone();
        if new_cfg.hotkey != old.hotkey {
            self.hotkey.set(&new_cfg.hotkey);
            if let Some(err) = self.hotkey.error.clone() {
                self.toast = Some((err, Instant::now() + Duration::from_secs(6)));
            }
        }
        if new_cfg.autostart != old.autostart {
            platform::set_autostart(new_cfg.autostart);
        }
        let index_changed = toml::to_string(&new_cfg.index).ok() != toml::to_string(&old.index).ok();
        let light = wants_light(&new_cfg);
        *self.s.cfg.write() = new_cfg.clone();
        if index_changed {
            let _ = self.s.indexer.send(IndexCmd::Reconfigure(new_cfg));
        }
        self.theme = Theme::new(light, platform::accent_color().unwrap_or(DEFAULT_ACCENT));
        apply_style(ctx, &self.theme, light);
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now() + Duration::from_millis(1500)));
    }

    /// Ejecuta el resultado. `alt` = Ctrl (⌘ en macOS): abrir ubicación.
    /// `shift` = como administrador (sólo Windows).
    fn activate(&mut self, idx: usize, alt: bool, shift: bool, ctx: &egui::Context) {
        let Some(item) = self.items.get(idx).cloned() else { return };
        if item.action.needs_confirm() && self.confirm != Some(idx) {
            self.confirm = Some(idx);
            return;
        }
        let mut hide = true;
        match &item.action {
            Action::None => hide = false,
            Action::Open(p) => {
                if alt {
                    platform::reveal(p);
                } else {
                    platform::open(&p.to_string_lossy());
                }
            }
            Action::App { id, path } => match (alt, path) {
                (true, Some(p)) => platform::reveal(std::path::Path::new(p)),
                _ => platform::launch_app(id, path.as_deref(), shift),
            },
            Action::Url(u) => {
                platform::open(u);
            }
            Action::Copy(text) => {
                platform::copy_to_clipboard(text);
                self.toast("Copiado al portapapeles");
                self.hide_at = Some(Instant::now() + Duration::from_millis(450));
                hide = false;
            }
            Action::Shell(cmd) => platform::run_in_terminal(cmd, shift),
            Action::Run(exe, args) => platform::run(exe, args, shift),
            Action::System(c) => platform::run_system(*c),
            Action::Kero(c) => hide = self.run_kero(*c, ctx),
        }
        if let Some(key) = &item.key {
            self.s.usage.write().record(key, &self.query);
        }
        if hide {
            self.hide(ctx);
        }
    }

    fn run_kero(&mut self, c: KeroCmd, ctx: &egui::Context) -> bool {
        match c {
            KeroCmd::Reindex => {
                let _ = self.s.indexer.send(IndexCmd::RebuildAll);
                self.toast("Reindexando en segundo plano…");
                false
            }
            KeroCmd::OpenConfig => {
                platform::open(&config::config_path().to_string_lossy());
                true
            }
            KeroCmd::AddFolder => {
                let cfg = self.s.cfg.clone();
                let indexer = self.s.indexer.clone();
                std::thread::spawn(move || {
                    if let Some(p) = platform::pick_folder() {
                        let s = p.to_string_lossy().into_owned();
                        let mut c = cfg.write();
                        if !c.index.roots.iter().any(|r| config::expand_path(r) == p) {
                            c.index.roots.push(s);
                            c.save();
                            let _ = indexer.send(IndexCmd::Reconfigure(c.clone()));
                        }
                    }
                });
                true
            }
            KeroCmd::ToggleAutostart => {
                let on = {
                    let mut c = self.s.cfg.write();
                    c.autostart = !c.autostart;
                    c.save();
                    c.autostart
                };
                self.cfg_mtime = std::fs::metadata(config::config_path()).and_then(|m| m.modified()).ok();
                platform::set_autostart(on);
                self.toast(if on { "Kero iniciará con el sistema" } else { "Inicio automático desactivado" });
                let _ = ctx;
                false
            }
            KeroCmd::Quit => {
                self.s.usage.read().save();
                std::process::exit(0);
            }
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let input_id = Id::unique("kero-input");
        let has_selection = egui::TextEdit::load_state(ctx, input_id)
            .and_then(|s| s.cursor.char_range())
            .is_some_and(|r| !r.is_empty());

        let mut actions = Vec::new();
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                Event::Key { key, pressed: true, modifiers, .. } => {
                    let k = *key;
                    let handled = matches!(
                        k,
                        Key::ArrowUp
                            | Key::ArrowDown
                            | Key::Enter
                            | Key::Escape
                            | Key::Tab
                            | Key::PageUp
                            | Key::PageDown
                    );
                    if handled {
                        // `command` = ⌘ en macOS, Ctrl en el resto.
                        actions.push((k, modifiers.command, modifiers.shift));
                    }
                    !handled
                }
                Event::Copy if !has_selection && !self.items.is_empty() => {
                    actions.push((Key::C, true, false));
                    false
                }
                _ => true,
            });
        });

        for (key, ctrl, shift) in actions {
            let n = self.items.len();
            match key {
                Key::ArrowDown if n > 0 => self.select((self.selected + 1) % n),
                Key::ArrowUp if n > 0 => self.select((self.selected + n - 1) % n),
                Key::Tab if n > 0 => {
                    if let Some(c) = self.items[self.selected].complete.clone() {
                        self.query = c;
                        self.set_cursor_end(ctx);
                        self.send_query();
                    } else if shift {
                        self.select((self.selected + n - 1) % n);
                    } else {
                        self.select((self.selected + 1) % n);
                    }
                }
                Key::PageDown if n > 0 => self.select((self.selected + MAX_ROWS).min(n - 1)),
                Key::PageUp if n > 0 => self.select(self.selected.saturating_sub(MAX_ROWS)),
                Key::Enter if n > 0 => self.activate(self.selected, ctrl, shift, ctx),
                Key::Escape => {
                    if self.query.is_empty() {
                        self.hide(ctx);
                    } else {
                        self.query.clear();
                        self.items.clear();
                        self.send_query();
                    }
                }
                Key::C => {
                    if let Some(t) = self.items.get(self.selected).and_then(|i| i.action.copy_text()) {
                        platform::copy_to_clipboard(&t);
                        self.toast("Copiado al portapapeles");
                    }
                }
                _ => {}
            }
        }
    }

    fn set_cursor_end(&self, ctx: &egui::Context) {
        let id = Id::unique("kero-input");
        if let Some(mut st) = egui::TextEdit::load_state(ctx, id) {
            let end = egui::text::CCursor::new(self.query.chars().count());
            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
            st.store(ctx, id);
        }
    }

    fn select(&mut self, i: usize) {
        self.selected = i;
        self.confirm = None;
        if i < self.scroll {
            self.scroll = i;
        } else if i >= self.scroll + MAX_ROWS {
            self.scroll = i + 1 - MAX_ROWS;
        }
    }

    fn handle_tray(&mut self, ctx: &egui::Context) {
        let cmds: Vec<TrayCmd> = std::mem::take(&mut *self.s.signals.tray.lock());
        for c in cmds {
            match c {
                TrayCmd::Show => self.show(ctx),
                TrayCmd::Reindex => {
                    let _ = self.s.indexer.send(IndexCmd::RebuildAll);
                }
                TrayCmd::Settings => {
                    platform::open(&config::config_path().to_string_lossy());
                }
                TrayCmd::Quit => {
                    self.s.usage.read().save();
                    std::process::exit(0);
                }
            }
        }
    }

    // ------------------------------------------------------------------ pintura

    /// Alto del panel según el número de resultados (puntos lógicos).
    pub fn panel_height(&self) -> f32 {
        let rows = self.items.len().min(MAX_ROWS);
        INPUT_H + if rows > 0 { 1.0 + 8.0 + rows as f32 * ROW_H + 6.0 + FOOTER_H } else { 0.0 }
    }

    fn paint(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = &{ self.theme };
        let full = ui.max_rect();
        let panel_w = full.width() - 2.0 * MARGIN;
        let rows = self.items.len().min(MAX_ROWS);
        let has_results = rows > 0;
        let target_h = self.panel_height();
        // Con ventana opaca el alto lo marca la propia ventana: no se anima.
        let h = if LAYERED { ctx.animate_value_with_time(Id::unique("panel-h"), target_h, 0.09) } else { target_h };
        let panel = Rect::from_min_size(pos2(full.left() + MARGIN, full.top() + TOP), vec2(panel_w, h));

        let painter = ui.painter().clone();
        if LAYERED {
            let shadow = Shadow { offset: [0, 10], blur: 32, spread: 0, color: t.shadow };
            painter.add(shadow.as_shape(panel, CornerRadius::same(RADIUS)));
        }
        painter.rect_filled(panel, CornerRadius::same(RADIUS), t.bg);
        painter.rect_stroke(panel, CornerRadius::same(RADIUS), Stroke::new(1.0, t.border), StrokeKind::Inside);

        // Cerrar al hacer clic fuera del panel.
        let bg = ui.interact(full, Id::unique("kero-bg"), Sense::click());
        if bg.clicked() && !bg.interact_pointer_pos().is_some_and(|p| panel.contains(p)) {
            self.hide(ctx);
            return;
        }

        // ---- Barra de búsqueda
        let input_rect = Rect::from_min_size(panel.min, vec2(panel_w, INPUT_H));
        painter.text(
            pos2(input_rect.left() + 26.0, input_rect.center().y + 1.0),
            Align2::CENTER_CENTER,
            Glyph::Search.symbol(),
            icon_font(18.0),
            t.muted,
        );
        let edit_rect = Rect::from_min_max(
            pos2(input_rect.left() + 50.0, input_rect.top() + 14.0),
            pos2(input_rect.right() - 18.0, input_rect.bottom() - 14.0),
        );
        let hint = egui::RichText::new("Buscar apps, archivos o calcular…  (? ayuda)").color(t.muted).size(21.0);
        let edit = egui::TextEdit::singleline(&mut self.query)
            .id(Id::unique("kero-input"))
            .font(FontId::proportional(21.0))
            .text_color(t.text)
            .hint_text(hint)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_width(edit_rect.width())
            .align(Align2::LEFT_CENTER);
        let resp = ui.put(edit_rect, edit);
        if self.focus_input || !resp.has_focus() {
            resp.request_focus();
            self.focus_input = false;
        }
        if resp.changed() {
            self.send_query();
        }

        if !has_results {
            self.paint_toast(&painter, panel);
            return;
        }

        // ---- Resultados
        let sep_y = input_rect.bottom();
        painter.hline(panel.x_range().shrink(1.0), sep_y, Stroke::new(1.0, t.faint));
        let list_top = sep_y + 1.0 + 4.0;
        let list_left = panel.left() + 8.0;
        let list_w = panel_w - 16.0;
        let list_rect = Rect::from_min_size(pos2(list_left, list_top), vec2(list_w, rows as f32 * ROW_H));
        let clip = painter.with_clip_rect(list_rect.intersect(panel));

        // Selección con desplazamiento animado.
        let sel_vis = self.selected.saturating_sub(self.scroll) as f32;
        let sel_y = ctx.animate_value_with_time(Id::unique("sel-y"), list_top + sel_vis * ROW_H, 0.06);
        let sel_rect = Rect::from_min_size(pos2(list_left, sel_y), vec2(list_w, ROW_H));
        clip.rect_filled(sel_rect, CornerRadius::same(9), t.sel);
        clip.rect_filled(
            Rect::from_min_size(pos2(list_left + 1.0, sel_y + 15.0), vec2(3.0, ROW_H - 30.0)),
            CornerRadius::same(2),
            t.accent,
        );

        // Movimiento real del ratón (no el que provoca el scroll del teclado).
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let moved = pointer != self.last_pointer;
        self.last_pointer = pointer;
        let wheel = ctx.input(|i| i.smooth_scroll_delta.y);
        if wheel.abs() > 0.5 && list_rect.contains(pointer.unwrap_or_default()) {
            let n = self.items.len();
            if wheel < 0.0 && self.scroll + MAX_ROWS < n {
                self.scroll += 1;
            } else if wheel > 0.0 && self.scroll > 0 {
                self.scroll -= 1;
            }
            self.selected = self.selected.clamp(self.scroll, (self.scroll + MAX_ROWS - 1).min(n - 1));
        }

        let mut clicked = None;
        let end = (self.scroll + rows).min(self.items.len());
        for (vi, idx) in (self.scroll..end).enumerate() {
            let row = Rect::from_min_size(pos2(list_left, list_top + vi as f32 * ROW_H), vec2(list_w, ROW_H));
            let r = ui.interact(row, Id::unique(("row", idx)), Sense::click());
            if r.hovered() && moved && pointer.is_some() {
                self.selected = idx;
            }
            if r.clicked() {
                clicked = Some(idx);
            }
            self.paint_row(&clip, row, idx);
        }

        // Barra de desplazamiento discreta.
        let n = self.items.len();
        if n > MAX_ROWS {
            let track_h = list_rect.height() - 8.0;
            let thumb_h = (track_h * MAX_ROWS as f32 / n as f32).max(18.0);
            let max_scroll = (n - MAX_ROWS) as f32;
            let y = list_rect.top() + 4.0 + (track_h - thumb_h) * (self.scroll as f32 / max_scroll);
            painter.rect_filled(
                Rect::from_min_size(pos2(panel.right() - 6.0, y), vec2(3.0, thumb_h)),
                CornerRadius::same(2),
                t.faint,
            );
        }

        // ---- Pie
        let foot_top = panel.bottom() - FOOTER_H;
        painter.hline(panel.x_range().shrink(1.0), foot_top, Stroke::new(1.0, t.faint));
        let fy = foot_top + FOOTER_H / 2.0;
        let status = &self.s.status;
        let left = if status.busy.load(Ordering::Relaxed) {
            "Indexando…".to_string()
        } else {
            format!(
                "{} archivos · {:.1} ms",
                fmt_thousands(status.files.load(Ordering::Relaxed)),
                self.search_micros as f64 / 1000.0
            )
        };
        painter.text(pos2(panel.left() + 16.0, fy), Align2::LEFT_CENTER, left, FontId::proportional(12.0), t.muted);

        if let Some(item) = self.items.get(self.selected) {
            let mut x = panel.right() - 14.0;
            for (keys, label) in item.action.hints().iter().rev() {
                x = self.paint_hint(&painter, x, fy, keys, label);
                x -= 14.0;
            }
        }

        self.paint_toast(&painter, panel);

        if let Some(i) = clicked {
            let m = ctx.input(|i| i.modifiers);
            self.activate(i, m.ctrl, m.shift, ctx);
        }
    }

    /// Pinta "Etiqueta [tecla]" alineado a la derecha terminando en `right`. Devuelve el nuevo borde izquierdo.
    fn paint_hint(&self, p: &egui::Painter, right: f32, y: f32, keys: &str, label: &str) -> f32 {
        let t = &self.theme;
        let kg = p.layout_no_wrap(keys.to_string(), FontId::proportional(11.0), t.muted);
        let kw = kg.size().x + 10.0;
        let key_rect = Rect::from_min_size(pos2(right - kw, y - 9.0), vec2(kw, 18.0));
        p.rect_filled(key_rect, CornerRadius::same(4), t.faint);
        p.galley(pos2(key_rect.left() + 5.0, y - kg.size().y / 2.0), kg, t.muted);
        let lg = p.layout_no_wrap(label.to_string(), FontId::proportional(12.0), t.muted);
        let lx = key_rect.left() - 6.0 - lg.size().x;
        p.galley(pos2(lx, y - lg.size().y / 2.0), lg, t.muted);
        lx
    }

    fn paint_row(&self, p: &egui::Painter, row: Rect, idx: usize) {
        let t = &self.theme;
        let item = &self.items[idx];
        let selected = idx == self.selected;

        // Icono
        let ic = Rect::from_center_size(pos2(row.left() + 26.0, row.center().y), Vec2::splat(28.0));
        match &item.icon {
            Icon::Color([r, g, b]) => {
                let sw = ic.shrink(3.0);
                p.rect_filled(sw, CornerRadius::same(6), Color32::from_rgb(*r, *g, *b));
                p.rect_stroke(sw, CornerRadius::same(6), Stroke::new(1.0, t.border), StrokeKind::Inside);
            }
            Icon::Glyph(g) => {
                let color = if selected { t.text } else { t.muted };
                p.text(ic.center(), Align2::CENTER_CENTER, g.symbol(), icon_font(18.0), color);
            }
            Icon::None => {}
            other => match self.icons.get(other) {
                Some(tex) => {
                    p.image(tex.id(), ic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                // Sin icono del sistema: inicial de la app en un cuadro discreto.
                None if matches!(other, Icon::App(_)) => {
                    let sq = ic.shrink(2.0);
                    p.rect_filled(sq, CornerRadius::same(7), t.sel);
                    let initial: String = item.title.chars().take(1).flat_map(char::to_uppercase).collect();
                    p.text(sq.center(), Align2::CENTER_CENTER, initial, semibold(14.0), t.muted);
                }
                None => {
                    let g = if matches!(other, Icon::Path(_, true)) { Glyph::Folder } else { Glyph::File };
                    p.text(ic.center(), Align2::CENTER_CENTER, g.symbol(), icon_font(18.0), t.muted);
                }
            },
        }

        // Textos
        let text_left = row.left() + 52.0;
        let right_pad = if selected { 120.0 } else { 16.0 };
        let max_w = row.right() - text_left - right_pad;
        let title_job = highlighted(&item.title, &item.highlight, semibold(15.0), t.text, t.accent, max_w);
        let title = p.layout_job(title_job);
        let confirming = self.confirm == Some(idx);
        let (sub_text, sub_color) = if confirming {
            ("Pulsa Enter otra vez para confirmar".to_string(), t.danger)
        } else {
            (item.subtitle.clone(), t.muted)
        };
        let mut sub_job = LayoutJob::simple_singleline(sub_text, FontId::proportional(12.0), sub_color);
        sub_job.wrap.max_width = max_w;
        sub_job.wrap.max_rows = 1;
        sub_job.wrap.break_anywhere = true;
        let sub = p.layout_job(sub_job);
        let total = title.size().y + sub.size().y;
        let y0 = row.center().y - total / 2.0;
        p.galley(pos2(text_left, y0), title, t.text);
        p.galley(pos2(text_left, y0 + total - sub.size().y), sub, sub_color);

        if selected && !matches!(item.action, Action::None) {
            let label = item.action.primary_label();
            {
                let x = row.right() - 12.0;
                self.paint_hint(p, x, row.center().y, "Enter", label);
            }
        }
    }

    fn paint_toast(&self, p: &egui::Painter, panel: Rect) {
        let Some((msg, until)) = &self.toast else { return };
        if Instant::now() > *until {
            return;
        }
        let t = &self.theme;
        let g = p.layout_no_wrap(msg.clone(), FontId::proportional(12.5), t.text);
        let size = g.size() + vec2(24.0, 12.0);
        let r = Rect::from_min_size(pos2(panel.right() - size.x - 14.0, panel.top() + (INPUT_H - size.y) / 2.0), size);
        p.rect_filled(r, CornerRadius::same(8), t.sel);
        p.galley(r.min + vec2(12.0, 6.0), g, t.text);
    }
}

fn highlighted(text: &str, hl: &[u32], font: FontId, color: Color32, accent: Color32, max_w: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = max_w;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let fmt = |c: Color32| egui::TextFormat { font_id: font.clone(), color: c, ..Default::default() };
    let mut run = String::new();
    let mut run_hl = false;
    let mut hi = hl.iter().peekable();
    for (ci, ch) in text.chars().enumerate() {
        let is_hl = hi.peek().is_some_and(|&&h| h as usize == ci);
        if is_hl {
            hi.next();
        }
        if is_hl != run_hl && !run.is_empty() {
            job.append(&run, 0.0, fmt(if run_hl { accent } else { color }));
            run.clear();
        }
        run_hl = is_hl;
        run.push(ch);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, fmt(if run_hl { accent } else { color }));
    }
    job
}

fn fmt_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

fn apply_style(ctx: &egui::Context, t: &Theme, light: bool) {
    let mut visuals = if light { egui::Visuals::light() } else { egui::Visuals::dark() };
    visuals.text_cursor.stroke = Stroke::new(2.0, t.accent);
    visuals.selection.bg_fill = t.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, t.text);
    visuals.widgets.noninteractive.fg_stroke.color = t.text;
    visuals.window_fill = Color32::TRANSPARENT;
    visuals.panel_fill = Color32::TRANSPARENT;
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|s| {
        s.interaction.selectable_labels = false;
        s.animation_time = 0.08;
    });
}

impl KeroApp {
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// El host debe posicionar, pintar y luego mostrar la ventana.
    pub fn take_needs_show(&mut self) -> bool {
        std::mem::take(&mut self.needs_show)
    }

    /// El host debe ocultar la ventana.
    pub fn take_needs_hide(&mut self) -> bool {
        std::mem::take(&mut self.needs_hide)
    }

    pub fn request_hide(&mut self) {
        self.hide_at = Some(Instant::now());
    }

    /// Próximo momento en que la app necesita despertar aunque nada cambie.
    pub fn wake_after(&self) -> Option<Duration> {
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        let mut at = |t: Instant| next = Some(next.map_or(t, |n: Instant| n.min(t)));
        if let Some(t) = self.hide_at {
            at(t);
        }
        if let Some((_, until)) = &self.toast
            && *until > now
        {
            at(*until);
        }
        if !self.visible && !self.trimmed {
            at(self.hidden_at + TRIM_AFTER);
        }
        next.map(|t| t.saturating_duration_since(now))
    }

    pub fn logic(&mut self, ctx: &egui::Context, focused: bool) {
        if self.s.signals.toggle.swap(false, Ordering::Relaxed) {
            if self.visible && focused {
                self.hide(ctx);
            } else {
                self.show(ctx);
            }
        }
        self.handle_tray(ctx);
        self.poll_results();

        if self.hide_at.is_some_and(|at| Instant::now() >= at) {
            self.hide(ctx);
        }

        if self.visible {
            if focused {
                self.had_focus = true;
            }
            // Perder el foco (clic en otra ventana) oculta el lanzador.
            if !focused && self.had_focus && !self.pinned && self.hide_at.is_none() {
                self.hide(ctx);
            }
        } else if !self.trimmed && self.hidden_at.elapsed() >= TRIM_AFTER {
            platform::trim_working_set();
            self.trimmed = true;
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.visible {
            self.handle_keys(&ctx);
        }
        self.paint(ui, &ctx);
    }
}
