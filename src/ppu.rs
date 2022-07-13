mod regs;

use crate::bus::Mirroring;
use regs::{AddrReg, ControllerReg, MaskReg, StatusReg};

pub struct Ppu {
    chr: Vec<u8>,
    vram: [u8; 2048],
    palette: [u8; 32],
    oam: [u8; 256],
    mirroring: Mirroring,

    controller: ControllerReg,
    addr: AddrReg,
    mask: MaskReg,
    status: StatusReg,
    oam_addr: u8,
    data_buf: u8, // Buffered RAM/ROM data
    vertical_scroll: u8,
    horizontal_scroll: u8,
    on_vert_scroll: bool,

    pub scanline: u16,
    cycles: usize,
    nmi_triggered: bool,
}

const REG_CONTROLLER: u16 = 0x2000;
const REG_MASK: u16 = 0x2001;
const REG_STATUS: u16 = 0x2002;
const REG_OAM_ADDR: u16 = 0x2003;
const REG_OAM_DATA: u16 = 0x2004;
const REG_SCROLL: u16 = 0x2005;
const REG_ADDR: u16 = 0x2006;
const REG_DATA: u16 = 0x2007;

const PPU_BUS_MIRROR_MASK: u16 = 0x2007;

impl Ppu {
    const CYCLES_PER_LINE: usize = 341;

    const LINES_PER_FRAME: u16 = 262;
    const LAST_RENDER_LINE: u16 = 239;
    const VBLANK_START_LINE: u16 = 241;

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
            status: StatusReg::new(),
            oam_addr: 0,
            data_buf: 0,
            vertical_scroll: 0,
            horizontal_scroll: 0,
            on_vert_scroll: true,
            scanline: 0,
            cycles: 0,
            nmi_triggered: false,
        }
    }

    /// Progress by N clock cycles
    /// Returns line number to be rendered
    pub fn tick(&mut self, cycles: u8) -> bool {
        self.cycles += cycles as usize;
        if self.cycles >= Ppu::CYCLES_PER_LINE {
            self.cycles -= Ppu::CYCLES_PER_LINE;

            match self.scanline {
                0..=Ppu::LAST_RENDER_LINE => {
                    self.scanline += 1;
                    return true;
                }
                Ppu::VBLANK_START_LINE => {
                    self.scanline += 1;
                    self.status.vblank = true;
                    if self.controller.generate_nmi {
                        self.nmi_triggered = true;
                    }
                }
                Ppu::LINES_PER_FRAME => {
                    self.scanline = 0;
                    self.status.vblank = false;
                }
                // 240, 242 - 260
                _ => self.scanline += 1,
            }
        }
        return false;
    }

    pub fn nmi_triggered(&mut self) -> bool {
        if self.nmi_triggered {
            println!("NMI triggered!");
            self.nmi_triggered = false;
            return true;
        }
        return false;
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        let addr = addr & PPU_BUS_MIRROR_MASK;
        match addr {
            REG_STATUS => {
                let old_status = self.status.into();
                self.status.vblank = false;
                old_status
            }
            REG_OAM_DATA => self.oam_read(),
            REG_DATA => self.data_read(),
            _ => panic!("Read from unsupported PPU address at 0x{:x}", addr),
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        let addr = addr & PPU_BUS_MIRROR_MASK;
        match addr {
            REG_CONTROLLER => {
                let old_nmi_val = self.controller.generate_nmi;
                self.controller = data.into();
                if !old_nmi_val && self.controller.generate_nmi && self.status.vblank {
                    self.nmi_triggered = true;
                }
            }
            REG_MASK => self.mask = data.into(),
            REG_OAM_ADDR => self.oam_addr = data,
            REG_OAM_DATA => self.oam_write(data),
            REG_SCROLL => self.scroll_write(data),
            REG_ADDR => self.addr.write(data),
            REG_DATA => self.data_write(data),
            _ => panic!("Write to read-only PPU register at 0x{:x}", addr),
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
            0x2000..=0x3EFF | 0x3F20..=0x3FFF => {
                self.data_buf = self.vram[self.mirrored_vram_addr(addr) as usize];
                old_buf
            }
            0x3F00..=0x3F1F => {
                self.data_buf = self.vram[self.mirrored_vram_addr(addr) as usize];
                self.palette[self.palette_idx(addr)]
            }
            _ => panic!("Data read from unsupported PPU address at 0x{:x}", addr),
        }
    }

    fn data_write(&mut self, data: u8) {
        let addr = self.addr.get();
        self.addr.increment(self.controller.get_increment());

        match addr {
            0..=0x1FFF => panic!("Write to CHR ROM address {:X}", addr),
            0x2000..=0x3EFF | 0x3F20..=0x3FFF => {
                self.vram[self.mirrored_vram_addr(addr) as usize] = data
            }
            0x3F00..=0x3F1F => self.palette[self.palette_idx(addr)] = data,
            _ => panic!("Data write to unsupported PPU address at 0x{:x}", addr),
        }
    }

    fn oam_read(&mut self) -> u8 {
        let addr = self.oam_addr;
        self.oam_addr = self.oam_addr.wrapping_add(1);
        self.oam[addr as usize]
    }

    fn oam_write(&mut self, data: u8) {
        self.oam[self.oam_addr as usize] = data;
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }

    fn scroll_write(&mut self, data: u8) {
        if self.on_vert_scroll {
            self.vertical_scroll = data;
        } else {
            self.horizontal_scroll = data;
        }
        self.on_vert_scroll = !self.on_vert_scroll;
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

    fn palette_idx(&self, addr: u16) -> usize {
        let addr = if addr >= 0x3f10 && addr % 4 == 0 {
            addr - 0x3f10
        } else {
            addr - 0x3f00
        };
        addr as usize
    }

    pub fn get_background_color(&self) -> u8 {
        self.palette[0]
    }

    pub fn get_background_palette(&self, palette: u8) -> (u8, u8, u8, u8) {
        let idx = (1 + 4 * palette) as usize;
        (
            self.palette[0],
            self.palette[idx],
            self.palette[idx + 1],
            self.palette[idx + 3],
        )
    }

    pub fn get_sprite_palette(&self, palette: u8) -> (u8, u8, u8) {
        let idx = (17 + 4 * palette) as usize;
        (
            self.palette[idx],
            self.palette[idx + 1],
            self.palette[idx + 3],
        )
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

    #[test]
    fn test_ppu_data_write_vram() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        ppu.write(0x2006, 0x21);
        ppu.write(0x2006, 0x45);

        ppu.write(0x2007, 0x56);
        ppu.write(0x2007, 0x65);

        assert_eq!(ppu.vram[0x145], 0x56);
        assert_eq!(ppu.vram[0x146], 0x65);
    }

    #[test]
    fn test_ppu_oam_read() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        ppu.oam[0x21] = 0x56;
        ppu.write(0x2003, 0x21);

        assert_eq!(ppu.read(0x2004), 0x56);
    }

    #[test]
    fn test_ppu_oam_write() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        ppu.write(0x2003, 0x21);
        ppu.write(0x2004, 0x56);

        assert_eq!(ppu.oam[0x21], 0x56);
    }

    #[test]
    fn test_scroll_write() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        ppu.write(0x2005, 0x21);
        ppu.write(0x2005, 0x56);

        assert_eq!(ppu.vertical_scroll, 0x21);
        assert_eq!(ppu.horizontal_scroll, 0x56);

        ppu.write(0x2005, 0x17);
        ppu.write(0x2005, 0x34);

        assert_eq!(ppu.vertical_scroll, 0x17);
        assert_eq!(ppu.horizontal_scroll, 0x34);
    }
}
