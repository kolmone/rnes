mod regs;

use crate::bus::Mirroring;
use core::panic;
use regs::{AddrReg, ControllerReg, MaskReg, StatusReg};

use self::regs::ScrollReg;

pub struct Ppu {
    chr: Vec<u8>,
    vram: [u8; 2048],
    palette: [u8; 32],
    oam: [u8; 256],
    pub mirroring: Mirroring,

    pub controller: ControllerReg,
    addr: AddrReg,
    mask: MaskReg,
    status: StatusReg,
    pub scroll: ScrollReg,
    pub oam_addr: u8,
    read_buf: u8, // Buffered RAM/ROM data

    pub scanline: u16,
    cycles: usize,
    nmi_triggered: bool,

    pub sprite_line: [(u8, bool, bool); 256],
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

    const LAST_LINE: u16 = 261;
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
            read_buf: 0,
            scroll: ScrollReg::new(),
            scanline: 0,
            cycles: 0,
            nmi_triggered: false,
            sprite_line: [(0, false, false); 256],
        }
    }

    /// Progress by one PPU clock cycle
    pub fn tick(&mut self) -> bool {
        self.cycles += 1;
        if self.cycles == Ppu::CYCLES_PER_LINE {
            self.cycles = 0;

            match self.scanline {
                0..=Ppu::LAST_RENDER_LINE => {
                    self.evaluate_sprites();
                    self.scanline += 1;
                    return true;
                }
                Ppu::VBLANK_START_LINE => {
                    self.scanline += 1;
                    self.status.set_vblank(true);
                    if self.controller.generate_nmi() {
                        self.nmi_triggered = true;
                    }
                }
                Ppu::LAST_LINE => {
                    self.status.set_sprite0_hit(false);
                    self.scanline = 0;
                    self.status.set_vblank(false);
                }
                // 240, 242 - 260
                _ => self.scanline += 1,
            }
        }
        false
    }

    pub fn nmi_triggered(&mut self) -> bool {
        if self.nmi_triggered {
            // println!("NMI triggered!");
            self.nmi_triggered = false;
            return true;
        }
        false
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        let addr = addr & PPU_BUS_MIRROR_MASK;
        match addr {
            REG_STATUS => {
                self.addr.reset_latch();
                let old_status = self.status.0;
                self.status.set_vblank(false);
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
                let old_nmi_val = self.controller.generate_nmi();
                self.controller.0 = data;
                if !old_nmi_val && self.controller.generate_nmi() && self.status.vblank() {
                    self.nmi_triggered = true;
                }
                println!(
                    "CTRL 0b{:08b} written at scanline {}, base nametable is {}",
                    data,
                    self.scanline,
                    self.controller.nametable()
                );
            }
            REG_MASK => self.mask.0 = data,
            REG_OAM_ADDR => self.oam_addr = data,
            REG_OAM_DATA => self.oam_write(data),
            REG_SCROLL => self.scroll.write(data),
            REG_ADDR => self.addr.write(data),
            REG_DATA => self.data_write(data),
            _ => panic!("Write to read-only PPU register at 0x{:x}", addr),
        }
    }

    fn data_read(&mut self) -> u8 {
        let addr = self.addr.get();
        self.addr.increment(self.controller.get_increment());

        let old_buf = self.read_buf;
        match addr {
            0..=0x1FFF => {
                self.read_buf = self.chr[addr as usize];
                old_buf
            }
            0x2000..=0x3EFF | 0x3F20..=0x3FFF => {
                self.read_buf = self.vram[self.mirrored_vram_addr(addr)];
                old_buf
            }
            0x3F00..=0x3F1F => {
                self.read_buf = self.vram[self.mirrored_vram_addr(addr)];
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
            0x2000..=0x3EFF | 0x3F20..=0x3FFF => self.vram[self.mirrored_vram_addr(addr)] = data,
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

    /// Translates given VRAM address to actual VRAM location
    /// This includes removing address offset and mirroring based on current mirroring scheme
    fn mirrored_vram_addr(&self, addr: u16) -> usize {
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

    fn palette_idx(&self, addr: u16) -> usize {
        let addr = if addr >= 0x3f10 && addr % 4 == 0 {
            addr - 0x3f10
        } else {
            addr - 0x3f00
        };
        addr as usize
    }

    fn background_palette(&self, palette: u8) -> [u8; 4] {
        let idx = (1 + 4 * palette) as usize;

        [
            self.palette[0],
            self.palette[idx],
            self.palette[idx + 1],
            self.palette[idx + 2],
        ]
    }

    fn sprite_color(&self, palette: u8, idx: u8) -> u8 {
        let idx = (16 + 4 * palette + idx) as usize;
        if idx >= 32 {
            panic!("Requesting color {} from sprite palette {}", idx, palette);
        }
        self.palette[idx]
    }

    /// Get pointer to CHR ROM for the tile at specific x index on given scanline
    pub fn tile_idx(&self, screen: u8, scanline: u16, tile_num: u8) -> u8 {
        let vram_idx = scanline / 8 * 32 + (tile_num as u16);
        let vram_base = screen as u16 * 0x400;
        self.vram[(vram_base + vram_idx) as usize]
    }

    /// Get one row of a tile's pixel data (2 bits per pixel = 16 bits)
    ///
    /// DCBA98 76543210
    /// ---------------
    /// 0HRRRR CCCCPTTT
    /// |||||| |||||+++- T: Fine Y offset, the row number within a tile
    /// |||||| ||||+---- P: Bit plane (0: "lower"; 1: "upper")
    /// |||||| ++++----- C: Tile column
    /// ||++++---------- R: Tile row
    /// |+-------------- H: Half of pattern table (0: "left"; 1: "right")
    /// +--------------- 0: Pattern table is at $0000-$1FFF
    pub fn bg_tile_data(&self, screen: u8, scanline: u16, tile_num: u8) -> ([u8; 8], [u8; 4]) {
        let tile_idx = self.tile_idx(screen, scanline, tile_num);
        let row = scanline % 8;

        (
            self.bg_tile_row(tile_idx, row),
            self.background_palette(self.bg_attribute(screen, scanline, tile_num)),
        )
    }

    fn bg_tile_row(&self, tile_idx: u8, row: u16) -> [u8; 8] {
        let base = self.controller.background_half() * 0x1000;
        let tile_ptr = base + (tile_idx as usize) * 16 + row as usize;
        let mut lower_bits = self.chr[tile_ptr];
        let mut upper_bits = self.chr[tile_ptr + 8];

        let mut values = [0; 8];
        for value in &mut values {
            *value = (lower_bits & 0x1) + ((upper_bits & 0x1) << 1);
            lower_bits >>= 1;
            upper_bits >>= 1;
        }
        values
    }

    fn bg_attribute(&self, screen: u8, scanline: u16, tile_num: u8) -> u8 {
        let attribute_idx = self.attribute_idx(scanline, tile_num);
        let vram_base = 0x3c0 + screen as usize * 0x400;
        let attribute = self.vram[vram_base + attribute_idx];

        match ((tile_num / 2) % 2, (scanline / 16) % 2) {
            (0, 0) => attribute & 0x03,        // top left
            (1, 0) => (attribute >> 2) & 0x03, // top right
            (0, 1) => (attribute >> 4) & 0x03, // bottom left
            (1, 1) => (attribute >> 6) & 0x03, // bottom right
            _ => panic!("not reachable"),
        }
    }

    // Each attribute byte maps to a 32x32 pixel area
    fn attribute_idx(&self, scanline: u16, tile_num: u8) -> usize {
        (scanline / 32 * 8 + (tile_num / 4) as u16) as usize
    }

    fn evaluate_sprites(&mut self) {
        self.sprite_line = [(self.palette[0], false, false); 256];
        // First line can't have sprites
        if self.scanline == 0 {
            return;
        }

        let y = self.scanline as usize - 1; // Sprites are drawn on off by one scanline
        let yy = y as u8;

        let line_sprites: Vec<Sprite> = self
            .sprites()
            .filter(|item| yy >= item.y_pos && yy < item.y_pos + 8)
            .collect();

        for x in 0..256 {
            for sprite in &line_sprites {
                let x_coord = sprite.x_pos as usize;

                if x >= x_coord && x < x_coord + 8 {
                    let x_idx = x - x_coord;
                    let y_idx = y - sprite.y_pos as usize;
                    let color = self.sprite_pixel_data(sprite, x_idx, y_idx);
                    if color != 0 {
                        if sprite.idx == 0 {
                            // println!("Sprite zero hit at {}, {}", x, y);
                            self.status.set_sprite0_hit(true);
                        }
                        let color = self.sprite_color(sprite.attributes & 0x3, color);
                        let priority = sprite.attributes & 0x20 != 0;
                        self.sprite_line[x] = (color, true, priority);
                        break;
                    }
                }
            }
        }
    }

    fn sprite_pixel_data(&self, sprite: &Sprite, x_idx: usize, y_idx: usize) -> u8 {
        let row = if sprite.attributes & 0x80 != 0 {
            7 - y_idx
        } else {
            y_idx
        };
        let base = self.controller.sprite_half() * 0x1000;
        let tile_ptr = base + (sprite.tile_idx as usize) * 16 + row;
        let lower_bits = self.chr[tile_ptr];
        let upper_bits = self.chr[tile_ptr + 8];

        let idx = if sprite.attributes & 0x40 != 0 {
            x_idx
        } else {
            7 - x_idx
        };

        (((lower_bits >> idx) & 0x1) + (((upper_bits >> idx) & 0x1) << 1)) & 0x3
    }

    fn sprites(&self) -> SpriteIterator {
        SpriteIterator {
            array: &self.oam,
            idx: 0,
        }
    }
}

struct Sprite {
    pub idx: u8,
    pub y_pos: u8,
    pub x_pos: u8,
    pub tile_idx: u8,
    pub attributes: u8,
}

struct SpriteIterator<'a> {
    array: &'a [u8],
    idx: u8,
}

impl<'a> Iterator for SpriteIterator<'a> {
    type Item = Sprite;

    fn next(&mut self) -> Option<Self::Item> {
        if self.array.len() < 4 {
            None
        } else {
            let sprite = Sprite {
                y_pos: self.array[0],
                tile_idx: self.array[1],
                attributes: self.array[2],
                x_pos: self.array[3],
                idx: self.idx,
            };
            self.idx += 1;
            self.array = &self.array[4..];
            Some(sprite)
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

        ppu.palette[0x13] = 0x67;

        ppu.write(0x2006, 0x3f);
        ppu.write(0x2006, 0x12);

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

        assert_eq!(ppu.scroll.hori_scroll, 0x21);
        assert_eq!(ppu.scroll.vert_scroll, 0x56);

        ppu.write(0x2005, 0x17);
        ppu.write(0x2005, 0x34);

        assert_eq!(ppu.scroll.hori_scroll, 0x17);
        assert_eq!(ppu.scroll.vert_scroll, 0x34);
    }

    #[test]
    fn test_attribute_indexing() {
        let ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        assert_eq!(ppu.attribute_idx(0, 0), 0);
        assert_eq!(ppu.attribute_idx(0, 3), 0);
        assert_eq!(ppu.attribute_idx(31, 4), 1);
        assert_eq!(ppu.attribute_idx(32, 3), 8);
    }
}
