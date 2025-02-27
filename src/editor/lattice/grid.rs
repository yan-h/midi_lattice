use crate::MidiLatticeParams;
use crate::ShowZAxis;
use crate::Voices;

use crate::assets;
use crate::editor::color::*;
use crate::editor::make_icon_paint;
use crate::midi::MidiVoice;
use crate::tuning::*;

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::vizia::vg::FontId;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::atomic::Ordering;
use std::sync::MutexGuard;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use triple_buffer::Output;

use crate::editor::{CORNER_RADIUS, PADDING};

pub const NODE_SIZE: f32 = 50.0;

pub struct Grid {
    params: Arc<MidiLatticeParams>,

    // Reads voices from the audio thread
    voices_output: Arc<Mutex<Output<Voices>>>,

    // Need interior mutability to allow mutation from draw()
    font_info: Mutex<FontInfo>,

    // Need interior mutability to allow mutation from draw()
    animation_info: Mutex<AnimationInfo>,
}

/// All the information relevant to displaying voices on a grid. A simplified version of
/// `MidiVoice`
#[derive(Debug, Clone, Copy)]
pub struct Voice {
    pitch_class: PitchClass,
    pitch: f32,
    channel: u8,
}

impl Voice {
    const fn new(channel: u8, pitch: f32, pitch_class: PitchClass) -> Self {
        Voice {
            pitch_class,
            pitch,
            channel,
        }
    }

    const fn get_pitch_class(&self) -> PitchClass {
        self.pitch_class
    }

    const fn get_pitch(&self) -> f32 {
        self.pitch
    }

    const fn get_channel(&self) -> u8 {
        self.channel
    }
}

impl PartialEq for Voice {
    fn eq(&self, other: &Self) -> bool {
        self.pitch_class == other.pitch_class
    }
}
impl Eq for Voice {}

impl PartialOrd for Voice {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.pitch_class.partial_cmp(&other.pitch_class)
    }
}

impl Ord for Voice {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.pitch_class.cmp(&other.pitch_class)
    }
}

/// Additional state for displaying things that aren't captured by the current voices
pub struct AnimationInfo {
    /// Recent pitch classes are highlighted for a short duration.
    /// This stores the set of recent voices, with the amount of time left for each.
    recent_pitch_classes: HashMap<PitchClass, Duration>,

    /// Timestamp of the last draw() call
    last_tick: Instant,
}

/// Stores info about fonts for femtovg's canvas.
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
            animation_info: Mutex::new(AnimationInfo {
                recent_pitch_classes: HashMap::new(),
                last_tick: Instant::now(),
            }),
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

    fn update_and_get_highlighted_pitch_classes(
        &self,
        voices: &Vec<Voice>,
        highlight_duration: Duration,
    ) -> Vec<PitchClass> {
        let mut animation_info: MutexGuard<'_, AnimationInfo> = self.animation_info.lock().unwrap();
        let time_since_last_draw: Duration = Instant::now() - animation_info.last_tick;

        // Tick timer on all pitch classes
        for time_left in animation_info.recent_pitch_classes.values_mut() {
            if time_since_last_draw > *time_left {
                *time_left = Duration::ZERO;
            } else {
                *time_left -= time_since_last_draw;
                // Limit to current highlight duration. Prevents long-lived higlights if duration
                // parameter is reduced significantly
                *time_left = highlight_duration.min(*time_left);
            }
        }

        animation_info.last_tick = Instant::now();

        // Refresh currently playing pitch classes
        for voice in voices.iter() {
            // Don't count ignored or outline-only channels
            if voice.get_channel() <= 13 {
                animation_info
                    .recent_pitch_classes
                    .insert(voice.get_pitch_class(), highlight_duration);
            }
        }

        // Drop expired pitch classes
        animation_info
            .recent_pitch_classes
            .retain(|_, v: &mut Duration| *v > Duration::ZERO);

        // Collect, sort and return set of surviving pitch classes
        let mut result: Vec<PitchClass> = animation_info
            .recent_pitch_classes
            .keys()
            .cloned()
            .collect();
        result.sort();

        result
    }
}

// Contains all plugin parameters needed for drawing the grid
struct GridParams {
    grid_width: i32,
    grid_height: i32,
    grid_x: f32,
    grid_y: f32,
    grid_z: i32,
    show_z_axis: ShowZAxis,
    darkest_pitch: f32,
    brightest_pitch: f32,
    c_offset: PitchClass,
    three_tuning: PitchClass,
    five_tuning: PitchClass,
    seven_tuning: PitchClass,
    tuning_tolerance: PitchClassDistance,
}

