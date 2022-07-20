mod regs;

use crate::bus::Mirroring;
use regs::{ControllerReg, MaskReg, StatusReg};

use self::regs::ScrollReg;

#[derive(Clone, Copy)]
struct Sprite {
    sprite_idx: u8,
    x_pos: u8,
    y_pos: u8,
    tile_idx: u8,
    attributes: u8,
    pattern: u16,
}

pub struct Ppu {
    chr: Vec<u8>,
    vram: [u8; 2048],
    palette: [u8; 32],
    oam: [u8; 4 * 64],
    render_oam: [Sprite; 8],
    prefetch_oam: [Sprite; 8],
    pub mirroring: Mirroring,

    ctrl: ControllerReg,
    mask: MaskReg,
    status: StatusReg,
    scroll: ScrollReg,
    vaddr: ScrollReg,

    pub oam_addr: u8,
    read_buf: u8, // Buffered RAM/ROM data

    scanline: isize,
    x: usize,

    pub nmi_up: bool,

    pub frame: [u8; 256 * 240],

    bg_pattern_shift: u32,
    bg_attr_shift: u32,

    read_addr: u16,
    sp_in_idx: usize,
    sp_out_idx: usize,
    sp_render_idx: usize,
    pattern_addr: u16,
    pattern: u16,
    sprite_data: u8,
    attribute: u8,
    cycle: usize,
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

    const LAST_LINE: isize = 261;
    const RENDER_LINES: isize = 240;
    const VBLANK_START_LINE: isize = 241;

    pub fn new(chr: Vec<u8>, mirroring: Mirroring) -> Self {
        let empty_sprite = Sprite {
            sprite_idx: 0,
            attributes: 0,
            pattern: 0,
            tile_idx: 0,
            x_pos: 0,
            y_pos: 0,
        };
        Self {
            chr,
            vram: [0; 2048],
            palette: [0; 32],
            oam: [0; 4 * 64],
            prefetch_oam: [empty_sprite; 8],
            render_oam: [empty_sprite; 8],
            mirroring,
            ctrl: ControllerReg::new(),
            mask: MaskReg::new(),
            status: StatusReg::new(),
            oam_addr: 0,
            read_buf: 0,
            scroll: ScrollReg::new(),
            vaddr: ScrollReg::new(),
            scanline: 0,
            x: 0,
            nmi_up: false,
            frame: [0; 256 * 240],
            bg_pattern_shift: 0,
            bg_attr_shift: 0,
            read_addr: 0,
            sp_in_idx: 0,
            sp_out_idx: 0,
            sp_render_idx: 0,
            pattern_addr: 0,
            pattern: 0,
            attribute: 0,
            sprite_data: 0,
            cycle: 0,
        }
    }

    /// Progress by one PPU clock cycle
    pub fn tick(&mut self) -> bool {
        self.cycle += 1;
        self.nmi_up = self.status.vblank() && self.ctrl.generate_nmi();

        if self.scanline < Ppu::RENDER_LINES {
            if self.mask.show_bg() | self.mask.show_sprites() {
                self.render_tick();
            }
            if self.scanline >= 0 && self.x < 256 {
                self.draw_pixel();
            }
        }

        self.x += 1;
        if self.x >= Ppu::CYCLES_PER_LINE {
            self.x = 0;
            self.scanline += 1;
            match self.scanline {
                Ppu::LAST_LINE => {
                    self.scanline = -1;
                    self.status.0 = 0;
                    // println!("Vblank cleared");
                    self.frame = [0; 256 * 240];
                }
                Ppu::VBLANK_START_LINE => {
                    self.status.set_vblank(true);
                    return true;
                }
                _ => (),
            }
        }
        false
    }

