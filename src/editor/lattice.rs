use crate::MidiLatticeParams;
use crate::Voices;

use crate::assets;
use crate::midi::Voice;
use crate::tuning::NoteNameInfo;
use crate::tuning::PrimeCountVector;
use crate::tuning::CENTS_EPSILON;
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

pub const NODE_SIZE: f32 = 50.0;
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

        self.add_fonts_to_canvas(canvas);

        let bounds = cx.bounds();
        let scale: f32 = cx.style.dpi_factor as f32;

        self.draw_container_rectangle(canvas, &bounds, scale);

        let sorted_voices = self.get_sorted_voices();
        let grid_dimensions = Coordinates {
            x: self.params.grid_params.width.load(Ordering::Relaxed) as i32,
            y: self.params.grid_params.height.load(Ordering::Relaxed) as i32,
        };
        // Cents tuning for each prime number
        let tuning_params = TuningParams {
            three: self.params.tuning_params.three.value(),
            five: self.params.tuning_params.five.value(),
            seven: self.params.tuning_params.seven.value(),
        };

        // Draw lattice nodes one by one
        // x = threes
        for x in 0..grid_dimensions.x {
            // y = fives
            for y in 0..grid_dimensions.y {
                self.draw_node(
                    canvas,
                    bounds,
                    scale,
                    grid_dimensions,
                    tuning_params,
                    &sorted_voices,
                    Coordinates { x, y },
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

#[derive(Clone, Copy)]
struct TuningParams {
    three: f32,
    five: f32,
    seven: f32,
}

#[derive(Clone, Copy)]
struct Coordinates {
    x: i32,
    y: i32,
}

// Helper methods for drawing
impl Lattice {
    /// Adds necessary fonts to the canvas, but only if this hasn't be done before
    /// Prevents memory leak from adding fonts every frame
    fn add_fonts_to_canvas(&self, canvas: &mut Canvas) {
        if !self.loaded_font.load(Ordering::Relaxed) {
            let _ = canvas.add_font_mem(assets::ROBOTO_MONO_REGULAR);
            self.loaded_font.store(true, Ordering::Relaxed);
        }
    }

    /// Retrieves the list of voices from the triple buffer, and returns a sorted vector of them
    fn get_sorted_voices(&self) -> Vec<Voice> {
        let mut voices_output = self.voices_output.lock().unwrap();
        let mut sorted_voices: Vec<Voice> = voices_output.read().values().cloned().collect();
        sorted_voices.sort_unstable_by(|v1, v2| {
            v1.get_pitch_class()
                .partial_cmp(&v2.get_pitch_class())
                .unwrap()
        });
        sorted_voices
    }

    /// Draws the "container" rectangle behind the lattice
    fn draw_container_rectangle(&self, canvas: &mut Canvas, bounds: &BoundingBox, scale: f32) {
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
    }

    /// Draws a single node of the lattice
    fn draw_node(
        &self,
        canvas: &mut Canvas,
        bounds: BoundingBox,
        scale: f32,
        grid_dimensions: Coordinates,
        tuning_params: TuningParams,
        sorted_voices: &Vec<Voice>,
        grid_coords: Coordinates,
    ) {
        // Offsets for the coordinates of C on the grid (makes it as close as possible to the center)
        let tuning_coord_offsets = Coordinates {
            x: ((grid_dimensions.x - 1) / 2) as i32,
            y: (grid_dimensions.y / 2) as i32,
        };
        // Number of factors of each prime in the pitch class represented by this node
        let primes = PrimeCountVector::new(
            tuning_coord_offsets.y - i32::from(grid_coords.y),
            i32::from(grid_coords.x - tuning_coord_offsets.x),
            0,
        );
        // Pitch class represented by this node
        let pitch_class =
            primes.cents(tuning_params.three, tuning_params.five, tuning_params.seven);
        // Display info for pitch class
        let note_name_info: NoteNameInfo = primes.note_name_info();

        // Voices whose pitch class matches this node's pitch class
        let matching_voices: Vec<Voice> = get_matching_voices(pitch_class, &sorted_voices);

        // Origin point for drawing this node
        let (draw_x, draw_y) = (
            bounds.x
                + (INNER_PADDING
                    + (grid_coords.x as f32) * NODE_SIZE
                    + (grid_coords.x as f32) * NODE_GAP)
                    * scale,
            bounds.y
                + (INNER_PADDING
                    + (grid_coords.y as f32) * NODE_SIZE
                    + (grid_coords.y as f32) * NODE_GAP)
                    * scale,
        );

        // Node rectangle
        let mut path = vg::Path::new();
        path.rounded_rect(
            draw_x,
            draw_y,
            NODE_SIZE * scale,
            NODE_SIZE * scale,
            (CONTAINER_CORNER_RADIUS - INNER_PADDING * 0.5) * scale,
        );
        path.close();
        if matching_voices.len() > 0 {
            canvas.fill_path(
                &mut path,
                &vg::Paint::color(vg::Color::rgbf(
                    0xa0 as f32 / 255.0,
                    0x77 as f32 / 255.0,
                    0x77 as f32 / 255.0,
                )),
            );
        } else {
            canvas.fill_path(&mut path, &vg::Paint::color(NODE_COLOR));
        }

        // Note letter name
        let mut text_paint = vg::Paint::color(HIGHLIGHT_COLOR);
        text_paint.set_font_size(NODE_SIZE * 0.60 * scale);
        text_paint.set_text_align(vg::Align::Right);
        let _ = canvas.fill_text(
            draw_x + NODE_SIZE * 0.47 * scale,
            draw_y + NODE_SIZE * 0.59 * scale,
            format!("{}", note_name_info.letter_name),
            &text_paint,
        );

        // Sharps or flats
        text_paint.set_font_size(NODE_SIZE * 0.30 * scale);
        text_paint.set_text_align(vg::Align::Left);
        let _ = canvas.fill_text(
            draw_x + NODE_SIZE * 0.48 * scale,
            draw_y + NODE_SIZE * 0.33 * scale,
            note_name_info.sharps_or_flats_str(),
            &text_paint,
        );

        // Syntonic commas
        let _ = canvas.fill_text(
            draw_x + NODE_SIZE * 0.48 * scale,
            draw_y + NODE_SIZE * 0.61 * scale,
            note_name_info.syntonic_comma_str(),
            &text_paint,
        );

        // Tuning in cents
        text_paint.set_font_size(NODE_SIZE * 0.23 * scale);
        text_paint.set_text_align(vg::Align::Center);
        // Adjust pitch class value to make 1199.999 display as 0 instead of 1200.00
        let rounded_pitch_class = if pitch_class >= 1199.995 {
            0.0
        } else {
            pitch_class
        };
        let _ = canvas.fill_text(
            draw_x + NODE_SIZE * 0.5 * scale,
            draw_y + NODE_SIZE * 0.87 * scale,
            format!("{:.2}", rounded_pitch_class),
            &text_paint,
        );
    }
}

/// Given a pitch class an a vector of voices sorted by their pitch class, returns a
/// vector of voices (almost) equal to that pitch class.
fn get_matching_voices(pitch_class: f32, sorted_voices: &Vec<Voice>) -> Vec<Voice> {
    let mut matching_voices: Vec<Voice> = Vec::new();
    // Start at the first voice whose pitch class is greater than or equal to the node's
    let start_idx =
        sorted_voices.partition_point(|v| v.get_pitch_class() < pitch_class - CENTS_EPSILON);
    if start_idx == sorted_voices.len() {
        return matching_voices;
    }
    let mut idx = start_idx;
    // Iterate circularly through the list of sorted voices
    // until the current voice doesn't match,
    // or we've made it back to the starting point
    while (sorted_voices[idx].get_pitch_class() - pitch_class).abs() <= CENTS_EPSILON {
        matching_voices.push(sorted_voices[idx]);
        if idx == sorted_voices.len() - 1 {
            idx = 0;
        } else {
            idx += 1;
        }
        if idx == start_idx {
            break;
        }
    }
    matching_voices
}