impl GridParams {
    fn new(params: &MidiLatticeParams) -> GridParams {
        GridParams {
            grid_width: params.grid_params.width.load(Ordering::Relaxed) as i32,
            grid_height: params.grid_params.height.load(Ordering::Relaxed) as i32,
            grid_x: params.grid_params.x.value(),
            grid_y: params.grid_params.y.value(),
            grid_z: params.grid_params.z.value(),
            show_z_axis: params.grid_params.show_z_axis.value(),
            darkest_pitch: params.grid_params.darkest_pitch.value(),
            brightest_pitch: params.grid_params.brightest_pitch.value(),
            c_offset: PitchClass::from_cents_f32(params.tuning_params.c_offset.value()),
            three_tuning: PitchClass::from_cents_f32(params.tuning_params.three.value()),
            five_tuning: PitchClass::from_cents_f32(params.tuning_params.five.value()),
            seven_tuning: PitchClass::from_cents_f32(params.tuning_params.seven.value()),
            tuning_tolerance: PitchClassDistance::from_cents_f32(
                params.tuning_params.tolerance.value(),
            ),
        }
    }
}

/// Arguments used to draw the grid. Passed into sub-methods of [`Grid::draw()`].
struct DrawGridArgs {
    scaled_node_size: f32,
    scaled_padding: f32,
    scaled_corner_radius: f32,
    bounds: BoundingBox,
    sorted_voices: Vec<Voice>,
    font_id: Option<FontId>,
    mono_font_id: Option<FontId>,
    highlighted_pitch_classes: Vec<PitchClass>,
}

impl DrawGridArgs {
    fn new(
        grid: &Grid,
        grid_params: &GridParams,
        cx: &mut DrawContext,
        canvas: &mut Canvas,
    ) -> DrawGridArgs {
        let (font_id, mono_font_id): (Option<FontId>, Option<FontId>) =
            grid.load_and_get_fonts(canvas);

        let sorted_voices = grid.get_sorted_voices();

        let highlight_duration =
            Duration::from_secs_f32(grid.params.grid_params.highlight_time.value());

        let highlighted_pitch_classes =
            grid.update_and_get_highlighted_pitch_classes(&sorted_voices, highlight_duration);

        let scaled_padding = PADDING * cx.scale_factor();

        // We can't just use `NODE_SIZE` here because that turns out to be slightly too big in
        // practice. Not sure why. Calculating it off the actual width/height works better.
        let scaled_node_size = (cx.bounds().width()
            - scaled_padding * (grid_params.grid_width as f32 + 1.0))
            / grid_params.grid_width as f32;

        DrawGridArgs {
            scaled_node_size,
            scaled_padding,
            scaled_corner_radius: CORNER_RADIUS * cx.scale_factor(),
            bounds: cx.bounds(),
            sorted_voices,
            font_id,
            mono_font_id,
            highlighted_pitch_classes,
        }
    }
}

struct DrawNodeArgs {
    draw: bool,
    draw_node_x: f32,
    draw_node_y: f32,
    base_z: i32,
    pitch_class: PitchClass,
    note_name_info: NoteNameInfo,
    colors: Vec<vg::Color>,
    draw_outline: bool,
    outline_width: f32,
    highlighted: bool,
}

impl DrawNodeArgs {
    fn new(
        params: &GridParams,
        args: &DrawGridArgs,
        base_x: i32,
        base_y: i32,
        base_z: i32,
        primes: PrimeCountVector,
    ) -> Self {
        let (draw_node_x, draw_node_y): (f32, f32) = (
            args.bounds.x
                + (args.scaled_padding
                    + (base_x as f32 - params.grid_x.rem_euclid(1.0))
                        * (args.scaled_node_size + args.scaled_padding)),
            args.bounds.y
                + (args.scaled_padding
                    + ((base_y as f32 + params.grid_y.rem_euclid(1.0))
                        * (args.scaled_node_size + args.scaled_padding))),
        );

        // Pitch class represented by this node
        let pitch_class: PitchClass =
            primes.pitch_class(params.three_tuning, params.five_tuning, params.seven_tuning)
                + params.c_offset;

        let matching_voices =
            get_matching_voices(pitch_class, &args.sorted_voices, params.tuning_tolerance);

        let highlighted = pitch_class_matches_any_in_sorted_vec(
            pitch_class,
            &args.highlighted_pitch_classes,
            params.tuning_tolerance,
        );

        let note_name_info = primes.note_name_info();

        // Determine colors and outline
        let mut colors: Vec<vg::Color> = Vec::with_capacity(15);
        let mut draw_outline = false;
        for v in &matching_voices {
            if v.get_channel() <= 13 {
                colors.push(note_color(
                    v.get_channel(),
                    v.get_pitch(),
                    params.darkest_pitch,
                    params.brightest_pitch,
                ));
            } else if v.get_channel() == 14 {
                draw_outline = true;
            }
        }

        // I think this sorts primarily by hue, which is what we want
        colors.sort_by(|a, b| a.partial_cmp(b).unwrap());
        colors.dedup();

        let draw = match base_z {
            // Always draw main nodes
            0 => true,
            // Nodes that aren't at zero on the Z axis are only drawn when they match a note
            -1 | 1 => matching_voices.len() != 0 || highlighted,
            _ => false,
        };

        DrawNodeArgs {
            draw,
            draw_node_x,
            draw_node_y,
            base_z,
            pitch_class,
            note_name_info,
            colors,
            draw_outline,
            outline_width: args.scaled_padding * OUTLINE_PADDING_RATIO,
            highlighted,
        }
    }
}

