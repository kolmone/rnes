use bitbash::bitfield;

use super::common::{Envelope, LengthCounter};

bitfield! {
    #[derive(Default)]
    pub struct Pulse{
        r0: u8,
        r1: u8,
        r2: u8,
        r3: u8,
        idx: u8,
        timer: u16,
        period: u16,
        target_period: u16,
        sequencer: usize,
        sweep_period: i8,
        sw_reload: bool,
        enable: bool,

        env: Envelope,
        lc: LengthCounter,

        pub sample: u8,
    }

    field volume: u8 = r0[0..4];
    field envelope: u8 = r0[0..4];
    field const_vol: bool = r0[4];
    field env_loop: bool = r0[5];
    field counter_halt: bool = r0[5];
    field duty: usize = r0[6..8];

    field sw_shift: u8 = r1[0..3];
    field sw_negate: bool = r1[3];
    field sw_period: u8 = r1[4..7];
    field sw_enable: bool = r1[7];

    field timer_lo: u8 = r2[0..8];
    field timer_hi: u8 = r3[0..3];
    field timer: u16 = r2[0..8] ~ r3[0..3];
    field counter_load: u8 = r3[3..8];
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
        let period_shifted = self.period >> self.sw_shift();
        self.target_period = if self.sw_negate() {
            if self.idx == 0 {
                self.period - period_shifted - 1
            } else {
                self.period - period_shifted
            }
        } else {
            self.period + period_shifted
        };

        if !self.enable || self.lc.muting || self.period < 8 || self.target_period > 0x7FF {
            self.sample = 0;
        } else {
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
            let volume = if self.const_vol() {
                self.volume()
            } else {
                self.env.value
            };
            self.sample = volume * Pulse::DUTY_TABLES[self.duty()][self.sequencer];
        }
    }

    pub fn tick_half_frame(&mut self) {
        // Sweep divider always updated no matter if enabled
        self.sweep_period -= 1;

        if self.sweep_period < 0
            && self.sw_enable()
            && self.sw_shift() > 0
            && self.period >= 8
            && self.target_period <= 0x7FF
        {
            self.period = self.target_period;
        }
        if self.sweep_period < 0 || self.sw_reload {
            self.sw_reload = false;
            self.sweep_period = self.sw_period() as i8;
        }

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

    pub fn write_r1(&mut self, data: u8) {
        self.r1 = data;
        self.sw_reload = true;
    }

    pub fn write_r2(&mut self, data: u8) {
        self.r2 = data;
        self.period = self.timer();
    }

    pub fn write_r3(&mut self, data: u8) {
        self.r3 = data;
        self.period = self.timer();
        self.sequencer = 0;
        if self.enable {
            self.lc.counter = super::LENGTH_VALUES[self.counter_load() as usize];
        };
        self.env.reset = true;
    }
}
