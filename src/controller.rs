#[derive(Clone, Copy)]
pub enum Button {
    A = 0,
    B,
    Select,
    Start,
    Up,
    Down,
    Left,
    Right,
}

pub struct Controller {
    buttons: [bool; 8],
    strobe: bool,
    read_ptr: usize,
}

impl Controller {
    pub fn new() -> Self {
        Self {
            buttons: [false; 8],
            strobe: false,
            read_ptr: 0,
        }
    }

    pub fn set_button_state(&mut self, button: Button, state: bool) {
        self.buttons[button as usize] = state;
    }

    pub fn write(&mut self, data: u8) {
        if data & 0x1 != 0 {
            self.strobe = true;
        } else {
            if self.strobe == true {
                self.strobe = false;
                self.read_ptr = 0;
            }
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.strobe {
            return self.buttons[0] as u8;
        } else if self.read_ptr < 8 {
            let val = self.buttons[self.read_ptr as usize] as u8;
            self.read_ptr += 1;
            return val;
        } else {
            return 1;
        }
    }
}