fn prepare_canvas(_cx: &mut DrawContext, canvas: &mut Canvas, args: &DrawGridArgs) {
    // Hides everything out of args.bounds - for nodes that stick out when scrolling
    canvas.intersect_scissor(
        args.bounds.x + args.scaled_padding * OUTLINE_PADDING_RATIO,
        args.bounds.y + args.scaled_padding * OUTLINE_PADDING_RATIO,
        args.bounds.w - args.scaled_padding * OUTLINE_PADDING_RATIO * 2.0,
        args.bounds.h - args.scaled_padding * OUTLINE_PADDING_RATIO * 2.0,
    );

    // Carve out entire background, with half padding around.
    // This is necessary to use clipping when drawing with femtovg's composite operations.
    // We'll put the background back afterwards in `finish_canvas`.
    canvas.global_composite_operation(vg::CompositeOperation::DestinationOut);
    let mut background_path = vg::Path::new();
    background_path.rect(
        args.bounds.x + args.scaled_padding * 0.75, // Add buffer above 0.5 to avoid dark lines
        args.bounds.y + args.scaled_padding * 0.75,
        args.bounds.w - args.scaled_padding * 1.5,
        args.bounds.h - args.scaled_padding * 1.5,
    );
    canvas.fill_path(&background_path, &vg::Paint::color(BACKGROUND_COLOR));
    canvas.global_composite_operation(vg::CompositeOperation::SourceOver);
}

