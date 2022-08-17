use std::collections::HashMap;
use std::thread::yield_now;
use std::time::Duration;
use std::time::SystemTime;

use egui_sdl2_gl::egui::Color32;
use eyre::Result;
use sdl2::Sdl;

use super::fw_error;
use crate::console::controller::Button;
use crate::console::controller::Controller;
use crate::console::SCREEN_HEIGHT;
use crate::console::SCREEN_WIDTH;
use egui_sdl2_gl::egui::CtxRef;
use egui_sdl2_gl::egui::TextureId;
use egui_sdl2_gl::egui::Vec2;
use egui_sdl2_gl::egui::{self, Frame};
use egui_sdl2_gl::painter::Painter;
use egui_sdl2_gl::EguiStateHandler;
use eyre::eyre;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseUtil;
use sdl2::video::FullscreenType;
use sdl2::video::GLContext;
use sdl2::video::Window;
use sdl2::EventPump;

const WINDOW_WIDTH: u32 = (SCREEN_WIDTH * 3) as u32;
const WINDOW_HEIGHT: u32 = (SCREEN_HEIGHT * 3) as u32;

pub const RENDER_WIDTH: usize = SCREEN_WIDTH;
pub const RENDER_HEIGHT: usize = SCREEN_HEIGHT;

const ASPECT_RATIO: f32 = SCREEN_WIDTH as f32 / SCREEN_HEIGHT as f32;

pub struct Ui {
    _gl_context: GLContext,
    mouse: MouseUtil,
    event_pump: EventPump,
    window: Window,
    keymap: HashMap<Keycode, Button>,
    egui_context: CtxRef,
    egui_painter: Painter,
    egui_state: EguiStateHandler,
    egui_texture: TextureId,
    next_render_time: SystemTime,
    menu_timeout_start: SystemTime,
    prev_cursor_pos: egui::Pos2,
}

impl Ui {
    pub fn new(sdl: &Sdl, fullscreen: bool) -> Result<Self> {
        let video = fw_error!(sdl.video());

        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_double_buffer(true);
        gl_attr.set_multisample_samples(4);
        gl_attr.set_framebuffer_srgb_compatible(true);
        gl_attr.set_context_version(3, 2);

        let mut window = video
            .window("rN3S", WINDOW_WIDTH, WINDOW_HEIGHT)
            .opengl()
            .resizable()
            .build()?;

        let gl_context = fw_error!(window.gl_create_context());
        assert_eq!(gl_attr.context_profile(), sdl2::video::GLProfile::Core);
        assert_eq!(gl_attr.context_version(), (3, 2));

        fw_error!(window
            .subsystem()
            .gl_set_swap_interval(sdl2::video::SwapInterval::Immediate));

        if fullscreen {
            let mut mode = fw_error!(window.display_mode());
            mode.refresh_rate = 60;
            let desktop_mode = fw_error!(video.desktop_display_mode(0));
            mode.w = desktop_mode.w;
            mode.h = desktop_mode.h;
            fw_error!(window.set_display_mode(mode));
            fw_error!(window.set_fullscreen(sdl2::video::FullscreenType::True));
            fw_error!(window
                .subsystem()
                .gl_set_swap_interval(sdl2::video::SwapInterval::VSync));
        }

        let (mut egui_painter, egui_state) = egui_sdl2_gl::with_sdl2(
            &window,
            egui_sdl2_gl::ShaderVersion::Default,
            egui_sdl2_gl::DpiScaling::Custom(1.25),
        );
        let egui_context = egui::CtxRef::default();
        let srgba: Vec<Color32> = vec![Color32::TRANSPARENT; RENDER_WIDTH * RENDER_HEIGHT];
        let egui_texture =
            egui_painter.new_user_texture((RENDER_WIDTH, RENDER_HEIGHT), &srgba, false);

        let mouse = sdl.mouse();
        let event_pump = fw_error!(sdl.event_pump());

        Ok(Self {
            _gl_context: gl_context,
            mouse,
            event_pump,
            keymap: Self::build_keymap(),
            window,
            egui_context,
            egui_painter,
            egui_state,
            egui_texture,
            next_render_time: SystemTime::now() + Duration::from_nanos(16_666_666),
            menu_timeout_start: SystemTime::now(),
            prev_cursor_pos: egui::Pos2::default(),
        })
    }

