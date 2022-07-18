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

bitfield! {
    pub struct ControllerReg(pub u8);
    pub new();

    pub field nametable:       u8 = [0..=1];
    pub field increment:       bool = [2];
    pub field sprite_half:     usize = [3];
    pub field background_half: usize = [4];
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
    pub field left_background: bool = [1];
    pub field left_sprites:    bool = [2];
    pub field show_background: bool = [3];
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