fn finish_canvas(_cx: &mut DrawContext, canvas: &mut Canvas, args: &DrawGridArgs) {
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
    canvas.fill_path(&background_path_refill, &vg::Paint::color(BACKGROUND_COLOR));
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

const OUTLINE_PADDING_RATIO: f32 = 0.5;
const TOP: f32 = PI * 1.5;
const RIGHT: f32 = PI * 2.0;

/// Draw a node where there are no factors of 7 in the pitch class. This is the regular-sized
/// rounded rectangle that is always displayed, and covers most of the grid area.
/// If smaller nodes for 7 are displayed, this node changes appearance to make room.
fn draw_node_zero_z(
    canvas: &mut Canvas,
    params: &GridParams,
    args: &DrawGridArgs,
    node_args: &DrawNodeArgs,
    draw_z_pos: bool,
    draw_z_neg: bool,
) {
    draw_main_node_square(canvas, args, node_args);
    draw_note_name(canvas, params, args, node_args, draw_z_pos, draw_z_neg);
    draw_tuning_cents(canvas, args, node_args, draw_z_neg);
    if draw_z_pos {
        remove_top_right_corner(canvas, args, node_args);
    }
    if draw_z_neg {
        remove_bottom_left_corner(canvas, args, node_args);
    }

    fn draw_main_node_square(canvas: &mut Canvas, args: &DrawGridArgs, node_args: &DrawNodeArgs) {
        let mut node_path = vg::Path::new();
        node_path.rounded_rect(
            node_args.draw_node_x,
            node_args.draw_node_y,
            args.scaled_node_size,
            args.scaled_node_size,
            args.scaled_corner_radius,
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
                    (node_args.colors.len() * 3) as u8,
                );
                canvas.global_composite_operation(vg::CompositeOperation::SourceOver);
            }
        } else {
            canvas.fill_path(
                &mut node_path,
                &vg::Paint::color(if node_args.highlighted {
                    HIGHLIGHT_COLOR
                } else {
                    BASE_COLOR
                }),
            );
        }

        // Draw outline for channel 16
        if node_args.draw_outline {
            canvas.stroke_path(
                &node_path,
                &make_icon_paint(TEXT_COLOR, node_args.outline_width),
            );
        }
    }

    fn draw_note_name(
        canvas: &mut Canvas,
        params: &GridParams,
        args: &DrawGridArgs,
        node_args: &DrawNodeArgs,
        draw_z_pos: bool,
        draw_z_neg: bool,
    ) {
        let mut text_paint = vg::Paint::color(TEXT_COLOR);
        text_paint.set_text_align(vg::Align::Right);

        let show_syntonic_commas = params
            .three_tuning
            .multiply(4)
            .distance_to(params.five_tuning)
            > params.tuning_tolerance;
        let max_accidental_str_len = (if show_syntonic_commas {
            node_args.note_name_info.syntonic_commas.abs()
        } else {
            0
        })
        .max(node_args.note_name_info.sharps_or_flats.abs())
        .min(2);

        let (letter_name_size, align_x, letter_name_y) = if !draw_z_pos && !draw_z_neg {
            // Standard position
            (0.60, 0.48, 0.58)
        } else if !draw_z_pos && draw_z_neg {
            // Centered horizontally on top half
            (0.50, 0.48, 0.44)
        } else if draw_z_pos && !draw_z_neg {
            // Centered vertically on left half
            match max_accidental_str_len {
                0 => (0.60, 0.44, 0.58),
                1 => (0.45, 0.32, 0.58),
                _ => (0.37, 0.26, 0.58),
            }
        } else {
            // Squished into top left corner
            match max_accidental_str_len {
                0 => (0.45, 0.38, 0.41),
                1 => (0.45, 0.30, 0.41),
                _ => (0.36, 0.25, 0.385),
            }
        };

        let accidentals_size = letter_name_size * 0.48;
        let sharps_flats_y = letter_name_y - accidentals_size * 0.88;
        let syntonic_commas_y = sharps_flats_y + accidentals_size * 0.84;

        text_paint.set_font_size(args.scaled_node_size * letter_name_size);

        // Letter name
        args.mono_font_id.map(|f| text_paint.set_font(&[f]));
        let _ = canvas.fill_text(
            node_args.draw_node_x + args.scaled_node_size * align_x,
            node_args.draw_node_y + args.scaled_node_size * letter_name_y,
            format!("{}", node_args.note_name_info.letter_name),
            &text_paint,
        );

        // Sharps or flats
        text_paint.set_font_size(args.scaled_node_size * accidentals_size);
        text_paint.set_text_align(vg::Align::Left);
        let _ = canvas.fill_text(
            node_args.draw_node_x + args.scaled_node_size * align_x,
            node_args.draw_node_y + args.scaled_node_size * sharps_flats_y,
            node_args.note_name_info.sharps_or_flats_str(),
            &text_paint,
        );

        // Syntonic commas - only displayed if four perfect fifths don't make a third
        if show_syntonic_commas {
            let _ = canvas.fill_text(
                node_args.draw_node_x + args.scaled_node_size * align_x,
                node_args.draw_node_y + args.scaled_node_size * syntonic_commas_y,
                node_args.note_name_info.syntonic_comma_str(),
                &text_paint,
            );
        }
    }

    fn draw_tuning_cents(
        canvas: &mut Canvas,
        args: &DrawGridArgs,
        node_args: &DrawNodeArgs,
        draw_z_neg: bool,
    ) {
        let mut text_paint = vg::Paint::color(TEXT_COLOR);
        text_paint.set_text_align(vg::Align::Center);
        args.font_id.map(|f| text_paint.set_font(&[f]));
        if draw_z_neg {
            text_paint.set_font_size(args.scaled_node_size * 0.21);
            let removed_square_size =
                MINI_NODE_SIZE_RATIO * args.scaled_node_size + args.scaled_padding;
            let (x, y) = (
                node_args.draw_node_x + removed_square_size,
                node_args.draw_node_y + removed_square_size,
            );
            let size = args.scaled_node_size - removed_square_size;

            let _ = canvas.fill_text(
                x + size * 0.5,
                y + size * 0.48,
                node_args.pitch_class.trunc_cents().to_string(),
                &text_paint,
            );

            text_paint.set_font_size(args.scaled_node_size * 0.18);
            let rounded_pitch_class = node_args.pitch_class.round(2);
            let _ = canvas.fill_text(
                x + size * 0.5,
                y + size * 0.8,
                format!(
                    ".{}{}",
                    rounded_pitch_class.get_decimal_digit_num(0),
                    rounded_pitch_class.get_decimal_digit_num(1),
                ),
                &text_paint,
            );
        } else {
            text_paint.set_font_size(args.scaled_node_size * 0.25);
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
    }

    fn remove_top_right_corner(canvas: &mut Canvas, args: &DrawGridArgs, node_args: &DrawNodeArgs) {
        let (mini_node_x, mini_node_y) = get_mini_node_pos(true, args, node_args);
        let mini_node_size: f32 = args.scaled_node_size * MINI_NODE_SIZE_RATIO;

        let (background_square_x, background_square_y) =
            (mini_node_x - args.scaled_padding, mini_node_y);
        let background_square_size = mini_node_size + args.scaled_padding;

        // Carve out top right region to make space and padding for mini-node
        let mut negative_path = vg::Path::new();

        // Main rectangle
        negative_path.rounded_rect_varying(
            background_square_x,
            background_square_y - node_args.outline_width * 0.6,
            background_square_size + node_args.outline_width * 0.6,
            background_square_size + node_args.outline_width * 0.6,
            0.0,
            0.0,
            0.0,
            args.scaled_corner_radius + args.scaled_padding - node_args.outline_width * 0.6 * 0.5,
        );

        let add_corner_negative_path = |path: &mut vg::Path, x: f32, y: f32| {
            path.move_to(x - args.scaled_corner_radius, y);
            path.arc_to(
                x,
                y,
                x,
                y + args.scaled_corner_radius,
                args.scaled_corner_radius,
            );
            path.line_to(
                x + node_args.outline_width * 0.6,
                y + args.scaled_corner_radius,
            );
            path.line_to(
                x + node_args.outline_width * 0.6,
                y - node_args.outline_width * 0.6,
            );
            path.line_to(
                x - args.scaled_corner_radius,
                y - node_args.outline_width * 0.6,
            );
            path.close();
        };

        // Top left corner
        add_corner_negative_path(&mut negative_path, background_square_x, background_square_y);

        // Top right corner
        add_corner_negative_path(
            &mut negative_path,
            background_square_x + background_square_size,
            background_square_y + background_square_size,
        );

        canvas.global_composite_operation(vg::CompositeOperation::DestinationOut);
        canvas.fill_path(&mut negative_path, &vg::Paint::color(BACKGROUND_COLOR));
        canvas.global_composite_operation(vg::CompositeOperation::SourceOver);

        if node_args.draw_outline {
            let mut outline_path = vg::Path::new();
            // top left
            outline_path.arc(
                background_square_x - args.scaled_corner_radius,
                background_square_y + args.scaled_corner_radius,
                args.scaled_corner_radius,
                TOP,
                RIGHT,
                vg::Solidity::Hole,
            );

            // bottom left (larger)
            outline_path.arc_to(
                background_square_x,
                background_square_y + background_square_size,
                background_square_x + args.scaled_corner_radius,
                background_square_y + background_square_size,
                args.scaled_corner_radius + args.scaled_padding,
            );

            // bottom right
            outline_path.arc(
                background_square_x + background_square_size - args.scaled_corner_radius,
                background_square_y + background_square_size + args.scaled_corner_radius,
                args.scaled_corner_radius,
                TOP,
                RIGHT,
                vg::Solidity::Hole,
            );

            canvas.stroke_path(
                &mut outline_path,
                &make_icon_paint(TEXT_COLOR, args.scaled_padding * OUTLINE_PADDING_RATIO),
            );
        }
    }

    fn remove_bottom_left_corner(
        canvas: &mut Canvas,
        args: &DrawGridArgs,
        node_args: &DrawNodeArgs,
    ) {
        let (mini_node_x, mini_node_y) = get_mini_node_pos(false, args, node_args);
        let mini_node_size: f32 = args.scaled_node_size * MINI_NODE_SIZE_RATIO;

        let (background_square_x, background_square_y) =
            (mini_node_x, mini_node_y - args.scaled_padding);
        let background_square_size = mini_node_size + args.scaled_padding;

        // Carve out top right region to make space and padding for mini-node
        let mut negative_path = vg::Path::new();

        // Main rectangle
        negative_path.rounded_rect_varying(
            background_square_x - node_args.outline_width * 0.6,
            background_square_y,
            background_square_size + node_args.outline_width * 0.6,
            background_square_size + node_args.outline_width * 0.6,
            0.0,
            args.scaled_corner_radius + args.scaled_padding - node_args.outline_width * 0.6 * 0.5,
            0.0,
            0.0,
        );

        let add_corner_negative_path = |path: &mut vg::Path, x: f32, y: f32| {
            path.move_to(x, y - args.scaled_corner_radius);
            path.arc_to(
                x,
                y,
                x + args.scaled_corner_radius,
                y,
                args.scaled_corner_radius,
            );
            path.line_to(
                x + args.scaled_corner_radius,
                y + node_args.outline_width * 0.6,
            );
            path.line_to(
                x - node_args.outline_width * 0.6,
                y + node_args.outline_width * 0.6,
            );
            path.line_to(
                x - node_args.outline_width * 0.6,
                y - args.scaled_corner_radius,
            );
            path.close();
        };

        // Top left corner
        add_corner_negative_path(&mut negative_path, background_square_x, background_square_y);

        // Top right corner
        add_corner_negative_path(
            &mut negative_path,
            background_square_x + background_square_size,
            background_square_y + background_square_size,
        );

        canvas.global_composite_operation(vg::CompositeOperation::DestinationOut);
        canvas.fill_path(&mut negative_path, &vg::Paint::color(BACKGROUND_COLOR));
        canvas.global_composite_operation(vg::CompositeOperation::SourceOver);

        if node_args.draw_outline {
            let mut outline_path = vg::Path::new();

            outline_path.move_to(
                background_square_x,
                background_square_y - args.scaled_corner_radius,
            );
            // top left
            outline_path.arc_to(
                background_square_x,
                background_square_y,
                background_square_x + args.scaled_corner_radius,
                background_square_y,
                args.scaled_corner_radius,
            );

            // bottom left (larger)
            outline_path.arc_to(
                background_square_x + background_square_size,
                background_square_y,
                background_square_x + background_square_size,
                background_square_y + args.scaled_corner_radius,
                args.scaled_corner_radius + args.scaled_padding,
            );

            // bottom right
            outline_path.arc_to(
                background_square_x + background_square_size,
                background_square_y + background_square_size,
                background_square_x + background_square_size + args.scaled_corner_radius,
                background_square_y + background_square_size,
                args.scaled_corner_radius,
            );
            canvas.stroke_path(
                &mut outline_path,
                &make_icon_paint(TEXT_COLOR, args.scaled_padding * OUTLINE_PADDING_RATIO),
            );
        }
    }
}

