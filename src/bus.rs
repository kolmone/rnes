use std::fmt;

use crate::{
    controller::{self, Controller},
    ppu::Ppu,
};

pub struct Bus<'call> {
    ram: [u8; 0x800],
    prg: Vec<u8>,
    ppu: Ppu,
    cycles: usize,
    controller: Controller,

    game_callback: Box<dyn FnMut(&Ppu, &mut Controller) + 'call>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Mirroring {
    Vertical,
    Horizontal,
    FourScreen,
}

impl fmt::Display for Mirroring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mirroring::Vertical => write!(f, "Vertical"),
            Mirroring::Horizontal => write!(f, "Horizontal"),
            Mirroring::FourScreen => write!(f, "FourScreen"),
        }
    }
}

pub struct Rom {
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
    pub mapper: u8,
    pub mirroring: Mirroring,
}

const INES_TAG: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const PRG_ROM_BANK_SIZE: usize = 0x4000;
const CHR_ROM_BANK_SIZE: usize = 0x2000;
impl Rom {
    pub fn new(raw: Vec<u8>) -> Result<Rom, String> {
        if raw[0..4] != INES_TAG {
            return Err("File is not in iNES file format".to_string());
        }

        let mapper = (raw[7] & 0b1111_0000) | (raw[6] >> 4);

        let ines_ver = (raw[7] >> 2) & 0b11;
        if ines_ver != 0 {
            return Err("NES2.0 format is not supported".to_string());
        }

        let four_screen = raw[6] & 0b1000 != 0;
        let vertical_mirroring = raw[6] & 0b1 != 0;
        let screen_mirroring = match (four_screen, vertical_mirroring) {
            (true, _) => Mirroring::FourScreen,
            (false, true) => Mirroring::Vertical,
            (false, false) => Mirroring::Horizontal,
        };

        let prg_rom_size = raw[4] as usize * PRG_ROM_BANK_SIZE;
        let chr_rom_size = raw[5] as usize * CHR_ROM_BANK_SIZE;

        let skip_trainer = raw[6] & 0b100 != 0;

        let prg_rom_start = 16 + if skip_trainer { 512 } else { 0 };
        let chr_rom_start = prg_rom_start + prg_rom_size;

        let mut chr = raw[chr_rom_start..(chr_rom_start + chr_rom_size)].to_vec();
        if chr.len() < 0x2000 {
            chr.resize(0x2000, 0);
        }

        Ok(Rom {
            prg: raw[prg_rom_start..(prg_rom_start + prg_rom_size)].to_vec(),
            chr,
            mapper,
            mirroring: screen_mirroring,
        })
    }
}

const RAM_START: u16 = 0x0000;
const RAM_END: u16 = 0x1FFF;
const PPU_REGISTERS_START: u16 = 0x2000;
const PPU_REGISTERS_END: u16 = 0x3FFF;
const OAM_DMA_ADDR: u16 = 0x4014;
const ROM_START: u16 = 0x8000;
const CONTROLLER1_ADDR: u16 = 0x4016;
const CONTROLLER2_ADDR: u16 = 0x4017;

const RAM_ADDR_MIRROR_MASK: u16 = 0x07FF;

impl<'call> Bus<'call> {
    pub fn new<F>(rom: Rom, game_callback: F) -> Self
    where
        F: FnMut(&Ppu, &mut controller::Controller) + 'call,
    {
        Self {
            ram: [0; 0x800],
            prg: rom.prg,
            ppu: Ppu::new(rom.chr, rom.mirroring),
            controller: Controller::new(),
            cycles: 0,
            game_callback: Box::from(game_callback),
        }
    }

    pub fn tick(&mut self, cycles: u8) {
        self.cycles += cycles as usize;
        for _ in 0..3 * cycles {
            if self.ppu.tick() {
                (self.game_callback)(&self.ppu, &mut self.controller);
            }
        }
    }

    pub fn get_nmi_state(&mut self) -> bool {
        self.ppu.nmi_up
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            RAM_START..=RAM_END => self.ram[(addr & RAM_ADDR_MIRROR_MASK) as usize],
            PPU_REGISTERS_START..=PPU_REGISTERS_END => self.ppu.read(addr),
            ROM_START.. => self.read_prg(addr),
            CONTROLLER1_ADDR => self.controller.read(),
            CONTROLLER2_ADDR => 0,
            _ => {
                println!("Read from unknown address 0x{:X}", addr);
                0
            }
        }
    }

    pub fn read_u16(&mut self, addr: u16) -> u16 {
        let lsb = self.read(addr) as u16;
        let msb = self.read(addr.wrapping_add(1)) as u16;
        (msb << 8) | lsb
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            RAM_START..=RAM_END => self.ram[(addr & RAM_ADDR_MIRROR_MASK) as usize] = data,
            PPU_REGISTERS_START..=PPU_REGISTERS_END => self.ppu.write(addr, data),
            OAM_DMA_ADDR => self.oam_dma(data),
            ROM_START.. => println!("Write to ROM space at address 0x{:X}", addr),
            CONTROLLER1_ADDR => self.controller.write(data),
            // _ => (),
            _ => println!("Write to unknown address 0x{:X}", addr),
        }
    }

    pub fn write_u16(&mut self, addr: u16, data: u16) {
        let msb = (data >> 8) as u8;
        let lsb = (data & 0xff) as u8;
        self.write(addr, lsb);
        self.write(addr.wrapping_add(1), msb);
    }

    fn read_prg(&self, mut addr: u16) -> u8 {
        addr -= ROM_START;
        if self.prg.len() == 0x4000 {
            addr %= 0x4000;
        }
        self.prg[addr as usize]
    }

    fn oam_dma(&mut self, page: u8) {
        println!("Performing OAM DMA to address {:x}", self.ppu.oam_addr);
        let start_addr = (page as u16) << 8;
        for i in 0..256 {
            let oam_data = self.read(start_addr + i);
            self.write(0x2004, oam_data);
            self.tick(2);
        }
        self.tick(1);
    }
}

mod cartridge {
    pub struct Cartridge {
        prg_rom: Vec<u8>,
        prg_ram: Vec<u8>,
        chr_rom: Vec<u8>,
        chr_ram: Vec<u8>,
        mapper: u8,
    }
}
