use crate::MidiLatticeParams;
use crate::Voices;
use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use triple_buffer::Output;

pub(crate) struct Lattice {
    params: Arc<MidiLatticeParams>,
    voices_output: Arc<Mutex<Output<Voices>>>,
}

impl Lattice {
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
        }
        .build(
            cx,
            // This is an otherwise empty element only used for custom drawing
            |_cx| (),
        )
    }
}

impl View for Lattice {
    fn element(&self) -> Option<&'static str> {
        Some("lattice")
    }

    fn draw(&self, _cx: &mut DrawContext, _canvas: &mut Canvas) {}

    fn event(&mut self, _cx: &mut EventContext, _event: &mut Event) {}
}
