use std::collections::HashMap;
use std::thread::yield_now;
use std::time::Duration;
use std::time::SystemTime;

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Q_BUTTERWORTH_F32};
use eyre::eyre;
use eyre::Result;
use rubato::{FftFixedOut, VecResampler};
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

        let audio_handler = AudioHandler::new(48000, 48000 / 120)?;

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
            self.next_render_time += Duration::from_nanos(16_666_666);
        }

        self.canvas.present();

        Ok(())
    }

    pub fn handle_audio(&mut self, apu: &Apu) {
        self.audio_handler
            .process(&apu.output, &mut self.audio_device);
    }
}

struct AudioHandler {
    input_buffer: Vec<f32>,
    output_data: Vec<Vec<f32>>,
    resampler: FftFixedOut<f32>,
    samples_processed: usize,
    samples_received: usize,
    bq: DirectForm2Transposed<f32>,
}

impl AudioHandler {
    fn new(out_freq: usize, buffer_len: usize) -> Result<Self> {
        let fft_resampler = FftFixedOut::new(crate::APU_FREQ, out_freq, buffer_len, 1, 1)?;
        let coeffs = match Coefficients::<f32>::from_params(
            biquad::Type::SinglePoleLowPass,
            48.khz(),
            14.khz(),
            Q_BUTTERWORTH_F32,
        ) {
            Ok(v) => v,
            Err(_) => return Err(eyre!("Failed to build filter coefficients")),
        };
        let bq = DirectForm2Transposed::<f32>::new(coeffs);
        Ok(Self {
            input_buffer: Vec::new(),
            output_data: vec![vec![0.0; buffer_len]; 1],
            resampler: fft_resampler,
            samples_processed: 0,
            samples_received: 0,
            bq,
        })
    }

    fn process(&mut self, input: &[f32], queue: &mut AudioQueue<f32>) {
        let samples = self.resampler.input_frames_next();
        self.samples_received += self.input_buffer.len();
        self.input_buffer.append(&mut input.to_vec());

        if self.samples_processed == 0 && self.input_buffer.len() < samples + samples / 2 {
            return;
        }

        if samples > self.input_buffer.len() {
            return;
        }
        // println!("Current buffer length is {}", queue.size() / 4);

        let input_data = vec![self.input_buffer[0..samples].to_vec(); 1];
        match self.resampler.process_into_buffer(
            &input_data,
            &mut self.output_data,
            Some(&[true; 1]),
        ) {
            Ok(()) => (),
            Err(e) => panic!("Resampling error {}", e),
        }

        let output: Vec<f32> = self.output_data[0]
            .iter()
            .map(|x| self.bq.run(*x))
            .collect();

        match queue.queue_audio(&output) {
            Ok(()) => (),
            Err(e) => panic!("Error queueing audio {}", e),
        }

        self.input_buffer.drain(0..samples);
        self.samples_processed += samples;
    }
}
