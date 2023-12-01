use crate::MidiLatticeParams;
use crate::Voices;

use crate::assets;
use crate::midi::Voice;
use crate::tuning::NoteNameInfo;
use crate::tuning::PitchClass;
use crate::tuning::PrimeCountVector;
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::vizia::vg::FontId;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use triple_buffer::Output;

use crate::editor::{COLOR_1, COLOR_2, COLOR_3, CONTAINER_CORNER_RADIUS, CONTAINER_PADDING};

pub const NODE_SIZE: f32 = 50.0;
pub const INNER_PADDING: f32 = CONTAINER_PADDING;
pub const NODE_GAP: f32 = CONTAINER_PADDING;

pub struct Grid {
    params: Arc<MidiLatticeParams>,

    // Reads voices from the audio thread
    voices_output: Arc<Mutex<Output<Voices>>>,

    // Need interior mutability to allow mutation from draw()
    font_info: Mutex<FontInfo>,
}

/// Stores info about fonts for femtovg's canvas.
/// I don't know a away to get access to the canvas apart from draw()
/// We only get an immutable reference to Lattice display in draw()
/// Therefore, we wrap this structure in a mutex and update when loading fonts in the first draw.
struct FontInfo {
    loaded: bool,
    font_id: Option<FontId>,
    mono_font_id: Option<FontId>,
}

impl Default for FontInfo {
    fn default() -> FontInfo {
        FontInfo {
            loaded: false,
            font_id: None,
            mono_font_id: None,
        }
    }
}
pub struct Node {
    pitch_class: f64,
    pitches: Vec<f32>,
}

impl Grid {
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
            font_info: Mutex::new(FontInfo::default()),
        }
        .build(cx, |_cx| {})
    }
}

impl View for Grid {
    fn element(&self) -> Option<&'static str> {
        Some("lattice-display")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {}

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let start_time = Instant::now();

        // Load donts, if they haven't been loaded before
        let mut font_info = self.font_info.lock().unwrap();
        if !font_info.loaded {
            font_info.loaded = true;
            font_info.font_id = canvas.add_font_mem(assets::ROBOTO_REGULAR).ok();
            font_info.mono_font_id = canvas.add_font_mem(assets::ROBOTO_MONO_REGULAR).ok();
        }

        let bounds = cx.bounds();
        let scale: f32 = cx.scale_factor() as f32;

        let sorted_voices = self.get_sorted_voices();
        let (grid_width, grid_height) = (
            self.params.grid_params.width.load(Ordering::Relaxed) as i32,
            self.params.grid_params.height.load(Ordering::Relaxed) as i32,
        );

        // Cents tuning for each prime number
        let (three, five, seven) = (
            self.params.tuning_params.three.value(),
            self.params.tuning_params.five.value(),
            self.params.tuning_params.seven.value(),
        );

        let (grid_x, grid_y) = (
            self.params.grid_params.x.value(),
            self.params.grid_params.y.value(),
        );

        // Offsets for the coordinates of C on the grid (makes it as close as possible to the center)
        let (tuning_x_offset, tuning_y_offset) =
            (((grid_width - 1) / 2) as i32, (grid_height / 2) as i32);

        // When grid x or y is not a round number, we need to add a row or column to avoid blanks
        let (extra_right, extra_top) = (
            if grid_x == grid_x.round() { 0 } else { 1 },
            if grid_y == grid_y.round() { 0 } else { 1 },
        );

