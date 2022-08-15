mod regs;

use regs::{ControllerReg, MaskReg, StatusReg};

use super::cartridge::Cartridge;

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
    vram: [u8; 2048],
    palette: [u8; 32],
    oam: [u8; 4 * 64],
    render_oam: [Sprite; 8],
    prefetch_oam: [Sprite; 8],

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

    pub fn new() -> Self {
        let empty_sprite = Sprite {
            sprite_idx: 0,
            attributes: 0,
            pattern: 0,
            tile_idx: 0,
            x_pos: 0,
            y_pos: 0,
        };
        Self {
            vram: [0; 2048],
            palette: [0; 32],
            oam: [0; 4 * 64],
            prefetch_oam: [empty_sprite; 8],
            render_oam: [empty_sprite; 8],
            ctrl: ControllerReg::default(),
            mask: MaskReg::default(),
            status: StatusReg::default(),
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

    pub fn reset(&mut self) {
        self.ctrl = ControllerReg::default();
        self.mask = MaskReg::default();
        self.scroll.reset_latch();
        self.scroll.data = 0;
        self.read_buf = 0;
        self.cycle = 0;
    }

    // Progress by one PPU clock cycle
    pub fn tick(&mut self, cartridge: &mut Cartridge) -> bool {
        self.cycle += 1;
        self.nmi_up = self.status.vblank && self.ctrl.generate_nmi;

        if self.scanline < Self::RENDER_LINES {
            if self.mask.show_bg | self.mask.show_sprites {
                self.render_tick(cartridge);
            }
            if self.scanline >= 0 && self.x < 256 {
                self.draw_pixel();
            }
        }

        self.x += 1;
        if self.x >= Self::CYCLES_PER_LINE {
            self.x = 0;
            self.scanline += 1;
            match self.scanline {
                Self::LAST_LINE => {
                    self.scanline = -1;
                    self.status.vblank = false;
                    self.status.sprite0_hit = false;
                    self.status.sprite_overflow = false;
                    // println!("Vblank cleared");
                    self.frame = [0; 256 * 240];
                }
                Self::VBLANK_START_LINE => {
                    self.status.vblank = true;
                    // println!("frame done after {} cycles", self.cycle);
                    self.cycle = 0;
                    return true;
                }
                _ => (),
            }
        }
        false
    }

    #[allow(clippy::too_many_lines)]
    fn render_tick(&mut self, cartridge: &mut Cartridge) {
        let tile_fetch = matches!(self.x, 0..=255 | 320..=335);

        match (self.x % 8, tile_fetch) {
            (0, _) | (2, false) => {
                // Read nametable (for sprites on (2, false))
                self.read_addr = 0x2000 + (self.vaddr.addr() & 0xFFF);
            }
            (1, false) => {
                // Todo: odd/even frame toggle?
                self.pattern_addr = 0x1000 * self.ctrl.bg_half
                    + 16 * self.internal_read(self.read_addr, cartridge) as u16
                    + self.vaddr.y_fine() as u16;
            }
            (1, true) => {
                // Todo: odd/even frame toggle?
                self.pattern_addr = 0x1000 * self.ctrl.bg_half
                    + 16 * self.internal_read(self.read_addr, cartridge) as u16
                    + self.vaddr.y_fine() as u16;

                self.bg_pattern_shift =
                    (self.bg_pattern_shift >> 16) | ((self.pattern as u32) << 16);
                // Repeat attribute 8 times
                self.bg_attr_shift =
                    (self.bg_attr_shift >> 16) + 0x5555_0000 * self.attribute as u32;
            }
            (2, true) => {
                // Read attribute
                self.read_addr = 0x23C0
                    + 0x400 * self.vaddr.base_nametable()
                    + 8 * (self.vaddr.y_coarse() >> 2)
                    + (self.vaddr.x_coarse() >> 2);
            }
            (3, true) => {
                let tile_attribute = self.internal_read(self.read_addr, cartridge);
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
                    sprite_line = self.ctrl.sprite_size as u16 - 1 - sprite_line;
                };
                // Set tile base address based on sprite size & tile index
                self.pattern_addr = if self.ctrl.sprite_size == 16 {
                    0x1000 * (sprite.tile_idx as u16 & 0x01)
                        + 0x10 * ((sprite.tile_idx as u16) & 0xFE)
                } else {
                    0x1000 * self.ctrl.sprite_half + 0x10 * (sprite.tile_idx as u16)
                };
                // Go to correct line in tile
                self.pattern_addr += (sprite_line & 0x7) + (sprite_line & 0x8) * 2;
            }
            (5, _) => self.pattern = self.internal_read(self.pattern_addr, cartridge) as u16,
            (7, _) => {
                // Interleave two bytes of pattern data
                let p = self.pattern
                    | ((self.internal_read(self.pattern_addr + 8, cartridge) as u16) << 8);
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
            if self.mask.show_sprites {
                self.oam_addr = 0;
            }
        }

        if self.mask.show_bg {
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
                        let y_end = self.sprite_data.wrapping_add(self.ctrl.sprite_size) as isize;
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
                        self.status.sprite_overflow = true;
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
        let draw_bg = self.mask.show_bg && (self.mask.show_left_bg || self.x > 8);
        let draw_sp = self.mask.show_sprites && (self.mask.show_left_sp || self.x > 8);

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
        let greyscale_mask = if self.mask.greyscale { 0x30 } else { 0x3F };
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
            let mut offset = (self.x as u16).wrapping_sub(sprite.x_pos as u16);
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
                self.status.sprite0_hit = true;
            }
            return Some((
                sprite.attributes & 0x20 != 0,
                sp_pixel as u8,
                (sprite.attributes & 3) + 4,
            ));
        }
        None
    }

    pub fn read(&mut self, addr: u16, cartridge: &mut Cartridge) -> u8 {
        let addr = addr & PPU_BUS_MIRROR_MASK;
        match addr {
            REG_STATUS => {
                self.scroll.reset_latch();
                let old_status = self.status.into();
                self.status.vblank = false;
                old_status
            }
            REG_OAM_DATA => self.oam_read(),
            REG_DATA => self.data_read(cartridge),
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8, cartridge: &mut Cartridge) {
        let addr = addr & PPU_BUS_MIRROR_MASK;
        match addr {
            REG_CONTROLLER => {
                self.ctrl = data.into();
                self.scroll.set_base_nametable(self.ctrl.nametable);
            }
            REG_MASK => self.mask = data.into(),
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
            REG_DATA => self.data_write(data, cartridge),
            _ => (),
        }
    }

    fn data_read(&mut self, cartridge: &mut Cartridge) -> u8 {
        let addr = self.vaddr.addr();
        self.vaddr.increment(self.ctrl.increment);

        let old_buf = self.read_buf;
        match addr {
            0..=0x1FFF => {
                self.read_buf = cartridge.read_ppu(addr);
                old_buf
            }
            0x2000..=0x3EFF => {
                self.read_buf = self.vram[cartridge.mirror_vram_addr(addr)];
                old_buf
            }
            0x3F00..=0x3FFF => {
                self.read_buf = self.vram[cartridge.mirror_vram_addr(addr)];
                self.palette[Self::palette_idx(addr)]
            }
            _ => panic!("Data read from unsupported PPU address at 0x{:x}", addr),
        }
    }

    fn internal_read(&mut self, addr: u16, cartridge: &mut Cartridge) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0..=0x1FFF => cartridge.read_ppu(addr),
            0x3F00.. => panic!("Internal read to palette"),
            _ => self.vram[cartridge.mirror_vram_addr(addr)],
        }
    }

    fn data_write(&mut self, data: u8, cartridge: &mut Cartridge) {
        let addr = self.vaddr.addr();
        self.vaddr.increment(self.ctrl.increment);

        match addr {
            0..=0x1FFF => cartridge.write_ppu(addr, data),
            0x2000..=0x3EFF => self.vram[cartridge.mirror_vram_addr(addr)] = data,
            0x3F00..=0x3FFF => self.palette[Self::palette_idx(addr)] = data,
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

    const fn palette_idx(addr: u16) -> usize {
        if addr >= 0x3f10 && addr % 4 == 0 {
            0
        } else {
            (addr & 0x001f) as usize
        }
    }
}