static MINI_NODE_SIZE_RATIO: f32 = 3.0 / 7.0;

fn get_mini_node_pos(
    z_positive: bool,
    args: &DrawGridArgs,
    node_args: &DrawNodeArgs,
) -> (f32, f32) {
    match z_positive {
        true => (
            node_args.draw_node_x
                + (args.scaled_node_size - args.scaled_node_size * MINI_NODE_SIZE_RATIO),
            node_args.draw_node_y,
        ),
        false => (
            node_args.draw_node_x,
            node_args.draw_node_y
                + (args.scaled_node_size - args.scaled_node_size * MINI_NODE_SIZE_RATIO),
        ),
    }
}
/// Draw a node with a factor of 7 in the pitch class.
/// This is a small rounded rectangle on the top right or bottom left of the "main" nodes
fn draw_node_nonzero_z(canvas: &mut Canvas, args: &DrawGridArgs, node_args: &DrawNodeArgs) {
    if !node_args.draw {
        return;
    }

    let mini_node_size: f32 = args.scaled_node_size * MINI_NODE_SIZE_RATIO;
    let (mini_node_x, mini_node_y) = get_mini_node_pos(node_args.base_z == 1, args, node_args);

    // Clear background
    canvas.global_composite_operation(vg::CompositeOperation::DestinationOut);
    let mut background_rect_path = vg::Path::new();
    canvas.fill_path(&mut background_rect_path, &vg::Paint::color(BASE_COLOR));
    canvas.global_composite_operation(vg::CompositeOperation::SourceOver);

    // Draw background rectangle
    let mut mini_node_path = vg::Path::new();
    mini_node_path.rounded_rect(
        mini_node_x,
        mini_node_y,
        mini_node_size,
        mini_node_size,
        args.scaled_corner_radius,
    );
    if node_args.colors.len() > 0 {
        canvas.fill_path(&mut mini_node_path, &vg::Paint::color(node_args.colors[0]));
    } else {
        canvas.fill_path(
            &mut mini_node_path,
            &vg::Paint::color(if node_args.highlighted {
                HIGHLIGHT_COLOR
            } else {
                BASE_COLOR
            }),
        );
    }

    // Draw stripes if needed
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

    // Draw outline if needed
    if node_args.draw_outline {
        canvas.stroke_path(
            &mini_node_path,
            &make_icon_paint(TEXT_COLOR, node_args.outline_width),
        );
    }

    // Draw text (first row; whole number cents)
    let mut text_paint = vg::Paint::color(TEXT_COLOR);
    text_paint.set_font_size(args.scaled_node_size * 0.19);
    text_paint.set_text_align(vg::Align::Center);
    args.font_id.map(|f| text_paint.set_font(&[f]));
    let _ = canvas.fill_text(
        mini_node_x + mini_node_size * 0.5,
        mini_node_y + mini_node_size * 0.5,
        node_args.pitch_class.trunc_cents().to_string(),
        &text_paint,
    );

    // Draw text (second row; fractional cents)
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

    fn event(&mut self, _cx: &mut EventContext, _event: &mut Event) {}

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let _start_time = Instant::now();

        let params: GridParams = GridParams::new(&self.params);
        let args: DrawGridArgs = DrawGridArgs::new(self, &params, cx, canvas);

        prepare_canvas(cx, canvas, &args);

        let grid_pitches: HashMap<PhysicalGridIndex, PcvsAtPhysicalGridIndex> =
            get_grid_indexed_prime_count_vectors(&params);

        for (idx, pcvs) in grid_pitches.into_iter() {
            let node_args_zero_z = DrawNodeArgs::new(&params, &args, idx.x, idx.y, 0, pcvs.zero_z);

            let pos_z_args: Option<DrawNodeArgs> = pcvs
                .pos_z
                .map(|pcv| DrawNodeArgs::new(&params, &args, idx.x, idx.y, 1, pcv));
            let neg_z_args: Option<DrawNodeArgs> = pcvs
                .neg_z
                .map(|pcv| DrawNodeArgs::new(&params, &args, idx.x, idx.y, -1, pcv));

            draw_node_zero_z(
                canvas,
                &params,
                &args,
                &node_args_zero_z,
                pos_z_args.as_ref().is_some_and(|node_args| node_args.draw),
                neg_z_args.as_ref().is_some_and(|node_args| node_args.draw),
            );

            pos_z_args.map(|node_args| draw_node_nonzero_z(canvas, &args, &node_args));
            neg_z_args.map(|node_args| draw_node_nonzero_z(canvas, &args, &node_args));
        }

        finish_canvas(cx, canvas, &args);
    }
}

