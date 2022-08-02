use crate::macros::bit_bool;

use super::common::Envelope;

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct Pulse {
    idx: u8,
    timer: u16,
    period: u16,
    target_period: u16,
    sequencer: usize,
    sweep_period: i8,
    sw_reload: bool,
    enable: bool,

    env: Envelope,
    pub length_counter: u8,

    pub output: u8,

    volume: u8,
    const_vol: bool,
    counter_halt: bool,
    duty: usize,
    sw_shift: u8,
    sw_negate: bool,
    sw_period: u8,
    sw_enable: bool,
    timer_start: u16,
}

impl Pulse {
    const DUTY_TABLES: [[u8; 8]; 4] = [
        [0, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 0, 0, 1, 1],
        [0, 0, 0, 0, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 0, 0],
    ];

    pub fn new(idx: u8) -> Self {
        Self {
            idx,
            ..Default::default()
        }
    }

    pub fn tick(&mut self) {
        if !self.enable {
            self.output = 0;
            return;
        }

        let period_shifted = self.period >> self.sw_shift;
        self.target_period = if self.sw_negate {
            if self.idx == 0 {
                self.period - period_shifted - 1
            } else {
                self.period - period_shifted
            }
        } else {
            self.period + period_shifted
        };

        let volume = if self.length_counter == 0 || self.period < 8 || self.target_period > 0x7FF {
            0
        } else if self.const_vol {
            self.volume
        } else {
            self.env.value
        };

        if self.timer == 0 {
            self.timer = self.period;
            if self.sequencer == 0 {
                self.sequencer = 7;
            } else {
                self.sequencer -= 1;
            }
        } else {
            self.timer -= 1;
        }
        self.output = volume * Self::DUTY_TABLES[self.duty][self.sequencer];
    }

    pub fn tick_half_frame(&mut self) {
        // Sweep divider always updated no matter if enabled
        self.sweep_period -= 1;

        if self.sweep_period < 0
            && self.sw_enable
            && self.sw_shift > 0
            && self.period >= 8
            && self.target_period <= 0x7FF
        {
            self.period = self.target_period;
        }
        if self.sweep_period < 0 || self.sw_reload {
            self.sw_reload = false;
            self.sweep_period = self.sw_period as i8;
        }

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

    // counter_load: u8 = r3[3..8];

    pub fn write_r0(&mut self, data: u8) {
        self.volume = data & 0xF;

        self.const_vol = bit_bool!(data, 4);
        self.counter_halt = bit_bool!(data, 5);
        self.env.divider_start = self.volume;
        self.env.looping = bit_bool!(data, 5);
        self.duty = (data >> 6) as usize;
    }

    pub fn write_r1(&mut self, data: u8) {
        self.sw_shift = data & 0x7;
        self.sw_negate = bit_bool!(data, 3);
        self.sw_enable = bit_bool!(data, 7);
        self.sw_period = (data >> 4) & 0x7;
        self.sw_reload = true;
    }

    pub fn write_r2(&mut self, data: u8) {
        self.timer_start = self.timer_start & 0xFF00 | data as u16;
        self.period = self.timer_start;
    }

    pub fn write_r3(&mut self, data: u8) {
        self.timer_start = self.timer_start & 0x00FF | (((data & 0x7) as u16) << 8);
        self.period = self.timer_start;
        self.sequencer = 0;
        if self.enable {
            let length_idx = data >> 3;
            self.length_counter = super::LENGTH_VALUES[length_idx as usize];
        };
        self.env.reset = true;
    }
}
