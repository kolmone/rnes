use crate::macros::bit_bool;

#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct Triangle{

    timer: u16,
    enable: bool,

    pub length_counter: u8,

    pub wave_ptr: usize,
    linear_counter: u8,
    reload_linear: bool,

    pub output: u8,

    linear_counter_start: u8,
    control: bool,
    counter_halt: bool,
    timer_start: u16,
}


impl Triangle {

    #[rustfmt::skip]
    const WAVE: [u8; 32] = [
        15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5,  4,  3,  2,  1,  0, 
        0,  1,  2,  3,  4,  5,  6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];

    pub fn tick(&mut self) {
        if !self.enable || self.length_counter == 0 || self.linear_counter == 0 {
            return;
        }
        
        if self.timer == 0 {
            self.timer = self.timer_start;
            if self.wave_ptr == 0 {
                self.wave_ptr = 31;
            } else {
                self.wave_ptr -= 1;
            }
        } else {
            self.timer -= 1;
        }
        self.output = Self::WAVE[self.wave_ptr];
    }

    pub fn tick_half_frame(&mut self) {
        if !self.counter_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub fn tick_quarter_frame(&mut self) {
        if self.reload_linear {
            self.linear_counter = self.linear_counter_start;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }

        // Disable reload if control is clear
        if !self.control {
            self.reload_linear = false;
        }
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enable = enable;
        if !enable {
            self.length_counter = 0;
        }
    }
    

    pub fn write_r0(&mut self, data: u8) {
        self.linear_counter_start = data & 0x7F;
        self.control = bit_bool!(data, 7);
        self.counter_halt = bit_bool!(data, 7);
    }

    pub fn write_r2(&mut self, data: u8) {
        self.timer_start = self.timer_start & 0xFF00 | data as u16;
    }

    pub fn write_r3(&mut self, data: u8) {
        self.timer_start = self.timer_start & 0x00FF | (((data & 0x7) as u16) << 8);
        if self.enable {
            let length_idx = data >> 3;
            self.length_counter = super::LENGTH_VALUES[length_idx as usize];
        };
        self.reload_linear = true;
    }
}