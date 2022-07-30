mod common;
mod dmc;
mod noise;
mod pulse;
mod triangle;

use dmc::Dmc;
use noise::Noise;
use pulse::Pulse;
use triangle::Triangle;

use super::cartridge::Cartridge;

pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,

    pub output: Vec<f32>,
    output_idx: usize,

    cycle: usize,

    irq_disable: bool,
    irq: bool,

    framec_cycle: usize,
    framec_mode: bool,
}

fn divide(dividend: f32, divisor: f32, zero_result: f32) -> f32 {
    if divisor == 0.0 {
        return zero_result;
    }
    dividend / divisor
}

impl Apu {
    pub fn new() -> Self {
        Self {
            pulse1: Pulse::new(0),
            pulse2: Pulse::new(1),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            output: vec![0.0; crate::APU_FREQ / 120],
            output_idx: 0,
            cycle: 0,
            irq_disable: false,
            irq: false,
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

            0x400C => self.noise.write_r0(data),
            0x400E => self.noise.write_r2(data),
            0x400F => self.noise.write_r3(data),

            0x4010 => self.dmc.write_r0(data),
            0x4011 => self.dmc.write_r1(data),
            0x4012 => self.dmc.write_r2(data),
            0x4013 => self.dmc.write_r3(data),

            0x4015 => {
                // println!("ctrl write {:X}", data);
                self.pulse1.set_enable(data & 0x01 != 0);
                self.pulse2.set_enable(data & 0x02 != 0);
                self.triangle.set_enable(data & 0x04 != 0);
                self.noise.set_enable(data & 0x08 != 0);
                self.dmc.set_enable(data & 0x10 != 0);
            }
            0x4017 => {
                self.irq_disable = data & 0x40 != 0;
                self.irq = if self.irq_disable { false } else { self.irq };
                self.framec_mode = data & 0x80 != 0;
                if self.framec_mode {
                    self.cycle = 18639;
                }
            }
            _ => (),
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                let mut val = (self.pulse1.length_counter > 0) as u8;
                val |= ((self.pulse2.length_counter > 0) as u8) << 1;
                val |= ((self.triangle.length_counter > 0) as u8) << 2;
                val |= ((self.noise.length_counter > 0) as u8) << 3;
                val |= ((self.dmc.bytes_remaining > 0) as u8) << 4;
                val |= (self.irq as u8) << 6;
                val |= (self.dmc.irq as u8) << 7;

                self.irq = false;

                val
            }
            _ => 0,
        }
    }

    pub const fn irq_active(&self) -> bool {
        self.irq | self.dmc.irq
    }

    pub fn tick(&mut self, cartridge: &mut Cartridge) -> bool {
        self.cycle += 1;

        self.tick_frame_counter();

        self.triangle.tick();
        self.dmc.tick(cartridge);
        if self.cycle % 2 == 0 {
            self.pulse1.tick();
            self.pulse2.tick();
            self.noise.tick();
        }

        // let pulse1_out = 0.0;
        // let pulse2_out = 0.0;
        // let tri_out = 0.0;
        // let noise_out = 0.0;
        // let dmc_out = 0.0;

        let pulse1_out = self.pulse1.output as f32;
        let pulse2_out = self.pulse2.output as f32;
        let tri_out = self.triangle.output as f32;
        let noise_out = self.noise.output as f32;
        let dmc_out = self.dmc.output as f32;
        let total_pulse_out = divide(
            95.88,
            divide(8128.0, pulse1_out + pulse2_out, -100.0) + 100.0,
            0.0,
        );
        let tnd_tmp = tri_out / 8227.0 + noise_out / 12241.0 + dmc_out / 22638.0;
        let tnd_out = divide(159.79, divide(1.0, tnd_tmp, -100.0) + 100.0, 0.0);
        let output = total_pulse_out + tnd_out - 0.5;
        self.output[self.output_idx] = output * 0.5;

        self.output_idx += 1;
        if self.output_idx >= self.output.len() {
            self.output_idx = 0;
            return true;
        }
        false
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
                    self.irq = !self.irq_disable;
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
        self.triangle.tick_quarter_frame();
        self.noise.tick_quarter_frame();
    }

    fn tick_half_frame(&mut self) {
        self.pulse1.tick_half_frame();
        self.pulse2.tick_half_frame();
        self.triangle.tick_half_frame();
        self.noise.tick_half_frame();
    }
}

#[rustfmt::skip]
const LENGTH_VALUES: [u8; 32] = 
    [10, 254, 20, 2,  40, 4,  80, 6,  160, 8,  60, 10, 14, 12, 26, 14,
     12, 16,  24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30];
