mod frame;

use crate::ppu::Ppu;

pub struct Renderer {}

impl Renderer {
    pub fn render_line(&mut self, ppu: &Ppu) {
        println!("Rendering line {}", ppu.scanline - 1);
    }
}
