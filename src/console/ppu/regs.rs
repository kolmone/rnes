#![allow(clippy::range_plus_one)]
#![allow(clippy::use_self)]

use bitbash::bitfield;

use crate::macros::bit_bool;
use crate::macros::bool_u8;

#[derive(Default)]
pub struct ControllerReg {
    pub nametable: u16,
    pub increment: u16,
    pub sprite_half: u16,
    pub bg_half: u16,
    pub sprite_size: u8,
    pub ppu_master: bool,
    pub generate_nmi: bool,
}

impl From<u8> for ControllerReg {
    fn from(data: u8) -> Self {
        Self {
            nametable: data as u16 & 0x3,
            increment: if bit_bool!(data, 2) { 32 } else { 1 },
            sprite_half: ((data as u16) >> 3) & 0x1,
            bg_half: ((data as u16) >> 4) & 0x1,
            sprite_size: if bit_bool!(data, 5) { 16 } else { 8 },
            ppu_master: bit_bool!(data, 6),
            generate_nmi: bit_bool!(data, 7),
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct MaskReg {
    pub greyscale: bool,
    pub show_left_bg: bool,
    pub show_left_sp: bool,
    pub show_bg: bool,
    pub show_sprites: bool,
    pub emphasize_red: bool,
    pub emphasize_green: bool,
    pub emphasize_blue: bool,
}

impl From<u8> for MaskReg {
    fn from(data: u8) -> Self {
        Self {
            greyscale: bit_bool!(data, 0),
            show_left_bg: bit_bool!(data, 1),
            show_left_sp: bit_bool!(data, 2),
            show_bg: bit_bool!(data, 3),
            show_sprites: bit_bool!(data, 4),
            emphasize_red: bit_bool!(data, 5),
            emphasize_green: bit_bool!(data, 6),
            emphasize_blue: bit_bool!(data, 7),
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct StatusReg {
    pub sprite_overflow: bool, // = [5];
    pub sprite0_hit: bool,     // = [6];
    pub vblank: bool,          // = [7];
}

impl From<StatusReg> for u8 {
    fn from(v: StatusReg) -> Self {
        bool_u8!(v.sprite_overflow, 5) | bool_u8!(v.sprite0_hit, 6) | bool_u8!(v.vblank, 7)
    }
}

bitfield! {
    #[derive(Copy, Clone)]
    pub struct ScrollReg {
        pub data: u32,
        pub offset: bool,
    }

    pub field addr:             u16  = data[3..=18];
    pub field x_fine:           u8   = data[0..=2];
    pub field x_coarse:         u16  = data[3..=7];
    pub field y_coarse:         u16  = data[8..=12];
    pub field base_nametable:   u16  = data[13..=14];
    pub field base_nametable_h: u8   = data[13];
    pub field base_nametable_v: u8   = data[14];
    pub field y_fine:           u8   = data[15..=17];
    pub field addr_lo:          u8   = data[3..=10];
    pub field addr_hi:          u8   = data[11..=18];
    pub field x:                u8   = data[0..=7];
    pub field y:                u8   = data[15..=17] ~ data[8..=12];
}

impl ScrollReg {
    pub const fn new() -> Self {
        Self {
            data: 0,
            offset: false,
        }
    }

    pub fn write_scroll(&mut self, data: u8) {
        if self.offset {
            self.set_y(data);
        } else {
            self.set_x(data);
        }
        self.offset = !self.offset;
    }

    pub fn inc_x_coarse(&mut self) -> bool {
        self.set_x_coarse((self.x_coarse() + 1) & 0x1F);
        self.x_coarse() == 0
    }

    pub fn inc_y(&mut self) -> bool {
        self.set_y(self.y().wrapping_add(1));
        self.y() == 0
    }

    pub fn increment(&mut self, inc: u16) {
        self.set_addr((self.addr() + inc) & 0x3fff);
    }

    pub fn reset_latch(&mut self) {
        self.offset = false;
    }

    pub fn write_addr(&mut self, data: u8) {
        if self.offset {
            self.set_addr_lo(data);
        } else {
            self.set_addr_hi(data & 0x3F);
        }

        self.offset = !self.offset;
    }
}
