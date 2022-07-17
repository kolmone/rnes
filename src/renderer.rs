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

    pub fn render_line(
        &mut self,
        ppu: &Ppu,
        canvas: &mut Canvas<Window>,
        texture: &mut Texture,
        debug: bool,
    ) {
        self.render_background_line(ppu, 0);
        self.render_background_line(ppu, 1);
        self.render_sprites(ppu);

        if (ppu.scanline - 1) == 239 {
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
            if debug {
                texture
                    .update(screen_rects.0, &frames.0.data, 256 * 3)
                    .unwrap();
                texture
                    .update(screen_rects.1, &frames.1.data, 256 * 3)
                    .unwrap();
            }

            let mut bg_frame = Frame::new();

            if ppu.vertical_scroll > 0 {
                let scroll = ppu.vertical_scroll as usize;

                for y in 0..Frame::HEIGHT - scroll {
                    for x in 0..Frame::WIDTH {
                        let pixel = frames.0.pixel(x, y + scroll);
                        bg_frame.set_pixel(x, y, pixel.0, pixel.1);
                    }
                }

                for y in 0..scroll {
                    for x in 0..Frame::WIDTH {
                        let pixel = frames.1.pixel(x, y);
                        bg_frame.set_pixel(x, Frame::HEIGHT - scroll + y, pixel.0, pixel.1);
                    }
                }
            } else if ppu.horizontal_scroll > 0 {
                let scroll = ppu.horizontal_scroll as usize;

                for y in 0..Frame::HEIGHT {
                    for x in 0..Frame::WIDTH - scroll {
                        let pixel = frames.0.pixel(x + scroll, y);
                        bg_frame.set_pixel(x, y, pixel.0, pixel.1);
                    }
                }

                for y in 0..Frame::HEIGHT {
                    for x in 0..scroll {
                        let pixel = frames.1.pixel(x, y);
                        bg_frame.set_pixel(Frame::WIDTH - scroll + x, y, pixel.0, pixel.1);
                    }
                }
            } else {
                bg_frame = frames.0.clone();
            }

            if debug {
                let sprite_rect = Rect::new(0, 240, 256, 240);
                texture
                    .update(sprite_rect, &self.sprite_frame.data, 256 * 3)
                    .unwrap();
            }

            for y in 0..240 {
                for x in 0..256 {
                    if self.sprite_frame.opaque(x, y) {
                        let pixel = self.sprite_frame.pixel(x, y);
                        bg_frame.set_pixel(x, y, pixel.0, pixel.1);
                    }
                }
            }

            let final_rect = if debug {
                Rect::new(256, 240, 256, 240)
            } else {
                Rect::new(0, 0, 256, 240)
            };
            texture.update(final_rect, &bg_frame.data, 256 * 3).unwrap();

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
            let (tile_data, tile_palette) = ppu.bg_tile_data(screen, y, tile_on_scanline);

            for i in 0..8 {
                let rgb = self.palette.palette[tile_palette[tile_data[i] as usize] as usize];
                frame.set_pixel(x + 7 - i, y as usize, rgb, tile_data[i] != 0);
            }
            x += 8;
        }
    }

    fn render_sprites(&mut self, ppu: &Ppu) {
        let y = ppu.scanline - 1;
        let line = &ppu.sprite_line;
        for x in 0..256 {
            self.sprite_frame.set_pixel(
                x,
                y as usize,
                self.palette.palette[line[x].0 as usize],
                line[x].1,
            );
        }
    }
}
