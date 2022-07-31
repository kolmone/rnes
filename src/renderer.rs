mod palette;

use crate::Ppu;
use egui_sdl2_gl::egui::Color32;
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

    pub fn render_texture(&mut self, ppu: &Ppu) -> Vec<Color32> {
        let mut texture = vec![Color32::DARK_RED; 256 * 240];
        for (idx, pixel) in ppu.frame.iter().enumerate() {
            let (r, g, b) = self.palette.palette[*pixel as usize];
            texture[idx] = Color32::from_rgb(r, g, b);
        }

        texture
    }
}
