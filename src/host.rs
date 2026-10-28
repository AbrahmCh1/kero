//! Bucle de eventos: winit + egui-winit + rasterizador CPU + ventana en capa.
//!
//! Sólo se pinta cuando algo cambia, y nunca mientras la ventana está oculta.

use crate::raster::Raster;
use crate::ui::{self, KeroApp};
use crate::win::{self, Hwnd};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowId, WindowLevel};

pub enum UserEvent {
    Repaint(Duration),
}

/// Tope de fotogramas mientras hay animaciones (~100 fps).
const MIN_FRAME: Duration = Duration::from_millis(10);

type MakeApp = Box<dyn FnOnce(&egui::Context, Hwnd) -> KeroApp>;

pub struct Host {
    ctx: egui::Context,
    make_app: Option<MakeApp>,
    app: Option<KeroApp>,
    window: Option<Arc<Window>>,
    hwnd: Option<Hwnd>,
    state: Option<egui_winit::State>,
    raster: Raster,
    surface: Option<win::LayeredSurface>,
    repaint_at: Option<Instant>,
    last_frame: Instant,
    focused: bool,
}

pub fn run(ctx: egui::Context, make_app: MakeApp) {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();
    ctx.set_request_repaint_callback(move |info| {
        let _ = proxy.send_event(UserEvent::Repaint(info.delay));
    });
    let mut host = Host {
        ctx,
        make_app: Some(make_app),
        app: None,
        window: None,
        hwnd: None,
        state: None,
        raster: Raster::default(),
        surface: None,
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

    fn frame(&mut self) {
        let (Some(app), Some(window), Some(hwnd)) = (self.app.as_mut(), self.window.clone(), self.hwnd) else {
            return;
        };
        self.repaint_at = None;
        self.last_frame = Instant::now();

        app.logic(&self.ctx, self.focused);
        let show_now = app.take_needs_show();
        if show_now {
            win::position_on_cursor_monitor(hwnd);
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

        let size = window.inner_size();
        let (w, h) = (size.width as usize, size.height as usize);
        if w > 0 && h > 0 {
            let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
            self.raster.set_textures(&out.textures_delta);
            let surface = match self.surface.take() {
                Some(s) if s.w == w && s.h == h => s,
                _ => win::LayeredSurface::new(w, h),
            };
            let mut surface = surface;
            self.raster.paint(&prims, surface.pixels(), w, h, out.pixels_per_point);
            surface.present(hwnd);
            self.surface = Some(surface);
            self.raster.free_textures(&out.textures_delta);
        }

        if show_now {
            win::show_and_focus(hwnd);
        }
        // Si la app se ocultó en este fotograma no hace falta volver a pintar.
        if !app.is_visible() {
            if let Some(d) = app.wake_after() {
                schedule(&mut self.repaint_at, d);
            }
            return;
        }
        if let Some(vo) = out.viewport_output.get(&egui::ViewportId::ROOT) {
            if vo.repaint_delay < Duration::from_secs(3600) {
                schedule(&mut self.repaint_at, vo.repaint_delay.max(MIN_FRAME));
            }
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
        let attrs = Window::default_attributes()
            .with_title("Kero")
            .with_decorations(false)
            .with_resizable(false)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(ui::WIN_W, ui::WIN_H))
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_skip_taskbar(true);
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => Hwnd(h.hwnd.get()),
            _ => panic!("ventana Win32 esperada"),
        };
        win::make_layered(hwnd);
        let state = egui_winit::State::new(
            self.ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            None,
            Some(8192),
        );
        let make_app = self.make_app.take().expect("app factory");
        self.app = Some(make_app(&self.ctx, hwnd));
        self.state = Some(state);
        self.window = Some(window);
        self.hwnd = Some(hwnd);
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
                self.frame();
            }
        }
        match self.repaint_at {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at.max(self.last_frame + MIN_FRAME))),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}
