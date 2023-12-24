use nih_plug::prelude::*;
use nih_plug_vizia::vizia::view::View;
use nih_plug_vizia::vizia::{prelude::*, vg};
use nih_plug_vizia::widgets::ParamEvent;
use triple_buffer::Output;

use crate::tuning::*;
use crate::{TuningParams, Voices};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::editor::{
    intersects_box, make_icon_stroke_paint, COLOR_0, COLOR_1, COLOR_2, COLOR_3, CORNER_RADIUS,
};

use super::PADDING;

pub struct TuningLearnButton {
    learn_active: bool,
    tuning_params: Arc<TuningParams>,
    voices_output: Arc<Mutex<Output<Voices>>>,
}

pub enum TickEvent {
    Tick,
}

impl TuningLearnButton {
    pub fn new<LParams, LVoices>(
        cx: &mut Context,
        tuning_params: LParams,
        voices_output: LVoices,
    ) -> Handle<Self>
    where
        LParams: Lens<Target = Arc<TuningParams>>,
        LVoices: Lens<Target = Arc<Mutex<Output<Voices>>>>,
    {
        Self {
            tuning_params: tuning_params.get(cx),
            voices_output: voices_output.get(cx),
            learn_active: false,
        }
        .build(cx, |cx| {
            // Emit an event ~60 times per second to update tuning
            cx.spawn(move |cx_proxy| loop {
                let _ = cx_proxy.emit(TickEvent::Tick);
                thread::sleep(Duration::from_millis(16));
            });
        })
    }
}

