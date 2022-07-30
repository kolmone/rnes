use eyre::eyre;
use eyre::Result;

pub enum MapperEvent {}

pub enum Mirroring {
    Vertical,
    Horizontal,
    FourScreen,
    SingleScreenLower,
    SingleScreenUpper,
}

impl Default for Mirroring {
    fn default() -> Self {
        Self::Vertical
    }
}

pub trait Mapper {
    fn read_cpu(&mut self, addr: u16) -> u8;
    fn write_cpu(&mut self, addr: u16, data: u8);
    fn read_ppu(&mut self, addr: u16) -> u8;
    fn write_ppu(&mut self, addr: u16, data: u8);
    fn mirror_vram(&self, addr: u16) -> usize;

    fn trigger_event(&mut self, _event: MapperEvent) {
        todo!("No cartridge event support yet")
    }

    fn irq_active(&self) -> bool {
        false
    }
}

pub fn get_mapper(
    mapper: u8,
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_ram_size: usize,
    mirroring: Mirroring,
) -> Result<Box<dyn Mapper>> {
    println!("Using mapper {}", mapper);

    match mapper {
        0 => Ok(Box::new(Mapper000::new(
            prg_rom,
            chr_rom,
            chr_ram_size,
            mirroring,
        ))),
        1 => Ok(Box::new(Mapper001::new(
            &prg_rom,
            &chr_rom,
            chr_ram_size,
            mirroring,
        ))),
        _ => Err(eyre!("Unsupported mapper {}", mapper)),
    }
}

// Horizontal mirroring - first two 1kB areas map to first 1kB of VRAM
const fn mirror_horizontal(addr: u16) -> usize {
    if addr & 0x800 == 0 {
        ((addr & !0x400) % 0x800) as usize
    } else {
        ((addr | 0x400) % 0x800) as usize
    }
}

// Vertical mirroring - first and third 1kB areas map to first 1kB of VRAM
// Just fold address down to 0x0 - 0x7FF
const fn mirror_vertical(addr: u16) -> usize {
    (addr % 0x800) as usize
}

// Single screen mirroring is just the selected half of the memory
const fn mirror_single(addr: u16, screen_b: bool) -> usize {
    if screen_b {
        (addr % 0x400 + 0x400) as usize
    } else {
        (addr % 0x400) as usize
    }
}

pub struct Mapper000 {
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    mirroring: Mirroring,
}

impl Mapper000 {
    fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>, chr_ram_size: usize, mirroring: Mirroring) -> Self {
        Self {
            prg_rom,
            chr_rom,
            prg_ram: vec![0; 0x2000],
            chr_ram: vec![0; chr_ram_size],
            mirroring,
        }
    }
}

impl Mapper for Mapper000 {
    fn trigger_event(&mut self, _event: MapperEvent) {
        todo!("No cartridge event support yet")
    }

    fn read_cpu(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize],
            0x8000.. => self.prg_rom[(addr - 0x8000) as usize % self.prg_rom.len()],
            _ => 0,
        }
    }

    fn write_cpu(&mut self, addr: u16, data: u8) {
        if let 0x6000..=0x7FFF = addr {
            self.prg_ram[(addr - 0x6000) as usize] = data;
        }
    }

    fn read_ppu(&mut self, addr: u16) -> u8 {
        let use_chr_ram = !self.chr_ram.is_empty();

        match addr {
            0..=0x1FFF if use_chr_ram => self.chr_ram[addr as usize],
            0..=0x1FFF => self.chr_rom[addr as usize],
            _ => panic!("PPU reading from address {:X}", addr),
        }
    }

    fn write_ppu(&mut self, addr: u16, data: u8) {
        let use_chr_ram = !self.chr_ram.is_empty();

        match addr {
            0..=0x1FFF if use_chr_ram => self.chr_ram[addr as usize] = data,
            0..=0x1FFF => self.chr_rom[addr as usize] = data,
            _ => panic!("PPU writing to address {:X}", addr),
        }
    }

    /// Translates given VRAM address to actual VRAM location
    /// This includes removing address offset and mirroring based on current mirroring scheme
    fn mirror_vram(&self, addr: u16) -> usize {
        match self.mirroring {
            Mirroring::Vertical => mirror_vertical(addr),
            Mirroring::Horizontal => mirror_horizontal(addr),
            _ => panic!("Unsupported mirroring mode for mappper 000!"),
        }
    }
}

