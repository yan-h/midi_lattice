use crate::assets;
use crate::GridParams;

use crate::editor::lattice::grid;
use crate::editor::lattice::Lattice;
use crate::editor::resizer::Resizer;
use crate::editor::tuning_learn_button::TuningLearnButton;
use crate::MidiLatticeParams;
use crate::Voices;
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::vizia::vg::Paint;
use std::cmp::{max, min};
use std::sync::atomic::Ordering;

use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;
use nih_plug_vizia::{create_vizia_editor, ViziaTheming};

use std::sync::{Arc, Mutex};
use triple_buffer::Output;

mod lattice;
mod resizer;
mod scale_button;
mod tuning_learn_button;

pub const BOTTOM_REGION_HEIGHT: f32 = grid::NODE_SIZE * 0.7 + CONTAINER_PADDING * 2.0;
pub const RIGHT_REGION_WIDTH: f32 = grid::NODE_SIZE * 0.7 + CONTAINER_PADDING * 2.0;

pub const CONTAINER_PADDING: f32 = grid::NODE_SIZE * 0.09;
pub const CONTAINER_CORNER_RADIUS: f32 = CONTAINER_PADDING;

#[derive(Lens, Clone)]
pub struct Data {
    params: Arc<MidiLatticeParams>,
    voices_output: Arc<Mutex<Output<Voices>>>,
}

impl Data {
    pub fn new(params: Arc<MidiLatticeParams>, voices_output: Arc<Mutex<Output<Voices>>>) -> Self {
        Self {
            params,
            voices_output,
        }
    }
}

impl Model for Data {}

pub const MIN_GRID_WIDTH: u8 = 4;
pub const MIN_GRID_HEIGHT: u8 = 4;
pub const MAX_GRID_WIDTH: u8 = 12;
pub const MAX_GRID_HEIGHT: u8 = 12;

pub const NON_GRID_HEIGHT: f32 = BOTTOM_REGION_HEIGHT + CONTAINER_PADDING * 2.0;
pub const NON_GRID_WIDTH: f32 = RIGHT_REGION_WIDTH + CONTAINER_PADDING * 2.0;

// Darkest color
pub static COLOR_0: vg::Color = vg::Color::rgbf(
    0x30 as f32 / 255.0,
    0x30 as f32 / 255.0,
    0x30 as f32 / 255.0,
);

pub static COLOR_1: vg::Color = vg::Color::rgbf(
    0x54 as f32 / 255.0,
    0x54 as f32 / 255.0,
    0x54 as f32 / 255.0,
);

pub static COLOR_2: vg::Color = vg::Color::rgbf(
    0x7A as f32 / 255.0,
    0x7A as f32 / 255.0,
    0x7A as f32 / 255.0,
);

// Brightest color
pub static COLOR_3: vg::Color = vg::Color::rgbf(
    0xff as f32 / 255.0,
    0xff as f32 / 255.0,
    0xff as f32 / 255.0,
);

// Colors for overlay buttons on lattice, which are only shown on mouse over.
pub static OVERLAY_COLOR_0: vg::Color = vg::Color::rgbaf(1.0, 1.0, 1.0, 0.4);
pub static OVERLAY_COLOR_1: vg::Color = vg::Color::rgbaf(1.0, 1.0, 1.0, 0.8);
pub static OVERLAY_COLOR_2: vg::Color = vg::Color::rgbaf(1.0, 1.0, 1.0, 1.0);

pub fn make_icon_paint(color: vg::Color, width: f32) -> Paint {
    let mut icon_paint = vg::Paint::color(color);
    icon_paint.set_line_width(width);
    icon_paint.set_line_cap(vg::LineCap::Round);
    icon_paint.set_line_cap_end(vg::LineCap::Round);
    icon_paint.set_line_cap_start(vg::LineCap::Round);
    icon_paint.set_line_join(vg::LineJoin::Round);
    icon_paint
}

pub fn make_icon_stroke_paint(color: vg::Color, scale: f32) -> Paint {
    make_icon_paint(color, CONTAINER_CORNER_RADIUS * scale)
}

pub fn width_to_grid_width(width: f32) -> u8 {
    min(
        MAX_GRID_WIDTH,
        max(
            MIN_GRID_WIDTH,
            ((width - NON_GRID_WIDTH) / (grid::NODE_SIZE + grid::NODE_GAP)) as u8,
        ),
    )
}

pub fn height_to_grid_height(height: f32) -> u8 {
    min(
        MAX_GRID_HEIGHT,
        max(
            MIN_GRID_HEIGHT,
            ((height - NON_GRID_HEIGHT) / (grid::NODE_SIZE + grid::NODE_GAP)) as u8,
        ),
    )
}

pub fn vizia_state(grid_params: Arc<GridParams>) -> Arc<ViziaState> {
    ViziaState::new(move || {
        let width: u32 = ((grid::NODE_SIZE + grid::NODE_GAP)
            * (grid_params.width.load(Ordering::Relaxed) as f32)
            + NON_GRID_WIDTH) as u32;
        let height: u32 = ((grid::NODE_SIZE + grid::NODE_GAP)
            * (grid_params.height.load(Ordering::Relaxed) as f32)
            + NON_GRID_HEIGHT) as u32;
        (width, height)
    })
}

pub fn create(data: Data) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        data.params.editor_state.clone(),
        ViziaTheming::None,
        move |cx, _| {
            let _ = cx.add_stylesheet(include_str!("../assets/theme.css"));

            assets::register_quicksand(cx);
            cx.set_default_font(&[assets::QUICKSAND]);

            data.clone().build(cx);

            HStack::new(cx, |cx| {
                let button_dimensions = BOTTOM_REGION_HEIGHT - CONTAINER_PADDING * 2.0;

                TuningLearnButton::new(
                    cx,
                    Data::params.map(|p| p.tuning_params.clone()),
                    Data::voices_output,
                )
                .position_type(PositionType::ParentDirected)
                .left(Units::Pixels(0.0))
                .height(Units::Pixels(button_dimensions))
                .width(Units::Pixels(button_dimensions));
            })
            .position_type(PositionType::SelfDirected)
            .top(Units::Stretch(1.0))
            .bottom(Units::Pixels(CONTAINER_PADDING))
            .left(Units::Pixels(CONTAINER_PADDING))
            .right(Units::Pixels(CONTAINER_PADDING))
            .height(Units::Pixels(
                BOTTOM_REGION_HEIGHT - 2.0 * CONTAINER_PADDING,
            ));

            Lattice::new(cx, Data::params, Data::voices_output)
                .position_type(PositionType::SelfDirected)
                .bottom(Units::Pixels(BOTTOM_REGION_HEIGHT))
                .left(Units::Pixels(CONTAINER_PADDING))
                .top(Units::Pixels(CONTAINER_PADDING))
                .right(Units::Pixels(RIGHT_REGION_WIDTH));

            Resizer::new(cx)
                .position_type(PositionType::SelfDirected)
                .right(Units::Pixels(CONTAINER_PADDING))
                .bottom(Units::Pixels(CONTAINER_PADDING))
                .top(Units::Stretch(1.0))
                .left(Units::Stretch(1.0))
                .width(Units::Pixels(RIGHT_REGION_WIDTH - CONTAINER_PADDING * 2.0))
                .height(Units::Pixels(
                    BOTTOM_REGION_HEIGHT - CONTAINER_PADDING * 2.0,
                ));
        },
    )
}

fn intersects_box(bounds: BoundingBox, (x, y): (f32, f32)) -> bool {
    x >= bounds.x && y >= bounds.y && x <= bounds.x + bounds.w && y <= bounds.y + bounds.h
}