    fn render_tick(&mut self) {
        let tile_fetch = matches!(self.x, 0..=255 | 320..=335);

        match (self.x % 8, tile_fetch) {
            (0, _) | (2, false) => {
                // Read nametable (for sprites on (2, false))
                self.read_addr = 0x2000 + (self.vaddr.addr() & 0xFFF);
            }
            (1, false) => {
                // Todo: odd/even frame toggle?
                self.pattern_addr = 0x1000 * self.ctrl.bg_half()
                    + 16 * self.internal_read(self.read_addr) as u16
                    + self.vaddr.y_fine() as u16;
            }
            (1, true) => {
                // Todo: odd/even frame toggle?
                self.pattern_addr = 0x1000 * self.ctrl.bg_half()
                    + 16 * self.internal_read(self.read_addr) as u16
                    + self.vaddr.y_fine() as u16;

                self.bg_pattern_shift =
                    (self.bg_pattern_shift >> 16) | ((self.pattern as u32) << 16);
                // Repeat attribute 8 times
                self.bg_attr_shift =
                    (self.bg_attr_shift >> 16) + 0x55550000 * self.attribute as u32;
            }
            (2, true) => {
                // Read attribute
                self.read_addr = 0x23C0
                    + 0x400 * self.vaddr.base_nametable()
                    + 8 * (self.vaddr.y_coarse() >> 2)
                    + (self.vaddr.x_coarse() >> 2);
            }
            (3, true) => {
                let tile_attribute = self.internal_read(self.read_addr);
                // Each attribute byte maps to 4x4 tiles
                // And each 2-bit attribute covers 2x2 tiles
                let offset_in_byte =
                    (self.vaddr.x_coarse() & 0x2) + 2 * (self.vaddr.y_coarse() & 0x2);
                self.attribute = (tile_attribute >> offset_in_byte) & 0x3;

                // Increment X coordinate to prepare for next tile
                // Go to next nametable if coordinate wraps
                if self.vaddr.inc_x_coarse() {
                    self.vaddr
                        .set_base_nametable_h(1 - self.vaddr.base_nametable_h());
                }
            }
            // Prepare sprite for rendering
            (3, false) if self.sp_render_idx < self.sp_out_idx => {
                let sprite = &self.prefetch_oam[self.sp_render_idx];
                self.render_oam[self.sp_render_idx] = *sprite;
                let mut sprite_line = self.scanline as u16 - sprite.y_pos as u16;
                // Vertical flipping
                if sprite.attributes & 0x80 != 0 {
                    sprite_line = self.ctrl.act_sprite_size() as u16 - 1 - sprite_line;
                };
                // Set tile base address based on sprite size & tile index
                self.pattern_addr = if self.ctrl.sprite_size() {
                    0x1000 * (sprite.tile_idx as u16 & 0x01)
                        + 0x10 * ((sprite.tile_idx as u16) & 0xFE)
                } else {
                    0x1000 * self.ctrl.sprite_half() + 0x10 * (sprite.tile_idx as u16)
                };
                // Go to correct line in tile
                self.pattern_addr += (sprite_line & 0x7) + (sprite_line & 0x8) * 2;
            }
            (5, _) => self.pattern = self.internal_read(self.pattern_addr) as u16,
            (7, _) => {
                // Interleave two bytes of pattern data
                let p = self.pattern | ((self.internal_read(self.pattern_addr + 8) as u16) << 8);
                let p = (p & 0xF00F) | ((p & 0x0F00) >> 4) | ((p & 0x00F0) << 4);
                let p = (p & 0xC3C3) | ((p & 0x3030) >> 2) | ((p & 0x0C0C) << 2);
                let p = (p & 0x9999) | ((p & 0x4444) >> 1) | ((p & 0x2222) << 1);
                self.pattern = p;
                if !tile_fetch && self.sp_render_idx < self.sp_out_idx {
                    self.render_oam[self.sp_render_idx].pattern = self.pattern;
                    self.sp_render_idx += 1;
                }
            }
            _ => (),
        }

        // Reset sprite status at the start of the line
        if self.x == 0 {
            self.sp_in_idx = 0;
            self.sp_out_idx = 0;
            if self.mask.show_sprites() {
                self.oam_addr = 0;
            }
        }

        if self.mask.show_bg() {
            // Reset vertical & horizontal scrolling at start of frame
            if self.x == 304 && self.scanline == -1 {
                self.vaddr.set_addr(self.scroll.addr());
            }
            // Reset horizontal scrolling at the end of each scanline
            else if self.x == 256 {
                self.vaddr.set_x_coarse(self.scroll.x_coarse());
                self.vaddr
                    .set_base_nametable_h(self.scroll.base_nametable_h());
                self.sp_render_idx = 0;
            }
        }

        // Increment Y coordinate at the end of scanline
        // Go to next nametable if coordinate wraps
        if self.x == 251 && !self.vaddr.inc_y() && self.vaddr.y_coarse() == 30 {
            self.vaddr.set_y_coarse(0);
            self.vaddr
                .set_base_nametable_v(1 - self.vaddr.base_nametable_v());
        }

        // Evaluate sprites visible on next scanline
        // Every other cycle is just read from current OAM address, see else branch
        let sprite_store_cycle = self.x >= 64 && self.x < 256 && self.x % 2 != 0;
        if sprite_store_cycle {
            let oam_addr = self.oam_addr & 0x3;
            self.oam_addr = self.oam_addr.wrapping_add(1);

            match oam_addr {
                0 if self.sp_in_idx >= 64 => {
                    self.oam_addr = 0;
                }
                0 => {
                    self.sp_in_idx += 1;
                    if self.sp_out_idx < 8 {
                        self.prefetch_oam[self.sp_out_idx].y_pos = self.sprite_data;
                        self.prefetch_oam[self.sp_out_idx].sprite_idx = self.oam_addr / 4;
                        let y_start = self.sprite_data as isize;
                        let y_end =
                            self.sprite_data.wrapping_add(self.ctrl.act_sprite_size()) as isize;
                        // If sprite not in range, go to next one
                        if self.scanline < y_start || self.scanline >= y_end {
                            self.oam_addr = self.oam_addr.wrapping_add(3);
                            // Weird hardcoded value for sprite #2
                            if self.sp_in_idx == 2 {
                                self.oam_addr = 8;
                            }
                        }
                    }
                }
                1 => {
                    if self.sp_out_idx < 8 {
                        self.prefetch_oam[self.sp_out_idx].tile_idx = self.sprite_data;
                    }
                }
                2 => {
                    if self.sp_out_idx < 8 {
                        self.prefetch_oam[self.sp_out_idx].attributes = self.sprite_data;
                    }
                }
                3 => {
                    if self.sp_out_idx < 8 {
                        self.prefetch_oam[self.sp_out_idx].x_pos = self.sprite_data;
                        self.sp_out_idx += 1;
                    } else {
                        // Found more than 8 sprites
                        // println!("Sprite overflow");
                        self.status.set_sprite_overflow(true);
                    }
                    if self.sp_in_idx == 2 {
                        self.oam_addr = 8;
                    }
                }
                _ => unimplemented!("Shouldn't be reachable"),
            }
        } else {
            self.sprite_data = self.oam[self.oam_addr as usize];
        }
    }

