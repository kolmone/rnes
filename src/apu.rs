use std::sync::mpsc::Sender;

use bitbash::bitfield;

pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,

    output: Vec<f32>,
    output_idx: usize,
    tx: Sender<Vec<f32>>,
}

struct Triangle {}

struct Noise {}

impl Apu {
    pub fn new(tx: Sender<Vec<f32>>) -> Self {
        Apu {
            pulse1: Pulse::new(),
            pulse2: Pulse::new(),
            triangle: Triangle {},
            noise: Noise {},
            output: vec![0.0; 59561],
            output_idx: 0,
            tx,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        // println!("Writing {:2X} to {:4X}", data, addr);
        match addr {
            0x4000 => self.pulse1.r0 = data,
            0x4001 => self.pulse1.r1 = data,
            0x4002 => self.pulse1.r2 = data,
            0x4003 => {
                self.pulse1.r3 = data;
                self.pulse1.sequencer = 0;
            }
            0x4004 => self.pulse2.r0 = data,
            0x4005 => self.pulse2.r1 = data,
            0x4006 => self.pulse2.r2 = data,
            0x4007 => {
                self.pulse2.r3 = data;
                self.pulse2.sequencer = 0;
            }
            _ => (),
        }
    }

    pub fn read(&self, _addr: u16) -> u8 {
        // let addr = addr & 0x17;
        // match addr {
        //     _ => todo!(),
        // }
        0
    }

    pub fn tick(&mut self) {
        let pulse1_out = self.pulse1.tick() as f32;
        let pulse2_out = self.pulse2.tick() as f32;
        let tri_out = 8.0;
        let noise_out = 8.0;
        let dmc_out = 64.0;
        let tnd_out = 0.00851 * tri_out + 0.00494 * noise_out + 0.00335 * dmc_out;
        let pulse_out = 0.00752 * (pulse1_out + pulse2_out);
        let output = pulse_out + tnd_out - 0.44232;
        // println!("Output is {}", output);
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
}

bitfield! {
    pub struct Pulse{
        r0: u8,
        r1: u8,
        r2: u8,
        r3: u8,
        timer: u16,
        sequencer: usize,
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

    pub fn tick(&mut self) -> u8 {
        if self.timer() < 8 {
            return 8;
        }

        if self.timer == 0 {
            self.timer = self.timer();
            if self.sequencer == 0 {
                self.sequencer = 7;
            } else {
                self.sequencer -= 1;
            }
        } else {
            self.timer -= 1;
        }
        self.volume() * Pulse::DUTY_TABLES[self.duty()][self.sequencer]
    }
}
