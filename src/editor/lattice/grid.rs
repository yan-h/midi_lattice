use crate::MidiLatticeParams;
use crate::Voices;

use crate::assets;
use crate::editor::make_icon_paint;
use crate::editor::make_icon_stroke_paint;
use crate::editor::COLOR_0;
use crate::editor::COLOR_1_DARKER;
use crate::midi::Voice;
use crate::tuning::NoteNameInfo;
use crate::tuning::PitchClass;
use crate::tuning::PitchClassDistance;
use crate::tuning::PrimeCountVector;
use crate::tuning::CENTS_TO_MICROCENTS;
use color_space::Lch;
use color_space::Rgb;
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::vizia::vg::FontId;
use once_cell::sync::Lazy;
use std::f32::consts::PI;
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

    fn load_and_get_fonts(&self, canvas: &mut Canvas) -> (Option<FontId>, Option<FontId>) {
        let mut font_info = self.font_info.lock().unwrap();
        if !font_info.loaded {
            font_info.loaded = true;
            font_info.font_id = canvas.add_font_mem(assets::ROBOTO_REGULAR).ok();
            font_info.mono_font_id = canvas.add_font_mem(assets::ROBOTO_MONO_REGULAR).ok();
        }
        (font_info.font_id, font_info.mono_font_id)
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

/// Arguments used to draw the grid. Passed into sub-methods of [`Grid::draw()`].
struct DrawGridArgs {
    scale: f32,
    scaled_node_size: f32,
    scaled_padding: f32,
    scaled_corner_radius: f32,
    bounds: BoundingBox,
    grid_width: i32,
    grid_height: i32,
    grid_x: f32,
    grid_y: f32,
    grid_z: i32,
    sorted_voices: Vec<Voice>,
    three_tuning: PitchClass,
    five_tuning: PitchClass,
    seven_tuning: PitchClass,
    font_id: Option<FontId>,
    mono_font_id: Option<FontId>,
}

impl DrawGridArgs {
    fn new(grid: &Grid, cx: &mut DrawContext, canvas: &mut Canvas) -> DrawGridArgs {
        let (font_id, mono_font_id): (Option<FontId>, Option<FontId>) =
            grid.load_and_get_fonts(canvas);

        DrawGridArgs {
            scale: cx.scale_factor(),
            scaled_node_size: NODE_SIZE * cx.scale_factor(),
            scaled_padding: PADDING * cx.scale_factor(),
            scaled_corner_radius: CORNER_RADIUS * cx.scale_factor(),
            bounds: cx.bounds(),
            grid_width: grid.params.grid_params.width.load(Ordering::Relaxed) as i32,
            grid_height: grid.params.grid_params.height.load(Ordering::Relaxed) as i32,
            grid_x: grid.params.grid_params.x.value(),
            grid_y: grid.params.grid_params.y.value(),
            grid_z: grid.params.grid_params.z.value(),
            sorted_voices: grid.get_sorted_voices(),
            three_tuning: PitchClass::from_cents_f32(grid.params.tuning_params.three.value()),
            five_tuning: PitchClass::from_cents_f32(grid.params.tuning_params.five.value()),
            seven_tuning: PitchClass::from_cents_f32(grid.params.tuning_params.seven.value()),
            font_id,
            mono_font_id,
        }
    }
}

struct DrawNodeArgs {
    draw_node_x: f32,
    draw_node_y: f32,
    base_x: i32,
    base_y: i32,
    base_z: i32,
    matching_voices: Vec<Voice>,
    pitch_class: PitchClass,
    note_name_info: NoteNameInfo,
    colors: Vec<vg::Color>,
    draw_outline: bool,
}

impl DrawNodeArgs {
    fn new(
        args: &DrawGridArgs,
        base_x: i32,
        base_y: i32,
        base_z: i32,
        primes: PrimeCountVector,
    ) -> Self {
        let (draw_node_x, draw_node_y): (f32, f32) = (
            args.bounds.x + ((base_x as f32) * NODE_SIZE + (base_x as f32) * PADDING) * args.scale
                - ((args.grid_x.rem_euclid(1.0)) * (NODE_SIZE + PADDING) * args.scale),
            args.bounds.y
                + ((base_y as f32) * NODE_SIZE + (base_y as f32) * PADDING) * args.scale
                + ((args.grid_y.rem_euclid(1.0)) * (NODE_SIZE + PADDING) * args.scale),
        );

        // Pitch class represented by this node
        let pitch_class: PitchClass =
            primes.pitch_class(args.three_tuning, args.five_tuning, args.seven_tuning);

        let matching_voices = get_matching_voices(pitch_class, &args.sorted_voices);
        let note_name_info = primes.note_name_info();

        // Voices whose pitch class matches this node's pitch class
        // In comments, we'll use 0-indexed channels to match the code. So 15 is the max.
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

        let draw_outline: bool = channels[15];

        DrawNodeArgs {
            draw_node_x,
            draw_node_y,
            base_x,
            base_y,
            base_z,
            pitch_class,
            matching_voices,
            note_name_info,
            colors,
            draw_outline,
        }
    }
}

fn prepare_canvas(cx: &mut DrawContext, canvas: &mut Canvas, args: &DrawGridArgs) {
    // Hides everything out of args.bounds - for nodes that stick out when scrolling
    canvas.intersect_scissor(
        args.bounds.x - args.scaled_padding * 0.5,
        args.bounds.y - args.scaled_padding * 0.5,
        args.bounds.w + args.scaled_padding,
        args.bounds.h + args.scaled_padding,
    );

    // Carve out entire background, with half padding around.
    // This is necessary to use clipping when drawing with femtovg's composite operations.
    // We'll put the background back afterwards.
    canvas.global_composite_operation(vg::CompositeOperation::DestinationOut);
    let mut background_path = vg::Path::new();
    background_path.rounded_rect(
        args.bounds.x - args.scaled_padding * 0.4,
        args.bounds.y - args.scaled_padding * 0.4,
        args.bounds.w + args.scaled_padding * 0.8,
        args.bounds.h + args.scaled_padding * 0.8,
        (CORNER_RADIUS - PADDING * 0.2 * 0.5) * args.scale,
    );
    canvas.fill_path(&background_path, &vg::Paint::color(COLOR_1));
    canvas.global_composite_operation(vg::CompositeOperation::SourceOver);
}

fn finish_canvas(cx: &mut DrawContext, canvas: &mut Canvas, args: &DrawGridArgs) {
    // Restore the background rectangle that we removed in prepare_canvas()
    canvas.global_composite_operation(vg::CompositeOperation::DestinationOver);
    let mut background_path_refill = vg::Path::new();
    background_path_refill.rounded_rect(
        args.bounds.x - args.scaled_padding,
        args.bounds.y - args.scaled_padding,
        args.bounds.w + args.scaled_padding * 2.0,
        args.bounds.h + args.scaled_padding * 2.0,
        args.scaled_corner_radius,
    );
    canvas.fill_path(&background_path_refill, &vg::Paint::color(COLOR_1));
}

fn draw_node(canvas: &mut Canvas, args: &DrawGridArgs, node_args: &DrawNodeArgs) {
    if node_args.base_z == 0 {
        draw_node_zero_z(canvas, args, node_args);
    } else {
        draw_node_nonzero_z(canvas, args, node_args);
    }
}

fn draw_extra_colors(
    canvas: &mut Canvas,
    node_args: &DrawNodeArgs,
    x: f32,
    y: f32,
    size: f32,
    half_num_stripes: u8,
) {
    if node_args.colors.len() > 0 {
        for stripe_idx in 0..half_num_stripes * 2 {
            if stripe_idx as usize % node_args.colors.len() == 0 {
                continue;
            }
            let mut color_path = vg::Path::new();
            if stripe_idx < half_num_stripes {
                let stripe_offset_a: f32 = stripe_idx as f32 / half_num_stripes as f32 * size;
                let stripe_offset_b: f32 = (stripe_idx + 1) as f32 / half_num_stripes as f32 * size;
                color_path.move_to(x + stripe_offset_a, y);
                color_path.line_to(x + stripe_offset_b, y);
                color_path.line_to(x, y + stripe_offset_b);
                color_path.line_to(x, y + stripe_offset_a);
                color_path.close();
            } else {
                let stripe_offset_a: f32 =
                    (stripe_idx - half_num_stripes) as f32 / half_num_stripes as f32 * size;
                let stripe_offset_b: f32 =
                    (stripe_idx - half_num_stripes + 1) as f32 / half_num_stripes as f32 * size;
                color_path.move_to(x + size, y + stripe_offset_a);
                color_path.line_to(x + size, y + stripe_offset_b);
                color_path.line_to(x + stripe_offset_b, y + size);
                color_path.line_to(x + stripe_offset_a, y + size);
                color_path.close();
            }
            canvas.fill_path(
                &color_path,
                &vg::Paint::color(node_args.colors[stripe_idx as usize % node_args.colors.len()]),
            );
        }
    }

    canvas.global_composite_operation(vg::CompositeOperation::SourceOver);
}

/// Draw a node where there are no factors of 7 in the pitch class. This is the regular-sized
/// rounded rectangle that is always displayed, and covers most of the grid area.
fn draw_node_zero_z(canvas: &mut Canvas, args: &DrawGridArgs, node_args: &DrawNodeArgs) {
    // Node rectangle
    let mut node_path = vg::Path::new();
    node_path.rounded_rect(
        node_args.draw_node_x,
        node_args.draw_node_y,
        NODE_SIZE * args.scale,
        NODE_SIZE * args.scale,
        (CORNER_RADIUS - PADDING) * args.scale,
    );

    if node_args.colors.len() > 0 {
        canvas.fill_path(&mut node_path, &vg::Paint::color(node_args.colors[0]));
        if node_args.colors.len() > 1 {
            canvas.global_composite_operation(vg::CompositeOperation::Atop);
            draw_extra_colors(
                canvas,
                node_args,
                node_args.draw_node_x,
                node_args.draw_node_y,
                args.scaled_node_size,
                7,
            );
            canvas.global_composite_operation(vg::CompositeOperation::SourceOver);
        }
    } else {
        canvas.fill_path(&mut node_path, &vg::Paint::color(COLOR_2));
    }

    // Draw outline for channel 16
    if node_args.draw_outline {
        canvas.stroke_path(
            &node_path,
            &make_icon_paint(COLOR_3, args.scaled_padding * 0.25),
        );
    }

    // Note letter name
    let mut text_paint = vg::Paint::color(COLOR_3);
    text_paint.set_font_size(args.scaled_node_size * 0.60);
    text_paint.set_text_align(vg::Align::Right);
    args.mono_font_id.map(|f| text_paint.set_font(&[f]));
    let _ = canvas.fill_text(
        node_args.draw_node_x + args.scaled_node_size * 0.48,
        node_args.draw_node_y + args.scaled_node_size * 0.58,
        format!("{}", node_args.note_name_info.letter_name),
        &text_paint,
    );

    // Sharps or flats
    text_paint.set_font_size(args.scaled_node_size * 0.29);
    text_paint.set_text_align(vg::Align::Left);
    let _ = canvas.fill_text(
        node_args.draw_node_x + args.scaled_node_size * 0.48,
        node_args.draw_node_y + args.scaled_node_size * 0.33,
        node_args.note_name_info.sharps_or_flats_str(),
        &text_paint,
    );

    // Syntonic commas - only displayed if four perfect fifths don't make a third
    if (args.three_tuning.multiply(4)).distance_to(args.five_tuning)
        < PitchClassDistance::from_microcents(4000)
    {
        let _ = canvas.fill_text(
            node_args.draw_node_x + args.scaled_node_size * 0.48,
            node_args.draw_node_y + args.scaled_node_size * 0.59,
            node_args.note_name_info.syntonic_comma_str(),
            &text_paint,
        );
    }

    // Tuning in cents
    text_paint.set_font_size(args.scaled_node_size * 0.25);
    text_paint.set_text_align(vg::Align::Center);
    args.font_id.map(|f| text_paint.set_font(&[f]));
    let rounded_pitch_class = node_args.pitch_class.round(2);
    let _ = canvas.fill_text(
        node_args.draw_node_x + args.scaled_node_size * 0.5,
        node_args.draw_node_y + args.scaled_node_size * 0.88,
        format!(
            "{}.{}{}",
            node_args.pitch_class.trunc_cents(),
            rounded_pitch_class.get_decimal_digit_num(0),
            rounded_pitch_class.get_decimal_digit_num(1),
        ),
        &text_paint,
    );
}

/// Draw a node with a factor of 7 in the pitch class.
/// This is a small rounded rectangle on the top right or bottom left of the "main" nodes
fn draw_node_nonzero_z(canvas: &mut Canvas, args: &DrawGridArgs, node_args: &DrawNodeArgs) {
    let dependent_seven = (args.three_tuning.multiply(2) + args.five_tuning.multiply(2))
        .distance_to(args.seven_tuning)
        < PitchClassDistance::from_microcents(4000);

    if node_args.matching_voices.len() == 0 || node_args.base_z.abs() != 1 || dependent_seven {
        return;
    }

    let mini_node_protrusion: f32 = args.scaled_padding * 0.0;
    let mini_node_size: f32 = args.scaled_node_size * 3.0 / 7.0;
    let (mini_node_x, mini_node_y) = if node_args.base_z == 1 {
        (
            node_args.draw_node_x + (args.scaled_node_size - mini_node_size) + mini_node_protrusion,
            node_args.draw_node_y - mini_node_protrusion,
        )
    } else {
        (
            node_args.draw_node_x - mini_node_protrusion,
            node_args.draw_node_y + (args.scaled_node_size - mini_node_size) + mini_node_protrusion,
        )
    };

    let mut background_rect_path = vg::Path::new();
    /*
                            background_rect_path.rounded_rect(
                                mini_node_x - PADDING * scale,
                                mini_node_y - PADDING * scale,
                                mini_node_size + PADDING * scale * 2.0,
                                mini_node_size + PADDING * scale * 2.0,
                                (CORNER_RADIUS) * scale,
                            );
    */

    let mut background_rect_arc_path = vg::Path::new();
    let mut second_background_rect_arc_path = vg::Path::new();
    if node_args.base_z == 1 {
        background_rect_path.rounded_rect_varying(
            mini_node_x - args.scaled_padding,
            mini_node_y - args.scaled_padding * 0.25,
            mini_node_size + args.scaled_padding * 1.25,
            mini_node_size + args.scaled_padding * 1.25,
            0.0,
            0.0,
            0.0,
            CORNER_RADIUS * args.scale,
        );

        // Top left arc
        background_rect_arc_path.arc(
            mini_node_x - CORNER_RADIUS * args.scale,
            mini_node_y + CORNER_RADIUS * args.scale - args.scaled_padding,
            (CORNER_RADIUS - PADDING * 0.5) * args.scale,
            PI * 1.5,
            PI * 2.0,
            vg::Solidity::Hole,
        );

        // Bottom right arc
        second_background_rect_arc_path.arc(
            mini_node_x + mini_node_size - CORNER_RADIUS * args.scale + args.scaled_padding,
            mini_node_y + mini_node_size + CORNER_RADIUS * args.scale,
            (CORNER_RADIUS - PADDING * 0.5) * args.scale,
            PI * 1.5,
            PI * 2.0,
            vg::Solidity::Hole,
        );
    } else {
        background_rect_path.rounded_rect_varying(
            mini_node_x - args.scaled_padding * 0.25,
            mini_node_y - args.scaled_padding,
            mini_node_size + args.scaled_padding * 1.25,
            mini_node_size + args.scaled_padding * 1.25,
            0.0,
            args.scaled_corner_radius,
            0.0,
            0.0,
        );

        // Top left arc
        background_rect_arc_path.arc(
            mini_node_x + args.scaled_corner_radius - args.scaled_padding,
            mini_node_y - args.scaled_corner_radius, // - PADDING * scale,
            (CORNER_RADIUS - PADDING * 0.5) * args.scale,
            PI * 0.5,
            PI * 1.0,
            vg::Solidity::Hole,
        );

        // Bottom right arc
        second_background_rect_arc_path.arc(
            mini_node_x + mini_node_size + args.scaled_corner_radius,
            mini_node_y + mini_node_size - args.scaled_corner_radius + args.scaled_padding,
            (CORNER_RADIUS - PADDING * 0.5) * args.scale,
            PI * 0.5,
            PI * 1.0,
            vg::Solidity::Hole,
        );
    }

    canvas.global_composite_operation(vg::CompositeOperation::DestinationOut);

    canvas.fill_path(&mut background_rect_path, &vg::Paint::color(COLOR_1));

    let arc_paint = make_icon_paint(COLOR_1, args.scaled_padding);
    canvas.stroke_path(&mut background_rect_arc_path, &arc_paint);
    canvas.stroke_path(&mut second_background_rect_arc_path, &arc_paint);

    canvas.global_composite_operation(vg::CompositeOperation::SourceOver);

    let mut mini_node_path = vg::Path::new();
    mini_node_path.rounded_rect(
        mini_node_x,
        mini_node_y,
        mini_node_size,
        mini_node_size,
        (CORNER_RADIUS - PADDING) * args.scale,
    );
    canvas.fill_path(&mut mini_node_path, &vg::Paint::color(node_args.colors[0]));

    canvas.global_composite_operation(vg::CompositeOperation::Atop);
    draw_extra_colors(
        canvas,
        node_args,
        mini_node_x,
        mini_node_y,
        mini_node_size,
        3,
    );
    canvas.global_composite_operation(vg::CompositeOperation::SourceOver);

    // Draw text
    let mut text_paint = vg::Paint::color(COLOR_3);
    text_paint.set_font_size(args.scaled_node_size * 0.19);
    text_paint.set_text_align(vg::Align::Center);
    args.font_id.map(|f| text_paint.set_font(&[f]));
    let _ = canvas.fill_text(
        mini_node_x + mini_node_size * 0.5,
        mini_node_y + mini_node_size * 0.5,
        node_args.pitch_class.trunc_cents().to_string(),
        &text_paint,
    );

    text_paint.set_font_size(args.scaled_node_size * 0.16);
    let rounded_pitch_class = node_args.pitch_class.round(2);
    let _ = canvas.fill_text(
        mini_node_x + mini_node_size * 0.5,
        mini_node_y + mini_node_size * 0.83,
        format!(
            ".{}{}",
            rounded_pitch_class.get_decimal_digit_num(0),
            rounded_pitch_class.get_decimal_digit_num(1),
        ),
        &text_paint,
    );
}

impl View for Grid {
    fn element(&self) -> Option<&'static str> {
        Some("lattice-display")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {}

    // TODO: factor this out into methods
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let start_time = Instant::now();

        let args: DrawGridArgs = DrawGridArgs::new(self, cx, canvas);

        prepare_canvas(cx, canvas, &args);

        // When grid x or y is not a round number, we need to add a row or column to avoid blanks
        let (extra_right, extra_top) = (
            if args.grid_x == args.grid_x.round() {
                0
            } else {
                1
            },
            if args.grid_y == args.grid_y.round() {
                0
            } else {
                1
            },
        );

        // Offsets for the coordinates of C on the grid (makes it as close as possible to the center)
        let (x_offset, y_offset) = (
            ((args.grid_width - 1) / 2) as i32,
            (args.grid_height / 2) as i32,
        );

        // Draw lattice nodes one by one
        // z = sevens
        for base_z in [0, -1, 1] {
            // x = fives
            for base_x in 0..args.grid_width + extra_right {
                // y = threes
                for base_y in -extra_top..args.grid_height {
                    // Number of factors of each prime in the pitch class represented by this node
                    let primes = PrimeCountVector::new(
                        y_offset - i32::from(base_y) + args.grid_y.floor() as i32,
                        i32::from(base_x - x_offset) + args.grid_x.floor() as i32,
                        base_z + args.grid_z,
                    );

                    let node_args: DrawNodeArgs =
                        DrawNodeArgs::new(&args, base_x, base_y, base_z, primes);

                    draw_node(canvas, &args, &node_args);
                }
            }
        }

        finish_canvas(cx, canvas, &args);

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
