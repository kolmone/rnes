mod renderer;
mod ui;

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Q_BUTTERWORTH_F32};

use eyre::eyre;
use eyre::Result;
use rubato::InterpolationParameters;
use rubato::InterpolationType;
use rubato::WindowFunction;
use rubato::{Resampler, SincFixedIn};
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    Sdl,
};

use crate::macros::fw_error;
use crate::{console::apu::Apu, console::controller::Controller, console::ppu::Ppu};
use renderer::Renderer;
use ui::Ui;

pub struct Emulator {
    renderer: Renderer,
    audio_handler: AudioHandler,
    audio_device: AudioQueue<f32>,
    ui: Ui,
}

impl Emulator {
    pub fn new(fullscreen: bool) -> Result<Self> {
        let sdl = fw_error!(sdl2::init());

        let renderer = Renderer::new()?;
        let audio_device = Self::init_audio(&sdl)?;

        let audio_handler = AudioHandler::new(48000, crate::APU_FREQ / 120)?;

        let ui = Ui::new(&sdl, fullscreen)?;

        Ok(Self {
            renderer,
            audio_handler,
            audio_device,
            ui,
        })
    }

    fn init_audio(sdl: &Sdl) -> Result<AudioQueue<f32>> {
        let audio_spec = AudioSpecDesired {
            freq: Some(48000),
            channels: Some(1),
            samples: Some(1024),
        };
        let audio = fw_error!(sdl.audio());

        let device = fw_error!(audio.open_queue(None, &audio_spec));
        device.resume();
        Ok(device)
    }

    pub fn handle_io(&mut self, ppu: &Ppu, controller: &mut Controller) {
        let game_texture = self.renderer.render_texture(ppu);
        self.ui.update(game_texture, controller);
        self.ui.handle_input(controller);
    }

    pub fn handle_audio(&mut self, apu: &Apu) -> Result<()> {
        self.audio_handler
            .process(&apu.output, &mut self.audio_device)
    }
}

struct AudioHandler {
    output_data: Vec<Vec<f32>>,
    resampler: SincFixedIn<f32>,
    samples_processed: usize,
    samples_received: usize,
    lp_14khz: DirectForm2Transposed<f32>,
    hp_90hz: DirectForm2Transposed<f32>,
    hp_440hz: DirectForm2Transposed<f32>,
    average_buff: usize,
    pub average_history: Vec<f32>,
}

impl AudioHandler {
    const TARGET_BUFFER_LEN: usize = 1200;
    const BUFFER_LEN_TOLERANCE: usize = 50;
    const BUFFER_LOW_LIMIT: usize = Self::TARGET_BUFFER_LEN - Self::BUFFER_LEN_TOLERANCE;
    const BUFFER_HIGH_LIMIT: usize = Self::TARGET_BUFFER_LEN + Self::BUFFER_LEN_TOLERANCE;

    const RATIO_FILL: f64 = 1.003;
    const RATIO_EMPTY: f64 = 1.0 / Self::RATIO_FILL;
    const RATIO_NORMAL: f64 = 1.0;

    fn new(out_freq: usize, input_len: usize) -> Result<Self> {
        let params = InterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: InterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let resampler = SincFixedIn::new(
            out_freq as f64 / crate::APU_FREQ as f64,
            1.01,
            params,
            input_len,
            1,
        )?;

        let coeffs = match Coefficients::<f32>::from_params(
            biquad::Type::SinglePoleLowPass,
            48.khz(),
            14.khz(),
            Q_BUTTERWORTH_F32,
        ) {
            Ok(v) => v,
            Err(_) => return Err(eyre!("Failed to build filter coefficients")),
        };

        let lp_14khz = DirectForm2Transposed::<f32>::new(coeffs);

        let omega = 2.0 * core::f32::consts::PI * 90.0 / 48000.0;
        let alpha = 1.0 / (omega + 1.0);
        let coeffs = Coefficients {
            a1: -alpha,
            a2: 0.0,
            b0: alpha,
            b1: -alpha,
            b2: 0.0,
        };
        let hp_90hz = DirectForm2Transposed::<f32>::new(coeffs);

        let omega = 2.0 * core::f32::consts::PI * 440.0 / 48000.0;
        let alpha = 1.0 / (omega + 1.0);
        let coeffs = Coefficients {
            a1: -alpha,
            a2: 0.0,
            b0: alpha,
            b1: -alpha,
            b2: 0.0,
        };
        let hp_440hz = DirectForm2Transposed::<f32>::new(coeffs);

        Ok(Self {
            output_data: vec![vec![0.0; resampler.output_frames_max()]; 1],
            resampler,
            samples_processed: 0,
            samples_received: 0,
            lp_14khz,
            hp_90hz,
            hp_440hz,
            average_buff: 0,
            average_history: vec![0.0; 100],
        })
    }

    fn process(&mut self, input: &[f32], queue: &mut AudioQueue<f32>) -> Result<()> {
        if self.samples_received == 0 {
            match queue.queue_audio(&[0.0; 1200]) {
                Ok(_) => (),
                Err(e) => return Err(eyre!(e)),
            }
        }

        let samples = self.resampler.input_frames_next();
        self.samples_received += input.len();

        let queue_size = queue.size() / 4;
        self.average_buff -= self.average_buff / 100;
        self.average_buff += queue_size as usize / 100;

        self.average_history = self.average_history[1..].to_vec();
        self.average_history.push(queue_size as f32);
        // println!("Average buffer length is {}", self.average_buff);

        match self.average_buff {
            0..=Self::BUFFER_LOW_LIMIT => self
                .resampler
                .set_resample_ratio_relative(Self::RATIO_FILL)?,
            Self::BUFFER_HIGH_LIMIT.. => self
                .resampler
                .set_resample_ratio_relative(Self::RATIO_EMPTY)?,
            _ => self
                .resampler
                .set_resample_ratio_relative(Self::RATIO_NORMAL)?,
        }

        // println!("next samples is {}", self.resampler.output_frames_next());

        self.resampler
            .process_into_buffer(&[input; 1], &mut self.output_data, Some(&[true; 1]))?;
        // println!("Out buffer is {} samples", self.output_data[0].len());

        let output: Vec<f32> = self.output_data[0]
            .iter()
            .map(|x| self.lp_14khz.run(*x))
            // .map(|x| self.hp_90hz.run(x))
            // .map(|x| self.hp_440hz.run(x))
            .collect();

        match queue.queue_audio(&output) {
            Ok(_) => (),
            Err(e) => return Err(eyre!(e)),
        }

        self.samples_processed += samples;
        Ok(())
    }
}
