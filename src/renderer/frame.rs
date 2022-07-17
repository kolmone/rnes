#[derive(Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub opaque: Vec<bool>,
}

impl Frame {}
impl Frame {
    pub const WIDTH: usize = 256;
    pub const HEIGHT: usize = 240;

    pub fn new() -> Self {
        Frame {
            data: vec![0; Frame::WIDTH * Frame::HEIGHT * 3],
            opaque: vec![false; Frame::WIDTH * Frame::HEIGHT],
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, rgb: (u8, u8, u8), opaque: bool) {
        let base = (y * Frame::WIDTH + x) * 3;
        self.data[base] = rgb.0;
        self.data[base + 1] = rgb.1;
        self.data[base + 2] = rgb.2;
        let bool_idx = y * Frame::WIDTH + x;
        self.opaque[bool_idx] = opaque;
    }

    pub fn pixel(&self, x: usize, y: usize) -> ((u8, u8, u8), bool) {
        let base = (y * Frame::WIDTH + x) * 3;
        let bool_idx = y * Frame::WIDTH + x;
        (
            (self.data[base], self.data[base + 1], self.data[base + 2]),
            self.opaque[bool_idx],
        )
    }

    pub fn opaque(&self, x: usize, y: usize) -> bool {
        self.opaque[y * Frame::WIDTH + x]
    }
}
