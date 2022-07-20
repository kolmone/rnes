use bitbash::bitfield;

struct Apu {
    pulse1: PulseReg,
    pulse2: PulseReg,
    triangle: Triangle,
    noise: Noise,
}

struct Triangle {}

struct Noise {}

impl Apu {
    fn new() -> Self {
        Apu {
            pulse1: PulseReg::new(),
            pulse2: PulseReg::new(),
            triangle: Triangle {},
            noise: Noise {},
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        let addr = addr & 0x17;
        match addr {
            0x4000 => self.pulse1.0[0] = data,
            0x4001 => self.pulse1.0[1] = data,
            0x4002 => self.pulse1.0[2] = data,
            0x4003 => self.pulse1.0[3] = data,
            _ => todo!(),
        }
    }

    pub fn tick(&mut self) {}
}

bitfield! {
    pub struct PulseReg([u8; 4]);
    pub new();

    pub field volume: u8 = 0[0..4];
    pub field envelope: u8 = 0[0..4];
    pub field const_vol: bool = 0[4];
    pub field env_loop: bool = 0[5];
    pub field counter_halt: bool = 0[5];
    pub field duty: u8 = 0[6..8];

    pub field sw_shift: u8 = 1[0..3];
    pub field sw_negate: bool = 1[3];
    pub field sw_period: u8 = 1[4..7];
    pub field sw_enable: bool = 1[7];

    pub field timer_lo: u8 = 2[0..8];
    pub field timer_hi: u8 = 3[0..3];
    pub field timer: u16 = 2[0..8] ~ 3[0..3];
    pub field counter_load: u8 = 3[3..8];

}
