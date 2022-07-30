use bitbash::bitfield;

bitfield! {
    pub struct ControllerReg(pub u8);
    pub new();

    pub field nametable:       u8 = [0..=1];
    pub field increment:       bool = [2];
    pub field sprite_half:     u16 = [3];
    pub field bg_half:         u16 = [4];
    pub field sprite_size:     bool = [5];
    pub field ppu_master:      bool = [6];
    pub field generate_nmi:    bool = [7];
}

impl ControllerReg {
    pub fn get_increment(&self) -> u16 {
        if self.increment() {
            32
        } else {
            1
        }
    }

    pub fn act_sprite_size(&self) -> u8 {
        if self.sprite_size() {
            16
        } else {
            8
        }
    }
}

bitfield! {
    pub struct MaskReg(pub u8);
    pub new();

    pub field greyscale:       bool = [0];
    pub field show_left_bg:    bool = [1];
    pub field show_left_sp:    bool = [2];
    pub field show_bg:         bool = [3];
    pub field show_sprites:    bool = [4];
    pub field emphasize_red:   bool = [5];
    pub field emphasize_green: bool = [6];
    pub field emphasize_blue:  bool = [7];
}

bitfield! {
    pub struct StatusReg(pub u8);
    pub new();

    pub field sprite_overflow: bool = [5];
    pub field sprite0_hit:     bool = [6];
    pub field vblank:          bool = [7];
}

bitfield! {
    #[derive(Copy, Clone)]
    pub struct ScrollReg {
        pub data: u32,
        pub offset: bool,
    }

    pub field addr:             u16  = data[3..19];
    pub field x_fine:           u8   = data[0..3];
    pub field x_coarse:         u16  = data[3..8];
    pub field y_coarse:         u16  = data[8..13];
    pub field base_nametable:   u16  = data[13..15];
    pub field base_nametable_h: u8   = data[13];
    pub field base_nametable_v: u8   = data[14];
    pub field y_fine:           u8   = data[15..18];
    pub field addr_lo:          u8   = data[3..11];
    pub field addr_hi:          u8   = data[11..19];
    pub field x:                u8   = data[0..8];
    pub field y:                u8   = data[15..18] ~ data[8..13];
}

impl ScrollReg {
    pub fn new() -> Self {
        ScrollReg {
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