// An (x, y) position on the grid, as it appears on the screen
#[derive(PartialEq, PartialOrd, Eq, Ord, Copy, Clone, Debug, Hash)]
struct PhysicalGridIndex {
    x: i32,
    y: i32,
}

// All of the prime count vectors at a specific physical grid position
struct PcvsAtPhysicalGridIndex {
    zero_z: PrimeCountVector,
    pos_z: Option<PrimeCountVector>,
    neg_z: Option<PrimeCountVector>,
}

impl PcvsAtPhysicalGridIndex {
    fn all_prime_count_vectors(&self) -> Vec<PrimeCountVector> {
        let mut result = Vec::new();
        result.push(self.zero_z);
        self.pos_z.map(|pcv| result.push(pcv));
        self.neg_z.map(|pcv| result.push(pcv));
        return result;
    }
}

fn get_grid_indexed_prime_count_vectors(
    params: &GridParams,
) -> HashMap<PhysicalGridIndex, PcvsAtPhysicalGridIndex> {
    let show_zs = match params.show_z_axis {
        ShowZAxis::Yes => true,
        ShowZAxis::No => false,
        ShowZAxis::Auto => {
            // Whether the seventh harmonic is equal to the meantone minor seventh
            // i.e. whether it's equal to two perfect fourths
            let dependent_seven = (params.three_tuning.multiply(-2))
                .distance_to(params.seven_tuning)
                <= params.tuning_tolerance;
            !dependent_seven
        }
    };

    let mut result = HashMap::new();
    let (extra_right, extra_top) = (
        if params.grid_x == params.grid_x.round() {
            0
        } else {
            1
        },
        if params.grid_y == params.grid_y.round() {
            0
        } else {
            1
        },
    );

    let (ref_pitch_x, ref_pitch_y) = (
        ((params.grid_width - 1) / 2) as i32,
        (params.grid_height / 2) as i32,
    );

    for physical_x in 0..params.grid_width + extra_right {
        for physical_y in -extra_top..params.grid_height {
            let threes = ref_pitch_y - i32::from(physical_y) + params.grid_y.floor() as i32;
            let fives = i32::from(physical_x - ref_pitch_x) + params.grid_x.floor() as i32;
            result.insert(
                PhysicalGridIndex {
                    x: physical_x,
                    y: physical_y,
                },
                PcvsAtPhysicalGridIndex {
                    zero_z: PrimeCountVector::new(threes, fives, params.grid_z),
                    pos_z: if show_zs {
                        Some(PrimeCountVector::new(threes, fives, params.grid_z + 1))
                    } else {
                        None
                    },
                    neg_z: if show_zs {
                        Some(PrimeCountVector::new(threes, fives, params.grid_z - 1))
                    } else {
                        None
                    },
                },
            );
        }
    }
    result
}