impl View for TuningLearnButton {
    fn element(&self) -> Option<&'static str> {
        Some("tuning-learn-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|tick_event: &TickEvent, _meta| match *tick_event {
            TickEvent::Tick => {
                if self.learn_active {
                    self.learn_tuning(cx);
                }
            }
        });
        event.map(|window_event, _meta| match *window_event {
            WindowEvent::PressDown { mouse: _ } => {
                self.learn_active = !self.learn_active;
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let scale: f32 = cx.scale_factor() as f32;
        let bounds = cx.bounds();
        let highlighted: bool =
            self.learn_active || intersects_box(bounds, (cx.mouse().cursorx, cx.mouse().cursory));

        let mut container_path = vg::Path::new();
        container_path.rounded_rect(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            crate::editor::CORNER_RADIUS * scale,
        );
        container_path.close();

        let paint = vg::Paint::color(if self.learn_active {
            COLOR_3
        } else if highlighted {
            COLOR_2
        } else {
            COLOR_1
        });
        canvas.fill_path(&mut container_path, &paint);

        let icon_line_width: f32 = PADDING * scale;
        let icon_padding: f32 = PADDING * scale + icon_line_width * 0.5;

        // Draw tuning symbol
        let mut icon_path = vg::Path::new();
        icon_path.move_to(bounds.x + bounds.w * 0.38, bounds.y + icon_padding);
        icon_path.line_to(bounds.x + bounds.w * 0.38, bounds.y + bounds.h * 0.5);
        icon_path.line_to(bounds.x + bounds.w * 0.62, bounds.y + bounds.h * 0.5);
        icon_path.line_to(bounds.x + bounds.w * 0.62, bounds.y + icon_padding);
        icon_path.move_to(bounds.x + bounds.w * 0.5, bounds.y + bounds.h * 0.5);
        icon_path.line_to(
            bounds.x + bounds.w * 0.5,
            bounds.y + bounds.h - icon_padding,
        );
        icon_path.close();

        let icon_paint = make_icon_stroke_paint(COLOR_0, scale);

        canvas.stroke_path(&mut icon_path, &icon_paint);
    }
}

// How close an interval needs to be to its just interval to be autodetected
const LEARN_RANGE: PitchClassDistance = PitchClassDistance::from_cents(40);

const DEFAULT_C: PitchClass = PitchClass::from_microcents(0);
const TUNE_C_TOLERANCE: PitchClassDistance =
    PitchClassDistance::from_microcents(50 * CENTS_TO_MICROCENTS);

impl TuningLearnButton {
    /// Attempts to tune C; and primes 3, 5, and 7; based on the sounding pitch classes
    fn learn_tuning(&self, cx: &mut EventContext) {
        let mut voices_output = self.voices_output.lock().unwrap();

        let mut pitch_classes: Vec<PitchClass> = voices_output
            .read()
            .values()
            .map(|voice| voice.get_pitch_class())
            .collect();
        std::mem::drop(voices_output);
        pitch_classes.sort_unstable();
        pitch_classes.dedup();

        self.learn_c_tuning(cx, &pitch_classes);
        self.learn_intervals_tuning(cx, &pitch_classes);
    }

    /// Tunes C to the best approximation present in the given list of pitch classes.
    /// Only pitch classes within 50 cents of C in MIDI (~262 Hz) are considered
    fn learn_c_tuning(&self, cx: &mut EventContext, sorted_pitch_classes: &Vec<PitchClass>) {
        // Tune C
        let mut best_c: Option<PitchClass> = None;
        for pitch_class in sorted_pitch_classes {
            if pitch_class.distance_to(DEFAULT_C) <= TUNE_C_TOLERANCE {
                best_c = match best_c {
                    None => Some(*pitch_class),
                    Some(c) => Some(
                        if c.distance_to(DEFAULT_C) < pitch_class.distance_to(DEFAULT_C) {
                            c
                        } else {
                            *pitch_class
                        },
                    ),
                };
            }
        }
        best_c.map(|new_c| {
            let c_cents: f32 = new_c.to_cents_f32();
            let zero_centered_c_cents: f32 = if c_cents > 600.0 {
                c_cents - 1200.0
            } else {
                c_cents
            };
            cx.emit(ParamEvent::BeginSetParameter(&self.tuning_params.c_offset).upcast());
            cx.emit(
                ParamEvent::SetParameter(&self.tuning_params.c_offset, zero_centered_c_cents)
                    .upcast(),
            );
            cx.emit(ParamEvent::EndSetParameter(&self.tuning_params.c_offset).upcast());
        });
    }

    /// Tunes primes 3, 5, and 7 to the best approximation among the current sounding pitch classes.
    /// Only considers approximations within [`LEARN_RANGE`] cents of the true interval.
    fn learn_intervals_tuning(
        &self,
        cx: &mut EventContext,
        sorted_pitch_classes: &Vec<PitchClass>,
    ) {
        // Tune intervals
        let mut best_three: Option<PitchClass> = None;
        let mut best_five: Option<PitchClass> = None;
        let mut best_seven: Option<PitchClass> = None;

        let update_best_tuning =
            |best: &mut Option<PitchClass>, interval: PitchClass, target: PitchClass| {
                let diff = interval.distance_to(target);
                if diff <= LEARN_RANGE {
                    match best {
                        Some(best_tuning) => {
                            if diff < best_tuning.distance_to(target) {
                                *best = Some(interval);
                            }
                        }
                        None => {
                            *best = Some(interval);
                        }
                    }
                }
            };

        let mut i = sorted_pitch_classes.iter();
        while let Some(pc_a) = i.next() {
            let mut j = i.clone();
            while let Some(pc_b) = j.next() {
                // Test A - B as well as B - A.
                // For example, a tuning for the perfect fourth implies a one for the perfect fifth.
                // This is true because this plugin assumes perfectly tuned octaves.
                let interval: PitchClass = *pc_a - *pc_b;
                let flipped_interval: PitchClass = -interval;

                //nih_log!("{} {}", interval, flipped_interval);
                update_best_tuning(&mut best_three, interval, THREE_JUST);
                update_best_tuning(&mut best_five, interval, FIVE_JUST);
                update_best_tuning(&mut best_seven, interval, SEVEN_JUST);
                update_best_tuning(&mut best_three, flipped_interval, THREE_JUST);
                update_best_tuning(&mut best_five, flipped_interval, FIVE_JUST);
                update_best_tuning(&mut best_seven, flipped_interval, SEVEN_JUST);
            }
        }

        let mut update_tuning_param =
            |tuning_param: &FloatParam, opt_tuning: Option<PitchClass>| match opt_tuning {
                Some(tuning) => {
                    // nih_dbg!(tuning);
                    cx.emit(ParamEvent::BeginSetParameter(tuning_param).upcast());
                    cx.emit(ParamEvent::SetParameter(tuning_param, tuning.to_cents_f32()).upcast());
                    cx.emit(ParamEvent::EndSetParameter(tuning_param).upcast());
                }
                None => (),
            };

        update_tuning_param(&self.tuning_params.three, best_three);
        update_tuning_param(&self.tuning_params.five, best_five);
        update_tuning_param(&self.tuning_params.seven, best_seven);
    }
}
