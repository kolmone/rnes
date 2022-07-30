use crate::emulator::Emulator;

use super::{apu::Apu, cartridge::Cartridge, controller::Controller, ppu::Ppu};
use eyre::Result;

pub struct Bus<'a> {
    ram: [u8; 0x800],
    ppu: Ppu,
    apu: Apu,
    cycles: usize,
    controller: Controller,
    cartridge: Cartridge,

    emulator: &'a mut Emulator,
}

const RAM_START: u16 = 0x0000;
const RAM_END: u16 = 0x1FFF;
const PPU_REGISTERS_START: u16 = 0x2000;
const PPU_REGISTERS_END: u16 = 0x3FFF;
const OAM_DMA_ADDR: u16 = 0x4014;
const CONTROLLER1_ADDR: u16 = 0x4016;
const CONTROLLER2_ADDR: u16 = 0x4017;

const RAM_ADDR_MIRROR_MASK: u16 = 0x07FF;

impl<'a> Bus<'a> {
    pub fn new(cartridge: Cartridge, emulator: &'a mut Emulator) -> Self {
        Self {
            ram: [0; 0x800],
            ppu: Ppu::new(),
            apu: Apu::new(),
            controller: Controller::new(),
            cycles: 0,
            cartridge,
            emulator,
        }
    }

    pub fn tick(&mut self, cycles: u8) -> Result<()> {
        self.cycles += cycles as usize;
        for _ in 0..cycles {
            if self.apu.tick(&mut self.cartridge) {
                self.emulator.handle_audio(&self.apu);
            }
        }
        for _ in 0..3 * cycles {
            if self.ppu.tick(&mut self.cartridge) {
                self.emulator.render_screen(&self.ppu)?;
                self.emulator.handle_input(&mut self.controller);
            }
        }
        Ok(())
    }

    pub fn nmi_active(&mut self) -> bool {
        self.ppu.nmi_up
    }

    pub fn irq_active(&mut self) -> bool {
        self.cartridge.irq_active() | self.apu.irq_active()
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            RAM_START..=RAM_END => self.ram[(addr & RAM_ADDR_MIRROR_MASK) as usize],
            PPU_REGISTERS_START..=PPU_REGISTERS_END => self.ppu.read(addr, &mut self.cartridge),
            CONTROLLER1_ADDR => self.controller.read(),
            CONTROLLER2_ADDR => 0,
            0x4000..=0x4017 => self.apu.read(addr),

            0x4020.. => self.cartridge.read_cpu(addr),

            _ => {
                println!("Read from unknown address 0x{:X}", addr);
                0
            }
        }
    }

    pub fn read_u16(&mut self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi = self.read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    pub fn write(&mut self, addr: u16, data: u8) -> Result<()> {
        match addr {
            RAM_START..=RAM_END => self.ram[(addr & RAM_ADDR_MIRROR_MASK) as usize] = data,
            PPU_REGISTERS_START..=PPU_REGISTERS_END => {
                self.ppu.write(addr, data, &mut self.cartridge);
            }

            OAM_DMA_ADDR => self.oam_dma(data)?,
            CONTROLLER1_ADDR => self.controller.write(data),
            0x4000..=0x4017 => self.apu.write(addr, data),

            0x4020.. => self.cartridge.write_cpu(addr, data),

            _ => println!("Write to unknown address 0x{:X}", addr),
        }
        Ok(())
    }

    fn oam_dma(&mut self, page: u8) -> Result<()> {
        // println!("Performing OAM DMA to address {:x}", self.ppu.oam_addr);
        let start_addr = (page as u16) << 8;
        for i in 0..256 {
            let oam_data = self.read(start_addr + i);
            self.write(0x2004, oam_data)?;
            self.tick(2)?;
        }
        self.tick(1)
    }
}
