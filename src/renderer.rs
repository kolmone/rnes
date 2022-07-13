mod frame;

use crate::Ppu;
use frame::Frame;

pub struct Renderer {
    frame: Frame,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            frame: Frame::new(),
        }
    }

    pub fn render_line(&mut self, ppu: &Ppu) {
        println!("Rendering line {}", ppu.scanline - 1);

        let frame = Frame::new();

        // Draw each tile
        for x in 0..32 {}
    }
}
