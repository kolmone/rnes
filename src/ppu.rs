/*controller: u8,
mask: u8,
status: u8,
oam_address: u8,
oam_data: u8,
scroll: u8,
address: u8,
data: u8,
oam_dma: u8,*/

use std::ops::Add;

use crate::bus::Mirroring;

#[derive(Debug)]
pub struct Ppu {
    chr: Vec<u8>,
    vram: [u8; 2048],
    palette: [u8; 32],
    oam: [u8; 256],
    mirroring: Mirroring,

    controller: ControllerReg,
    addr: AddrReg,
    mask: MaskReg,
    data_buf: u8, // Buffered RAM/ROM data
}

const REG_CONTROLLER: u16 = 0x2000;
const REG_MASK: u16 = 0x2001;
const REG_STATUS: u16 = 0x2002;
const REG_OAM_ADDR: u16 = 0x2003;
const REG_OAM_DATA: u16 = 0x2004;
const REG_SCROLL: u16 = 0x2005;
const REG_ADDR: u16 = 0x2006;
const REG_DATA: u16 = 0x2007;
const REG_OAM_DMA: u16 = 0x4014;

const PPU_BUS_MIRROR_MASK: u16 = 0x2007;

#[derive(Debug)]
struct AddrReg {
    msb: u8,
    lsb: u8,
    on_msb: bool,
}

impl AddrReg {
    pub fn new() -> Self {
        Self {
            msb: 0,
            lsb: 0,
            on_msb: true,
        }
    }

    pub fn write(&mut self, data: u8) {
        if self.on_msb {
            self.msb = data;
            self.sanitize();
        } else {
            self.lsb = data;
        }

        self.on_msb = !self.on_msb;
    }

    pub fn increment(&mut self, inc: u8) {
        let old_lsb = self.lsb;
        self.lsb = old_lsb.wrapping_add(inc);
        if self.lsb < old_lsb {
            self.msb = self.msb.wrapping_add(1);
        }
        self.sanitize();
    }

    pub fn get(&self) -> u16 {
        (self.msb as u16) << 8 | (self.lsb as u16)
    }

    pub fn reset_latch(&mut self) {
        self.on_msb = true;
    }

    fn sanitize(&mut self) {
        self.msb &= 0x3F;
    }
}

#[derive(Debug)]
struct ControllerReg {
    nametable1: bool,
    nametable2: bool,
    increment: bool,
    sprite_addr: bool,
    background_addr: bool,
    sprite_size: bool,
    ppu_master: bool,
    generate_nmi: bool,
}

impl Into<ControllerReg> for u8 {
    fn into(self) -> ControllerReg {
        ControllerReg {
            nametable1: self & 0x1 != 0,
            nametable2: self & 0x2 != 0,
            increment: self & 0x4 != 0,
            sprite_addr: self & 0x8 != 0,
            background_addr: self & 0x10 != 0,
            sprite_size: self & 0x20 != 0,
            ppu_master: self & 0x40 != 0,
            generate_nmi: self & 0x80 != 0,
        }
    }
}

impl ControllerReg {
    fn new() -> Self {
        0.into()
    }

    fn get_increment(&self) -> u8 {
        if self.increment {
            32
        } else {
            1
        }
    }
}

#[derive(Debug)]
struct MaskReg {
    greyscale: bool,
    left_background: bool,
    left_sprites: bool,
    show_background: bool,
    show_sprites: bool,
    emphasize_red: bool,
    emphasize_green: bool,
    emphasize_blue: bool,
}

impl Into<MaskReg> for u8 {
    fn into(self) -> MaskReg {
        MaskReg {
            greyscale: self & 0x1 != 0,
            left_background: self & 0x2 != 0,
            left_sprites: self & 0x4 != 0,
            show_background: self & 0x8 != 0,
            show_sprites: self & 0x10 != 0,
            emphasize_red: self & 0x20 != 0,
            emphasize_green: self & 0x40 != 0,
            emphasize_blue: self & 0x80 != 0,
        }
    }
}

