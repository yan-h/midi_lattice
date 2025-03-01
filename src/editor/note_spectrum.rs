use crate::midi::MidiVoice;
use crate::tuning::*;

use crate::MidiLatticeParams;
use crate::Voices;

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::sync::{Arc, Mutex};
use triple_buffer::Output;

use crate::editor::color::*;

use crate::editor::CORNER_RADIUS;

use super::lattice::grid::get_sorted_grid_pitch_classes;

pub struct NoteSpectrum {
    params: Arc<MidiLatticeParams>,
    voices_output: Arc<Mutex<Output<Voices>>>,
}

impl NoteSpectrum {
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

impl View for NoteSpectrum {
    fn element(&self) -> Option<&'static str> {
        Some("lattice")
    }

    fn event(&mut self, _cx: &mut EventContext, _event: &mut Event) {}

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // Background rectangle
        let mut background_path = vg::Path::new();
        background_path.rounded_rect(
            cx.bounds().x,
            cx.bounds().y,
            cx.bounds().width(),
            cx.bounds().height(),
            CORNER_RADIUS * cx.scale_factor(),
        );
        canvas.fill_path(&background_path, &vg::Paint::color(BASE_COLOR));

        let min_pitch: f32 = 60.0 - 12.0 * 3.0;
        let max_pitch: f32 = 60.0 + 12.0 * 4.0;

        // Set up data structures
        let mut voices_output = self.voices_output.lock().unwrap();
        let voices: Vec<MidiVoice> = voices_output.read().values().cloned().collect();
        std::mem::drop(voices_output);

        let sorted_grid_pitch_classes: Vec<PitchClass> =
            get_sorted_grid_pitch_classes(&self.params);
        let tuning_tolerance =
            PitchClassDistance::from_cents_f32(self.params.tuning_params.tolerance.value());

        // Draw voices
        for voice in voices {
            if voice.get_channel() == 15 {
                continue;
            }
            let pitch = voice.get_pitch();
            let color = note_color(
                voice.get_channel(),
                pitch,
                self.params.grid_params.darkest_pitch.value(),
                self.params.grid_params.brightest_pitch.value(),
            );

            let pitch_idx = if pitch < min_pitch {
                min_pitch
            } else if pitch > max_pitch {
                max_pitch
            } else {
                (pitch - min_pitch) / (max_pitch - min_pitch)
            };

            let matches_grid_pitch_class = pitch_class_matches_any_in_sorted_vec(
                PitchClass::from_midi_note_f32(pitch),
                &sorted_grid_pitch_classes,
                tuning_tolerance,
            );
            let mut pitch_path = vg::Path::new();

            let start_x = cx.bounds().x
                + if matches_grid_pitch_class {
                    0.0
                } else {
                    0.2 * cx.bounds().width()
                };

            pitch_path.move_to(
                start_x,
                cx.bounds().y + cx.bounds().height() - pitch_idx * cx.bounds().height(),
            );
            pitch_path.line_to(
                cx.bounds().x + cx.bounds().width(),
                cx.bounds().y + cx.bounds().height() - pitch_idx * cx.bounds().height(),
            );

            let mut paint = vg::Paint::color(color);
            paint.set_line_width(1.5 * cx.scale_factor());
            paint.set_line_cap(vg::LineCap::Butt);
            canvas.stroke_path(&pitch_path, &paint);
        }

        // Notches on side
        for half_octave in -10..11i32 {
            let notch_pitch = 60.0 + 6.0 * half_octave as f32;
            if notch_pitch < min_pitch + 1.0 || notch_pitch > max_pitch - 1.0 {
                continue;
            }
            let pitch_idx = (notch_pitch - min_pitch) / (max_pitch - min_pitch);
            let mut notch_path = vg::Path::new();
            let (length, width): (f32, f32) = if half_octave.rem_euclid(2) == 0 {
                (0.2, 3.0)
            } else {
                (0.1, 2.0)
            };
            notch_path.move_to(
                cx.bounds().x + cx.bounds().width() * (1.0 - length),
                cx.bounds().y + cx.bounds().height() - pitch_idx * cx.bounds().height(),
            );
            notch_path.line_to(
                cx.bounds().x + cx.bounds().width(),
                cx.bounds().y + cx.bounds().height() - pitch_idx * cx.bounds().height(),
            );

            let mut notch_paint = vg::Paint::color(BACKGROUND_COLOR);
            notch_paint.set_line_width(width * cx.scale_factor());
            notch_paint.set_line_cap(vg::LineCap::Round);

            canvas.stroke_path(&notch_path, &notch_paint);
        }
    }
}
