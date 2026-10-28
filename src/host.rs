//! Bucle de eventos: winit + egui-winit + rasterizador CPU.
//!
//! Windows presenta con una ventana en capa (`UpdateLayeredWindow`, alfa real);
//! macOS y Linux, con softbuffer en una ventana opaca del tamaño del panel.
//! Sólo se pinta cuando algo cambia, y nunca mientras la ventana está oculta.

use crate::raster::Raster;
use crate::ui::{self, KeroApp};
use egui::epaint::ClippedPrimitive;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId, WindowLevel};

pub enum UserEvent {
    Repaint(Duration),
}

/// Tope de fotogramas mientras hay animaciones (~100 fps).
const MIN_FRAME: Duration = Duration::from_millis(10);

/// Recibe el factor de escala de la pantalla.
type MakeApp = Box<dyn FnOnce(&egui::Context, f32) -> KeroApp>;

pub struct Host {
    ctx: egui::Context,
    make_app: Option<MakeApp>,
    app: Option<KeroApp>,
    window: Option<Arc<Window>>,
    presenter: Option<Presenter>,
    state: Option<egui_winit::State>,
    raster: Raster,
    repaint_at: Option<Instant>,
    last_frame: Instant,
    focused: bool,
}

static PROXY: std::sync::OnceLock<std::sync::Mutex<EventLoopProxy<UserEvent>>> = std::sync::OnceLock::new();

/// Despierta el bucle de eventos desde cualquier hilo (atajo, bandeja, IPC…).
///
/// No basta con `egui::Context::request_repaint`: mientras la ventana está
/// oculta no corre ninguna pasada de egui, egui cree que ya hay un repintado
/// pendiente y no vuelve a avisar.
pub fn wake() {
    if let Some(p) = PROXY.get()
        && let Ok(p) = p.lock()
    {
        let _ = p.send_event(UserEvent::Repaint(Duration::ZERO));
    }
}

pub fn run(ctx: egui::Context, make_app: MakeApp) {
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        // Sin icono en el Dock ni menú propio: es una utilidad de fondo.
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    let event_loop = builder.build().expect("event loop");
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();
    let _ = PROXY.set(std::sync::Mutex::new(proxy.clone()));
    ctx.set_request_repaint_callback(move |info| {
        let _ = proxy.send_event(UserEvent::Repaint(info.delay));
    });
    let mut host = Host {
        ctx,
        make_app: Some(make_app),
        app: None,
        window: None,
        presenter: None,
        state: None,
        raster: Raster::default(),
        repaint_at: Some(Instant::now()),
        last_frame: Instant::now(),
        focused: false,
    };
    let _ = event_loop.run_app(&mut host);
}

impl Host {
    fn schedule(&mut self, delay: Duration) {
        schedule(&mut self.repaint_at, delay);
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        let (Some(app), Some(window), Some(presenter)) =
            (self.app.as_mut(), self.window.clone(), self.presenter.as_mut())
        else {
            return;
        };
        self.repaint_at = None;
        self.last_frame = Instant::now();

        app.logic(&self.ctx, self.focused);
        if app.take_needs_hide() {
            presenter.hide(&window, el);
        }
        let show_now = app.take_needs_show();
        if show_now {
            presenter.position(&window);
        }
        if !app.is_visible() {
            if let Some(d) = app.wake_after() {
                schedule(&mut self.repaint_at, d);
            }
            return;
        }

        let state = self.state.as_mut().expect("egui state");
        let raw = state.take_egui_input(&window);
        let out = self.ctx.run_ui(raw, |ui| app.ui(ui));
        state.handle_platform_output(&window, out.platform_output);

        if !ui::LAYERED {
            presenter.fit_height(&window, app.panel_height());
        }
        let size = window.inner_size();
        let (w, h) = (size.width as usize, size.height as usize);
        if w > 0 && h > 0 {
            let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
            self.raster.set_textures(&out.textures_delta);
            presenter.present(&self.raster, &prims, w, h, out.pixels_per_point);
            self.raster.free_textures(&out.textures_delta);
        }

        if app.take_needs_hide() {
            presenter.hide(&window, el);
        } else if show_now {
            presenter.show(&window);
        }
        // Si la app se ocultó en este fotograma no hace falta volver a pintar.
        if !app.is_visible() {
            if let Some(d) = app.wake_after() {
                schedule(&mut self.repaint_at, d);
            }
            return;
        }
        if let Some(vo) = out.viewport_output.get(&egui::ViewportId::ROOT)
            && vo.repaint_delay < Duration::from_secs(3600)
        {
            schedule(&mut self.repaint_at, vo.repaint_delay.max(MIN_FRAME));
        }
        if let Some(d) = app.wake_after() {
            schedule(&mut self.repaint_at, d);
        }
    }
}

fn schedule(slot: &mut Option<Instant>, delay: Duration) {
    let at = Instant::now() + delay;
    *slot = Some(slot.map_or(at, |t| t.min(at)));
}