pub struct Mapper001 {
    prg_banks: Vec<Vec<u8>>,
    prg_ram_banks: Vec<Vec<u8>>,
    chr_banks: Vec<Vec<u8>>,
    mirroring: Mirroring,

    buffer: usize,
    bit_idx: usize,

    prg_bank0: usize,
    prg_bank1: usize,
    prg_ram_bank: usize,
    chr_bank0: usize,
    chr_bank1: usize,

    chr_independent_banks: bool,
    prg_mode: Mapper001PrgMode,
}

#[derive(PartialEq)]
enum Mapper001PrgMode {
    SwitchBoth,
    FixFirst,
    FixLast,
}

impl Mapper001 {
    const PRG_ROM_BANK_SIZE: usize = 16 * 1024;
    const CHR_ROM_BANK_SIZE: usize = 8 * 1024;
    const PRG_RAM_BANK_SIZE: usize = 8 * 1024;
    const PRG_RAM_BANKS: usize = 4;

    fn new(prg_rom: &[u8], chr_rom: &[u8], _chr_ram_size: usize, mirroring: Mirroring) -> Self {
        let prg_banks = prg_rom
            .chunks(Self::PRG_ROM_BANK_SIZE)
            .map(<[u8]>::to_vec)
            .collect();

        let mut chr_banks = chr_rom
            .chunks(Self::CHR_ROM_BANK_SIZE)
            .map(<[u8]>::to_vec)
            .collect::<Vec<Vec<u8>>>();

        if chr_banks.is_empty() {
            chr_banks = vec![vec![0; Self::CHR_ROM_BANK_SIZE]; 16];
        }

        Self {
            prg_banks,
            chr_banks,
            prg_ram_banks: vec![vec![0; Self::PRG_RAM_BANK_SIZE]; Self::PRG_RAM_BANKS],
            mirroring,
            buffer: 0,
            bit_idx: 0,
            prg_bank0: 0,
            prg_bank1: 1,
            prg_ram_bank: 0,
            chr_bank0: 0,
            chr_bank1: 1,
            chr_independent_banks: false,
            prg_mode: Mapper001PrgMode::FixLast,
        }
    }

    fn store_buffer(&mut self, addr: u16) {
        // println!("Writing 0b{:b} to {:X}", self.buffer, addr);
        match addr {
            0x8000..=0x9FFF => self.write_control(self.buffer),
            0xA000..=0xBFFF if !self.chr_independent_banks => self.chr_bank0 = self.buffer & 0x1E,
            0xA000..=0xBFFF => self.chr_bank0 = self.buffer,
            0xC000..=0xDFFF => self.chr_bank1 = self.buffer,
            0xE000..=0xFFFF => {
                let bank = self.buffer & 0xF;
                match self.prg_mode {
                    Mapper001PrgMode::SwitchBoth => self.prg_bank0 = bank & 0xE,
                    Mapper001PrgMode::FixFirst => self.prg_bank1 = bank,
                    Mapper001PrgMode::FixLast => self.prg_bank0 = bank,
                }
            }
            _ => unreachable!(),
        }
    }

    fn write_control(&mut self, data: usize) {
        self.mirroring = match data & 0x3 {
            0 => Mirroring::SingleScreenLower,
            1 => Mirroring::SingleScreenUpper,
            2 => Mirroring::Vertical,
            3 => Mirroring::Horizontal,
            _ => unreachable!(),
        };
        self.prg_mode = match (data >> 2) & 0x3 {
            0 | 1 => Mapper001PrgMode::SwitchBoth,
            2 => Mapper001PrgMode::FixFirst,
            3 => Mapper001PrgMode::FixLast,
            _ => unreachable!(),
        };
        self.chr_independent_banks = data & 0x10 != 0;
    }

