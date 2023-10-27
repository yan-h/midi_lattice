use crate::assets;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaTheming};
use std::sync::Arc;

use crate::MidiLatticeParams;

pub(crate) fn create(params: Arc<MidiLatticeParams>) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        params.editor_state.clone(),
        ViziaTheming::None,
        move |cx, _| {
            assets::register_quicksand(cx);
            cx.set_default_font(&[assets::QUICKSAND]);
            Label::new(cx, "MIDI Lattice")
                .font_size(30.0)
                .font_weight(Weight::NORMAL);
        },
    )
}
