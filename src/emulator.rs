use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
};

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Q_BUTTERWORTH_F32};
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
    pub fn new(fullscreen: bool) -> Self {
        let sdl = sdl2::init().unwrap();
        let mut window = sdl
            .video()
            .unwrap()
            .window("N3S", 256 * 4, 240 * 4)
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let audio_spec = AudioSpecDesired {
            freq: Some(48000),
            channels: Some(1),
            samples: Some(1024),
        };
        let audio = sdl.audio().unwrap();
        let (audio_handler, audio_tx) = AudioHandler::new(
            audio_spec.freq.unwrap() as usize,
            audio_spec.samples.unwrap() as usize,
        );

        if fullscreen {
            let mut mode = window.display_mode().unwrap();
            mode.refresh_rate = 60;
            window
                .set_fullscreen(sdl2::video::FullscreenType::True)
                .unwrap();
            window.set_display_mode(mode).unwrap();
        }

        let event_pump = sdl.event_pump().unwrap();

        let device = audio
            .open_playback(None, &audio_spec, move |_spec| audio_handler)
            .unwrap();
        device.resume();

        let renderer = Renderer::new();

        let canvas = window.into_canvas().present_vsync().build().unwrap();
        let tex_creator = canvas.texture_creator();

        Self {
            audio_tx,
            event_pump,
            keymap: Self::build_keymap(),
            canvas,
            tex_creator,
            renderer,
            _device: device,
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

    pub fn audio_tx(&mut self) -> Sender<Vec<f32>> {
        self.audio_tx.clone()
    }

    pub fn handle_input(&mut self, controller: &mut Controller) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => std::process::exit(0),
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => std::process::exit(0),
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

    pub fn render_screen(&mut self, ppu: &Ppu) {
        let mut texture = self
            .tex_creator
            .create_texture_target(PixelFormatEnum::RGB24, 256, 240)
            .unwrap();

        self.renderer.render_texture(ppu, &mut texture);
        self.canvas.copy(&texture, None, None).unwrap();
        self.canvas.present();
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
            match self.rx.try_recv() {
                Ok(mut vec) => {
                    if self.samples_received == 0 {
                        self.samples_processed = 0;
                    }
                    self.samples_received += vec.len();
                    self.input_buffer.append(&mut vec);
                }
                Err(_) => {
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
    fn new(out_freq: usize, buffer_len: usize) -> (Self, Sender<Vec<f32>>) {
        let fft_resampler = FftFixedOut::new(crate::APU_FREQ, out_freq, buffer_len, 1, 1).unwrap();
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let coeffs = Coefficients::<f32>::from_params(
            biquad::Type::SinglePoleLowPass,
            48.khz(),
            14.khz(),
            Q_BUTTERWORTH_F32,
        )
        .unwrap();
        let bq = DirectForm2Transposed::<f32>::new(coeffs);
        (
            AudioHandler {
                input_buffer: Vec::new(),
                output_data: vec![vec![0.0; buffer_len]; 1],
                resampler: fft_resampler,
                rx,
                samples_processed: 0,
                samples_received: 0,
                bq,
            },
            tx,
        )
    }
}
