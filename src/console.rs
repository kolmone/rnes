mod apu;
mod bus;
mod cartridge;
pub mod controller;
pub mod cpu;
pub mod ppu;

use std::{
    thread::yield_now,
    time::{Duration, SystemTime},
};

use crate::emulator::Emulator;
use bus::Bus;
use cartridge::Cartridge;
use controller::Controller;
use cpu::Cpu;
use ppu::Ppu;

pub struct Console<'a> {
    cpu: Cpu<'a>,
}

impl<'a> Console<'a> {
    pub fn new(fullscreen: bool, rom: Vec<u8>, emulator: &'a mut Emulator) -> Self {
        let mut expected_timestamp = SystemTime::now() + Duration::from_nanos(16666667);
        let mut _prev_timestamp = SystemTime::now();
        let bus = Bus::new(
            Cartridge::new(rom).unwrap(),
            emulator.audio_tx(),
            move |ppu: &Ppu, controller: &mut Controller| {
                if !fullscreen {
                    let mut now = SystemTime::now();
                    if now < expected_timestamp {
                        while now < expected_timestamp {
                            yield_now();
                            now = SystemTime::now();
                        }
                    } else {
                        println!("Arrived late");
                    }
                    _prev_timestamp = expected_timestamp;
                    expected_timestamp += Duration::from_nanos(16666667);
                }
                emulator.render_screen(ppu);
                emulator.handle_input(controller);
            },
        );
        let cpu = Cpu::new(bus);

        Self { cpu }
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
        // todo: also reset bus with apu & ppu,
    }

    pub fn run_with_callback<F>(&mut self, callback: F)
    where
        F: FnMut(&mut Cpu),
    {
        self.cpu.run_with_callback(callback);
    }
}