        // Draw lattice nodes one by one
        // x = threes
        for x in 0..grid_width + extra_right {
            // y = fives
            for y in -extra_top..grid_height {
                // Number of factors of each prime in the pitch class represented by this node
                let primes = PrimeCountVector::new(
                    tuning_y_offset - i32::from(y) + (grid_y.floor() as i32),
                    i32::from(x - tuning_x_offset) + (grid_x.floor() as i32),
                    0,
                );

                // Pitch class represented by this node
                let pitch_class: PitchClass = primes.pitch_class(three, five, seven);

                // Display info for pitch class
                let note_name_info: NoteNameInfo = primes.note_name_info();

                // Voices whose pitch class matches this node's pitch class
                let matching_voices: Vec<Voice> = get_matching_voices(pitch_class, &sorted_voices);

                // Origin point for drawing this node
                let (draw_x, draw_y) = (
                    bounds.x + ((x as f32) * NODE_SIZE + (x as f32) * NODE_GAP) * scale
                        - ((grid_x.rem_euclid(1.0)) * (NODE_SIZE + NODE_GAP) * scale),
                    bounds.y
                        + ((y as f32) * NODE_SIZE + (y as f32) * NODE_GAP) * scale
                        + ((grid_y.rem_euclid(1.0)) * (NODE_SIZE + NODE_GAP) * scale),
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
                    canvas.fill_path(&mut path, &vg::Paint::color(COLOR_2));
                }

                // Note letter name
                let mut text_paint = vg::Paint::color(COLOR_3);
                text_paint.set_font_size(NODE_SIZE * 0.60 * scale);
                text_paint.set_text_align(vg::Align::Right);
                font_info.mono_font_id.map(|f| text_paint.set_font(&[f]));
                let _ = canvas.fill_text(
                    draw_x + NODE_SIZE * 0.48 * scale,
                    draw_y + NODE_SIZE * 0.58 * scale,
                    format!("{}", note_name_info.letter_name),
                    &text_paint,
                );

                // Sharps or flats
                text_paint.set_font_size(NODE_SIZE * 0.29 * scale);
                text_paint.set_text_align(vg::Align::Left);
                let _ = canvas.fill_text(
                    draw_x + NODE_SIZE * 0.48 * scale,
                    draw_y + NODE_SIZE * 0.33 * scale,
                    note_name_info.sharps_or_flats_str(),
                    &text_paint,
                );

                // Syntonic commas - only displayed if four perfect fifths don't make a third
                if ((self.params.tuning_params.three.value() * 4.0).rem_euclid(1200.0)
                    - self.params.tuning_params.five.value())
                .abs()
                    > 0.001
                {
                    let _ = canvas.fill_text(
                        draw_x + NODE_SIZE * 0.48 * scale,
                        draw_y + NODE_SIZE * 0.59 * scale,
                        note_name_info.syntonic_comma_str(),
                        &text_paint,
                    );
                }

                // Tuning in cents
                text_paint.set_font_size(NODE_SIZE * 0.26 * scale);
                text_paint.set_text_align(vg::Align::Center);
                font_info.font_id.map(|f| text_paint.set_font(&[f]));
                let cents_f32 = pitch_class.to_cents_f32();
                let _ = canvas.fill_text(
                    draw_x + NODE_SIZE * 0.5 * scale,
                    draw_y + NODE_SIZE * 0.88 * scale,
                    format!(
                        "{:.2}",
                        // Adjust pitch class value to make 1199.999 display as 0 instead of 1200.00
                        if cents_f32 >= 1199.995 {
                            0.0
                        } else {
                            cents_f32
                        }
                    ),
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
}
// Helper methods for drawing
impl Grid {
    /// Retrieves the list of voices from the triple buffer, and returns a vector of them
    /// sorted by pitch class.
    fn get_sorted_voices(&self) -> Vec<Voice> {
        let mut voices_output = self.voices_output.lock().unwrap();
        let mut sorted_voices: Vec<Voice> = voices_output.read().values().cloned().collect();
        sorted_voices.sort_unstable_by(|v1, v2| v1.get_pitch_class().cmp(&v2.get_pitch_class()));
        sorted_voices
    }
}

/// Returns the subset of a vector of voices with a given pitch class.
fn get_matching_voices(pitch_class: PitchClass, sorted_voices: &Vec<Voice>) -> Vec<Voice> {
    let mut matching_voices: Vec<Voice> = Vec::new();
    // Start at the first voice whose pitch class is greater than or equal to the node's
    let start_idx = sorted_voices.partition_point(|v| v.get_pitch_class() < pitch_class);
    if start_idx == sorted_voices.len() {
        return matching_voices;
    }
    let mut idx = start_idx;
    // Iterate circularly through the list of sorted voices
    // until the current voice doesn't match,
    // or we've made it back to the starting point
    while sorted_voices[idx].get_pitch_class() == pitch_class {
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
