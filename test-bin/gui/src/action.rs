// SPDX-License-Identifier: MIT

use super::debug::debug;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

fn draw_window(window: &Window) -> Result<(), String> {
    // Note that Wayland requires something to be drawn in the window before it will show it.
    // Does this draw right in Wayland?  No.  Do we care?  No.
    let context = Context::new(window).map_err(|e| format!("surface context failed: {e}"))?;
    let mut surface =
        Surface::new(&context, window).map_err(|e| format!("surface init failed: {e}"))?;

    let size = window.inner_size();
    let width = NonZeroU32::new(size.width.max(1)).unwrap();
    let height = NonZeroU32::new(size.height.max(1)).unwrap();
    surface
        .resize(width, height)
        .map_err(|e| format!("surface resize failed: {e}"))?;

    let mut buffer = surface
        .buffer_mut()
        .map_err(|e| format!("surface buffer failed: {e}"))?;
    for pixel in buffer.iter_mut() {
        *pixel = 0x00233644;
    }
    buffer
        .present()
        .map_err(|e| format!("surface present failed: {e}"))
}

struct TimedWindowApp {
    window: Option<Window>,
    close_at: Option<Instant>,
}

impl TimedWindowApp {
    fn new() -> Self {
        Self {
            window: None,
            close_at: None,
        }
    }
}

impl ApplicationHandler for TimedWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("grackle-zero GUI test")
            .with_inner_size(LogicalSize::new(360.0, 120.0));

        match event_loop.create_window(attrs) {
            Ok(window) => {
                if let Err(e) = draw_window(&window) {
                    debug(format!("initial draw failed: {e}"));
                }
                window.request_redraw();
                self.close_at = Some(Instant::now() + Duration::from_secs(5));
                self.window = Some(window);
                if let Some(deadline) = self.close_at {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
                }
            }
            Err(e) => {
                debug(format!("window creation failed: {e}"));
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested | WindowEvent::Resized(_) => {
                if let Some(window) = self.window.as_ref() {
                    if let Err(e) = draw_window(window) {
                        debug(format!("draw failed: {e}"));
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(deadline) = self.close_at {
            if Instant::now() >= deadline {
                event_loop.exit();
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        }
    }
}

fn run_timed_window() -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("event loop init failed: {e}"))?;
    let mut app = TimedWindowApp::new();
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("event loop run failed: {e}"))
}

pub(crate) fn perform(arg: String) {
    debug(format!("attempting GUI launch with {}", arg));
    if let Err(e) = run_timed_window() {
        panic!("GUI launch attempt failed: {e}");
    }
}
