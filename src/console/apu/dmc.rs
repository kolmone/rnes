use crate::macros::bit_bool;

use crate::console::cartridge::Cartridge;

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct Dmc {
    enable: bool,
    timer: u16,
    silence: bool,
    pub irq: bool,

    sample_buffer: Option<u8>,
    start_sample: bool,
    sample_addr: u16,
    pub bytes_remaining: u16,

    shift_register: u8,
    bits_remaining: i8,

    pub output: u8,

    rate: u16,
    dmc_loop: bool,
    irq_enable: bool,
    next_sample_addr: u16,
    sample_len: u16,
}

impl Dmc {
    const RATE: [u16; 16] = [
        428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
    ];

    pub fn tick(&mut self, cartridge: &mut Cartridge) {
        if !self.enable {
            return;
        }

        if self.sample_buffer == None && self.bytes_remaining > 0 {
            self.sample_buffer = Some(cartridge.read_cpu(self.sample_addr));

            // Sample address always starts from
            self.sample_addr = if self.sample_addr == 0xFFFF {
                0x8000
            } else {
                self.sample_addr + 1
            };

            self.bytes_remaining -= 1;
            if self.bytes_remaining == 0 && self.dmc_loop {
                self.start_sample = true;
            } else if self.bytes_remaining == 0 {
                self.irq = self.irq_enable;
            }
        }

        if self.start_sample {
            self.sample_addr = 0xC000 | (self.next_sample_addr << 6);
            self.bytes_remaining = self.sample_len;
            self.start_sample = false;
        }

        if self.timer == 0 {
            self.timer = self.rate;

            // Silence flag is set when sample buffer is empty
            if !self.silence {
                let decrement = self.shift_register & 0x01 == 0;
                if decrement && self.output >= 2 {
                    self.output -= 2;
                } else if !decrement && self.output <= 125 {
                    self.output += 2;
                }
            }

            self.shift_register >>= 1;

            self.bits_remaining -= 1;
            if self.bits_remaining <= 0 {
                self.bits_remaining = 8;
                if let Some(value) = self.sample_buffer.take() {
                    self.shift_register = value;
                    self.silence = false;
                } else {
                    self.silence = true;
                }
            }
        } else {
            self.timer -= 1;
        }
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enable = enable;
        self.irq = false;
        if enable {
            self.start_sample = self.bytes_remaining == 0;
        } else {
            self.bytes_remaining = 0;
        }
    }

    pub fn write_r0(&mut self, data: u8) {
        self.irq_enable = bit_bool!(data, 7);
        self.dmc_loop = bit_bool!(data, 6);
        self.rate = Self::RATE[(data & 0xF) as usize] - 1;
        self.timer = self.rate;
        self.irq = if self.irq_enable { self.irq } else { false };
    }

    pub fn write_r1(&mut self, data: u8) {
        self.output = data & 0x7F;
    }

    pub fn write_r2(&mut self, data: u8) {
        self.next_sample_addr = data as u16;
    }

    pub fn write_r3(&mut self, data: u8) {
        self.sample_len = data as u16 * 16 + 1;
    }
}
