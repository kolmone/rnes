use bitbash::bitfield;

bitfield! {
    pub struct Pulse{
        r0: u8,
        r1: u8,
        r2: u8,
        r3: u8,
        timer: u16,
        period: u16,
        target_period: u16,
        sequencer: usize,
        sweep_period: i8,
        sample: u8,
        sw_reload: u8,
        length_counter: u8,
        enable: u8,
        length_counter_zero: u8,
        envelope_counter: u8,
        envelope_divider: u8,
        reset_envelope: u8,
    }
    pub new();

    pub field volume: u8 = r0[0..4];
    pub field envelope: u8 = r0[0..4];
    pub field const_vol: bool = r0[4];
    pub field env_loop: bool = r0[5];
    pub field counter_halt: bool = r0[5];
    pub field duty: usize = r0[6..8];

    pub field sw_shift: u8 = r1[0..3];
    pub field sw_negate: bool = r1[3];
    pub field sw_period: u8 = r1[4..7];
    pub field sw_enable: bool = r1[7];

    pub field timer_lo: u8 = r2[0..8];
    pub field timer_hi: u8 = r3[0..3];
    pub field timer: u16 = r2[0..8] ~ r3[0..3];
    pub field counter_load: u8 = r3[3..8];
}

impl Pulse {
    const DUTY_TABLES: [[u8; 8]; 4] = [
        [0, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 0, 0, 1, 1],
        [0, 0, 0, 0, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 0, 0],
    ];

    pub fn tick(&mut self, odd: bool) -> u8 {
        let period_shifted = self.period >> self.sw_shift();
        self.target_period = if self.sw_negate() {
            self.period - period_shifted - 1
        } else {
            self.period + period_shifted
        };

        if odd {
            if self.enable == 0
                || self.length_counter_zero != 0
                || self.period < 8
                || self.target_period > 0x7FF
            {
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
                    self.envelope_counter
                };
                self.sample = volume * Pulse::DUTY_TABLES[self.duty()][self.sequencer];
            }
        }
        self.sample
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
        if self.sweep_period < 0 || self.sw_reload != 0 {
            self.sw_reload = 0;
            self.sweep_period = self.sw_period() as i8;
        }

        if !self.counter_halt() {
            if self.length_counter > 0 {
                self.length_counter -= 1;
            }
            if self.length_counter == 0 {
                self.length_counter_zero = 1;
            } else {
                self.length_counter_zero = 0;
            }
        }
    }

    pub fn tick_quarter_frame(&mut self) {
        // Envelope
        if self.reset_envelope != 0 {
            self.envelope_divider = self.envelope();
            self.envelope_counter = 15;
            self.reset_envelope = 0;
        } else if self.envelope_divider == 0 {
            self.envelope_divider = self.envelope();
            if self.env_loop() && self.envelope_counter == 0 {
                self.envelope_counter = 15;
            } else if self.envelope_counter > 0 {
                self.envelope_counter -= 1;
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enable = enable as u8;
        if !enable {
            self.length_counter = 0;
        }
    }

    pub fn write_r0(&mut self, data: u8) {
        self.r0 = data;
    }

    pub fn write_r1(&mut self, data: u8) {
        self.r1 = data;
        self.sw_reload = 1;
    }

    pub fn write_r2(&mut self, data: u8) {
        self.r2 = data;
        self.period = self.timer();
    }

    pub fn write_r3(&mut self, data: u8) {
        self.r3 = data;
        self.period = self.timer();
        self.sequencer = 0;
        if self.enable != 0 {
            self.length_counter = super::LENGTH_VALUES[self.counter_load() as usize];
        };
        self.reset_envelope = 1;
    }
}
