use std::collections::HashMap;
use std::thread::yield_now;
use std::time::Duration;
use std::time::SystemTime;

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Q_BUTTERWORTH_F32};
use eyre::eyre;
use eyre::Result;
use rubato::InterpolationParameters;
use rubato::InterpolationType;
use rubato::WindowFunction;
use rubato::{Resampler, SincFixedIn};
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    event::Event,
    keyboard::Keycode,
    pixels::PixelFormatEnum,
    render::{Canvas, TextureCreator},
    video::{Window, WindowContext},
    EventPump, Sdl,
};

use crate::{
    console::apu::Apu,
    console::controller::{Button, Controller},
    console::ppu::Ppu,
    renderer::Renderer,
};

type TexCreator = TextureCreator<WindowContext>;

pub struct Emulator {
    event_pump: EventPump,
    keymap: HashMap<Keycode, Button>,
    canvas: Canvas<Window>,
    tex_creator: TexCreator,
    renderer: Renderer,
    audio_handler: AudioHandler,
    audio_device: AudioQueue<f32>,
    fullscreen: bool,
    next_render_time: SystemTime,
}

impl Emulator {
    pub fn new(fullscreen: bool) -> Result<Self> {
        let sdl = match sdl2::init() {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };

        let event_pump = match sdl.event_pump() {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };

        let (canvas, tex_creator) = Self::init_video(&sdl, fullscreen)?;
        let renderer = Renderer::new()?;
        let audio_device = Self::init_audio(&sdl)?;

        let audio_handler = AudioHandler::new(48000, crate::APU_FREQ / 120)?;

        Ok(Self {
            event_pump,
            keymap: Self::build_keymap(),
            canvas,
            tex_creator,
            renderer,
            audio_device,
            audio_handler,
            fullscreen,
            next_render_time: SystemTime::now() + Duration::from_nanos(16_666_666),
        })
    }

    fn init_video(sdl: &Sdl, fullscreen: bool) -> Result<(Canvas<Window>, TexCreator)> {
        let video = match sdl.video() {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };

        let mut window = video
            .window("N3S", 256 * 3, 240 * 3)
            .position_centered()
            .resizable()
            .build()?;

        if fullscreen {
            let mut mode = match window.display_mode() {
                Ok(v) => v,
                Err(e) => return Err(eyre!(e)),
            };
            mode.refresh_rate = 60;
            let desktop_mode = match video.desktop_display_mode(0) {
                Ok(v) => v,
                Err(e) => return Err(eyre!(e)),
            };
            mode.w = desktop_mode.w;
            mode.h = desktop_mode.h;
            match window.set_fullscreen(sdl2::video::FullscreenType::True) {
                Ok(_) => (),
                Err(e) => return Err(eyre!(e)),
            }
            match window.set_display_mode(mode) {
                Ok(_) => (),
                Err(e) => return Err(eyre!(e)),
            }
        }

        let mut canvas = window.into_canvas().present_vsync().build()?;
        canvas.set_logical_size(256, 240)?;
        let tex_creator = canvas.texture_creator();

        Ok((canvas, tex_creator))
    }

    fn init_audio(sdl: &Sdl) -> Result<AudioQueue<f32>> {
        let audio_spec = AudioSpecDesired {
            freq: Some(48000),
            channels: Some(1),
            samples: Some(1024),
        };
        let audio = match sdl.audio() {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };

        let device = match audio.open_queue(None, &audio_spec) {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };
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
                _ => { /* do nothing */ }
            }
        }
    }

    pub fn render_screen(&mut self, ppu: &Ppu) -> Result<()> {
        let mut texture =
            self.tex_creator
                .create_texture_target(PixelFormatEnum::RGB24, 256, 240)?;

        self.renderer.render_texture(ppu, &mut texture)?;
        match self.canvas.copy(&texture, None, None) {
            Ok(_) => (),
            Err(e) => return Err(eyre!(e)),
        }

        if !self.fullscreen {
            let mut now = SystemTime::now();
            if now < self.next_render_time {
                while now < self.next_render_time {
                    yield_now();
                    now = SystemTime::now();
                }
            } else {
                println!("Arrived late");
            }
            self.next_render_time = now + Duration::from_nanos(16_666_666);
        }

        self.canvas.present();

        Ok(())
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
}

impl AudioHandler {
    const TARGET_BUFFER_LEN: usize = 800;
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

        self.average_buff -= self.average_buff / 100;
        self.average_buff += queue.size() as usize / 100 / 4;
        println!("Average buffer length is {}", self.average_buff);

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
