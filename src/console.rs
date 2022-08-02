pub mod apu;
mod bus;
mod cartridge;
pub mod controller;
pub mod cpu;
pub mod ppu;

use eyre::Result;

use crate::emulator::Emulator;
use bus::Bus;
use cartridge::Cartridge;
use cpu::Cpu;

pub struct Console<'a> {
    cpu: Cpu<'a>,
}

pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;

impl<'a> Console<'a> {
    pub fn new(rom: &[u8], emulator: &'a mut Emulator) -> Result<Self> {
        let bus = Bus::new(Cartridge::new(rom)?, emulator);
        let cpu = Cpu::new(bus);

        Ok(Self { cpu })
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
        // todo: also reset bus with apu & ppu,
    }

    pub fn run_with_callback<F>(&mut self, callback: F) -> Result<()>
    where
        F: FnMut(&mut Cpu),
    {
        self.cpu.run_with_callback(callback)
    }
}