    fn draw_pixel(&mut self) {
        let draw_bg = self.mask.show_bg() && (self.mask.show_left_bg() || self.x > 8);
        let draw_sp = self.mask.show_sprites() && (self.mask.show_left_sp() || self.x > 8);

        let (mut pixel, mut attribute) = (0, 0);

        if draw_bg {
            (pixel, attribute) = self.bg_pixel();
        }
        // else if (self.vaddr.addr() & 0x3F00) == 0x3F00 && !draw_bg && !draw_sp {
        //     pixel = self.vaddr.addr() as u8;
        // }

        if draw_sp {
            if let Some(sprite_pixel) = self.sprite_pixel(pixel) {
                if !sprite_pixel.0 || pixel == 0 {
                    pixel = sprite_pixel.1;
                    attribute = sprite_pixel.2;
                }
            }
        }

        let palette_idx = (attribute * 4 + pixel) as usize;
        let greyscale_mask = if self.mask.greyscale() { 0x30 } else { 0x3F };
        let pixel = self.palette[palette_idx] & greyscale_mask;
        self.frame[self.scanline as usize * 256 + self.x] = pixel;
    }

    fn bg_pixel(&self) -> (u8, u8) {
        // Scrolling within the current tile
        let fine_x = (self.x & 0x7) as u8;
        let tile_x = (fine_x + self.scroll.x_fine() + 8 * (fine_x != 0) as u8) & 0xF;
        let shift = (0xF - tile_x) * 2;

        let pixel = ((self.bg_pattern_shift >> shift) & 0x3) as u8;
        let attribute = if pixel > 0 {
            ((self.bg_attr_shift >> shift) & 0x3) as u8
        } else {
            0
        };
        (pixel, attribute)
    }

    fn sprite_pixel(&mut self, pixel: u8) -> Option<(bool, u8, u8)> {
        for sprite in self.render_oam.iter().take(self.sp_render_idx) {
            // Check current X position against sprite position
            let mut offset = (self.x as u8).wrapping_sub(sprite.x_pos);
            if offset >= 8 {
                continue;
            }
            // Vertical flip
            if sprite.attributes & 0x40 == 0 {
                offset = 7 - offset;
            }
            let sp_pixel = (sprite.pattern >> (offset * 2)) & 0x3;
            // Skip transparent pixels
            if sp_pixel == 0 {
                continue;
            }
            if pixel > 0 && sprite.sprite_idx == 0 {
                // println!("Sprite zero hit");
                self.status.set_sprite0_hit(true);
            }
            return Some((
                sprite.attributes & 0x20 != 0,
                sp_pixel as u8,
                (sprite.attributes & 3) + 4,
            ));
        }
        None
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        let addr = addr & PPU_BUS_MIRROR_MASK;
        match addr {
            REG_STATUS => {
                self.scroll.reset_latch();
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
                self.ctrl.0 = data;
                self.scroll.set_base_nametable(self.ctrl.nametable() as u16);
            }
            REG_MASK => self.mask.0 = data,
            REG_OAM_ADDR => self.oam_addr = data,
            REG_OAM_DATA => self.oam_write(data),
            REG_SCROLL => self.scroll.write_scroll(data),
            REG_ADDR => {
                self.scroll.write_addr(data);
                // If LSB was just written, update address in v
                if !self.scroll.offset {
                    self.vaddr.set_addr(self.scroll.addr());
                }
            }
            REG_DATA => self.data_write(data),
            _ => panic!("Write to read-only PPU register at 0x{:x}", addr),
        }
    }

