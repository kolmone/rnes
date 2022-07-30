#![warn(trivial_numeric_casts)]
#![allow(clippy::bad_bit_mask)]

mod console;
mod emulator;
mod renderer;

use console::cpu::Cpu;
use console::ppu::Ppu;
use std::env;

// 21441960 / 12 = 1786830 - if NES ran at exactly 60 Hz
// const MAIN_FREQ: usize = 21441960;
const MAIN_FREQ: usize = 21442080; // 89342 PPU cycles * 60 * 4
const CPU_FREQ: usize = MAIN_FREQ / 12;
const APU_FREQ: usize = CPU_FREQ;
const _PPU_FREQ: usize = MAIN_FREQ / 4;

fn run_rom(file: &str, do_trace: bool, fullscreen: bool) {
    let rom: Vec<u8> = std::fs::read(file).expect("Unable to open rom file!");

    let mut emulator = emulator::Emulator::new(fullscreen);
    let mut console = console::Console::new(fullscreen, rom, &mut emulator);

    console.reset();

    console.run_with_callback(move |cpu| {
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
    let fullscreen = args.contains(&"--fs".to_owned());

    run_rom(&args[1], trace, fullscreen);
}
