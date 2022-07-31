use std::collections::HashMap;
use std::thread::yield_now;
use std::time::Duration;
use std::time::SystemTime;

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Q_BUTTERWORTH_F32};
use egui_sdl2_gl::egui;
use egui_sdl2_gl::egui::plot::Line;
use egui_sdl2_gl::egui::plot::Plot;
use egui_sdl2_gl::egui::plot::Values;
use egui_sdl2_gl::egui::Color32;
use egui_sdl2_gl::egui::CtxRef;
use egui_sdl2_gl::egui::Order;
use egui_sdl2_gl::egui::TextureId;
use egui_sdl2_gl::painter::Painter;
use egui_sdl2_gl::DpiScaling;
use egui_sdl2_gl::EguiStateHandler;
use egui_sdl2_gl::ShaderVersion;
use eyre::eyre;
use eyre::Result;
use rubato::InterpolationParameters;
use rubato::InterpolationType;
use rubato::WindowFunction;
use rubato::{Resampler, SincFixedIn};
use sdl2::video::FullscreenType;
use sdl2::video::GLContext;
use sdl2::video::GLProfile;
use sdl2::video::SwapInterval;
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    event::Event,
    keyboard::Keycode,
    video::Window,
    EventPump, Sdl,
};

use crate::{
    console::apu::Apu,
    console::controller::{Button, Controller},
    console::ppu::Ppu,
    renderer::Renderer,
};

macro_rules! fw_error {
    ( $x:expr ) => {
        match $x {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        }
    };
}

pub struct Emulator {
    event_pump: EventPump,
    keymap: HashMap<Keycode, Button>,
    _gl_context: GLContext,
    window: Window,
    renderer: Renderer,
    audio_handler: AudioHandler,
    audio_device: AudioQueue<f32>,
    next_render_time: SystemTime,
    egui_context: CtxRef,
    egui_painter: Painter,
    egui_state: EguiStateHandler,
    egui_texture: TextureId,
}

impl Emulator {
    pub fn new(fullscreen: bool) -> Result<Self> {
        let sdl = fw_error!(sdl2::init());

        let event_pump = fw_error!(sdl.event_pump());

        let (gl_context, window, egui_context, egui_painter, egui_state, egui_texture) =
            Self::init_video(&sdl, fullscreen)?;
        let renderer = Renderer::new()?;
        let audio_device = Self::init_audio(&sdl)?;

        let audio_handler = AudioHandler::new(48000, crate::APU_FREQ / 120)?;

        Ok(Self {
            event_pump,
            keymap: Self::build_keymap(),
            _gl_context: gl_context,
            window,
            renderer,
            audio_device,
            audio_handler,
            next_render_time: SystemTime::now() + Duration::from_nanos(16_666_666),
            egui_context,
            egui_painter,
            egui_state,
            egui_texture,
        })
    }

    fn init_video(
        sdl: &Sdl,
        fullscreen: bool,
    ) -> Result<(
        GLContext,
        Window,
        CtxRef,
        Painter,
        EguiStateHandler,
        TextureId,
    )> {
        let video = fw_error!(sdl.video());

        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(GLProfile::Core);
        gl_attr.set_double_buffer(true);
        gl_attr.set_multisample_samples(4);
        gl_attr.set_framebuffer_srgb_compatible(true);
        gl_attr.set_context_version(3, 2);

        let mut window = video
            .window("N3S", 256 * 3, 240 * 3)
            .opengl()
            .resizable()
            .build()?;

        let gl_context = fw_error!(window.gl_create_context());
        assert_eq!(gl_attr.context_profile(), GLProfile::Core);
        assert_eq!(gl_attr.context_version(), (3, 2));

        fw_error!(window
            .subsystem()
            .gl_set_swap_interval(SwapInterval::Immediate));

        if fullscreen {
            let mut mode = fw_error!(window.display_mode());
            mode.refresh_rate = 60;
            let desktop_mode = fw_error!(video.desktop_display_mode(0));
            mode.w = desktop_mode.w;
            mode.h = desktop_mode.h;
            fw_error!(window.set_display_mode(mode));
            fw_error!(window.set_fullscreen(sdl2::video::FullscreenType::True));
            fw_error!(window.subsystem().gl_set_swap_interval(SwapInterval::VSync));
        }

        let (mut painter, egui_state) =
            egui_sdl2_gl::with_sdl2(&window, ShaderVersion::Default, DpiScaling::Default);
        let egui_context = egui::CtxRef::default();
        let srgba: Vec<Color32> = vec![Color32::TRANSPARENT; 256 * 240];
        let egui_texture = painter.new_user_texture((256, 240), &srgba, false);

        Ok((
            gl_context,
            window,
            egui_context,
            painter,
            egui_state,
            egui_texture,
        ))
    }

