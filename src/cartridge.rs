pub struct Cartridge {
    mapper: Box<dyn Mapper>,
}

#[derive(PartialEq)]
pub enum Mirroring {
    Vertical,
    Horizontal,
    FourScreen,
}

impl Cartridge {
    const INES_TAG: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
    const PRG_ROM_BANK_SIZE: usize = 0x4000;
    const CHR_ROM_BANK_SIZE: usize = 0x2000;
    const PRG_RAM_BANK_SIZE: usize = 0x2000;
    const CHR_RAM_BANK_SIZE: usize = 0x2000;

    pub fn new(raw: Vec<u8>) -> Result<Self, String> {
        if raw[0..4] != Self::INES_TAG {
            return Err("File is not in iNES file format".to_string());
        }

        let mapper = (raw[7] & 0b1111_0000) | (raw[6] >> 4);
        if mapper != 0 {
            panic!("Unsupported mapper {}", mapper);
        }

        let ines_ver = (raw[7] >> 2) & 0b11;
        if ines_ver != 0 {
            return Err("NES2.0 format is not supported (for now)".to_string());
        }

        let four_screen = raw[6] & 0b1000 != 0;
        let vertical_mirroring = raw[6] & 0b1 != 0;
        let mirroring = match (four_screen, vertical_mirroring) {
            (true, _) => Mirroring::FourScreen,
            (false, true) => Mirroring::Vertical,
            (false, false) => Mirroring::Horizontal,
        };

        let prg_rom_size = raw[4] as usize * Self::PRG_ROM_BANK_SIZE;
        let chr_rom_size = raw[5] as usize * Self::CHR_ROM_BANK_SIZE;

        let skip_trainer = raw[6] & 0b100 != 0;

        let prg_rom_start = 16 + if skip_trainer { 512 } else { 0 };
        let chr_rom_start = prg_rom_start + prg_rom_size;

        let chr_rom = raw[chr_rom_start..(chr_rom_start + chr_rom_size)].to_vec();
        let chr_ram_size = (chr_rom_size == 0) as usize * Self::CHR_RAM_BANK_SIZE;

        let prg_rom = raw[prg_rom_start..(prg_rom_start + prg_rom_size)].to_vec();

        let mapper = Mapper000 {
            prg_rom,
            prg_ram: vec![0; Self::PRG_RAM_BANK_SIZE],
            chr_rom,
            chr_ram: vec![0; chr_ram_size],
            mirroring,
        };

        Ok(Cartridge {
            mapper: Box::new(mapper),
        })
    }

    pub fn read_cpu(&mut self, addr: u16) -> u8 {
        self.mapper.read_cpu(addr)
    }

    pub fn write_cpu(&mut self, addr: u16, data: u8) {
        self.mapper.write_cpu(addr, data)
    }

    pub fn read_ppu(&mut self, addr: u16) -> u8 {
        self.mapper.read_ppu(addr)
    }

    pub fn write_ppu(&mut self, addr: u16, data: u8) {
        self.mapper.write_ppu(addr, data)
    }

    pub fn mirror_vram_addr(&mut self, addr: u16) -> usize {
        self.mapper.mirror_vram(addr)
    }
}

enum MapperEvent {}

trait Mapper {
    fn read_cpu(&mut self, addr: u16) -> u8;
    fn write_cpu(&mut self, addr: u16, data: u8);
    fn read_ppu(&mut self, addr: u16) -> u8;
    fn write_ppu(&mut self, addr: u16, data: u8);
    fn mirror_vram(&self, addr: u16) -> usize;
    fn trigger_event(&mut self, event: MapperEvent);
}

struct Mapper000 {
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    mirroring: Mirroring,
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
            self.prg_ram[(addr - 0x6000) as usize] = data
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
        // Horizontal mirroring - first two 1kB areas map to first 1kB screen
        // Vertical mirroring - first and third 1kB areas map to first 1kB screen
        let mirror_half = addr & 0x2FFF; // 0x2000-0x3f00 -> 0x2000-0x3000
        let vram_idx = mirror_half - 0x2000; // 0x2000-0x3000 -> 0x0000-0x1000
        let table = vram_idx / 0x400; // Index of 0x400 sized table
        match (&self.mirroring, table) {
            (Mirroring::Vertical, 2 | 3) => (vram_idx - 0x800) as usize,
            (Mirroring::Horizontal, 1 | 2) => (vram_idx - 0x400) as usize,
            (Mirroring::Horizontal, 3) => (vram_idx - 0x800) as usize,
            _ => vram_idx as usize,
        }
    }
}
