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
use rubato::{InterpolationParameters, SincFixedIn, SincFixedOut, VecResampler};
use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    event::Event,
    keyboard::Keycode,
    pixels::PixelFormatEnum,
};
use std::{
    collections::HashMap,
    env,
    thread::yield_now,
    time::{Duration, SystemTime},
};

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

struct AudioData {
    data: Vec<f32>,
    pos: usize,
}

impl AudioCallback for AudioData {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        for sample in out.iter_mut() {
            *sample = if self.pos >= self.data.len() {
                0.0
            } else {
                let new = self.data[self.pos];
                self.pos += 1;
                new
            }
        }
    }
}

impl AudioData {
    fn fill(&mut self) {
        for (idx, sample) in self.data.iter_mut().enumerate() {
            let mut new_sample = (idx % 0x2000) as f32;
            new_sample /= 4096.0;
            *sample = new_sample - 1.0;
        }
    }
}
// 1789800

fn audio_test() {
    let sdl = sdl2::init().unwrap();
    let desired_spec = AudioSpecDesired {
        freq: Some(48000),
        channels: Some(1),
        samples: Some(1024),
    };

    let params = InterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        oversampling_factor: 128,
        interpolation: rubato::InterpolationType::Linear,
        window: rubato::WindowFunction::BlackmanHarris2,
    };
    let mut resampler =
        SincFixedIn::<f32>::new(48000.0 / 1789800.0, 1.0, params, 5 * 1789800, 1).unwrap();

    let mut data = AudioData {
        data: vec![0.0; 5 * 1789800],
        pos: 0,
    };
    data.fill();
    println!("processing data");
    let processed = resampler.process(&[data.data], None).unwrap();
    println!("data processed, len is {}", processed[0].len());
    let mut new_data = AudioData {
        data: vec![0.0; processed[0].len()],
        pos: 0,
    };
    for (idx, value) in processed[0].iter().enumerate() {
        new_data.data[idx] = *value;
    }

    let audio = sdl.audio().unwrap();
    let device = audio
        .open_playback(None, &desired_spec, |spec| {
            println!("{:?}", spec.freq);
            new_data
        })
        .unwrap();
    device.resume();
    std::thread::sleep(Duration::from_millis(5000));
}

fn run_rom(file: &str, do_trace: bool, render_debug: bool) {
    let sdl = sdl2::init().unwrap();
    let window = sdl
        .video()
        .unwrap()
        .window("N3S", 256 * 4, 240 * 4)
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

    let bus = Bus::new(
        Rom::new(rom).unwrap(),
        |ppu: &Ppu, controller: &mut Controller| {
            let mut now = SystemTime::now();
            while now < expected_timestamp {
                yield_now();
                now = SystemTime::now();
            }
            expected_timestamp += Duration::from_micros(16667);
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
                            println!("Button pressed");
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
    let args: Vec<String> = env::args().collect();

    if args.contains(&"--test".to_owned()) {
        audio_test();
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
