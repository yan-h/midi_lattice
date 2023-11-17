use crate::MidiLatticeParams;
use crate::Voices;

use crate::assets;
use nih_plug::{nih_dbg, nih_log};
use nih_plug_vizia::vizia::cache::BoundingBox;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use triple_buffer::Output;

use crate::editor::{
    CONTAINER_COLOR, CONTAINER_CORNER_RADIUS, CONTAINER_PADDING, HIGHLIGHT_COLOR, NODE_COLOR,
};

pub const NODE_SIZE: f32 = 100.0;
pub const INNER_PADDING: f32 = CONTAINER_PADDING;
pub const NODE_GAP: f32 = CONTAINER_PADDING;

pub struct Lattice {
    params: Arc<MidiLatticeParams>,
    voices_output: Arc<Mutex<Output<Voices>>>,
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
        let mut voices_output = self.voices_output.lock().unwrap();

        canvas.add_font_mem(assets::QUICKSAND_REGULAR);

        let voices: &Voices = voices_output.read();
        let mut nodes: Vec<Node> = Vec::with_capacity(49);
        let bounds = cx.bounds();

        let three: f32 = self.params.tuning_params.three.value() * 0.01;
        let five: f32 = self.params.tuning_params.five.value() * 0.01;
        let seven: f32 = self.params.tuning_params.seven.value() * 0.01;

        let (grid_width, grid_height) = (
            self.params.grid_params.width.load(Ordering::Relaxed),
            self.params.grid_params.height.load(Ordering::Relaxed),
        );
        let (x_tuning_offset, y_tuning_offset) = (
            -(((grid_width - 1) / 2) as i32),
            -((grid_height / 2) as i32),
        );

        let scale: f32 = cx.style.dpi_factor as f32;

        let container_width: f32 = (INNER_PADDING * 2.0
            + NODE_SIZE * grid_width as f32
            + NODE_GAP * (grid_width as f32 - 1.0))
            * scale;
        let container_height: f32 = (INNER_PADDING * 2.0
            + NODE_SIZE * grid_height as f32
            + NODE_GAP * (grid_height as f32 - 1.0))
            * scale;

        let mut container_path = vg::Path::new();
        container_path.rounded_rect(
            CONTAINER_PADDING * scale,
            CONTAINER_PADDING * scale,
            container_width,
            container_height,
            CONTAINER_CORNER_RADIUS * scale,
        );
        container_path.close();
        canvas.fill_path(&mut container_path, &vg::Paint::color(CONTAINER_COLOR));

        // x = threes
        for x_offset in 0..grid_width {
            // y = fives
            for y_offset in 0..grid_height {
                let (x, y) = (
                    (CONTAINER_PADDING
                        + INNER_PADDING
                        + (x_offset as f32) * NODE_SIZE
                        + (x_offset as f32) * NODE_GAP)
                        * scale,
                    (CONTAINER_PADDING
                        + INNER_PADDING
                        + (y_offset as f32) * NODE_SIZE
                        + (y_offset as f32) * NODE_GAP)
                        * scale,
                );
                let (tuning_x, tuning_y) = (
                    x_offset as i32 + x_tuning_offset,
                    y_offset as i32 + y_tuning_offset,
                );
                let pitch_class: f32 =
                    (three * (tuning_x as f32) + five * (tuning_y as f32)) % 12.0;

                let mut path = vg::Path::new();
                path.rounded_rect(
                    x,
                    y,
                    NODE_SIZE * scale,
                    NODE_SIZE * scale,
                    (CONTAINER_CORNER_RADIUS - INNER_PADDING * 0.5) * scale,
                );
                path.close();

                let paint = vg::Paint::color(NODE_COLOR);
                let origin_paint = vg::Paint::color(vg::Color::rgb(0xff, 0xff, 0x00));
                if tuning_x == 0 && tuning_y == 0 {
                    canvas.fill_path(&mut path, &origin_paint);
                } else {
                    canvas.fill_path(&mut path, &paint);
                }
                let mut text_paint = vg::Paint::color(HIGHLIGHT_COLOR);
                text_paint.set_font_size(NODE_SIZE * 0.5 * scale);
                text_paint.set_text_align(vg::Align::Center);

                let _ = canvas.fill_text(
                    x + NODE_SIZE * 0.5 * scale,
                    y + NODE_SIZE * 0.65 * scale,
                    format!("{} {}", tuning_x, tuning_y),
                    &text_paint,
                );
            }
        }
    }

    fn event(&mut self, _cx: &mut EventContext, _event: &mut Event) {}
}
