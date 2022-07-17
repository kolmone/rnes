#[derive(Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub opaque: Vec<bool>,
    pub priority: Vec<bool>,
}

pub struct BGPixel(pub (u8, u8, u8), pub bool);
pub struct SpritePixel(pub (u8, u8, u8), pub bool, pub bool);

impl Frame {
    pub const WIDTH: usize = 256;
    pub const HEIGHT: usize = 240;

    pub fn new() -> Self {
        Frame {
            data: vec![0; Frame::WIDTH * Frame::HEIGHT * 3],
            opaque: vec![false; Frame::WIDTH * Frame::HEIGHT],
            priority: vec![false; Frame::WIDTH * Frame::HEIGHT],
        }
    }

    pub fn set_bg_pixel(&mut self, x: usize, y: usize, pixel: BGPixel) {
        let base = (y * Frame::WIDTH + x) * 3;
        self.data[base] = pixel.0 .0;
        self.data[base + 1] = pixel.0 .1;
        self.data[base + 2] = pixel.0 .2;
        let bool_idx = y * Frame::WIDTH + x;
        self.opaque[bool_idx] = pixel.1;
    }

    pub fn set_sprite_pixel(&mut self, x: usize, y: usize, pixel: SpritePixel) {
        let base = (y * Frame::WIDTH + x) * 3;
        self.data[base] = pixel.0 .0;
        self.data[base + 1] = pixel.0 .1;
        self.data[base + 2] = pixel.0 .2;
        let bool_idx = y * Frame::WIDTH + x;
        self.opaque[bool_idx] = pixel.1;
        let bool_idx = y * Frame::WIDTH + x;
        self.priority[bool_idx] = pixel.2;
    }

    pub fn bg_pixel(&self, x: usize, y: usize) -> BGPixel {
        let base = (y * Frame::WIDTH + x) * 3;
        let bool_idx = y * Frame::WIDTH + x;
        BGPixel(
            (self.data[base], self.data[base + 1], self.data[base + 2]),
            self.opaque[bool_idx],
        )
    }

    pub fn sprite_pixel(&self, x: usize, y: usize) -> SpritePixel {
        let base = (y * Frame::WIDTH + x) * 3;
        let bool_idx = y * Frame::WIDTH + x;
        SpritePixel(
            (self.data[base], self.data[base + 1], self.data[base + 2]),
            self.opaque[bool_idx],
            self.priority[bool_idx],
        )
    }

    pub fn opaque(&self, x: usize, y: usize) -> bool {
        self.opaque[y * Frame::WIDTH + x]
    }

    pub fn priority(&self, x: usize, y: usize) -> bool {
        self.priority[y * Frame::WIDTH + x]
    }
}
