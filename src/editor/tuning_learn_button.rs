use nih_plug::prelude::*;
use nih_plug_vizia::vizia::view::View;
use nih_plug_vizia::vizia::{prelude::*, vg};
use nih_plug_vizia::widgets::ParamEvent;
use triple_buffer::Output;

use crate::midi::Voice;
use crate::tuning::{
    FIVE_12TET, FIVE_JUST, MAX_FIVE, MAX_SEVEN, MAX_THREE, MIN_FIVE, MIN_SEVEN, MIN_THREE,
    SEVEN_12TET, SEVEN_JUST, THREE_12TET, THREE_JUST, TUNING_RANGE_OFFSET,
};
use crate::{TuningParams, Voices};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::editor::{
    intersects_box, make_icon_stroke_paint, COLOR_0, COLOR_1, COLOR_2, COLOR_3,
    CONTAINER_CORNER_RADIUS,
};

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
            crate::editor::CONTAINER_CORNER_RADIUS * scale,
        );
        container_path.close();

        let paint = vg::Paint::color(if highlighted { COLOR_2 } else { COLOR_1 });
        canvas.fill_path(&mut container_path, &paint);

        let icon_line_width: f32 = CONTAINER_CORNER_RADIUS * scale;
        let icon_padding: f32 = CONTAINER_CORNER_RADIUS * scale + icon_line_width * 0.5;

        // Draw "+" or "-"
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
const LEARN_RANGE: f32 = 20.0;

impl TuningLearnButton {
    // Tunes primes 3, 5, and 7 to the best approximation within the current sounding intervals.
    // Only considers approximations within `LEARN_RANGE` cents of the true interval.
    fn learn_tuning(&self, cx: &mut EventContext) {
        let mut voices_output = self.voices_output.lock().unwrap();
        let voices: Vec<Voice> = voices_output.read().values().cloned().collect();
        std::mem::drop(voices_output);

        let mut best_three: Option<f32> = None;
        let mut best_five: Option<f32> = None;
        let mut best_seven: Option<f32> = None;

        let update_best_tuning = |best: &mut Option<f32>, target: f32, interval: f32| {
            if interval >= target - LEARN_RANGE && interval <= target + LEARN_RANGE {
                let diff: f32 = (interval - target).abs();
                match best {
                    Some(tuning) => {
                        if diff < (target - *tuning).abs() {
                            *best = Some(interval);
                        }
                    }
                    None => {
                        *best = Some(interval);
                    }
                }
            }
        };

        let mut i = voices.iter();
        while let Some(voice_a) = i.next() {
            let mut j = i.clone();
            while let Some(voice_b) = j.next() {
                let interval: f32 =
                    (voice_a.get_pitch_class() - voice_b.get_pitch_class()).rem_euclid(1200.0);
                let flipped_interval: f32 = 1200.0 - interval;
                /*
                nih_log!(
                    "{} {} | {} {}",
                    voice_a.get_pitch_class(),
                    voice_b.get_pitch_class(),
                    interval,
                    flipped_interval
                );*/
                update_best_tuning(&mut best_three, THREE_JUST, interval);
                update_best_tuning(&mut best_five, FIVE_JUST, interval);
                update_best_tuning(&mut best_seven, SEVEN_JUST, interval);
                update_best_tuning(&mut best_three, THREE_JUST, flipped_interval);
                update_best_tuning(&mut best_five, FIVE_JUST, flipped_interval);
                update_best_tuning(&mut best_seven, SEVEN_JUST, flipped_interval);
            }
        }

        let mut update_tuning_param =
            |tuning_param: &FloatParam, opt_tuning: Option<f32>| match opt_tuning {
                Some(tuning) => {
                    // nih_dbg!(tuning);
                    cx.emit(ParamEvent::BeginSetParameter(tuning_param).upcast());
                    cx.emit(ParamEvent::SetParameter(tuning_param, tuning).upcast());
                    cx.emit(ParamEvent::EndSetParameter(tuning_param).upcast());
                }
                None => (),
            };

        update_tuning_param(&self.tuning_params.three, best_three);
        update_tuning_param(&self.tuning_params.five, best_five);
        update_tuning_param(&self.tuning_params.seven, best_seven);
    }
}
