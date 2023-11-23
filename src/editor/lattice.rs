use crate::MidiLatticeParams;
use crate::Voices;

use crate::assets;
use crate::tuning::NoteNameInfo;
use crate::tuning::PrimeCountVector;
use nih_plug::{nih_dbg, nih_log};
use nih_plug_vizia::vizia::cache::BoundingBox;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use triple_buffer::Output;

use crate::editor::{
    CONTAINER_COLOR, CONTAINER_CORNER_RADIUS, CONTAINER_PADDING, HIGHLIGHT_COLOR, NODE_COLOR,
};

pub const NODE_SIZE: f32 = 40.0;
pub const INNER_PADDING: f32 = CONTAINER_PADDING;
pub const NODE_GAP: f32 = CONTAINER_PADDING;

pub struct Lattice {
    params: Arc<MidiLatticeParams>,
    voices_output: Arc<Mutex<Output<Voices>>>,

    // Need interior mutability just to allow mutation from &self in draw()
    loaded_font: AtomicBool,
}

pub struct Node {
    pitch_class: f64,
    pitches: Vec<f32>,
}

impl Lattice {
    pub fn new<LParams, LVoices>(
        cx: &mut Context,
        params: LParams,
        voices_output: LVoices,
    ) -> Handle<Self>
    where
        LParams: Lens<Target = Arc<MidiLatticeParams>>,
        LVoices: Lens<Target = Arc<Mutex<Output<Voices>>>>,
    {
        Self {
            params: params.get(cx),
            voices_output: voices_output.get(cx),
            loaded_font: AtomicBool::new(false),
        }
        .build(
            cx,
            // This is an otherwise empty element only used for custom drawing
            |_cx| (),
        )
    }
}

impl View for Lattice {
    fn element(&self) -> Option<&'static str> {
        Some("lattice")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let start_time = Instant::now();

        let mut voices_output = self.voices_output.lock().unwrap();

        // Load font if we haven't already
        if !self.loaded_font.load(Ordering::Relaxed) {
            let _ = canvas.add_font_mem(assets::JET_BRAINS_MONO_REGULAR);
            self.loaded_font.store(true, Ordering::Relaxed);
        }

        let voices: &Voices = voices_output.read();
        let bounds = cx.bounds();

        let three: f32 = self.params.tuning_params.three.value() * 0.01;
        let five: f32 = self.params.tuning_params.five.value() * 0.01;
        let seven: f32 = self.params.tuning_params.seven.value() * 0.01;

        let (grid_width, grid_height) = (
            self.params.grid_params.width.load(Ordering::Relaxed),
            self.params.grid_params.height.load(Ordering::Relaxed),
        );
        let (x_tuning_offset, y_tuning_offset) =
            (((grid_width - 1) / 2) as i32, ((grid_height / 2) as i32));

        let scale: f32 = cx.style.dpi_factor as f32;

        let mut nodes: Vec<Node> =
            Vec::with_capacity(usize::from(grid_width as usize * grid_height as usize));

        let mut container_path = vg::Path::new();
        container_path.rounded_rect(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            CONTAINER_CORNER_RADIUS * scale,
        );

        container_path.close();
        canvas.fill_path(&mut container_path, &vg::Paint::color(CONTAINER_COLOR));

        // x = threes
        for x_offset in 0..(grid_width as i32) {
            // y = fives
            for y_offset in 0..(grid_height as i32) {
                let (x, y) = (
                    bounds.x
                        + (INNER_PADDING
                            + (x_offset as f32) * NODE_SIZE
                            + (x_offset as f32) * NODE_GAP)
                            * scale,
                    bounds.y
                        + (INNER_PADDING
                            + (y_offset as f32) * NODE_SIZE
                            + (y_offset as f32) * NODE_GAP)
                            * scale,
                );
                let primes = PrimeCountVector::new(
                    y_tuning_offset - i32::from(y_offset),
                    i32::from(x_offset - x_tuning_offset),
                    0,
                );
                let note_name_info: NoteNameInfo = primes.note_name_info();

                // Background rectangle
                let mut path = vg::Path::new();
                path.rounded_rect(
                    x,
                    y,
                    NODE_SIZE * scale,
                    NODE_SIZE * scale,
                    (CONTAINER_CORNER_RADIUS - INNER_PADDING * 0.5) * scale,
                );
                path.close();
                canvas.fill_path(&mut path, &vg::Paint::color(NODE_COLOR));

                // Draw text
                let mut text_paint = vg::Paint::color(HIGHLIGHT_COLOR);
                text_paint.set_font_size(NODE_SIZE * 0.65 * scale);
                text_paint.set_text_align(vg::Align::Right);

                // Note letter name
                let _ = canvas.fill_text(
                    x + NODE_SIZE * 0.48 * scale,
                    y + NODE_SIZE * 0.65 * scale,
                    format!("{}", note_name_info.letter_name),
                    &text_paint,
                );

                // Sharps or flats
                text_paint.set_font_size(NODE_SIZE * 0.33 * scale);
                text_paint.set_text_align(vg::Align::Left);
                let _ = canvas.fill_text(
                    x + NODE_SIZE * 0.47 * scale,
                    y + NODE_SIZE * 0.38 * scale,
                    note_name_info.sharps_or_flats_str(),
                    &text_paint,
                );

                // Syntonic commas
                let _ = canvas.fill_text(
                    x + NODE_SIZE * 0.47 * scale,
                    y + NODE_SIZE * 0.67 * scale,
                    note_name_info.syntonic_comma_str(),
                    &text_paint,
                );
            }
        }
        /*
        nih_log!(
            "*** draw() finished in {} us",
            start_time.elapsed().as_micros()
        );
        */
    }

    fn event(&mut self, _cx: &mut EventContext, _event: &mut Event) {}
}