    fn data_read(&mut self) -> u8 {
        let addr = self.vaddr.addr();
        self.vaddr.increment(self.ctrl.get_increment());

        let old_buf = self.read_buf;
        match addr {
            0..=0x1FFF => {
                self.read_buf = self.chr[addr as usize];
                old_buf
            }
            0x2000..=0x3EFF => {
                self.read_buf = self.vram[self.mirrored_vram_addr(addr)];
                old_buf
            }
            0x3F00..=0x3FFF => {
                self.read_buf = self.vram[self.mirrored_vram_addr(addr)];
                self.palette[self.palette_idx(addr)]
            }
            _ => panic!("Data read from unsupported PPU address at 0x{:x}", addr),
        }
    }

    fn internal_read(&mut self, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0..=0x1FFF => self.chr[addr as usize],
            0x3F00.. => panic!("Internal read to palette"),
            _ => self.vram[self.mirrored_vram_addr(addr)],
        }
    }

    fn data_write(&mut self, data: u8) {
        let addr = self.vaddr.addr();
        self.vaddr.increment(self.ctrl.get_increment());

        match addr {
            // 0..=0x1FFF => println!("Write to CHR ROM address {:X}", addr),
            0..=0x1FFF => self.chr[addr as usize] = data,
            0x2000..=0x3EFF => self.vram[self.mirrored_vram_addr(addr)] = data,
            0x3F00..=0x3FFF => self.palette[self.palette_idx(addr)] = data,
            _ => panic!("Data write to unsupported PPU address at 0x{:x}", addr),
        }
    }

    fn oam_read(&mut self) -> u8 {
        let addr = self.oam_addr;
        if addr % 4 == 2 {
            return self.oam[addr as usize] & 0xE3;
        }
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
        if addr >= 0x3f10 && addr % 4 == 0 {
            0
        } else {
            (addr & 0x001f) as usize
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
    fn test_addr_write() {
        let mut reg = ScrollReg::new();
        assert_eq!(reg.addr(), 0x0000);

        reg.write_addr(0x32);
        reg.write_addr(0x10);

        assert_eq!(reg.addr(), 0x3210);
    }

    #[test]
    fn test_addr_reset() {
        let mut reg = ScrollReg::new();

        reg.write_addr(0x32);
        reg.reset_latch();
        reg.write_addr(0x10);
        reg.write_addr(0x32);

        assert_eq!(reg.addr(), 0x1032);
    }

    #[test]
    fn test_addr_increment() {
        let mut reg = ScrollReg::new();

        reg.write_addr(0x3f);
        reg.write_addr(0x00);
        assert_eq!(reg.addr(), 0x3f00);

        reg.increment(0xff);
        assert_eq!(reg.addr(), 0x3fff);
        reg.increment(0x1);
        assert_eq!(reg.addr(), 0x0000);
    }

    #[test]
    fn test_ppu_addr_write() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Horizontal);

        ppu.write(0x2006, 0x3f);
        ppu.write(0x2006, 0x12);

        assert_eq!(ppu.vaddr.addr(), 0x3f12);
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
        assert_eq!(ppu.read(0x2004), 0x56);
    }

    #[test]
    fn test_ppu_oam_write() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        ppu.write(0x2003, 0x21);
        ppu.write(0x2004, 0x56);
        ppu.write(0x2004, 0x65);

        assert_eq!(ppu.oam[0x21], 0x56);
        assert_eq!(ppu.oam[0x22], 0x65);
    }

    #[test]
    fn test_scroll_write() {
        let mut ppu = Ppu::new(vec![0; 0], Mirroring::Vertical);

        ppu.write(0x2005, 0x21);
        ppu.write(0x2005, 0x56);

        assert_eq!(ppu.scroll.x(), 0x21);
        assert_eq!(ppu.scroll.y(), 0x56);

        ppu.write(0x2005, 0x17);
        ppu.write(0x2005, 0x34);

        assert_eq!(ppu.scroll.x(), 0x17);
        assert_eq!(ppu.scroll.y(), 0x34);
    }
}