    fn init_audio(sdl: &Sdl) -> Result<AudioQueue<f32>> {
        let audio_spec = AudioSpecDesired {
            freq: Some(48000),
            channels: Some(1),
            samples: Some(1024),
        };
        let audio = fw_error!(sdl.audio());

        let device = fw_error!(audio.open_queue(None, &audio_spec));
        device.resume();
        Ok(device)
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

    pub fn handle_input(&mut self, controller: &mut Controller) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                }
                | Event::Quit { .. } => std::process::exit(0),
                Event::KeyDown { keycode, .. } => {
                    if let Some(key) = self.keymap.get(&keycode.unwrap_or(Keycode::Ampersand)) {
                        controller.set_button_state(*key, true);
                    }
                }
                Event::KeyUp { keycode, .. } => {
                    if let Some(key) = self.keymap.get(&keycode.unwrap_or(Keycode::Ampersand)) {
                        controller.set_button_state(*key, false);
                    }
                }
                _ => {
                    self.egui_state
                        .process_input(&self.window, event, &mut self.egui_painter);
                }
            }
        }
    }

    const ASPECT_RATIO: f32 = 256.0 / 240.0;
    fn get_game_pos(&self) -> (f32, f32, egui::Pos2) {
        let (ww, wh) = self.window.size();
        if (ww as f32) / (wh as f32) > Self::ASPECT_RATIO {
            // Screen wider than default
            let h = wh as f32;
            let w = h * Self::ASPECT_RATIO;
            let pos = egui::pos2((ww as f32 - w) / 2.0, 0.0);
            (w, h, pos)
        } else {
            // Screen taller than default
            let w = ww as f32;
            let h = w / Self::ASPECT_RATIO;
            let pos = egui::pos2(0.0, (wh as f32 - h) / 2.0);
            (w, h, pos)
        }
    }

    pub fn render_screen(&mut self, ppu: &Ppu) {
        // let start_time = SystemTime::now();
        self.egui_context.begin_frame(self.egui_state.input.take());

        unsafe {
            // Clear the screen
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        let (width, height, pos) = self.get_game_pos();

        let texture = self.renderer.render_texture(ppu);
        self.egui_painter
            .update_user_texture_data(self.egui_texture, &texture);

        // Draw the area containing the game
        egui::Area::new("game")
            .fixed_pos(pos)
            .order(Order::Background)
            .show(&self.egui_context, |ui| {
                ui.image(self.egui_texture, egui::vec2(width, height));
            });

        // Draw audio buffer depth graph
        egui::Window::new("audio buffer").show(&self.egui_context, |ui| {
            let line = Line::new(Values::from_ys_f32(&self.audio_handler.average_history));
            Plot::new("buffer depth")
                .view_aspect(1.0)
                .show(ui, |plot_ui| plot_ui.line(line));
        });

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
        // if minimized {
        //     println!("Minimized");
        // }

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

    pub fn handle_audio(&mut self, apu: &Apu) -> Result<()> {
        self.audio_handler
            .process(&apu.output, &mut self.audio_device)
    }
}

struct AudioHandler {
    output_data: Vec<Vec<f32>>,
    resampler: SincFixedIn<f32>,
    samples_processed: usize,
    samples_received: usize,
    lp_14khz: DirectForm2Transposed<f32>,
    hp_90hz: DirectForm2Transposed<f32>,
    hp_440hz: DirectForm2Transposed<f32>,
    average_buff: usize,
    pub average_history: Vec<f32>,
}

impl AudioHandler {
    const TARGET_BUFFER_LEN: usize = 1200;
    const BUFFER_LEN_TOLERANCE: usize = 50;
    const BUFFER_LOW_LIMIT: usize = Self::TARGET_BUFFER_LEN - Self::BUFFER_LEN_TOLERANCE;
    const BUFFER_HIGH_LIMIT: usize = Self::TARGET_BUFFER_LEN + Self::BUFFER_LEN_TOLERANCE;

    const RATIO_FILL: f64 = 1.003;
    const RATIO_EMPTY: f64 = 1.0 / Self::RATIO_FILL;
    const RATIO_NORMAL: f64 = 1.0;

    fn new(out_freq: usize, input_len: usize) -> Result<Self> {
        let params = InterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: InterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let resampler = SincFixedIn::new(
            out_freq as f64 / crate::APU_FREQ as f64,
            1.01,
            params,
            input_len,
            1,
        )?;

        let coeffs = match Coefficients::<f32>::from_params(
            biquad::Type::SinglePoleLowPass,
            48.khz(),
            14.khz(),
            Q_BUTTERWORTH_F32,
        ) {
            Ok(v) => v,
            Err(_) => return Err(eyre!("Failed to build filter coefficients")),
        };

        let lp_14khz = DirectForm2Transposed::<f32>::new(coeffs);

        let omega = 2.0 * core::f32::consts::PI * 90.0 / 48000.0;
        let alpha = 1.0 / (omega + 1.0);
        let coeffs = Coefficients {
            a1: -alpha,
            a2: 0.0,
            b0: alpha,
            b1: -alpha,
            b2: 0.0,
        };
        let hp_90hz = DirectForm2Transposed::<f32>::new(coeffs);

        let omega = 2.0 * core::f32::consts::PI * 440.0 / 48000.0;
        let alpha = 1.0 / (omega + 1.0);
        let coeffs = Coefficients {
            a1: -alpha,
            a2: 0.0,
            b0: alpha,
            b1: -alpha,
            b2: 0.0,
        };
        let hp_440hz = DirectForm2Transposed::<f32>::new(coeffs);

        Ok(Self {
            output_data: vec![vec![0.0; resampler.output_frames_max()]; 1],
            resampler,
            samples_processed: 0,
            samples_received: 0,
            lp_14khz,
            hp_90hz,
            hp_440hz,
            average_buff: 0,
            average_history: vec![0.0; 100],
        })
    }

    fn process(&mut self, input: &[f32], queue: &mut AudioQueue<f32>) -> Result<()> {
        if self.samples_received == 0 {
            match queue.queue_audio(&[0.0; 1200]) {
                Ok(_) => (),
                Err(e) => return Err(eyre!(e)),
            }
        }

        let samples = self.resampler.input_frames_next();
        self.samples_received += input.len();

        let queue_size = queue.size() / 4;
        self.average_buff -= self.average_buff / 100;
        self.average_buff += queue_size as usize / 100;

        self.average_history = self.average_history[1..].to_vec();
        self.average_history.push(queue_size as f32);
        // println!("Average buffer length is {}", self.average_buff);

        match self.average_buff {
            0..=Self::BUFFER_LOW_LIMIT => self
                .resampler
                .set_resample_ratio_relative(Self::RATIO_FILL)?,
            Self::BUFFER_HIGH_LIMIT.. => self
                .resampler
                .set_resample_ratio_relative(Self::RATIO_EMPTY)?,
            _ => self
                .resampler
                .set_resample_ratio_relative(Self::RATIO_NORMAL)?,
        }

        // println!("next samples is {}", self.resampler.output_frames_next());

        self.resampler
            .process_into_buffer(&[input; 1], &mut self.output_data, Some(&[true; 1]))?;
        // println!("Out buffer is {} samples", self.output_data[0].len());

        let output: Vec<f32> = self.output_data[0]
            .iter()
            .map(|x| self.lp_14khz.run(*x))
            .map(|x| self.hp_90hz.run(x))
            .map(|x| self.hp_440hz.run(x))
            .collect();

        match queue.queue_audio(&output) {
            Ok(_) => (),
            Err(e) => return Err(eyre!(e)),
        }

        self.samples_processed += samples;
        Ok(())
    }
}
