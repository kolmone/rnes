#![warn(trivial_numeric_casts)]

mod bus;
mod controller;
mod cpu;
mod ppu;
mod renderer;

use bus::{Bus, Rom};
use controller::{Button, Controller};
use core::panic;
use cpu::Cpu;
use ppu::Ppu;
use rand::Rng;
use renderer::Renderer;
use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::{Color, PixelFormatEnum},
    EventPump,
};
use std::{
    collections::HashMap,
    env,
    thread::sleep,
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
    return keymap;
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
            while SystemTime::now() < expected_timestamp {}
            expected_timestamp = SystemTime::now() + Duration::from_nanos(63613);

            renderer.render_line(&ppu, &mut canvas, &mut texture, render_debug);

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

fn handle_user_input(cpu: &mut Cpu, event_pump: &mut EventPump) {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. }
            // | Event::KeyDown {
            //     keycode: Some(Keycode::Escape),
            //     ..
            // } => std::process::exit(0),
            => std::process::exit(0),
            Event::KeyDown {
                keycode: Some(Keycode::W),
                ..
            } => {
                cpu.bus.write(0xff, 0x77);
            }
            Event::KeyDown {
                keycode: Some(Keycode::S),
                ..
            } => {
                cpu.bus.write(0xff, 0x73);
            }
            Event::KeyDown {
                keycode: Some(Keycode::A),
                ..
            } => {
                cpu.bus.write(0xff, 0x61);
            }
            Event::KeyDown {
                keycode: Some(Keycode::D),
                ..
            } => {
                cpu.bus.write(0xff, 0x64);
            }
            _ => { /* do nothing */ }
        }
    }
}

fn color(byte: u8) -> Color {
    match byte {
        0 => sdl2::pixels::Color::BLACK,
        1 => sdl2::pixels::Color::WHITE,
        2 | 9 => sdl2::pixels::Color::GREY,
        3 | 10 => sdl2::pixels::Color::RED,
        4 | 11 => sdl2::pixels::Color::GREEN,
        5 | 12 => sdl2::pixels::Color::BLUE,
        6 | 13 => sdl2::pixels::Color::MAGENTA,
        7 | 14 => sdl2::pixels::Color::YELLOW,
        _ => sdl2::pixels::Color::CYAN,
    }
}

fn read_screen_state(cpu: &mut Cpu, frame: &mut [u8; 32 * 3 * 32]) -> bool {
    let mut frame_idx = 0;
    let mut update = false;
    for i in 0x0200..0x600 {
        let color_idx = cpu.bus.read(i as u16);
        let (b1, b2, b3) = color(color_idx).rgb();
        if frame[frame_idx] != b1 || frame[frame_idx + 1] != b2 || frame[frame_idx + 2] != b3 {
            frame[frame_idx] = b1;
            frame[frame_idx + 1] = b2;
            frame[frame_idx + 2] = b3;
            update = true;
        }
        frame_idx += 3;
    }
    update
}

fn run_snake() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("N3S", (32 * 10) as u32, (32 * 10) as u32)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(10.0, 10.0).unwrap();

    let tex_creator = canvas.texture_creator();
    let mut texture = tex_creator
        .create_texture_target(PixelFormatEnum::RGB24, 32, 32)
        .unwrap();

    let mut screen_state = [0; 32 * 3 * 32];
    let mut rng = rand::thread_rng();

    // let mut renderer = Renderer::new();
    let rom: Vec<u8> = std::fs::read("snake.nes").expect("Unable to open snake.nes");
    let bus = Bus::new(Rom::new(rom).unwrap(), |_, _| ());
    let mut cpu = Cpu::new(bus);
    cpu.reset();

    cpu.run_with_callback(move |cpu| {
        // trace(cpu);
        handle_user_input(cpu, &mut event_pump);
        cpu.bus.write(0x00FE, rng.gen_range(1..16));

        if read_screen_state(cpu, &mut screen_state) {
            texture.update(None, &screen_state, 32 * 3).unwrap();
            canvas.copy(&texture, None, None).unwrap();
            canvas.present();
        }

        sleep(Duration::new(0, 70_000));
    });
}

fn trace(cpu: &mut Cpu) {
    // println!(
    //     "{:04X}  {:02X}  {:3} {:02X} {:02X}  A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
    //     cpu.program_counter,
    //     cpu.bus.read(cpu.program_counter),
    //     cpu.mnemonic,
    //     cpu.bus.read(cpu.program_counter + 1),
    //     cpu.bus.read(cpu.program_counter + 2),
    //     cpu.register_a,
    //     cpu.register_x,
    //     cpu.register_y,
    //     cpu.status.0,
    //     cpu.stack_pointer
    // );
    println!(
        "{:04X}  {:02X}  A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
        cpu.program_counter,
        cpu.bus.read(cpu.program_counter),
        cpu.register_a,
        cpu.register_x,
        cpu.register_y,
        cpu.status.0,
        cpu.stack_pointer
    );
}

fn run_nestest() {
    let rom: Vec<u8> = std::fs::read("nestest.nes").expect("Unable to open nestest.nes");
    let bus = Bus::new(Rom::new(rom).unwrap(), |_, _| ());
    let mut cpu = Cpu::new(bus);
    cpu.reset();
    cpu.program_counter = 0xc000;
    cpu.run_with_callback(move |cpu| {
        trace(cpu);
        if cpu.bus.read(0x0002) != 0 {
            panic!("Tests failed with code {:x} in 0x02", cpu.bus.read(0x0002));
        }
        if cpu.bus.read(0x0003) != 0 {
            panic!("Tests failed with code {:x} in 0x03", cpu.bus.read(0x0003));
        }
    });
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Must provide at least one parameter! Try:");
        println!("  snake          -- runs a snake game");
        println!("  nestest        -- runs nestest rom");
        println!("  <file>         -- runs given rom");
        return;
    }

    let trace = args.contains(&"--trace".to_owned());
    let render_debug = args.contains(&"--debug".to_owned());

    if args[1] == "snake".to_owned() {
        run_snake();
    } else if args[1] == "nestest".to_owned() {
        run_nestest();
    } else {
        run_rom(&args[1], trace, render_debug);
    }
}
