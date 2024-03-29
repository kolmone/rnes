#![warn(trivial_numeric_casts)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::bad_bit_mask)]

mod console;
mod emulator;

use console::cpu::Cpu;
use console::ppu::Ppu;
use eyre::Context;
use eyre::Result;
use std::env;

mod macros {
    macro_rules! bit_bool {
        ($value:ident, $bit:literal) => {
            ($value >> $bit) & 0x1 == 1
        };
    }
    macro_rules! bool_u8 {
        ($value:expr, $bit:literal) => {
            (($value as u8) << $bit)
        };
    }

    macro_rules! fw_error {
        ( $x:expr ) => {
            match $x {
                Ok(v) => v,
                Err(e) => return Err(eyre!(e)),
            }
        };
    }

    pub(crate) use bit_bool;
    pub(crate) use bool_u8;
    pub(crate) use fw_error;
}

// 21441960 / 12 = 1786830 - if NES ran at exactly 60 Hz
// const MAIN_FREQ: usize = 21441960;
const MAIN_FREQ: usize = 21_442_080; // 89342 PPU cycles * 60 * 4
const CPU_FREQ: usize = MAIN_FREQ / 12;
const APU_FREQ: usize = CPU_FREQ;
const _PPU_FREQ: usize = MAIN_FREQ / 4;

fn run_rom(file: &str, do_trace: bool, fullscreen: bool) -> Result<()> {
    let rom: Vec<u8> =
        std::fs::read(file).wrap_err_with(|| format!("Failed to open ROM file {}", file))?;

    let mut emulator = emulator::Emulator::new(fullscreen)?;
    let mut console = console::Console::new(&rom, &mut emulator)?;

    console.run_with_callback(move |cpu| {
        if do_trace {
            trace(cpu);
        }
    })
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
        u8::from(cpu.status),
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

fn main() -> Result<()> {
    env_logger::init();
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Must provide at least one parameter!");
        println!("  <file>         -- runs given rom");
        return Ok(());
    }

    let trace = args.contains(&"--trace".to_owned());
    let fullscreen = args.contains(&"--fs".to_owned());

    run_rom(&args[1], trace, fullscreen)?;
    Ok(())
}
