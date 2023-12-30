use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use crate::editor::color::*;
use crate::editor::*;

pub struct ScaleButton {
    direction: Direction,
}

const SCALE_CHANGE_AMOUNT: f64 = 0.1;
const MAX_SCALE: f64 = 1.5;
const MIN_SCALE: f64 = 0.6;

pub enum Direction {
    Up,
    Down,
}

impl ScaleButton {
    pub fn new(cx: &mut Context, direction: Direction) -> Handle<Self> {
        // Styling is done in the style sheet
        ScaleButton { direction }.build(cx, |_| {})
    }
}

impl View for ScaleButton {
    fn element(&self) -> Option<&'static str> {
        Some("scale-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let scale_factor = cx.user_scale_factor();
        event.map(|window_event, _meta| match *window_event {
            WindowEvent::PressDown { mouse: _ } => {
                match self.direction {
                    Direction::Up => {
                        cx.set_user_scale_factor(1.5);
                        /*
                        cx.set_user_scale_factor(
                            MAX_SCALE
                                .min((scale_factor * 10.0).round() * 0.1 + SCALE_CHANGE_AMOUNT),
                        );*/
                    }
                    Direction::Down => {
                        cx.set_user_scale_factor(0.5);
                        /*
                        cx.set_user_scale_factor(
                            MIN_SCALE
                                .max((scale_factor * 10.0).round() * 0.1 - SCALE_CHANGE_AMOUNT),
                        );
                        */
                    }
                }
            }
            _ => {}
        });

        nih_dbg!(cx.user_scale_factor());
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let scale: f32 = cx.scale_factor() as f32;
        let bounds = cx.bounds();
        let highlighted = intersects_box(bounds, (cx.mouse().cursorx, cx.mouse().cursory));

        let mut container_path = vg::Path::new();
        container_path.rounded_rect(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            crate::editor::CORNER_RADIUS * scale,
        );
        container_path.close();

        let paint = vg::Paint::color(if highlighted { COLOR_2 } else { COLOR_1 });
        canvas.fill_path(&mut container_path, &paint);

        let icon_line_width: f32 = CORNER_RADIUS * scale;
        let icon_padding: f32 = CORNER_RADIUS * scale + icon_line_width * 0.5;

        // Draw "+" or "-"
        let mut icon_path = vg::Path::new();
        icon_path.move_to(bounds.x + icon_padding, bounds.y + bounds.w * 0.5);
        icon_path.line_to(
            bounds.x + bounds.w - icon_padding,
            bounds.y + bounds.h * 0.5,
        );
        match self.direction {
            Direction::Up => {
                icon_path.move_to(bounds.x + bounds.w * 0.5, bounds.y + icon_padding);
                icon_path.line_to(
                    bounds.x + bounds.w * 0.5,
                    bounds.y + bounds.h - icon_padding,
                );
            }
            Direction::Down => {}
        }
        icon_path.close();

        let color = if highlighted { COLOR_3 } else { COLOR_2 };
        let icon_paint = make_icon_stroke_paint(color, scale);

        canvas.stroke_path(&mut icon_path, &icon_paint);
    }
}
