mod common;
mod noise;
mod pulse;
mod triangle;
mod dmc;

use noise::Noise;
use pulse::Pulse;
use std::sync::mpsc::Sender;
use triangle::Triangle;
use dmc::Dmc;

pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,

    output: Vec<f32>,
    output_idx: usize,
    tx: Sender<Vec<f32>>,

    cycle: usize,

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
    pub fn new(tx: Sender<Vec<f32>>) -> Self {
        Apu {
            pulse1: Pulse::new(0),
            pulse2: Pulse::new(1),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
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

            0x400C => self.noise.write_r0(data),
            0x400E => self.noise.write_r2(data),
            0x400F => self.noise.write_r3(data),

            0x4010 => self.dmc.write_r0(data),
            0x4011 => self.dmc.write_r1(data),
            0x4012 => self.dmc.write_r2(data),
            0x4013 => self.dmc.write_r3(data),

            0x4015 => {
                self.pulse1.set_enable(data & 0x01 != 0);
                self.pulse2.set_enable(data & 0x02 != 0);
                self.triangle.set_enable(data & 0x04 != 0);
                self.noise.set_enable(data & 0x08 != 0);
                self.dmc.set_enable(data & 0x10 != 0);
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

        self.triangle.tick();
        if self.cycle % 2 == 0 {
            self.pulse1.tick();
            self.pulse2.tick();
            self.noise.tick();
            self.dmc.tick();
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
        let pulse_out = divide(
            95.88,
            divide(8128.0, pulse1_out + pulse2_out, -100.0) + 100.0,
            0.0,
        );
        let tnd_tmp = tri_out / 8227.0 + noise_out / 12241.0 + dmc_out / 22638.0;
        let tnd_out = divide(159.79, divide(1.0, tnd_tmp, -100.0) + 100.0, 0.0);
        let output = pulse_out + tnd_out - 0.5;
        self.output[self.output_idx] = output * 0.5;

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
