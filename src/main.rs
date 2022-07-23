#![warn(trivial_numeric_casts)]
#![allow(clippy::bad_bit_mask)]

mod apu;
mod bus;
mod controller;
mod cpu;
mod ppu;
mod renderer;

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
    f32::consts::PI,
    thread::yield_now,
    time::{Duration, SystemTime},
};

// 21441960 / 12 = 1786830 - if NES ran at exactly 60 Hz
const MAIN_FREQ: usize = 21441960;
const CPU_FREQ: usize = MAIN_FREQ / 12;
const APU_FREQ: usize = CPU_FREQ;
const PPU_FREQ: usize = MAIN_FREQ / 4;
const CPU_FREQF: f64 = CPU_FREQ as f64;

fn build_keymap() -> HashMap<Keycode, Button> {
    let mut keymap = HashMap::new();
    keymap.insert(Keycode::Down, Button::Down);
    keymap.insert(Keycode::Up, Button::Up);
    keymap.insert(Keycode::Right, Button::Right);
    keymap.insert(Keycode::Left, Button::Left);
    keymap.insert(Keycode::Backspace, Button::Select);
    keymap.insert(Keycode::Return, Button::Start);
    keymap.insert(Keycode::X, Button::A);
    keymap.insert(Keycode::Z, Button::B);
    keymap
}

struct AudioHandler {
    input_data: Vec<Vec<f32>>,
    output_data: Vec<Vec<f32>>,
    resampler: FftFixedOut<f32>,
}

impl AudioCallback for AudioHandler {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        println!("{}", self.resampler.input_frames_next());
        match self.resampler.process_into_buffer(
            &self.input_data,
            &mut self.output_data,
            Some(&[true; 1]),
        ) {
            Ok(()) => out.clone_from_slice(&self.output_data[0]),
            Err(error) => panic!("{:?}", error),
        }
    }
}

impl AudioHandler {
    fn new(out_freq: usize, buffer_len: usize) -> Self {
        let resampler = FftFixedOut::<f32>::new(APU_FREQ, out_freq, buffer_len, 4, 1).unwrap();
        AudioHandler {
            input_data: vec![vec![0.0; 0]; 1],
            output_data: vec![vec![0.0; buffer_len]; 1],
            resampler,
        }
    }

    // fn fill(&mut self) {
    //     for (idx, sample) in self.data.iter_mut().enumerate() {
    //         let pos = 880.0 * (idx as f32) * 2.0 * PI / 1789800.0;
    //         *sample = pos.sin();
    //     }
    // }
}

fn audio_test() -> Result<(), String> {
    let freq = 48000;
    let buffer_len = 1024;

    let sdl = sdl2::init().unwrap();
    let desired_spec = AudioSpecDesired {
        freq: Some(freq),
        channels: Some(1),
        samples: Some(buffer_len),
    };
    let audio = sdl.audio()?;

    let audio_handler = AudioHandler::new(freq as usize, buffer_len as usize);

    // Play resampled data
    let device = audio.open_playback(None, &desired_spec, |_| audio_handler)?;
    device.resume();
    std::thread::sleep(Duration::from_secs(5));

    Ok(())
}

fn run_rom(file: &str, do_trace: bool, render_debug: bool) {
    let sdl = sdl2::init().unwrap();
    let window = sdl
        .video()
        .unwrap()
        .window("N3S", 256 * 3, 240 * 3)
        .position_centered()
        .build()
        .unwrap();

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
        |ppu: &Ppu, controller: &mut Controller| {
            let mut now = SystemTime::now();
            if now < expected_timestamp {
                println!(
                    "Frame done in {:?}!",
                    now.duration_since(prev_timestamp).unwrap()
                );
                while now < expected_timestamp {
                    yield_now();
                    now = SystemTime::now();
                }
            } else {
                // println!("Arrived late");
            }
            prev_timestamp = expected_timestamp;
            expected_timestamp += Duration::from_nanos(16666667);
            // println!(
            //     "Current time is {:?}\nNext systemtime is {:?}",
            //     now, expected_timestamp
            // );

            renderer.render_screen(ppu, &mut canvas, &mut texture, render_debug);

            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => std::process::exit(0),
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

    if args.contains(&"--test".to_owned()) {
        audio_test().unwrap();
        panic!("Sound test done!");
    }

    if args.len() < 2 {
        println!("Must provide at least one parameter!");
        println!("  <file>         -- runs given rom");
        return;
    }

    let trace = args.contains(&"--trace".to_owned());
    let render_debug = args.contains(&"--debug".to_owned());

    run_rom(&args[1], trace, render_debug);
}
