use crate::assets;
use crate::GridParams;

use crate::editor::resizer::Resizer;
use crate::MidiLatticeParams;
use crate::Voices;
use nih_plug::nih_dbg;
use nih_plug::nih_log;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::ResizeHandle;
use std::cmp::max;
use std::sync::atomic::Ordering;

use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;
use nih_plug_vizia::{create_vizia_editor, ViziaTheming};

use std::sync::{Arc, Mutex};
use triple_buffer::Output;

mod lattice;
mod resizer;

pub const BOTTOM_REGION_HEIGHT: f32 = lattice::NODE_SIZE + CONTAINER_PADDING * 2.0;
pub const RIGHT_REGION_WIDTH: f32 = lattice::NODE_SIZE + CONTAINER_PADDING * 2.0;

pub const CONTAINER_PADDING: f32 = 12.0;
pub const CONTAINER_CORNER_RADIUS: f32 = 12.0;

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

pub const MIN_GRID_WIDTH: u8 = 3;
pub const MIN_GRID_HEIGHT: u8 = 3;

pub const NON_GRID_HEIGHT: f32 = BOTTOM_REGION_HEIGHT + CONTAINER_PADDING * 2.0;
pub const NON_GRID_WIDTH: f32 = RIGHT_REGION_WIDTH + CONTAINER_PADDING * 2.0;

pub const NODE_COLOR: vg::Color = vg::Color::rgbf(
    0x76 as f32 / 255.0,
    0x76 as f32 / 255.0,
    0x76 as f32 / 255.0,
);

pub const CONTAINER_COLOR: vg::Color = vg::Color::rgbf(
    0x58 as f32 / 255.0,
    0x58 as f32 / 255.0,
    0x58 as f32 / 255.0,
);

pub const HIGHLIGHT_COLOR: vg::Color = vg::Color::rgbf(
    0xff as f32 / 255.0,
    0xff as f32 / 255.0,
    0xff as f32 / 255.0,
);

pub fn width_to_grid_width(width: f32) -> u8 {
    max(
        MIN_GRID_WIDTH,
        ((width - NON_GRID_WIDTH) / (lattice::NODE_SIZE + lattice::NODE_GAP)) as u8,
    )
}

pub fn height_to_grid_height(height: f32) -> u8 {
    max(
        MIN_GRID_HEIGHT,
        ((height - NON_GRID_HEIGHT) / (lattice::NODE_SIZE + lattice::NODE_GAP)) as u8,
    )
}

pub fn vizia_state(grid_params: Arc<GridParams>) -> Arc<ViziaState> {
    ViziaState::new(move || {
        let width: u32 = ((lattice::NODE_SIZE + lattice::NODE_GAP)
            * (grid_params.width.load(Ordering::Relaxed) as f32)
            + NON_GRID_WIDTH) as u32;
        let height: u32 = ((lattice::NODE_SIZE + lattice::NODE_GAP)
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
            cx.add_theme(include_str!("../assets/theme.css"));

            assets::register_quicksand(cx);
            cx.set_default_font(&[assets::QUICKSAND]);

            data.clone().build(cx);

            HStack::new(cx, |cx| {
                lattice::Lattice::new(cx, Data::params, Data::voices_output).class("container-box");
            });

            let scale: f32 = cx.user_scale_factor() as f32;
            nih_dbg!(cx.window_size());

            Resizer::new(
                cx,
                Data::params.map(|p: &Arc<MidiLatticeParams>| p.grid_params.clone()),
            )
            .right(Units::Pixels(CONTAINER_PADDING))
            .bottom(Units::Pixels(CONTAINER_PADDING))
            .left(Units::Stretch(1.0))
            .top(Units::Stretch(1.0))
            .width(Units::Pixels(RIGHT_REGION_WIDTH - CONTAINER_PADDING * 2.0))
            .height(Units::Pixels(
                BOTTOM_REGION_HEIGHT - CONTAINER_PADDING * 2.0,
            ));

            //  ResizeHandle::new(cx);
        },
    )
}
