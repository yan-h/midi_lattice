use nih_plug::prelude::*;
use nih_plug_vizia::vizia::view::View;
use nih_plug_vizia::vizia::{prelude::*, vg};
use triple_buffer::Output;

use crate::midi::Voice;
use crate::tuning::{
    FIVE_12TET, FIVE_JUST, MAX_FIVE, MAX_SEVEN, MAX_THREE, MIN_FIVE, MIN_SEVEN, MIN_THREE,
    SEVEN_12TET, SEVEN_JUST, THREE_12TET, THREE_JUST, TUNING_RANGE_OFFSET,
};
use crate::{TuningParams, Voices};
use std::sync::{Arc, Mutex};

use crate::editor::{
    intersects_box, make_icon_stroke_paint, COLOR_0, COLOR_1, COLOR_2, COLOR_3,
    CONTAINER_CORNER_RADIUS,
};

pub struct TuningLearnButton {
    learn_active: bool,
    tuning_params: Arc<TuningParams>,
    voices_output: Arc<Mutex<Output<Voices>>>,
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
        .build(
            cx,
            // This is an otherwise empty element only used for custom drawing
            |_cx| (),
        )
    }
}

impl View for TuningLearnButton {
    fn element(&self) -> Option<&'static str> {
        Some("tuning-learn-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, _meta| match *window_event {
            WindowEvent::PressDown { mouse: _ } => {
                self.learn_active = !self.learn_active;
                nih_log!("tuning learn btn press");
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

impl TuningLearnButton {
    fn learn_tuning(&self) {
        let mut voices_output = self.voices_output.lock().unwrap();
        let voices: Vec<Voice> = voices_output.read().values().cloned().collect();
        let mut best_three: Option<f32> = None;
        let mut best_five: Option<f32> = None;
        let mut best_seven: Option<f32> = None;

        let mut i = voices.iter();
        while let Some(x) = i.next() {
            let mut j = i.clone();
            while let Some(y) = j.next() {
                let interval: f32 = (x.get_pitch_class() - y.get_pitch_class()).rem_euclid(1200.00);

                let update_best_tuning =
                    |best: &mut Option<f32>, target: f32, min: f32, max: f32| {
                        let diff: f32 = (interval - target).abs();
                        if interval >= min && interval <= max {
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
                update_best_tuning(&mut best_three, THREE_JUST, MIN_THREE, MAX_THREE);
                update_best_tuning(&mut best_five, FIVE_JUST, MIN_FIVE, MAX_FIVE);
                update_best_tuning(&mut best_seven, SEVEN_JUST, MIN_SEVEN, MAX_SEVEN);
            }
        }
        /*
        match best_three {
            Some(tuning) => {
                cx.emit(ParamEvent::BeginSetParameter(self.tuning_params).upcast());
                cx.emit(
                    ParamEvent::SetParameter(&self.params.filter_stages, clamped_filter_stages)
                        .upcast(),
                );
                cx.emit(ParamEvent::EndSetParameter(&self.params.filter_stages).upcast());
            }
            None => (),
        }
        */
    }
}
