mod frame;

use crate::Ppu;
use frame::Frame;
use sdl2::{
    render::{Canvas, Texture},
    video::Window,
};

pub struct Renderer {
    frame: Frame,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            frame: Frame::new(),
        }
    }

    pub fn render_line(&mut self, ppu: &Ppu, canvas: &mut Canvas<Window>, texture: &mut Texture) {
        let y = ppu.scanline - 1;
        // println!("Rendering line {}", y);

        let mut x = 0;

        // Draw each tile
        for tile_on_scanline in 0..32 {
            let tile_data = ppu.get_tile_row_data(y, tile_on_scanline);

            for i in 0..8 {
                let rgb = match tile_data[i] {
                    0 => frame::PALETTE[0x01],
                    1 => frame::PALETTE[0x23],
                    2 => frame::PALETTE[0x27],
                    3 => frame::PALETTE[0x30],
                    _ => panic!("can't be"),
                };
                self.frame.set_pixel(x + 7 - i, y as usize, rgb);
            }
            x += 8;
        }

        if y == 239 {
            // println!("Presenting screen");
            // self.frame.print();
            texture.update(None, &self.frame.data, 256 * 3).unwrap();
            canvas.copy(&texture, None, None).unwrap();
            canvas.present();
        }
    }
}
