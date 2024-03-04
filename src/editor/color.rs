use std::panic;

use color_space::{Lch, Rgb};
use nih_plug_vizia::vizia::vg::{self, Color};
use once_cell::sync::Lazy;

const fn grey(rgb_value: f32) -> vg::Color {
    vg::Color::rgbf(rgb_value, rgb_value, rgb_value)
}

const MAX_COLOR_VALUE: f32 = 255.0;

// Darkest color - for background
pub static BACKGROUND_COLOR: vg::Color = grey(0x38 as f32 / MAX_COLOR_VALUE);

// For buttons and nodes in their default state.
pub static BASE_COLOR: vg::Color = grey(0x60 as f32 / MAX_COLOR_VALUE);

// For highlighted nodes, and moused over buttons.
pub static HIGHLIGHT_COLOR: vg::Color = grey(0x80 as f32 / MAX_COLOR_VALUE);

// For text, or focused buttons
pub static TEXT_COLOR: vg::Color = grey(0xff as f32 / MAX_COLOR_VALUE);

// Colors for overlay buttons on lattice, which are only shown on mouse over.
pub static OVERLAY_COLOR_BASE: vg::Color = vg::Color::rgbaf(1.0, 1.0, 1.0, 0.4);
pub static OVERLAY_COLOR_HOVER: vg::Color = vg::Color::rgbaf(1.0, 1.0, 1.0, 0.8);
pub static OVERLAY_COLOR_PRESS: vg::Color = vg::Color::rgbaf(1.0, 1.0, 1.0, 1.0);

// Maps channels to static colors
// Note: channel numbers here are 1 lower than the MIDI convention they're zero-indexed
pub static CHANNEL_COLORS: Lazy<[vg::Color; 9]> = Lazy::new(|| {
    [
        Lch::new(48.0, 45.0, 32.0),  // 0 red
        Lch::new(65.0, 60.0, 68.0),  // 1 orange
        Lch::new(80.0, 42.0, 83.0),  // 2 yellow
        Lch::new(65.0, 50.0, 120.0), // 3 green
        Lch::new(60.0, 40.0, 280.0), // 4 blue
        Lch::new(50.0, 55.0, 305.0), // 5 purple
        Lch::new(70.0, 30.0, 340.0), // 6 pink
        Lch::new(80.0, 0.0, 0.0),    // 7 white
        Lch::new(0.0, 0.0, 0.0),     // 8 black
                                     // 9-13 are colored based on pitch (see `note_color()`)
                                     // 14 is just an outline
                                     // 15 is ignored
    ]
    .map(|x| lch_to_vg_color(x))
});

fn lch_to_vg_color(lch_color: Lch) -> vg::Color {
    let rgbcolor = Rgb::from(lch_color);

    vg::Color::rgbf(
        rgbcolor.r as f32 / 255.0,
        rgbcolor.g as f32 / 255.0,
        rgbcolor.b as f32 / 255.0,
    )
}

pub fn note_color(channel: u8, pitch: f32, darkest_pitch: f32, brightest_pitch: f32) -> Color {
    if channel <= 8 {
        // These channels have a fixed color
        return CHANNEL_COLORS[usize::from(channel)];
    } else if channel <= 13 {
        // These channels are colored by pitch, on a gradient
        let pitch_color_index: f64 =
            ((pitch.min(brightest_pitch).max(darkest_pitch) - darkest_pitch)
                / (brightest_pitch - darkest_pitch).max(0.01)) as f64;
        return lch_to_vg_color(Lch::new(
            25.0 + pitch_color_index * 55.0,
            65.0 - pitch_color_index * 35.0,
            (-20.0 + pitch_color_index * 110.0).rem_euclid(360.0),
        ));
    } else if channel == 14 {
        return HIGHLIGHT_COLOR;
    } else {
        panic!("Invalid midi channel");
    }
}
