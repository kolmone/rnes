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
    pub const fn new() -> Self {
        Self {
            buttons: [false; 8],
            strobe: false,
            read_ptr: 0,
        }
    }

    // False positive from clippy?
    #[allow(clippy::only_used_in_recursion)]
    pub fn set_button_state(&mut self, button: Button, state: bool) {
        self.buttons[button as usize] = state;
    }

    pub fn write(&mut self, data: u8) {
        if data & 0x1 != 0 {
            self.strobe = true;
        } else if self.strobe {
            self.strobe = false;
            self.read_ptr = 0;
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.strobe {
            self.buttons[0] as u8
        } else if self.read_ptr < 8 {
            let val = self.buttons[self.read_ptr] as u8;
            self.read_ptr += 1;
            val
        } else {
            1
        }
    }
}
