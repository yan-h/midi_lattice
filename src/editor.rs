use crate::assets;

use crate::MidiLatticeParams;
use crate::Voices;

use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::ResizeHandle;
use nih_plug_vizia::{create_vizia_editor, ViziaTheming};

use std::sync::{Arc, Mutex};
use triple_buffer::Output;

mod lattice;

#[derive(Lens, Clone)]
pub(crate) struct Data {
    params: Arc<MidiLatticeParams>,
    voices_output: Arc<Mutex<Output<Voices>>>,
}

impl Data {
    pub(crate) fn new(
        params: Arc<MidiLatticeParams>,
        voices_output: Arc<Mutex<Output<Voices>>>,
    ) -> Self {
        Self {
            params,
            voices_output,
        }
    }
}

impl Model for Data {}

pub(crate) fn create(data: Data) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        data.params.editor_state.clone(),
        ViziaTheming::None,
        move |cx, _| {
            cx.add_theme(include_str!("../assets/widgets.css"));

            assets::register_quicksand(cx);
            cx.set_default_font(&[assets::QUICKSAND]);

            data.clone().build(cx);

            VStack::new(cx, |cx| {
                Label::new(cx, "editor")
                    .font_size(30.0)
                    .font_weight(Weight::NORMAL);
                lattice::Lattice::new(cx, Data::params, Data::voices_output);
            });

            ResizeHandle::new(cx);
        },
    )
}
