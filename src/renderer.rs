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
    final_frame: Frame,
    palette: Palette,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            frame_a: Frame::new(),
            frame_b: Frame::new(),
            final_frame: Frame::new(),
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

        let y = ppu.scanline as usize - 1;

        let frames = match (ppu.mirroring, ppu.controller.base_nametable()) {
            (Mirroring::Horizontal, 0 | 1) | (Mirroring::Vertical, 0 | 2) => {
                (&self.frame_a, &self.frame_b)
            }
            (Mirroring::Horizontal, 2 | 3) | (Mirroring::Vertical, 1 | 3) => {
                (&self.frame_b, &self.frame_a)
            }
            _ => panic!("Unsupported"),
        };

        if ppu.vertical_scroll == 0 && ppu.horizontal_scroll == 0 {
            for x in 0..Frame::WIDTH {
                let pixel = frames.0.bg_pixel(x, y);
                self.final_frame.set_bg_pixel(x, y, pixel);
            }
        } else if ppu.vertical_scroll > 0 {
            let scroll = ppu.vertical_scroll as usize;

            if y < Frame::HEIGHT - scroll {
                for x in 0..Frame::WIDTH {
                    let pixel = frames.0.bg_pixel(x, y + scroll);
                    self.final_frame.set_bg_pixel(x, y, pixel);
                }
            } else {
                for x in 0..Frame::WIDTH {
                    let pixel = frames.1.bg_pixel(x, y - (Frame::HEIGHT - scroll));
                    self.final_frame.set_bg_pixel(x, y, pixel);
                }
            }
        } else if ppu.horizontal_scroll > 0 {
            let scroll = ppu.horizontal_scroll as usize;

            for x in 0..Frame::WIDTH {
                let pixel = if x < Frame::WIDTH - scroll {
                    frames.0.bg_pixel(x + scroll, y)
                } else {
                    frames.1.bg_pixel(x - (Frame::WIDTH - scroll), y)
                };
                self.final_frame.set_bg_pixel(x, y, pixel);
            }
        }

        for x in 0..Frame::WIDTH {
            if self.sprite_frame.opaque(x, y)
                && (!self.sprite_frame.priority(x, y) || !self.final_frame.opaque(x, y))
            {
                let pixel = self.sprite_frame.sprite_pixel(x, y);
                self.final_frame.set_sprite_pixel(x, y, pixel);
            }
        }

        if y == 239 {
            if debug {
                self.draw_debug_screens(canvas, texture);
            } else {
                self.draw_screen(canvas, texture);
            }
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
                let pixel = frame::BGPixel(rgb, tile_data[i] != 0);
                frame.set_bg_pixel(x + 7 - i, y as usize, pixel);
            }
            x += 8;
        }
    }

    fn render_sprites(&mut self, ppu: &Ppu) {
        let y = ppu.scanline - 1;
        let line = &ppu.sprite_line;
        for x in 0..256 {
            self.sprite_frame.set_sprite_pixel(
                x,
                y as usize,
                frame::SpritePixel(
                    self.palette.palette[line[x].0 as usize],
                    line[x].1,
                    line[x].2,
                ),
            );
        }
    }

    fn draw_screen(&mut self, canvas: &mut Canvas<Window>, texture: &mut Texture) {
        let final_rect = Rect::new(0, 0, 256, 240);
        texture
            .update(final_rect, &self.final_frame.data, 256 * 3)
            .unwrap();

        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    }

    fn draw_debug_screens(&self, canvas: &mut Canvas<Window>, texture: &mut Texture) {
        let screen_rects = (Rect::new(0, 0, 256, 240), Rect::new(256, 0, 256, 240));
        texture
            .update(screen_rects.0, &self.frame_a.data, 256 * 3)
            .unwrap();
        texture
            .update(screen_rects.1, &self.frame_b.data, 256 * 3)
            .unwrap();

        let sprite_rect = Rect::new(0, 240, 256, 240);
        texture
            .update(sprite_rect, &self.sprite_frame.data, 256 * 3)
            .unwrap();

        let final_rect = Rect::new(256, 240, 256, 240);
        texture
            .update(final_rect, &self.final_frame.data, 256 * 3)
            .unwrap();

        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    }
}
