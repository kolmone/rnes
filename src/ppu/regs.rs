use bitbash::bitfield;

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
            self.msb = data & 0x3F;
        } else {
            self.lsb = data;
        }

        self.on_msb = !self.on_msb;
    }

    pub fn increment(&mut self, inc: u8) {
        let old_lsb = self.lsb;
        self.lsb = old_lsb.wrapping_add(inc);
        if self.lsb < old_lsb {
            self.msb = self.msb.wrapping_add(1) & 0x3F;
        }
    }

    pub fn get(&self) -> u16 {
        (self.msb as u16) << 8 | (self.lsb as u16)
    }

    pub fn reset_latch(&mut self) {
        self.on_msb = true;
    }
}

bitfield! {
    pub struct ControllerReg(pub u8);
    pub new();

    pub field nametable:       u8 = [0..=1];
    pub field increment:       bool = [2];
    pub field sprite_half:     usize = [3];
    pub field bg_half:         usize = [4];
    pub field sprite_size:     bool = [5];
    pub field ppu_master:      bool = [6];
    pub field generate_nmi:    bool = [7];
}

impl ControllerReg {
    pub fn get_increment(&self) -> u8 {
        if self.increment() {
            32
        } else {
            1
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
        pub on_vert_scroll: bool,
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
            on_vert_scroll: false,
        }
    }

    pub fn write(&mut self, data: u8) {
        if self.on_vert_scroll {
            self.set_y(data);
        } else {
            self.set_x(data);
        }
        if self.x() > 0 || self.y() > 0 {
            println!(
                "Vertical scroll is now {}, horizontal {}",
                self.y(),
                self.x()
            );
        }
        self.on_vert_scroll = !self.on_vert_scroll;
    }

    pub fn inc_x_coarse(&mut self) -> bool {
        self.set_x_coarse((self.x_coarse() + 1) & 0x1F);
        self.x_coarse() == 0
    }

    pub fn _inc_x_fine(&mut self) -> bool {
        self.set_x_fine((self.x_fine() + 1) & 0x7);
        self.x_fine() == 0
    }

    pub fn inc_y_coarse(&mut self) -> bool {
        self.set_y_coarse((self.y_coarse() + 1) & 0x1F);
        self.y_coarse() == 0
    }

    pub fn inc_y_fine(&mut self) -> bool {
        self.set_y_fine((self.y_fine() + 1) & 0x7);
        self.y_fine() == 0
    }
}
