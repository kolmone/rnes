mod frame;
mod palette;

use crate::Ppu;
use frame::Frame;
use palette::Palette;
use sdl2::{rect::Rect, render::Texture};

pub struct Renderer {
    palette: Palette,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            palette: Palette::new("cxa.pal"),
        }
    }

    pub fn render_texture(&mut self, ppu: &Ppu, texture: &mut Texture) {
        let mut frame = Frame::new();
        for (idx, pixel) in ppu.frame.iter().enumerate() {
            frame.set_pixel(idx % 256, idx / 256, self.palette.palette[*pixel as usize]);
        }

        texture
            .update(Rect::new(0, 0, 256, 240), &frame.data, 256 * 3)
            .unwrap();
    }
}
