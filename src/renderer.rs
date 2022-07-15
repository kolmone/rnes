mod frame;
mod palette;

use crate::Ppu;
use frame::Frame;
use palette::Palette;
use sdl2::{
    render::{Canvas, Texture},
    video::Window,
};

pub struct Renderer {
    frame: Frame,
    palette: Palette,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            frame: Frame::new(),
            palette: Palette::new("cxa.pal"),
        }
    }

    pub fn render_line(&mut self, ppu: &Ppu, canvas: &mut Canvas<Window>, texture: &mut Texture) {
        self.render_screen_line(ppu, 0);

        if (ppu.scanline - 1) == 239 {
            // println!("Presenting screen");
            texture.update(None, &self.frame.data, 256 * 3).unwrap();
            canvas.copy(&texture, None, None).unwrap();
            canvas.present();
        }
    }

    fn render_screen_line(&mut self, ppu: &Ppu, screen: u8) {
        let y = ppu.scanline - 1;
        // println!("Rendering line {}", y);

        let mut x = 0;

        // Draw each tile
        for tile_on_scanline in 0..32 {
            let (tile_data, attribute) = ppu.get_tile_row_data(y, tile_on_scanline);
            let tile_palette = ppu.get_background_palette(attribute);

            for i in 0..8 {
                let rgb = self.palette.palette[tile_palette[tile_data[i] as usize] as usize];
                self.frame.set_pixel(x + 7 - i, y as usize, rgb);
            }
            x += 8;
        }
    }
}