impl MaskReg {
    fn new() -> Self {
        0.into()
    }
}

impl Ppu {
    pub fn new(chr: Vec<u8>, mirroring: Mirroring) -> Self {
        Self {
            chr,
            vram: [0; 2048],
            palette: [0; 32],
            oam: [0; 256],
            mirroring,
            controller: ControllerReg::new(),
            addr: AddrReg::new(),
            mask: MaskReg::new(),
            data_buf: 0,
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        let addr = addr & PPU_BUS_MIRROR_MASK;
        match addr {
            REG_STATUS => todo!(),
            REG_OAM_DATA => todo!(),
            REG_DATA => self.data_read(),
            _ => panic!("Read from unsupported PPU address at 0x{:x}", addr),
        }
    }

    fn data_read(&mut self) -> u8 {
        let addr = self.addr.get();
        self.addr.increment(self.controller.get_increment());

        let old_buf = self.data_buf;
        match addr {
            0..=0x1FFF => {
                self.data_buf = self.chr[addr as usize];
                old_buf
            }
            0x2000..=0x3EFF => {
                self.data_buf = self.vram[self.mirrored_vram_addr(addr) as usize];
                old_buf
            }
            0x3F00..=0x3FFF => {
                self.data_buf = self.vram[self.mirrored_vram_addr(addr) as usize];
                self.palette[(addr - 0x3F00) as usize]
            }
            _ => panic!("Data read from unsupported PPU address at 0x{:x}", addr),
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        let addr = addr & PPU_BUS_MIRROR_MASK;
        match addr {
            REG_CONTROLLER => self.controller = data.into(),
            REG_MASK => self.mask = data.into(),
            REG_OAM_ADDR => todo!(),
            REG_OAM_DATA => todo!(),
            REG_SCROLL => todo!(),
            REG_ADDR => self.addr.write(data),
            REG_DATA => todo!(),
            _ => panic!("Write to read-only PPU register at 0x{:x}", addr),
        }
    }

    fn data_write(&mut self, data: u8) {
        let addr = self.addr.get();
        self.addr.increment(self.controller.get_increment());

        match addr {
            0..=0x1FFF => panic!("Write to CHR ROM address {:X}", addr),
            0x2000..=0x2FFF => self.vram[self.mirrored_vram_addr(addr) as usize] = data,
            0x3F00..=0x3FFF => self.palette[(addr - 0x3F00) as usize] = data,
            _ => panic!("Data write to unsupported PPU address at 0x{:x}", addr),
        }
    }

    // Horizontal mirroring - first two 1kB areas map to first 1kB screen
    // Vertical mirroring - first and third 1kB areas map to first 1kB screen
    fn mirrored_vram_addr(&self, addr: u16) -> u16 {
        let mirror_half = addr & 0x2FFF; // 0x2000-0x3f00 -> 0x2000-0x3000
        let vram_idx = mirror_half - 0x2000; // 0x2000-0x3000 -> 0x0000-0x1000
        let table = vram_idx / 0x400; // Index of 0x400 sized table
        match (&self.mirroring, table) {
            (Mirroring::Vertical, 2 | 3) => vram_idx - 0x800,
            (Mirroring::Horizontal, 1 | 2) => vram_idx - 0x400,
            (Mirroring::Horizontal, 3) => vram_idx - 0x800,
            _ => vram_idx,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_vertical_mirroring() {
        let ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);
        assert_eq!(ppu.mirrored_vram_addr(0x2356), 0x356);
        assert_eq!(ppu.mirrored_vram_addr(0x2556), 0x556);
        assert_eq!(ppu.mirrored_vram_addr(0x2956), 0x156);
        assert_eq!(ppu.mirrored_vram_addr(0x2e56), 0x656);

        assert_eq!(ppu.mirrored_vram_addr(0x3356), 0x356);
        assert_eq!(ppu.mirrored_vram_addr(0x3556), 0x556);
        assert_eq!(ppu.mirrored_vram_addr(0x3956), 0x156);
        assert_eq!(ppu.mirrored_vram_addr(0x3e56), 0x656);
    }

    #[test]
    fn test_horizontal_mirroring() {
        let ppu = Ppu::new(vec![0; 0], Mirroring::Horizontal);
        assert_eq!(ppu.mirrored_vram_addr(0x2356), 0x356);
        assert_eq!(ppu.mirrored_vram_addr(0x2556), 0x156);
        assert_eq!(ppu.mirrored_vram_addr(0x2956), 0x556);
        assert_eq!(ppu.mirrored_vram_addr(0x2e56), 0x656);

        assert_eq!(ppu.mirrored_vram_addr(0x3356), 0x356);
        assert_eq!(ppu.mirrored_vram_addr(0x3556), 0x156);
        assert_eq!(ppu.mirrored_vram_addr(0x3956), 0x556);
        assert_eq!(ppu.mirrored_vram_addr(0x3e56), 0x656);
    }

    #[test]
    fn test_addr_reg_write() {
        let mut addr_reg = AddrReg::new();
        assert_eq!(addr_reg.get(), 0x0000);

        addr_reg.write(0x32);
        addr_reg.write(0x10);

        assert_eq!(addr_reg.get(), 0x3210);
    }

    #[test]
    fn test_addr_reg_reset() {
        let mut addr_reg = AddrReg::new();

        addr_reg.write(0x32);
        addr_reg.reset_latch();
        addr_reg.write(0x10);
        addr_reg.write(0x32);

        assert_eq!(addr_reg.get(), 0x1032);
    }

    #[test]
    fn test_addr_reg_increment() {
        let mut addr_reg = AddrReg::new();

        addr_reg.write(0x3f);
        addr_reg.write(0x00);
        assert_eq!(addr_reg.get(), 0x3f00);

        addr_reg.increment(0xff);
        assert_eq!(addr_reg.get(), 0x3fff);
        addr_reg.increment(0x1);
        assert_eq!(addr_reg.get(), 0x0000);
    }

    #[test]
    fn test_ppu_addr_write() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Horizontal);

        ppu.write(0x2006, 0x3f);
        ppu.write(0x2006, 0x12);

        assert_eq!(ppu.addr.get(), 0x3f12);
    }

    #[test]
    fn test_ppu_data_read_rom() {
        let mut ppu = Ppu::new(vec![0, 0, 0x56, 0, 0], Mirroring::Horizontal);

        ppu.write(0x2006, 0x00);
        ppu.write(0x2006, 0x02);

        assert_eq!(ppu.read(0x2007), 0);
        assert_eq!(ppu.read(0x2007), 0x56);
        assert_eq!(ppu.read(0x2007), 0);
    }

    #[test]
    fn test_ppu_data_read_vram() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        ppu.vram[0x145] = 56;

        ppu.write(0x2006, 0x21);
        ppu.write(0x2006, 0x45);

        assert_eq!(ppu.read(0x2007), 0);
        assert_eq!(ppu.read(0x2007), 56);
        assert_eq!(ppu.read(0x2007), 0);
    }

    #[test]
    fn test_ppu_data_read_palette() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        ppu.palette[0x14] = 0x67;

        ppu.write(0x2006, 0x3f);
        ppu.write(0x2006, 0x13);

        assert_eq!(ppu.read(0x2007), 0x0);
        assert_eq!(ppu.read(0x2007), 0x67);
        assert_eq!(ppu.read(0x2007), 0);
    }

    #[test]
    fn test_ppu_data_read_increment() {
        let mut ppu = Ppu::new((0..100).collect(), Mirroring::Vertical);

        ppu.write(0x2000, 0x1 << 2);
        ppu.write(0x2006, 0x00);
        ppu.write(0x2006, 0x02);

        assert_eq!(ppu.read(0x2007), 0);
        assert_eq!(ppu.read(0x2007), 2);
        ppu.write(0x2000, 0);
        assert_eq!(ppu.read(0x2007), 34);
        assert_eq!(ppu.read(0x2007), 66);
        assert_eq!(ppu.read(0x2007), 67);
    }
}
