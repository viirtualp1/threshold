#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod graphics;
mod hud;
mod math;
mod scene;
mod sim;

use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use crate::graphics::Graphics;
use crate::sim::{STEP, Simulation};

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    graphics: Option<Graphics>,
    sim: Simulation,
    pressed: HashSet<KeyCode>,
    captured: bool,
    occluded: bool,
    last_frame: Option<Instant>,
    time_left: f32,
    smoke_test: bool,
    presented: bool,
    error: Option<Box<dyn Error>>,
}

impl App {
    fn set_capture(&mut self, enabled: bool) {
        let Some(window) = &self.window else { return };
        self.captured = false;
        if enabled {
            self.captured = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                .is_ok();
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
        }
        window.set_cursor_visible(!self.captured);
    }

    fn key(&mut self, key: KeyCode, state: ElementState, repeat: bool) {
        if state == ElementState::Released {
            self.pressed.remove(&key);
            return;
        }

        self.pressed.insert(key);
        if repeat {
            return;
        }

        match key {
            KeyCode::Escape => self.set_capture(false),
            KeyCode::KeyE => self.sim.interact(),
            KeyCode::KeyR => {
                self.sim = Simulation::default();
                self.time_left = 0.0;
            }
            KeyCode::KeyF => {
                if let Some(window) = &self.window {
                    let fullscreen = window
                        .fullscreen()
                        .is_none()
                        .then_some(Fullscreen::Borderless(None));
                    window.set_fullscreen(fullscreen);
                }
            }
            _ => {}
        }
    }

    fn axis(&self, positive: KeyCode, negative: KeyCode) -> f32 {
        f32::from(self.pressed.contains(&positive)) - f32::from(self.pressed.contains(&negative))
    }

    fn redraw(&mut self) -> Result<bool, Box<dyn Error>> {
        let now = Instant::now();
        let dt = self
            .last_frame
            .replace(now)
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32().min(0.05));
        let sideways = self.axis(KeyCode::KeyD, KeyCode::KeyA);
        let forward = self.axis(KeyCode::KeyW, KeyCode::KeyS);
        let yaw = self.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft);
        let pitch = self.axis(KeyCode::ArrowUp, KeyCode::ArrowDown);
        let sprint = self.pressed.contains(&KeyCode::ShiftLeft)
            || self.pressed.contains(&KeyCode::ShiftRight);

        self.sim.player.turn(yaw * 1.7 * dt, pitch * 1.25 * dt);
        self.time_left += dt;
        while self.time_left >= STEP {
            self.sim.step(sideways, forward, sprint);
            self.time_left -= STEP;
        }

        if let Some(graphics) = &mut self.graphics {
            let presented = graphics.draw(&self.sim, self.captured)?;
            if self.smoke_test && presented {
                graphics.wait_for_frame()?;
            }
            return Ok(presented);
        }

        Ok(false)
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: Box<dyn Error>) {
        self.error = Some(error);
        self.set_capture(false);
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("Threshold — Impossible Apartment")
            .with_inner_size(LogicalSize::new(1180.0, 740.0))
            .with_min_inner_size(LogicalSize::new(640.0, 410.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => return self.fail(event_loop, Box::new(error)),
        };

        match pollster::block_on(Graphics::new(window.clone())) {
            Ok(graphics) => self.graphics = Some(graphics),
            Err(error) => return self.fail(event_loop, error),
        }
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(graphics) = &mut self.graphics {
                    graphics.resize(size);
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                self.last_frame = None;
            }
            WindowEvent::Focused(false) => {
                self.pressed.clear();
                self.set_capture(false);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state, event.repeat);
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: ElementState::Pressed,
                ..
            } => {
                if !self.captured {
                    self.set_capture(true);
                }
                self.sim.interact();
            }
            WindowEvent::RedrawRequested if !self.occluded => match self.redraw() {
                Ok(true) if self.smoke_test => {
                    self.presented = true;
                    event_loop.exit();
                }
                Ok(_) => {}
                Err(error) => self.fail(event_loop, error),
            },
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if self.captured
            && let DeviceEvent::MouseMotion { delta: (x, y) } = event
        {
            self.sim
                .player
                .turn(x as f32 * 0.0022, -(y as f32) * 0.0022);
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if !self.occluded
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        smoke_test: std::env::args_os().any(|arg| arg == "--smoke-test"),
        ..App::default()
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    if app.smoke_test && !app.presented {
        return Err("Threshold closed before presenting a frame".into());
    }
    Ok(())
}
