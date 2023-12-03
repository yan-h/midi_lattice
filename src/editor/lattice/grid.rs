use crate::MidiLatticeParams;
use crate::Voices;

use crate::assets;
use crate::editor::make_icon_paint;
use crate::midi::Voice;
use crate::tuning::NoteNameInfo;
use crate::tuning::PitchClass;
use crate::tuning::PitchClassDistance;
use crate::tuning::PrimeCountVector;
use color_space::Lch;
use color_space::Rgb;
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::vizia::vg::FontId;
use once_cell::sync::Lazy;
use std::str::MatchIndices;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use triple_buffer::Output;

use crate::editor::{COLOR_1, COLOR_2, COLOR_3, CORNER_RADIUS, PADDING};

pub const NODE_SIZE: f32 = 50.0;

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

fn lch_to_vg_color(lch_color: Lch) -> vg::Color {
    let rgbcolor = Rgb::from(lch_color);

    vg::Color::rgbf(
        rgbcolor.r as f32 / 255.0,
        rgbcolor.g as f32 / 255.0,
        rgbcolor.b as f32 / 255.0,
    )
}

static CHANNEL_COLORS: Lazy<[vg::Color; 15]> = Lazy::new(|| {
    [
        Lch::new(50.0, 45.0, 35.0),  // 1 red
        Lch::new(65.0, 55.0, 70.0),  // 2 orange
        Lch::new(75.0, 60.0, 90.0),  // 3 yellow
        Lch::new(65.0, 50.0, 120.0), // 4 green
        Lch::new(60.0, 40.0, 280.0), // 5 blue
        Lch::new(50.0, 55.0, 305.0), // 6 purple
        Lch::new(70.0, 30.0, 340.0), // 7 pink
        // TODO: make 8-14 different from 1-7
        Lch::new(50.0, 45.0, 35.0),  // 8 red
        Lch::new(65.0, 55.0, 70.0),  // 9 orange
        Lch::new(75.0, 60.0, 90.0),  // 10 yellow
        Lch::new(65.0, 50.0, 120.0), // 11 green
        Lch::new(60.0, 40.0, 280.0), // 12 blue
        Lch::new(50.0, 55.0, 305.0), // 13 purple
        Lch::new(70.0, 30.0, 340.0), // 14 pink
        // Don't know what to do with 15. 16 is just an outline
        Lch::new(0.0, 0.0, 35.0), // 15 black, because why not
    ]
    .map(|x| lch_to_vg_color(x))
});

impl View for Grid {
    fn element(&self) -> Option<&'static str> {
        Some("lattice-display")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {}

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let start_time = Instant::now();

        // Load fonts, if they haven't been loaded before
        let mut font_info = self.font_info.lock().unwrap();
        if !font_info.loaded {
            font_info.loaded = true;
            font_info.font_id = canvas.add_font_mem(assets::ROBOTO_REGULAR).ok();
            font_info.mono_font_id = canvas.add_font_mem(assets::ROBOTO_MONO_REGULAR).ok();
        }

        let scale: f32 = cx.scale_factor();
        let bounds = cx.bounds();
        let (grid_width, grid_height) = (
            self.params.grid_params.width.load(Ordering::Relaxed) as i32,
            self.params.grid_params.height.load(Ordering::Relaxed) as i32,
        );
        let sorted_voices = self.get_sorted_voices();

        nih_dbg!(&sorted_voices);

        // Cents tuning for each prime number
        let (three, five, seven) = (
            self.params.tuning_params.three.value(),
            self.params.tuning_params.five.value(),
            self.params.tuning_params.seven.value(),
        );

        // Logical grid position
        let (grid_x, grid_y) = (
            self.params.grid_params.x.value(),
            self.params.grid_params.y.value(),
        );

        // Offsets for the coordinates of C on the grid (makes it as close as possible to the center)
        let (tuning_x_offset, tuning_y_offset) =
            (((grid_width - 1) / 2) as i32, (grid_height / 2) as i32);

        // Paints
        let outline_paint_bright = make_icon_paint(COLOR_3, PADDING * 0.25 * scale);
        let outline_paint_dark = make_icon_paint(COLOR_1, PADDING * 0.25 * scale);

        // Hides everything out of bounds - for nodes that stick out when scrolling
        canvas.intersect_scissor(bounds.x, bounds.y, bounds.w, bounds.h);

        // Carve out entire background, with half padding around.
        // This is necessary to use clipping when drawing with femtovg's composite operations.
        // We'll put the background back afterwards.
        canvas.global_composite_operation(vg::CompositeOperation::Xor);
        let mut background_path = vg::Path::new();
        background_path.rounded_rect(
            bounds.x - PADDING * 0.5 * scale,
            bounds.y - PADDING * 0.5 * scale,
            bounds.w + PADDING * scale,
            bounds.h + PADDING * scale,
            (CORNER_RADIUS - PADDING * 0.5 * 0.5) * scale,
        );
        canvas.fill_path(&background_path, &vg::Paint::color(COLOR_1));

        canvas.global_composite_operation(vg::CompositeOperation::SourceOver);

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
                // In comments, we'll use 0-indexed channels to match the code. So 15 is the max.
                let matching_voices: Vec<Voice> = get_matching_voices(pitch_class, &sorted_voices);
                let mut channels: [bool; 16] = [false; 16];
                for v in &matching_voices {
                    channels[v.get_channel() as usize] = true;
                }

                // channel 15 determines whether an outline is drawn, so we only go up to 14 here
                let mut colors: Vec<vg::Color> = Vec::with_capacity(15);
                for channel_num in 0..CHANNEL_COLORS.len() {
                    if channels[channel_num] {
                        colors.push(CHANNEL_COLORS[channel_num]);
                    }
                }

