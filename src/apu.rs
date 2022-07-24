use std::sync::mpsc::Sender;

use bitbash::bitfield;

use crate::cpu::__bitfield_StatusReg::zero;

pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,

    output: Vec<f32>,
    output_idx: usize,
    tx: Sender<Vec<f32>>,

    cycle: usize,

    framec_cycle: usize,
    framec_mode: bool,
}

struct Noise {}

fn divide(dividend: f32, divisor: f32, zero_result: f32) -> f32 {
    if divisor == 0.0 {
        return zero_result;
    }
    dividend / divisor
}

impl Apu {
    pub fn new(tx: Sender<Vec<f32>>) -> Self {
        Apu {
            pulse1: Pulse::new(),
            pulse2: Pulse::new(),
            triangle: Triangle::new(),
            noise: Noise {},
            output: vec![0.0; 10000],
            output_idx: 0,
            tx,
            cycle: 0,
            framec_cycle: 0,
            framec_mode: false,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        // println!("Writing {:2X} to {:4X}", data, addr);
        match addr {
            0x4000 => self.pulse1.write_r0(data),
            0x4001 => self.pulse1.write_r1(data),
            0x4002 => self.pulse1.write_r2(data),
            0x4003 => self.pulse1.write_r3(data),

            0x4004 => self.pulse2.write_r0(data),
            0x4005 => self.pulse2.write_r1(data),
            0x4006 => self.pulse2.write_r2(data),
            0x4007 => self.pulse2.write_r3(data),

            0x4008 => self.triangle.write_r0(data),
            0x400A => self.triangle.write_r2(data),
            0x400B => self.triangle.write_r3(data),

            0x4015 => {
                self.pulse1.set_lc_enable(data & 0x01 != 0);
                self.pulse2.set_lc_enable(data & 0x02 != 0);
                self.triangle.set_lc_enable(data & 0x04 != 0);  
            }
            0x4017 => {
                self.framec_mode = data & 0x80 != 0;
                if self.framec_mode {
                    self.cycle = 18639;
                }
            }
            _ => (),
        }
    }

    pub fn read(&self, _addr: u16) -> u8 {
        // match addr {
        //     _ => todo!(),
        // }
        0
    }

    pub fn tick(&mut self) {
        self.cycle += 1;

        self.tick_frame_counter();

        let pulse1_out = self.pulse1.tick(self.cycle % 2 != 0) as f32;
        let pulse2_out = self.pulse2.tick(self.cycle % 2 != 0) as f32;
        // let pulse1_out = 0.0;
        // let pulse2_out = 0.0;
        let tri_out = self.triangle.tick() as f32;
        let noise_out = 8.0;
        let dmc_out = 64.0;
        let pulse_out = divide(95.88, divide(8128.0, pulse1_out + pulse2_out, -100.0) + 100.0, 0.0);
        let tnd_tmp = tri_out / 8227.0 + noise_out / 12241.0 + dmc_out / 22638.0;
        let tnd_out = divide(159.79, divide(1.0, tnd_tmp, -100.0) + 100.0, 0.0);
        let output = pulse_out + tnd_out - 0.5;
        self.output[self.output_idx] = output;
        self.output_idx += 1;
        if self.output_idx >= self.output.len() {
            match self.tx.send(self.output.clone()) {
                Ok(()) => (),
                Err(e) => panic!("Send error: {}", e),
            }
            self.output_idx = 0;
        }
    }

    fn tick_frame_counter(&mut self) {
        if self.cycle % 2 == 0 {
            match self.framec_cycle {
                3728 | 11185 => self.tick_quarter_frame(),
                7456 => {
                    self.tick_quarter_frame();
                    self.tick_half_frame();
                }
                14914 if !self.framec_mode => {
                    self.tick_quarter_frame();
                    self.tick_half_frame();
                }
                18640 if self.framec_mode => {
                    self.tick_quarter_frame();
                    self.tick_half_frame();
                }
                _ => (),
            }
        } else {
            self.framec_cycle += 1;
            if self.framec_mode && self.framec_cycle == 18641
                || !self.framec_mode && self.framec_cycle == 14915
            {
                self.framec_cycle = 0;
            }
        }
    }

    fn tick_quarter_frame(&mut self) {
        self.pulse1.tick_quarter_frame();
        self.pulse2.tick_quarter_frame();
        // self.triangle.tick_quarter_frame();
    }

    fn tick_half_frame(&mut self) {
        self.pulse1.tick_half_frame();
        self.pulse2.tick_half_frame();
        self.triangle.tick_half_frame();
    }
}

#[rustfmt::skip]
const LENGTH_VALUES: [u8; 32] = 
    [10, 254, 20, 2,  40, 4,  80, 6,  160, 8,  60, 10, 14, 12, 26, 14,
     12, 16,  24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30];

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
        length_counter_enable: u8,
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
            self.period - period_shifted
        } else {
            self.period + period_shifted
        };

        if odd {
            if self.length_counter_enable == 0
                || self.length_counter_zero != 0
                || self.timer() < 8
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
        if self.sweep_period < 0 || self.sw_reload != 0 {
            self.sw_reload = 0;
            self.sweep_period = self.sw_period() as i8;
            if self.sw_enable() && self.period >= 8 && self.target_period <= 0x7FF {
                // println!(
                //     "Sweep updating timer! Was {}, now {}",
                //     self.period, self.target_period
                // );
                self.period = self.target_period;
            }
        }

        // Length counter
        if !self.counter_halt() && self.length_counter > 0 {
            self.length_counter -= 1;
            self.length_counter_zero = 0;
            if self.length_counter == 0 {
                println!("Muting channel");
                self.length_counter_zero = 1;
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

    pub fn set_lc_enable(&mut self, enable: bool) {
        self.length_counter_enable = enable as u8;
        self.length_counter = 0;
        self.length_counter_zero = 0;
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
        if self.length_counter_enable != 0 {
            self.length_counter = LENGTH_VALUES[self.counter_load() as usize];
        };
        self.reset_envelope = 1;
    }
}

bitfield! {
    pub struct Triangle{
        r0: u8,
        r2: u8,
        r3: u8,

        timer: u16,

        length_counter: u8,
        length_counter_enable: u8,
        length_counter_zero: u8,

        wave_ptr: usize,
        linear_counter: usize,
    }
    pub new();

    pub field linear_counter: u8 = r0[0..7];
    pub field control: bool = r0[7];
    pub field counter_halt: bool = r0[7];

    pub field timer_lo: u8 = r2[0..8];
    pub field timer_hi: u8 = r3[0..3];
    pub field timer: u16 = r2[0..8] ~ r3[0..3];
    pub field counter_load: u8 = r3[3..8];
}

impl Triangle {

    #[rustfmt::skip]
    const WAVE: [u8; 32] = [
        15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5,  4,  3,  2,  1,  0, 
        0,  1,  2,  3,  4,  5,  6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];

    pub fn tick(&mut self) -> u8 {
        if (self.length_counter_enable != 0 && self.length_counter_zero != 0) || self.linear_counter == 0 {
            return Triangle::WAVE[self.wave_ptr];
        } else if self.timer == 0 {
            self.timer = self.timer();
            if self.wave_ptr == 0 {
                self.wave_ptr = 31;
            } else {
                self.wave_ptr -= 1;
            }
        } else {
            self.timer -= 1;
        }
        Triangle::WAVE[self.wave_ptr]
    }

    pub fn tick_half_frame(&mut self) {
        // Length counter
        if !self.counter_halt() && self.length_counter > 0 {
            self.length_counter -= 1;
            self.length_counter_zero = 0;
            if self.length_counter == 0 {
                println!("Muting channel");
                self.length_counter_zero = 1;
            }
        }
    }

    // pub fn tick_quarter_frame(&mut self) {}

    pub fn set_lc_enable(&mut self, enable: bool) {
        self.length_counter_enable = enable as u8;
        self.length_counter = 0;
        self.length_counter_zero = 0;
    }

    pub fn write_r0(&mut self, data: u8) {
        self.r0 = data;
        self.linear_counter = 1;
    }

    pub fn write_r2(&mut self, data: u8) {
        self.r2 = data;
    }

    pub fn write_r3(&mut self, data: u8) {
        self.r3 = data;
        if self.length_counter_enable != 0 {
            self.length_counter = LENGTH_VALUES[self.counter_load() as usize];
        };
    }
}
