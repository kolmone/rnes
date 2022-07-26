use bitbash::bitfield;

bitfield! {
    #[derive(Default)]
    pub struct Dmc{
        r0: u8,
        r1: u8,
        r2: u8,
        r3: u8,

        enable: bool,

        pub output: u8,
    }

    field rate: u8 = r0[0..4];
    field dmc_loop: bool = r0[6];
    field irq_enable: bool = r0[7];

    field direct_load: u8 = r1[0..7];

    field sample_addr: u8 = r2[0..8];
    field sample_len: u8 = r3[0..8];
}

impl Dmc {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn tick(&mut self) {
        // if !self.enable {
        //     self.sample = 0;
        // } else {
        //     if self.timer == 0 {
        //         self.timer = self.period;
        //         if self.sequencer == 0 {
        //             self.sequencer = 7;
        //         } else {
        //             self.sequencer -= 1;
        //         }
        //     } else {
        //         self.timer -= 1;
        //     }
        //     let volume = if self.const_vol() {
        //         self.volume()
        //     } else {
        //         self.env.value
        //     };
        //     self.sample = volume * Pulse::DUTY_TABLES[self.duty()][self.sequencer];
        // }
    }

    pub fn _tick_half_frame(&mut self) {}

    pub fn _tick_quarter_frame(&mut self) {}

    pub fn set_enable(&mut self, enable: bool) {
        self.enable = enable;
    }

    pub fn write_r0(&mut self, data: u8) {
        self.r0 = data;
    }

    pub fn write_r1(&mut self, data: u8) {
        self.r1 = data;
        self.output = self.direct_load();
    }

    pub fn write_r2(&mut self, data: u8) {
        self.r2 = data;
    }

    pub fn write_r3(&mut self, data: u8) {
        self.r3 = data;
    }
}
