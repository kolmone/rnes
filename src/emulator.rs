use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
};

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Q_BUTTERWORTH_F32};
use eyre::eyre;
use eyre::Result;
use rubato::{FftFixedOut, VecResampler};
use sdl2::{
    audio::{AudioCallback, AudioDevice, AudioSpecDesired},
    event::Event,
    keyboard::Keycode,
    pixels::PixelFormatEnum,
    render::{Canvas, TextureCreator},
    video::{Window, WindowContext},
    EventPump,
};

use crate::{
    console::controller::{Button, Controller},
    console::ppu::Ppu,
    renderer::Renderer,
};

pub struct Emulator {
    audio_tx: Sender<Vec<f32>>,
    event_pump: EventPump,
    keymap: HashMap<Keycode, Button>,
    canvas: Canvas<Window>,
    tex_creator: TextureCreator<WindowContext>,
    renderer: Renderer,
    _device: AudioDevice<AudioHandler>, // Store reference to audio device so it's not killed
}

impl Emulator {
    pub fn new(fullscreen: bool) -> Result<Self> {
        let sdl = match sdl2::init() {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };

        let video = match sdl.video() {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };

        let mut window = video
            .window("N3S", 256 * 4, 240 * 4)
            .position_centered()
            .resizable()
            .build()?;

        let freq = 48000;
        let samples = 1024;
        let audio_spec = AudioSpecDesired {
            freq: Some(freq),
            channels: Some(1),
            samples: Some(samples),
        };
        let audio = match sdl.audio() {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };

        let (audio_handler, audio_tx) = AudioHandler::new(freq as usize, samples as usize)?;

        if fullscreen {
            let mut mode = match window.display_mode() {
                Ok(v) => v,
                Err(e) => return Err(eyre!(e)),
            };
            mode.refresh_rate = 60;
            match window.set_fullscreen(sdl2::video::FullscreenType::True) {
                Ok(_) => (),
                Err(e) => return Err(eyre!(e)),
            }
            match window.set_display_mode(mode) {
                Ok(_) => (),
                Err(e) => return Err(eyre!(e)),
            }
        }

        let event_pump = match sdl.event_pump() {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };

        let device = match audio.open_playback(None, &audio_spec, move |_spec| audio_handler) {
            Ok(v) => v,
            Err(e) => return Err(eyre!(e)),
        };
        device.resume();

        let renderer = Renderer::new()?;

        let canvas = window.into_canvas().present_vsync().build()?;
        let tex_creator = canvas.texture_creator();

        Ok(Self {
            audio_tx,
            event_pump,
            keymap: Self::build_keymap(),
            canvas,
            tex_creator,
            renderer,
            _device: device,
        })
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

    pub fn audio_tx(&mut self) -> Sender<Vec<f32>> {
        self.audio_tx.clone()
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
        self.canvas.present();

        Ok(())
    }
}

struct AudioHandler {
    input_buffer: Vec<f32>,
    output_data: Vec<Vec<f32>>,
    resampler: FftFixedOut<f32>,
    rx: Receiver<Vec<f32>>,
    samples_processed: usize,
    samples_received: usize,
    bq: DirectForm2Transposed<f32>,
}

impl AudioCallback for AudioHandler {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let samples = self.resampler.input_frames_next();
        while samples > self.input_buffer.len() {
            if let Ok(mut vec) = self.rx.try_recv() {
                if self.samples_received == 0 {
                    self.samples_processed = 0;
                }
                self.samples_received += vec.len();
                self.input_buffer.append(&mut vec);
            } else {
                if self.samples_received > 0 {
                    println!("No new audio available, adding empty");
                }
                let fill = match self.input_buffer.last() {
                    Some(val) => *val,
                    None => 0.0,
                };
                let mut empty = vec![fill; 10000];
                self.input_buffer.append(&mut empty);
            }
        }
        let input_data = vec![self.input_buffer[0..samples].to_vec(); 1];

        match self.resampler.process_into_buffer(
            &input_data,
            &mut self.output_data,
            Some(&[true; 1]),
        ) {
            Ok(()) => (),
            Err(e) => panic!("Resampling error {}", e),
        }
        for (idx, elem) in self.output_data[0].iter().enumerate() {
            out[idx] = self.bq.run(*elem);
        }

        self.input_buffer.drain(0..samples);
        self.samples_processed += samples;
    }
}

impl AudioHandler {
    fn new(out_freq: usize, buffer_len: usize) -> Result<(Self, Sender<Vec<f32>>)> {
        let fft_resampler = FftFixedOut::new(crate::APU_FREQ, out_freq, buffer_len, 1, 1)?;
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
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
        Ok((
            Self {
                input_buffer: Vec::new(),
                output_data: vec![vec![0.0; buffer_len]; 1],
                resampler: fft_resampler,
                rx,
                samples_processed: 0,
                samples_received: 0,
                bq,
            },
            tx,
        ))
    }
}
