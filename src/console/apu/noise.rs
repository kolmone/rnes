use crate::macros::bit_bool;

use super::common::Envelope;

#[allow(clippy::struct_excessive_bools)]
pub struct Noise {
    timer: u16,
    enable: bool,
    shift_register: u16,

    pub length_counter: u8,
    env: Envelope,

    pub output: u8,

    volume: u8,
    const_vol: bool,
    counter_halt: bool,
    mode: bool,
    period: u16,
}

impl Default for Noise {
    fn default() -> Self {
        Self {
            shift_register: 1,
            timer: 0,
            enable: false,
            length_counter: 0,
            env: Envelope::default(),
            output: 0,
            volume: 0,
            const_vol: false,
            counter_halt: false,
            mode: false,
            period: 0,
        }
    }
}

impl Noise {
    const TIMER_VALUES: [u16; 16] = [
        4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
    ];

    pub fn tick(&mut self) {
        if !self.enable {
            self.output = 0;
            return;
        }

        if self.timer == 0 {
            self.timer = self.period;
            let feedback = if self.mode {
                (self.shift_register ^ (self.shift_register >> 6)) & 0x1
            } else {
                (self.shift_register ^ (self.shift_register >> 1)) & 0x1
            };
            self.shift_register >>= 1;
            self.shift_register |= feedback << 14;
        } else {
            self.timer -= 1;
        }

        let volume = if self.const_vol {
            self.volume
        } else {
            self.env.value
        };

        if self.shift_register & 0x1 == 0 && self.length_counter > 0 {
            self.output = volume;
        } else {
            self.output = 0;
        }
    }

    pub fn tick_half_frame(&mut self) {
        if !self.counter_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub fn tick_quarter_frame(&mut self) {
        self.env.tick();
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enable = enable;
        if !enable {
            self.length_counter = 0;
        }
    }

    pub fn write_r0(&mut self, data: u8) {
        self.volume = data & 0xF;
        self.env.divider_start = self.volume;
        self.env.looping = bit_bool!(data, 5);
        self.counter_halt = bit_bool!(data, 5);
        self.const_vol = bit_bool!(data, 4);
    }

    pub fn write_r2(&mut self, data: u8) {
        self.mode = bit_bool!(data, 7);
        self.period = Self::TIMER_VALUES[(data & 0xF) as usize];
        self.timer = self.period;
    }

    pub fn write_r3(&mut self, data: u8) {
        let counter_load = data >> 3;
        if self.enable {
            self.length_counter = super::LENGTH_VALUES[counter_load as usize];
        };
        self.env.reset = true;
    }
}
