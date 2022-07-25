use bitbash::bitfield;

use super::common::{Envelope, LengthCounter};

bitfield! {
    #[derive(Default)]
    pub struct Noise{
        r0: u8,
        r2: u8,
        r3: u8,
        timer: u16,
        enable: bool,
        shift_register: u16,

        lc: LengthCounter,
        env: Envelope,
    }

    pub field volume: u8 = r0[0..4];
    pub field envelope: u8 = r0[0..4];
    pub field const_vol: bool = r0[4];
    pub field env_loop: bool = r0[5];
    pub field counter_halt: bool = r0[5];

    pub field mode: bool = r2[7];
    pub field period: u8 = r2[0..4];

    pub field counter_load: u8 = r3[3..8];
}

impl Noise {
    const TIMER_VALUES: [u16; 16] = [
        4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
    ];

    pub fn new() -> Self {
        Self {
            shift_register: 1,
            ..Default::default()
        }
    }

    pub fn tick(&mut self) -> u8 {
        if !self.enable {
            return 0;
        }

        if self.timer == 0 {
            self.timer = Noise::TIMER_VALUES[self.period() as usize];
            let feedback = if self.mode() {
                (self.shift_register ^ (self.shift_register >> 6)) & 0x1
            } else {
                (self.shift_register ^ (self.shift_register >> 1)) & 0x1
            };
            self.shift_register >>= 1;
            self.shift_register |= feedback << 14;
        } else {
            self.timer -= 1;
        }

        let volume = if self.const_vol() {
            self.volume()
        } else {
            self.env.value
        };

        if self.shift_register & 0x1 == 0 && !self.lc.muting {
            volume
        } else {
            0
        }
    }

    pub fn tick_half_frame(&mut self) {
        if !self.counter_halt() {
            self.lc.tick();
        }
    }

    pub fn tick_quarter_frame(&mut self) {
        self.env.tick();
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enable = enable;
        if !enable {
            self.lc.counter = 0;
        }
    }

    pub fn write_r0(&mut self, data: u8) {
        self.r0 = data;
        self.env.divider_start = self.envelope();
        self.env.looping = self.env_loop();
    }

    pub fn write_r2(&mut self, data: u8) {
        self.r2 = data;
        self.timer = Noise::TIMER_VALUES[self.period() as usize];
    }

    pub fn write_r3(&mut self, data: u8) {
        self.r3 = data;
        if self.enable {
            self.lc.counter = super::LENGTH_VALUES[self.counter_load() as usize];
        };
        self.env.reset = true;
    }
}
