use crate::assets;
use crate::GridParams;

use crate::editor::lattice::grid;
use crate::editor::lattice::Lattice;
use crate::editor::note_match_info::NoteMatchInfo;
use crate::editor::note_spectrum::NoteSpectrum;
use crate::editor::resizer::Resizer;
use crate::editor::tuning_learn_button::TuningLearnButton;
use crate::MidiLatticeParams;
use crate::Voices;
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

mod color;
mod lattice;
mod note_match_info;
mod note_spectrum;
mod resizer;
mod tuning_learn_button;

pub const BOTTOM_REGION_HEIGHT: f32 = grid::NODE_SIZE * 0.618 + PADDING;
pub const RIGHT_REGION_WIDTH: f32 = grid::NODE_SIZE * 0.618 + PADDING;

pub const PADDING: f32 = grid::NODE_SIZE * 0.08;
pub const CORNER_RADIUS: f32 = PADDING * 0.55;

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
pub const MAX_GRID_WIDTH: u8 = 30;
pub const MAX_GRID_HEIGHT: u8 = 30;

pub const NON_GRID_HEIGHT: f32 = BOTTOM_REGION_HEIGHT;
pub const NON_GRID_WIDTH: f32 = RIGHT_REGION_WIDTH;

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
    make_icon_paint(color, PADDING * scale)
}

pub fn width_to_grid_width(width: f32) -> u8 {
    min(
        MAX_GRID_WIDTH,
        max(
            MIN_GRID_WIDTH,
            ((width - NON_GRID_WIDTH) / (grid::NODE_SIZE + PADDING)) as u8,
        ),
    )
}

pub fn height_to_grid_height(height: f32) -> u8 {
    min(
        MAX_GRID_HEIGHT,
        max(
            MIN_GRID_HEIGHT,
            ((height - NON_GRID_HEIGHT) / (grid::NODE_SIZE + PADDING)) as u8,
        ),
    )
}

pub fn vizia_state(grid_params: Arc<GridParams>) -> Arc<ViziaState> {
    ViziaState::new(move || {
        let width: u32 = ((grid::NODE_SIZE + PADDING)
            * (grid_params.width.load(Ordering::Relaxed) as f32)
            + NON_GRID_WIDTH
            + PADDING) as u32;
        let height: u32 = ((grid::NODE_SIZE + PADDING)
            * (grid_params.height.load(Ordering::Relaxed) as f32)
            + NON_GRID_HEIGHT
            + PADDING) as u32;
        (width, height)
    })
}

pub fn create(data: Data) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        data.params.editor_state.clone(),
        ViziaTheming::None,
        move |cx, _gui_cx| {
            let _ = cx.add_stylesheet(include_str!("../assets/theme.css"));
            //ParamSetter::new(_gui_ctx.as_ref());
            assets::register_quicksand(cx);
            cx.set_default_font(&[assets::QUICKSAND]);

            data.clone().build(cx);

            Lattice::new(cx, Data::params, Data::voices_output)
                .position_type(PositionType::SelfDirected)
                .bottom(Units::Pixels(BOTTOM_REGION_HEIGHT))
                .left(Units::Pixels(0.0))
                .top(Units::Pixels(0.0))
                .right(Units::Pixels(RIGHT_REGION_WIDTH));

            NoteSpectrum::new(cx, Data::params, Data::voices_output)
                .position_type(PositionType::SelfDirected)
                .top(Units::Pixels(PADDING))
                .right(Units::Pixels(PADDING))
                .left(Units::Stretch(1.0))
                .bottom(Units::Pixels(BOTTOM_REGION_HEIGHT + PADDING))
                .width(Units::Pixels(RIGHT_REGION_WIDTH - PADDING));

            HStack::new(cx, |cx| {
                let button_dimensions = BOTTOM_REGION_HEIGHT - PADDING;

                TuningLearnButton::new(
                    cx,
                    Data::params.map(|p| p.tuning_params.clone()),
                    Data::voices_output,
                )
                .position_type(PositionType::ParentDirected)
                .left(Units::Pixels(0.0))
                .height(Units::Pixels(button_dimensions))
                .width(Units::Pixels(button_dimensions));

                NoteMatchInfo::new(cx, Data::params, Data::voices_output)
                    .left(Units::Pixels(PADDING))
                    .right(Units::Pixels(PADDING));

                Resizer::new(cx)
                    .position_type(PositionType::ParentDirected)
                    .bottom(Units::Pixels(PADDING))
                    .width(Units::Pixels(RIGHT_REGION_WIDTH - PADDING))
                    .height(Units::Pixels(BOTTOM_REGION_HEIGHT - PADDING));
            })
            .position_type(PositionType::SelfDirected)
            .top(Units::Stretch(1.0))
            .bottom(Units::Pixels(PADDING))
            .left(Units::Pixels(PADDING))
            .right(Units::Pixels(PADDING))
            .height(Units::Pixels(BOTTOM_REGION_HEIGHT - PADDING));
        },
    )
}

fn intersects_box(bounds: BoundingBox, (x, y): (f32, f32)) -> bool {
    x >= bounds.x && y >= bounds.y && x <= bounds.x + bounds.w && y <= bounds.y + bounds.h
}
