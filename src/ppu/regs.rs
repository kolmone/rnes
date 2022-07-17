pub struct AddrReg {
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

pub struct ControllerReg {
    nametable1: bool,
    nametable2: bool,
    increment: bool,
    pub sprite_half: bool,
    pub background_half: bool,
    pub sprite_size: bool,
    pub ppu_master: bool,
    pub generate_nmi: bool,
}

impl From<u8> for ControllerReg {
    fn from(controller: u8) -> Self {
        Self {
            nametable1: controller & 0x1 != 0,
            nametable2: controller & 0x2 != 0,
            increment: controller & 0x4 != 0,
            sprite_half: controller & 0x8 != 0,
            background_half: controller & 0x10 != 0,
            sprite_size: controller & 0x20 != 0,
            ppu_master: controller & 0x40 != 0,
            generate_nmi: controller & 0x80 != 0,
        }
    }
}

impl ControllerReg {
    pub fn new() -> Self {
        0.into()
    }

    pub fn get_increment(&self) -> u8 {
        if self.increment {
            32
        } else {
            1
        }
    }

    pub fn base_nametable(&self) -> u8 {
        (self.nametable1 as u8) | (self.nametable2 as u8) << 1
    }
}

pub struct MaskReg {
    greyscale: bool,
    left_background: bool,
    left_sprites: bool,
    show_background: bool,
    show_sprites: bool,
    emphasize_red: bool,
    emphasize_green: bool,
    emphasize_blue: bool,
}

impl From<u8> for MaskReg {
    fn from(mask: u8) -> Self {
        Self {
            greyscale: mask & 0x1 != 0,
            left_background: mask & 0x2 != 0,
            left_sprites: mask & 0x4 != 0,
            show_background: mask & 0x8 != 0,
            show_sprites: mask & 0x10 != 0,
            emphasize_red: mask & 0x20 != 0,
            emphasize_green: mask & 0x40 != 0,
            emphasize_blue: mask & 0x80 != 0,
        }
    }
}

impl MaskReg {
    pub fn new() -> Self {
        0.into()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct StatusReg {
    sprite_overflow: bool,
    pub sprite0_hit: bool,
    pub vblank: bool,
}

impl StatusReg {
    pub fn new() -> Self {
        Self {
            sprite_overflow: false,
            sprite0_hit: false,
            vblank: false,
        }
    }
}

impl From<StatusReg> for u8 {
    fn from(status: StatusReg) -> Self {
        (status.sprite_overflow as u8) << 5
            | (status.sprite0_hit as u8) << 6
            | (status.vblank as u8) << 7
    }
}