                // Origin point for drawing this node
                let (draw_x, draw_y) = (
                    bounds.x + ((x as f32) * NODE_SIZE + (x as f32) * PADDING) * scale
                        - ((grid_x.rem_euclid(1.0)) * (NODE_SIZE + PADDING) * scale),
                    bounds.y
                        + ((y as f32) * NODE_SIZE + (y as f32) * PADDING) * scale
                        + ((grid_y.rem_euclid(1.0)) * (NODE_SIZE + PADDING) * scale),
                );

                // Node rectangle
                let mut node_path = vg::Path::new();
                node_path.rounded_rect(
                    draw_x,
                    draw_y,
                    NODE_SIZE * scale,
                    NODE_SIZE * scale,
                    (CORNER_RADIUS - PADDING * 0.5) * scale,
                );
                if &colors.len() > &0 {
                    canvas.fill_path(&mut node_path, &vg::Paint::color(colors[0]));
                } else {
                    canvas.fill_path(&mut node_path, &vg::Paint::color(COLOR_2));
                }

                // Color overlay
                canvas.global_composite_operation(vg::CompositeOperation::Atop);
                let overlay_size: f32 = NODE_SIZE * scale; // + PADDING * 0.25 *  scale;
                let overlay_x = draw_x; // - PADDING *  scale * 0.125;
                let overlay_y = draw_y; // - PADDING *  scale * 0.125;
                if colors.len() > 0 {
                    for stripe_idx in 0..12 {
                        if stripe_idx % colors.len() == 0 {
                            continue;
                        }
                        let mut color_path = vg::Path::new();
                        if stripe_idx < 6 {
                            let stripe_offset_a: f32 = stripe_idx as f32 / 6.0 * overlay_size;
                            let stripe_offset_b: f32 = (stripe_idx + 1) as f32 / 6.0 * overlay_size;
                            color_path.move_to(overlay_x + stripe_offset_a, overlay_y);
                            color_path.line_to(overlay_x + stripe_offset_b, overlay_y);
                            color_path.line_to(overlay_x, overlay_y + stripe_offset_b);
                            color_path.line_to(overlay_x, overlay_y + stripe_offset_a);
                            color_path.close();
                        } else {
                            let stripe_offset_a: f32 = (stripe_idx - 6) as f32 / 6.0 * overlay_size;
                            let stripe_offset_b: f32 =
                                (stripe_idx - 6 + 1) as f32 / 6.0 * overlay_size;
                            color_path
                                .move_to(overlay_x + overlay_size, overlay_y + stripe_offset_a);
                            color_path
                                .line_to(overlay_x + overlay_size, overlay_y + stripe_offset_b);
                            color_path
                                .line_to(overlay_x + stripe_offset_b, overlay_y + overlay_size);
                            color_path
                                .line_to(overlay_x + stripe_offset_a, overlay_y + overlay_size);
                            color_path.close();
                        }
                        canvas.fill_path(
                            &color_path,
                            &vg::Paint::color(colors[stripe_idx % colors.len()]),
                        );
                    }
                }
                canvas.global_composite_operation(vg::CompositeOperation::SourceOver);

                // Draw outline for channel 16
                if channels[15] {
                    canvas.stroke_path(&node_path, &outline_paint_bright);
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
                text_paint.set_font_size(NODE_SIZE * 0.25 * scale);
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

        // Replace the background we removed earlier. Use a larger rectangle to avoid black lines
        // at border
        canvas.global_composite_operation(vg::CompositeOperation::DestinationOver);
        let mut background_path_refill = vg::Path::new();
        background_path_refill.rounded_rect(
            bounds.x - PADDING * 0.75 * scale,
            bounds.y - PADDING * 0.75 * scale,
            bounds.w + PADDING * 1.5 * scale,
            bounds.h + PADDING * 1.5 * scale,
            (CORNER_RADIUS - PADDING * 0.26 * 0.5) * scale,
        );
        canvas.fill_path(&background_path_refill, &vg::Paint::color(COLOR_1));

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

/// Tolerance of 1000 microcents - i.e. 0.001 cents.
/// Necessary because the tuning parameter is stored as a float, so less precise than [`PitchClass`]
const TOLERANCE: PitchClassDistance = PitchClassDistance::from_microcents(1000);

/// Returns the subset of a vector of voices with a given pitch class.
fn get_matching_voices(pitch_class: PitchClass, sorted_voices: &Vec<Voice>) -> Vec<Voice> {
    let mut matching_voices: Vec<Voice> = Vec::new();

    if sorted_voices.len() == 0 {
        return matching_voices;
    }

    // Start at the first voice whose pitch class is greater than or equal to the node's
    let start_idx: usize = sorted_voices.partition_point(|v| {
        v.get_pitch_class() < pitch_class
            && v.get_pitch_class().distance_to(pitch_class) > TOLERANCE
    });

    if start_idx == sorted_voices.len() {
        return matching_voices;
    }

    let mut idx = start_idx;

    // Loop forwards from start idx
    loop {
        if sorted_voices[idx]
            .get_pitch_class()
            .distance_to(pitch_class)
            > TOLERANCE
        {
            break;
        }
        matching_voices.push(sorted_voices[idx]);
        if idx == sorted_voices.len() - 1 {
            idx = 0;
        } else {
            idx += 1;
        }
        if idx == start_idx {
            return matching_voices;
        }
    }

    // Loop backwards from start idx
    idx = start_idx;
    loop {
        if idx == 0 {
            idx = sorted_voices.len() - 1;
        } else {
            idx -= 1;
        }
        if sorted_voices[idx]
            .get_pitch_class()
            .distance_to(pitch_class)
            > TOLERANCE
        {
            break;
        }
        matching_voices.push(sorted_voices[idx]);
    }

    matching_voices
}
