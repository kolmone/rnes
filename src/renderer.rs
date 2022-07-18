mod frame;
mod palette;

use crate::Ppu;
use frame::Frame;
use palette::Palette;
use sdl2::{
    rect::Rect,
    render::{Canvas, Texture},
    video::Window,
};

pub struct Renderer {
    palette: Palette,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            palette: Palette::new("cxa.pal"),
        }
    }

    pub fn render_screen(
        &mut self,
        ppu: &Ppu,
        canvas: &mut Canvas<Window>,
        texture: &mut Texture,
        debug: bool,
    ) {
        let final_rect = if debug {
            Rect::new(256, 240, 256, 240)
        } else {
            Rect::new(0, 0, 256, 240)
        };

        let mut frame = Frame::new();
        for (idx, pixel) in ppu.frame.iter().enumerate() {
            frame.set_pixel(
                idx % 256,
                idx / 256,
                self.palette.palette[(*pixel) as usize],
            );
        }

        texture.update(final_rect, &frame.data, 256 * 3).unwrap();

        canvas.copy(texture, None, None).unwrap();
        canvas.present();
    }

    // fn draw_debug_screens(&self, canvas: &mut Canvas<Window>, texture: &mut Texture) {
    //     let screen_rects = (Rect::new(0, 0, 256, 240), Rect::new(256, 0, 256, 240));
    //     texture
    //         .update(screen_rects.0, &self.frame_a.data, 256 * 3)
    //         .unwrap();
    //     texture
    //         .update(screen_rects.1, &self.frame_b.data, 256 * 3)
    //         .unwrap();

    //     let sprite_rect = Rect::new(0, 240, 256, 240);
    //     texture
    //         .update(sprite_rect, &self.sprite_frame.data, 256 * 3)
    //         .unwrap();

    //     let final_rect = Rect::new(256, 240, 256, 240);
    //     texture
    //         .update(final_rect, &self.final_frame.data, 256 * 3)
    //         .unwrap();

    //     canvas.copy(texture, None, None).unwrap();
    //     canvas.present();
    // }
}