pub fn get_sorted_grid_pitch_classes(params: &MidiLatticeParams) -> Vec<PitchClass> {
    let (three_tuning, five_tuning, seven_tuning) = (
        PitchClass::from_cents_f32(params.tuning_params.three.value()),
        PitchClass::from_cents_f32(params.tuning_params.five.value()),
        PitchClass::from_cents_f32(params.tuning_params.seven.value()),
    );
    let mut result: Vec<PitchClass> =
        get_grid_indexed_prime_count_vectors(&GridParams::new(&params))
            .values()
            .flat_map(|pcvs| pcvs.all_prime_count_vectors().into_iter())
            .map(|pcv| pcv.pitch_class(three_tuning, five_tuning, seven_tuning))
            .collect();
    result.sort();
    result
}

// Helper methods for drawing
impl Grid {
    /// Retrieves the list of `MidiVoice` from the triple buffer, and returns a vector of `Voice`
    /// sorted by pitch class.
    fn get_sorted_voices(&self) -> Vec<Voice> {
        let mut voices_output = self.voices_output.lock().unwrap();
        let mut result: Vec<Voice> = voices_output
            .read()
            .values()
            .cloned()
            .map(|v: MidiVoice| Voice::new(v.get_channel(), v.get_pitch(), v.get_pitch_class()))
            .collect();
        result.sort_unstable_by(|v1, v2| v1.pitch_class.cmp(&v2.pitch_class));
        result
    }
}

