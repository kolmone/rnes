mod palette;

use crate::console::SCREEN_HEIGHT;
use crate::console::SCREEN_WIDTH;
use crate::Ppu;
use eyre::Result;
use palette::Palette;

pub struct Renderer {
    palette: Palette,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            palette: Palette::new("cxa.pal")?,
        })
    }

    pub fn render_texture(&mut self, ppu: &Ppu) -> Vec<u8> {
        let mut texture = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
        for (idx, pixel) in ppu.frame.iter().enumerate() {
            let (r, g, b) = self.palette.palette[*pixel as usize];
            texture[idx * 4] = r;
            texture[idx * 4 + 1] = g;
            texture[idx * 4 + 2] = b;
            texture[idx * 4 + 3] = 255;
        }

        texture
    }
}