impl ApplicationHandler<UserEvent> for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let height = if ui::LAYERED { ui::WIN_H } else { ui::INPUT_H };
        let attrs = Window::default_attributes()
            .with_title("Kero")
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(ui::WIN_W, height))
            .with_window_level(WindowLevel::AlwaysOnTop);
        #[cfg(windows)]
        let attrs = {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs.with_skip_taskbar(true)
        };
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        let presenter = Presenter::new(&window);
        let state = egui_winit::State::new(
            self.ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            None,
            Some(8192),
        );
        let make_app = self.make_app.take().expect("app factory");
        self.app = Some(make_app(&self.ctx, window.scale_factor() as f32));
        self.state = Some(state);
        self.presenter = Some(presenter);
        self.window = Some(window);
        self.schedule(Duration::ZERO);
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone() else { return };
        match &event {
            WindowEvent::CloseRequested => {
                if let Some(app) = self.app.as_mut() {
                    app.request_hide();
                }
                self.schedule(Duration::ZERO);
                return;
            }
            WindowEvent::RedrawRequested => {
                self.schedule(Duration::ZERO);
                return;
            }
            WindowEvent::Focused(f) => {
                self.focused = *f;
            }
            _ => {}
        }
        if let Some(state) = self.state.as_mut() {
            let r = state.on_window_event(&window, &event);
            if r.repaint {
                self.schedule(Duration::ZERO);
            }
        }
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, event: UserEvent) {
        let UserEvent::Repaint(delay) = event;
        if delay < Duration::from_secs(3600) {
            self.schedule(delay);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(at) = self.repaint_at {
            // Respeta el tope de fps aunque se pida "repintar ya".
            let at = at.max(self.last_frame + MIN_FRAME);
            if Instant::now() >= at {
                self.frame(event_loop);
            }
        }
        match self.repaint_at {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at.max(self.last_frame + MIN_FRAME))),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}

// ----------------------------------------------------------------- Windows

#[cfg(windows)]
struct Presenter {
    hwnd: crate::platform::Hwnd,
    surface: Option<crate::platform::LayeredSurface>,
}

#[cfg(windows)]
impl Presenter {
    fn new(window: &Window) -> Self {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => crate::platform::Hwnd(h.hwnd.get()),
            _ => panic!("ventana Win32 esperada"),
        };
        crate::platform::make_layered(hwnd);
        Self { hwnd, surface: None }
    }

    fn position(&self, _window: &Window) {
        crate::platform::position_on_cursor_monitor(self.hwnd);
    }

    fn show(&self, _window: &Window) {
        crate::platform::show_and_focus(self.hwnd);
    }

    fn hide(&self, _window: &Window, _el: &ActiveEventLoop) {
        crate::platform::hide(self.hwnd);
    }

    fn fit_height(&self, _window: &Window, _points: f32) {}

    fn present(&mut self, raster: &Raster, prims: &[ClippedPrimitive], w: usize, h: usize, ppp: f32) {
        let mut surface = match self.surface.take() {
            Some(s) if s.w == w && s.h == h => s,
            _ => crate::platform::LayeredSurface::new(w, h),
        };
        raster.paint(prims, surface.pixels(), w, h, ppp);
        surface.present(self.hwnd);
        self.surface = Some(surface);
    }
}

// ------------------------------------------------------------ macOS / Linux

#[cfg(not(windows))]
struct Presenter {
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    _context: softbuffer::Context<Arc<Window>>,
}

#[cfg(not(windows))]
impl Presenter {
    fn new(window: &Arc<Window>) -> Self {
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");
        Self { surface, _context: context }
    }

    /// Centrado horizontalmente, a ~18% de altura del monitor actual.
    /// (En Wayland el compositor decide la posición y esto no tiene efecto.)
    fn position(&self, window: &Window) {
        let Some(mon) = window.current_monitor().or_else(|| window.primary_monitor()) else { return };
        let (mp, ms, ws) = (mon.position(), mon.size(), window.outer_size());
        let x = mp.x + (ms.width as i32 - ws.width as i32) / 2;
        let y = mp.y + (ms.height as f32 * 0.18) as i32;
        window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    }

    fn show(&self, window: &Window) {
        window.set_visible(true);
        window.focus_window();
    }

    fn hide(&self, window: &Window, _el: &ActiveEventLoop) {
        window.set_visible(false);
        #[cfg(target_os = "macos")]
        {
            // Devuelve el foco a la app anterior.
            use winit::platform::macos::ActiveEventLoopExtMacOS;
            _el.hide_application();
        }
    }

    /// La ventana es tan alta como el panel.
    fn fit_height(&self, window: &Window, points: f32) {
        let scale = window.scale_factor();
        let current = window.inner_size().height as f64 / scale;
        if (current - points as f64).abs() > 0.5 {
            let _ = window.request_inner_size(LogicalSize::new(ui::WIN_W, points));
        }
    }

    fn present(&mut self, raster: &Raster, prims: &[ClippedPrimitive], w: usize, h: usize, ppp: f32) {
        use std::num::NonZeroU32;
        let (Some(nw), Some(nh)) = (NonZeroU32::new(w as u32), NonZeroU32::new(h as u32)) else { return };
        if self.surface.resize(nw, nh).is_err() {
            return;
        }
        let Ok(mut buf) = self.surface.buffer_mut() else { return };
        // softbuffer espera 0RGB: el panel es opaco, así que el byte alto se ignora.
        raster.paint(prims, &mut buf, w, h, ppp);
        let _ = buf.present();
    }
}