/// Returns the subset of a vector of voices with a given pitch class.
fn get_matching_voices(
    pitch_class: PitchClass,
    sorted_voices: &Vec<Voice>,
    tuning_tolerance: PitchClassDistance,
) -> Vec<Voice> {
    let mut matching_voices: Vec<Voice> = Vec::new();

    if sorted_voices.len() == 0 {
        return matching_voices;
    }

    // Lowest pitch class that could match
    let mut start_idx: usize = sorted_voices.partition_point(|v| {
        v.get_pitch_class() < pitch_class - PitchClass::from(tuning_tolerance)
    });

    if start_idx == sorted_voices.len() {
        start_idx = 0;
    }

    let mut idx = start_idx;

    // Loop forwards from start idx
    loop {
        if sorted_voices[idx]
            .get_pitch_class()
            .distance_to(pitch_class)
            > tuning_tolerance
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
            > tuning_tolerance
        {
            break;
        }
        matching_voices.push(sorted_voices[idx]);
    }

    matching_voices
}

#[cfg(test)]
mod get_matching_voices_tests {
    use crate::{
        editor::lattice::grid::{get_matching_voices, Voice},
        tuning::{PitchClass, PitchClassDistance, OCTAVE_MICROCENTS},
    };

    #[test]
    fn matches_distance_less_than_or_equal_to_tolerance() {
        let mut output = get_matching_voices(
            PitchClass::from_microcents(100_000_000),
            &vec![
                Voice::new(0, 0.0, PitchClass::from_microcents(98_999_999)),
                Voice::new(0, 0.0, PitchClass::from_microcents(99_000_000)),
                Voice::new(0, 0.0, PitchClass::from_microcents(101_000_000)),
                Voice::new(0, 0.0, PitchClass::from_microcents(101_000_001)),
            ],
            PitchClassDistance::from_microcents(1_000_000),
        );
        output.sort();
        let mut target = vec![
            Voice::new(0, 0.0, PitchClass::from_microcents(99_000_000)),
            Voice::new(0, 0.0, PitchClass::from_microcents(101_000_000)),
        ];
        target.sort();
        assert_eq!(output, target);
    }

    #[test]
    fn slightly_positive_matches_slightly_negative() {
        let output = get_matching_voices(
            PitchClass::from_microcents(123),
            &vec![Voice::new(
                0,
                0.0,
                PitchClass::from_microcents(OCTAVE_MICROCENTS - 123),
            )],
            PitchClassDistance::from_microcents(246),
        );
        let target = vec![Voice::new(
            0,
            0.0,
            PitchClass::from_microcents(OCTAVE_MICROCENTS - 123),
        )];
        assert_eq!(output, target);
    }

    #[test]
    fn slightly_negative_matches_slightly_positive() {
        let output = get_matching_voices(
            PitchClass::from_microcents(OCTAVE_MICROCENTS - 123),
            &vec![Voice::new(0, 0.0, PitchClass::from_microcents(123))],
            PitchClassDistance::from_microcents(246),
        );
        let target = vec![Voice::new(0, 0.0, PitchClass::from_microcents(123))];
        assert_eq!(output, target);
    }

    #[test]
    fn slightly_positive_matches_slightly_negative_multiple_voices() {
        let mut output = get_matching_voices(
            PitchClass::from_microcents(123),
            &vec![
                Voice::new(0, 0.0, PitchClass::from_microcents(123)),
                Voice::new(0, 0.0, PitchClass::from_microcents(700_000_000)),
                Voice::new(0, 0.0, PitchClass::from_microcents(1100_000_000)),
                Voice::new(0, 0.0, PitchClass::from_microcents(OCTAVE_MICROCENTS - 123)),
            ],
            PitchClassDistance::from_microcents(246),
        );
        output.sort();
        let mut target = vec![
            Voice::new(0, 0.0, PitchClass::from_microcents(123)),
            Voice::new(0, 0.0, PitchClass::from_microcents(OCTAVE_MICROCENTS - 123)),
        ];
        target.sort();
        assert_eq!(output, target);
    }

    #[test]
    fn slightly_negative_matches_slightly_positive_multiple_voices() {
        let mut output = get_matching_voices(
            PitchClass::from_microcents(OCTAVE_MICROCENTS - 123),
            &vec![
                Voice::new(0, 0.0, PitchClass::from_microcents(123)),
                Voice::new(0, 0.0, PitchClass::from_microcents(700_000_000)),
                Voice::new(0, 0.0, PitchClass::from_microcents(1100_000_000)),
                Voice::new(0, 0.0, PitchClass::from_microcents(OCTAVE_MICROCENTS - 123)),
            ],
            PitchClassDistance::from_microcents(246),
        );
        output.sort();
        let mut target = vec![
            Voice::new(0, 0.0, PitchClass::from_microcents(123)),
            Voice::new(0, 0.0, PitchClass::from_microcents(OCTAVE_MICROCENTS - 123)),
        ];
        target.sort();
        assert_eq!(output, target);
    }
}
