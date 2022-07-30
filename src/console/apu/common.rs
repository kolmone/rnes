#[derive(Default)]
pub struct Envelope {
    pub divider: u8,
    pub value: u8,
    pub reset: bool,
    pub divider_start: u8,
    pub looping: bool,
}

impl Envelope {
    pub fn tick(&mut self) {
        if self.reset {
            self.divider = self.divider_start;
            self.value = 15;
            self.reset = false;
        } else if self.divider == 0 {
            self.divider = self.divider_start;
            if self.looping && self.value == 0 {
                self.value = 15;
            } else if self.value > 0 {
                self.value -= 1;
            }
        } else {
            self.divider -= 1;
        }
    }
}