    fn get_chr_ref(&mut self, addr: u16) -> &mut u8 {
        let idx = addr as usize % Self::CHR_ROM_BANK_SIZE;
        let bank = addr as usize / Self::CHR_ROM_BANK_SIZE;
        let banks = self.chr_banks.len();
        if bank == 0 {
            &mut self.chr_banks[self.chr_bank0 % banks][idx]
        } else if !self.chr_independent_banks {
            &mut self.chr_banks[(self.chr_bank0 + 1) % banks][idx]
        } else {
            &mut self.chr_banks[self.chr_bank1 % banks][idx]
        }
    }

    fn get_prg_ref(&mut self, addr: u16) -> &mut u8 {
        let idx = addr as usize % Self::PRG_ROM_BANK_SIZE;
        let bank = (addr - 0x8000) as usize / Self::PRG_ROM_BANK_SIZE;
        let banks = self.prg_banks.len();

        if bank == 0 && self.prg_mode == Mapper001PrgMode::FixFirst {
            &mut self.prg_banks[0][idx]
        } else if bank == 0 {
            &mut self.prg_banks[self.prg_bank0 % banks][idx]
        } else if self.prg_mode == Mapper001PrgMode::SwitchBoth {
            &mut self.prg_banks[(self.prg_bank0 + 1) % banks][idx]
        } else if self.prg_mode == Mapper001PrgMode::FixLast {
            &mut self.prg_banks[banks - 1][idx]
        } else {
            &mut self.prg_banks[self.prg_bank1 % banks][idx]
        }
    }
}

impl Mapper for Mapper001 {
    fn trigger_event(&mut self, _event: MapperEvent) {
        todo!("No cartridge event support yet")
    }

    fn read_cpu(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                self.prg_ram_banks[self.prg_ram_bank][(addr as usize) % Self::PRG_RAM_BANK_SIZE]
            }
            0x8000.. => *self.get_prg_ref(addr),
            _ => panic!("Unexpected CPU read from address {:X}", addr),
        }
    }

    fn write_cpu(&mut self, addr: u16, data: u8) {
        // println!("Write {:X} to mapper address {:X}", data, addr);
        match addr {
            0x6000..=0x7FFF => {
                self.prg_ram_banks[self.prg_ram_bank][(addr as usize) % Self::PRG_RAM_BANK_SIZE] =
                    data;
            }
            0x8000.. => {
                if data & 0x80 == 0 {
                    self.buffer |= (data as usize & 0x01) << self.bit_idx;
                    self.bit_idx += 1;
                    if self.bit_idx == 5 {
                        self.store_buffer(addr);
                        self.bit_idx = 0;
                        self.buffer = 0;
                    }
                } else {
                    self.bit_idx = 0;
                    self.prg_mode = Mapper001PrgMode::FixLast;
                }
            }
            _ => panic!("Unexpected CPU read from address {:X}", addr),
        }
    }

    fn read_ppu(&mut self, addr: u16) -> u8 {
        match addr {
            0..=0x1FFF => *self.get_chr_ref(addr),
            _ => panic!("PPU reading from address {:X}", addr),
        }
    }

    fn write_ppu(&mut self, addr: u16, data: u8) {
        match addr {
            0..=0x1FFF => *self.get_chr_ref(addr) = data,
            _ => panic!("PPU writing to address {:X}", addr),
        }
    }

    fn mirror_vram(&self, addr: u16) -> usize {
        match self.mirroring {
            Mirroring::Vertical => mirror_vertical(addr),
            Mirroring::Horizontal => mirror_horizontal(addr),
            Mirroring::SingleScreenLower => mirror_single(addr, false),
            Mirroring::SingleScreenUpper => mirror_single(addr, true),
            Mirroring::FourScreen => panic!("Unsupported mirroring for Mapper001"),
        }
    }
}