    fn scale_game(available_space: Vec2) -> Vec2 {
        let (w, h) = (available_space.x, available_space.y);
        if w / h > ASPECT_RATIO {
            // Screen wider than default
            let w = h * ASPECT_RATIO;
            // let pos = egui::pos2((ww as f32 - w) / 2.0, 0.0);
            Vec2::new(w, h)
        } else {
            // Screen taller than default
            let h = w / ASPECT_RATIO;
            // let pos = egui::pos2(0.0, (wh as f32 - h) / 2.0);
            Vec2::new(w, h)
        }
    }

    pub fn update(&mut self, game_texture: Vec<u8>, controller: &mut Controller) {
        // let start_time = SystemTime::now();
        self.egui_context.begin_frame(self.egui_state.input.take());

        unsafe {
            // Clear the screen
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        self.egui_painter
            .update_user_texture_rgba8_data(self.egui_texture, game_texture);
        egui::CentralPanel::default()
            .frame(Frame::none())
            .show(&self.egui_context, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.image(self.egui_texture, Self::scale_game(ui.available_size()));
                });
            });

        // Draw audio buffer depth graph
        // egui::Window::new("audio buffer").show(&self.egui_context, |ui| {
        //     let line = Line::new(Values::from_ys_f32(&self.audio_handler.average_history));
        //     Plot::new("buffer depth")
        //         .view_aspect(1.0)
        //         .show(ui, |plot_ui| plot_ui.line(line));
        // });

        let cursor_pos = self.egui_state.pointer_pos;
        if cursor_pos != self.prev_cursor_pos {
            self.prev_cursor_pos = cursor_pos;
            self.menu_timeout_start = SystemTime::now();
        }

        let elapsed = match SystemTime::now().duration_since(self.menu_timeout_start) {
            Ok(val) => val,
            Err(_) => Duration::from_secs(0),
        };

        let hide_panel = elapsed > Duration::from_secs(2);

        self.mouse.show_cursor(!hide_panel);

        if !hide_panel {
            egui::TopBottomPanel::top("top panel").show(&self.egui_context, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Load ROM").clicked() {
                            println!("Loading ROM!");
                        }
                        if ui.button("Reset").clicked() {
                            controller.reset();
                            ui.close_menu();
                        }
                        if ui.button("Quit").clicked() {
                            std::process::exit(0);
                        }
                    });
                });
            });
        }

        let (egui_output, paint_cmds) = self.egui_context.end_frame();
        self.egui_state.process_output(&self.window, &egui_output);

        let paint_jobs = self.egui_context.tessellate(paint_cmds);
        self.egui_painter
            .paint_jobs(None, paint_jobs, &self.egui_context.font_image());

        // println!(
        //     "Rendering took {:?}",
        //     SystemTime::now().duration_since(start_time).unwrap()
        // );

        let minimized = self.window.window_flags() & 64 != 0;
        if self.window.fullscreen_state() != FullscreenType::True || minimized {
            let mut now = SystemTime::now();
            if now < self.next_render_time {
                while now < self.next_render_time {
                    yield_now();
                    now = SystemTime::now();
                }
            } else {
                println!("Frame rendering late");
            }
            self.next_render_time = now + Duration::from_nanos(16_666_666);
        }
        self.window.gl_swap_window();
    }

    pub fn handle_input(&mut self, controller: &mut Controller) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                }
                | Event::Quit { .. } => std::process::exit(0),
                Event::KeyDown {
                    keycode: Some(Keycode::R),
                    ..
                } => {
                    controller.reset();
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(key) = self.keymap.get(&keycode.unwrap_or(Keycode::Ampersand)) {
                        controller.set_button_state(*key, true);
                    } else {
                        self.egui_state
                            .process_input(&self.window, event, &mut self.egui_painter);
                    }
                }
                Event::KeyUp { keycode, .. } => {
                    if let Some(key) = self.keymap.get(&keycode.unwrap_or(Keycode::Ampersand)) {
                        controller.set_button_state(*key, false);
                    } else {
                        self.egui_state
                            .process_input(&self.window, event, &mut self.egui_painter);
                    }
                }
                _ => {
                    self.egui_state
                        .process_input(&self.window, event, &mut self.egui_painter);
                }
            }
        }
    }

    fn build_keymap() -> HashMap<Keycode, Button> {
        HashMap::from([
            (Keycode::Down, Button::Down),
            (Keycode::Up, Button::Up),
            (Keycode::Right, Button::Right),
            (Keycode::Left, Button::Left),
            (Keycode::Q, Button::Select),
            (Keycode::W, Button::Start),
            (Keycode::S, Button::A),
            (Keycode::A, Button::B),
        ])
    }
}
