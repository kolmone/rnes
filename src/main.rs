#![warn(trivial_numeric_casts)]
#![allow(clippy::bad_bit_mask)]

mod apu;
mod bus;
mod controller;
mod cpu;
mod ppu;
mod renderer;

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Q_BUTTERWORTH_F32};
use bus::{Bus, Rom};
use controller::{Button, Controller};
use cpu::Cpu;
use ppu::Ppu;
use renderer::Renderer;
use rubato::{FftFixedOut, VecResampler};
use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    event::Event,
    keyboard::Keycode,
    pixels::PixelFormatEnum,
};
use std::{
    collections::HashMap,
    env,
    sync::mpsc::{self, Receiver, Sender},
    thread::yield_now,
    time::{Duration, SystemTime},
};

// 21441960 / 12 = 1786830 - if NES ran at exactly 60 Hz
// const MAIN_FREQ: usize = 21441960;
const MAIN_FREQ: usize = 21442080; // 89342 PPU cycles * 60 * 4
const CPU_FREQ: usize = MAIN_FREQ / 12;
const APU_FREQ: usize = CPU_FREQ;
const _PPU_FREQ: usize = MAIN_FREQ / 4;

fn build_keymap() -> HashMap<Keycode, Button> {
    let mut keymap = HashMap::new();
    keymap.insert(Keycode::Down, Button::Down);
    keymap.insert(Keycode::Up, Button::Up);
    keymap.insert(Keycode::Right, Button::Right);
    keymap.insert(Keycode::Left, Button::Left);
    keymap.insert(Keycode::Q, Button::Select);
    keymap.insert(Keycode::W, Button::Start);
    keymap.insert(Keycode::S, Button::A);
    keymap.insert(Keycode::A, Button::B);
    keymap
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
        // let start = SystemTime::now();
        let samples = self.resampler.input_frames_next();
        // println!(
        //     "Processing {} samples, {} samples in buffer",
        //     samples,
        //     self.input_buffer.len()
        // );
        while samples > self.input_buffer.len() {
            match self.rx.try_recv() {
                Ok(mut vec) => {
                    if self.samples_received == 0 {
                        // println!("Got first samples");
                        self.samples_processed = 0;
                    }
                    self.samples_received += vec.len();
                    self.input_buffer.append(&mut vec);
                }
                Err(_) => {
                    println!("No new audio available, adding empty");
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
            // Ok(()) => out.clone_from_slice(&self.output_data[0]),
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
        let fft_resampler = FftFixedOut::new(APU_FREQ, out_freq, buffer_len, 1, 1).unwrap();
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

fn run_rom(file: &str, do_trace: bool, render_debug: bool, fullscreen: bool) {
    let sdl = sdl2::init().unwrap();
    let mut window = sdl
        .video()
        .unwrap()
        .window("N3S", 256 * 4, 240 * 4)
        .position_centered()
        .build()
        .unwrap();

    let audio_spec = AudioSpecDesired {
        freq: Some(48000),
        channels: Some(1),
        samples: Some(1024),
    };
    let audio = sdl.audio().unwrap();
    let (audio_handler, tx) = AudioHandler::new(
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

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();

    let tex_creator = canvas.texture_creator();

    let mut texture = if render_debug {
        tex_creator
            .create_texture_target(PixelFormatEnum::RGB24, 256 * 2, 240 * 2)
            .unwrap()
    } else {
        tex_creator
            .create_texture_target(PixelFormatEnum::RGB24, 256, 240)
            .unwrap()
    };

    let mut event_pump = sdl.event_pump().unwrap();

    let rom: Vec<u8> = std::fs::read(file).expect("Unable to open rom file!");
    let mut renderer = Renderer::new();
    let keymap = build_keymap();

    let mut expected_timestamp = SystemTime::now();
    let mut prev_timestamp = SystemTime::now();

    let bus = Bus::new(
        Rom::new(rom).unwrap(),
        tx,
        |ppu: &Ppu, controller: &mut Controller| {
            if !fullscreen {
                let mut now = SystemTime::now();
                if now < expected_timestamp {
                    // println!(
                    //     "Frame done in {:?}!",
                    //     now.duration_since(prev_timestamp).unwrap()
                    // );
                    while now < expected_timestamp {
                        yield_now();
                        now = SystemTime::now();
                    }
                } else {
                    println!("Arrived late");
                }
                prev_timestamp = expected_timestamp;
                expected_timestamp += Duration::from_nanos(16666667);
            }
            renderer.render_screen(ppu, &mut canvas, &mut texture, render_debug);

            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => std::process::exit(0),
                    Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => std::process::exit(0),
                    Event::KeyDown { keycode, .. } => {
                        if let Some(key) = keymap.get(&keycode.unwrap_or(Keycode::Ampersand)) {
                            controller.set_button_state(*key, true);
                        }
                    }
                    Event::KeyUp { keycode, .. } => {
                        if let Some(key) = keymap.get(&keycode.unwrap_or(Keycode::Ampersand)) {
                            controller.set_button_state(*key, false);
                        }
                    }
                    _ => { /* do nothing */ }
                }
            }
        },
    );
    let mut cpu = Cpu::new(bus);

    cpu.reset();

    let device = audio
        .open_playback(None, &audio_spec, |_spec| audio_handler)
        .unwrap();
    device.resume();

    cpu.run_with_callback(move |cpu| {
        if do_trace {
            trace(cpu);
        }
    });
}

fn trace(cpu: &mut Cpu) {
    println!(
        "{:04X}  {:02X}  {:3} {:02X} {:02X}  A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
        cpu.program_counter,
        cpu.bus.read(cpu.program_counter),
        cpu.mnemonic,
        cpu.bus.read(cpu.program_counter + 1),
        cpu.bus.read(cpu.program_counter + 2),
        cpu.register_a,
        cpu.register_x,
        cpu.register_y,
        cpu.status.0,
        cpu.stack_pointer
    );
    // println!(
    //     "{:04X}  {:02X}  A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
    //     cpu.program_counter,
    //     cpu.bus.read(cpu.program_counter),
    //     cpu.register_a,
    //     cpu.register_x,
    //     cpu.register_y,
    //     cpu.status.0,
    //     cpu.stack_pointer
    // );
}

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Must provide at least one parameter!");
        println!("  <file>         -- runs given rom");
        return;
    }

    let trace = args.contains(&"--trace".to_owned());
    let render_debug = args.contains(&"--debug".to_owned());
    let fullscreen = args.contains(&"--full".to_owned());

    run_rom(&args[1], trace, render_debug, fullscreen);
}
