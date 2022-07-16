mod frame;
mod palette;

use crate::{bus::Mirroring, Ppu};
use frame::Frame;
use palette::Palette;
use sdl2::{
    rect::Rect,
    render::{Canvas, Texture},
    video::Window,
};

pub struct Renderer {
    frame_a: Frame,
    frame_b: Frame,
    sprite_frame: Frame,
    palette: Palette,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            frame_a: Frame::new(),
            frame_b: Frame::new(),
            sprite_frame: Frame::new(),
            palette: Palette::new("cxa.pal"),
        }
    }

    pub fn render_line(&mut self, ppu: &Ppu, canvas: &mut Canvas<Window>, texture: &mut Texture) {
        self.render_background_line(ppu, 0);
        self.render_background_line(ppu, 1);
        self.render_sprites(ppu);

        if (ppu.scanline - 1) == 239 {
            // println!(
            //     "{} mirroring, nametable is {}",
            //     ppu.mirroring,
            //     ppu.controller.get_base_nametable()
            // );
            let screen_rects = (Rect::new(0, 0, 256, 240), Rect::new(256, 0, 256, 240));
            let frames = match (ppu.mirroring, ppu.controller.base_nametable()) {
                (Mirroring::Horizontal, 0 | 1) | (Mirroring::Vertical, 0 | 2) => {
                    (&self.frame_a, &self.frame_b)
                }
                (Mirroring::Horizontal, 2 | 3) | (Mirroring::Vertical, 1 | 3) => {
                    (&self.frame_b, &self.frame_a)
                }
                _ => panic!("Unsupported"),
            };
            texture
                .update(screen_rects.0, &frames.0.data, 256 * 3)
                .unwrap();
            texture
                .update(screen_rects.1, &frames.1.data, 256 * 3)
                .unwrap();

            if ppu.vertical_scroll > 0 {
                let scroll = ppu.vertical_scroll as i32;
                let scroll_rects = (
                    Rect::new(256, 240 + scroll, 256, 240 - scroll as u32),
                    Rect::new(256, 240 + 240 - scroll, 256, scroll as u32),
                );

                texture
                    .update(scroll_rects.0, &frames.0.data, 256 * 3)
                    .unwrap();
                texture
                    .update(scroll_rects.1, &frames.1.data, 256 * 3)
                    .unwrap();
            } else if ppu.horizontal_scroll > 0 {
                let scroll = ppu.horizontal_scroll as i32;
                let scroll_rects = (
                    Rect::new(256 + scroll, 240, 256 - scroll as u32, 240),
                    Rect::new(256 + 256 - scroll, 240, scroll as u32, 240),
                );

                texture
                    .update(scroll_rects.0, &frames.0.data, 256 * 3)
                    .unwrap();
                texture
                    .update(scroll_rects.1, &frames.1.data, 256 * 3)
                    .unwrap();
            } else {
                let final_rect = Rect::new(256, 240, 256, 240);
                texture.update(final_rect, &frames.0.data, 256 * 3).unwrap();
            }

            canvas.copy(&texture, None, None).unwrap();
            canvas.present();
        }
    }

    fn render_background_line(&mut self, ppu: &Ppu, screen: u8) {
        let y = ppu.scanline - 1;

        let frame = if screen == 0 {
            &mut self.frame_a
        } else {
            &mut self.frame_b
        };

        let mut x = 0;

        // Draw each tile
        for tile_on_scanline in 0..32 {
            let (tile_data, tile_palette) = ppu.tile_row_data(screen, y, tile_on_scanline);

            for i in 0..8 {
                let rgb = self.palette.palette[tile_palette[tile_data[i] as usize] as usize];
                frame.set_pixel(x + 7 - i, y as usize, rgb);
            }
            x += 8;
        }
    }

    fn render_sprites(&mut self, ppu: &Ppu) {}
}
