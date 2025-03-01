use std::sync::{Arc, Mutex};

use nih_plug_vizia::vizia::{
    prelude::*,
    vg::{self, FontId},
};
use triple_buffer::Output;

use crate::editor::color::*;
use crate::tuning::*;
use crate::{assets, MidiLatticeParams, Voices};

use crate::editor::lattice::grid::get_sorted_grid_pitch_classes;

/// Text indicating how many sounding voices match a pitch class on the grid.
pub struct NoteMatchInfo {
    params: Arc<MidiLatticeParams>,

    // Reads voices from the audio thread
    voices_output: Arc<Mutex<Output<Voices>>>,
}

impl NoteMatchInfo {
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
        .build(cx, |_cx| {})
    }
}

impl View for NoteMatchInfo {
    fn element(&self) -> Option<&'static str> {
        Some("note-match-info")
    }

    fn event(&mut self, _cx: &mut EventContext, _event: &mut Event) {}

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let scale: f32 = cx.scale_factor() as f32;
        let bounds = cx.bounds();

        // Draw background
        let mut container_path = vg::Path::new();
        container_path.rounded_rect(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            crate::editor::CORNER_RADIUS * scale,
        );
        container_path.close();

        let paint = vg::Paint::color(BASE_COLOR);
        canvas.fill_path(&mut container_path, &paint);

        // Compute matched voices
        let mut voices_output = self.voices_output.lock().unwrap();
        let voice_pitch_classes: Vec<PitchClass> = voices_output
            .read()
            .values()
            .cloned()
            .map(|v| v.get_pitch_class())
            .collect();
        std::mem::drop(voices_output);

        let tuning_tolerance =
            PitchClassDistance::from_cents_f32(self.params.tuning_params.tolerance.value());
        let sorted_grid_pitch_classes: Vec<PitchClass> =
            get_sorted_grid_pitch_classes(&self.params);

        let mut num_matched_voices: u32 = 0;
        for voice_pitch_class in &voice_pitch_classes {
            if pitch_class_matches_any_in_sorted_vec(
                voice_pitch_class.clone(),
                &sorted_grid_pitch_classes,
                tuning_tolerance,
            ) {
                num_matched_voices += 1;
            }
        }

        // Draw text
        let num_voices = voice_pitch_classes.len();
        let text_to_display: String = if num_voices == 0 {
            "No notes playing".to_string()
        } else if num_voices == num_matched_voices as usize {
            format!("All {} notes matched", num_matched_voices)
        } else {
            format!("{}/{} notes matched", num_matched_voices, voice_pitch_classes.len())
        };

        let mut text_paint = vg::Paint::color(TEXT_COLOR);
        text_paint.set_text_align(vg::Align::Left);
        text_paint.set_font_size(15.0 * scale);
        let _ = canvas.fill_text(
            bounds.x + 3.0 * scale,
            bounds.y + 15.0 * scale,
            text_to_display,
            &text_paint,
        );
    }
}
