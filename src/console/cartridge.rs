pub mod mappers;

use eyre::eyre;
use eyre::Result;

use mappers::{get_mapper, Mapper, Mirroring};

pub struct Cartridge {
    pub mapper: Box<dyn Mapper>,
}

impl Cartridge {
    const INES_TAG: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
    const PRG_ROM_BANK_SIZE: usize = 0x4000;
    const CHR_ROM_BANK_SIZE: usize = 0x2000;
    const _PRG_RAM_BANK_SIZE: usize = 0x2000;
    const CHR_RAM_BANK_SIZE: usize = 0x2000;

    pub fn new(rom: &[u8]) -> Result<Self> {
        if rom[0..4] != Self::INES_TAG {
            return Err(eyre!("File is not in iNES file format"));
        }

        let ines_ver = (rom[7] >> 2) & 0b11;
        if ines_ver != 0 {
            return Err(eyre!("NES2.0 format is not supported (for now)"));
        }

        let mapper = (rom[7] & 0xF0) | (rom[6] >> 4);
        let four_screen = rom[6] & 0b1000 != 0;
        let vertical_mirroring = rom[6] & 0b1 != 0;
        let mirroring = match (four_screen, vertical_mirroring) {
            (true, _) => Mirroring::FourScreen,
            (false, true) => Mirroring::Vertical,
            (false, false) => Mirroring::Horizontal,
        };

        let skip_trainer = rom[6] & 0b100 != 0;

        let prg_rom_start = 16 + if skip_trainer { 512 } else { 0 };
        let prg_rom_len = rom[4] as usize * Self::PRG_ROM_BANK_SIZE;
        let prg_rom = rom[prg_rom_start..(prg_rom_start + prg_rom_len)].to_vec();

        let chr_rom_start = prg_rom_start + prg_rom_len;
        let chr_rom_len = rom[5] as usize * Self::CHR_ROM_BANK_SIZE;
        let chr_rom = rom[chr_rom_start..(chr_rom_start + chr_rom_len)].to_vec();

        let mapper = get_mapper(
            mapper,
            prg_rom,
            chr_rom,
            (chr_rom_len == 0) as usize * Self::CHR_RAM_BANK_SIZE,
            mirroring,
        )?;

        Ok(Self { mapper })
    }

    pub fn read_cpu(&mut self, addr: u16) -> u8 {
        self.mapper.read_cpu(addr)
    }

    pub fn write_cpu(&mut self, addr: u16, data: u8) {
        self.mapper.write_cpu(addr, data);
    }

    pub fn read_ppu(&mut self, addr: u16) -> u8 {
        self.mapper.read_ppu(addr)
    }

    pub fn write_ppu(&mut self, addr: u16, data: u8) {
        self.mapper.write_ppu(addr, data);
    }

    pub fn mirror_vram_addr(&mut self, addr: u16) -> usize {
        self.mapper.mirror_vram(addr)
    }

    pub fn irq_active(&self) -> bool {
        self.mapper.irq_active()
    }
}
