
use bitbash::bitfield;

bitfield! {
    #[derive(Default)]
    pub struct Triangle{
        r0: u8,
        r2: u8,
        r3: u8,

        timer: u16,
        enable: bool,

        length_counter: u8,

        wave_ptr: usize,
        linear_counter: u8,
        reload_linear: bool,

        pub output: u8,
    }

    field linear_counter: u8 = r0[0..7];
    field control: bool = r0[7];
    field counter_halt: bool = r0[7];

    field timer_lo: u8 = r2[0..8];
    field timer_hi: u8 = r3[0..3];
    field timer: u16 = r2[0..8] ~ r3[0..3];
    field counter_load: u8 = r3[3..8];
}

impl Triangle {

    #[rustfmt::skip]
    const WAVE: [u8; 32] = [
        15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5,  4,  3,  2,  1,  0, 
        0,  1,  2,  3,  4,  5,  6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];

    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    pub fn tick(&mut self) {
        if !self.enable || self.length_counter == 0 || self.linear_counter == 0 {
            return;
        }
        
        if self.timer == 0 {
            self.timer = self.timer();
            if self.wave_ptr == 0 {
                self.wave_ptr = 31;
            } else {
                self.wave_ptr -= 1;
            }
        } else {
            self.timer -= 1;
        }
        self.output = Triangle::WAVE[self.wave_ptr];
    }

    pub fn tick_half_frame(&mut self) {
        if !self.counter_halt() && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub fn tick_quarter_frame(&mut self) {
        if self.reload_linear {
            self.linear_counter = self.linear_counter();
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }

        // Disable reload if control is clear
        if !self.control() {
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
        self.r0 = data;
    }

    pub fn write_r2(&mut self, data: u8) {
        self.r2 = data;
    }

    pub fn write_r3(&mut self, data: u8) {
        self.r3 = data;
        if self.enable {
            self.length_counter = super::LENGTH_VALUES[self.counter_load() as usize];
        };
        self.reload_linear = true;
    }
}