//! A resize handle for uniformly scaling a plugin GUI.

use crate::editor::{intersects_box, CORNER_RADIUS, PADDING};
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::make_icon_stroke_paint;

use crate::editor::color::*;

/// A resize handle placed at the bottom right of the window that lets you resize the window.
///
/// Needs to be the last element in the GUI because of how event targetting in Vizia works right
/// now.
pub struct Resizer {
    /// Will be set to `true` if we're dragging the parameter. Resetting the parameter or entering a
    /// text value should not initiate a drag.
    drag_active: bool,

    /// The scale factor when we started dragging. This is kept track of separately to avoid
    /// accumulating rounding errors.
    start_scale_factor: f64,
    /// The DPI factor when we started dragging, includes both the HiDPI scaling and the user
    /// scaling factor. This is kept track of separately to avoid accumulating rounding errors.
    start_dpi_factor: f32,
    /// The cursor position in physical screen pixels when the drag started.
    start_physical_coordinates: (f32, f32),
}

impl Resizer {
    /// Create a resize handle at the bottom right of the window. This should be created at the top
    /// level. Dragging this handle around will cause the window to be resized.
    pub fn new(cx: &mut Context) -> Handle<Self> {
        // Styling is done in the style sheet
        Resizer {
            drag_active: false,
            start_scale_factor: 1.0,
            start_dpi_factor: 1.0,
            start_physical_coordinates: (0.0, 0.0),
        }
        .build(cx, |_| {})
    }
}

impl View for Resizer {
    fn element(&self) -> Option<&'static str> {
        Some("resize-handle")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, _meta| match *window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.capture();
                cx.set_active(true);
                nih_log!("down");

                self.drag_active = true;
                self.start_scale_factor = cx.user_scale_factor();
                self.start_dpi_factor = cx.scale_factor();
                self.start_physical_coordinates = (
                    cx.mouse().cursorx * self.start_dpi_factor,
                    cx.mouse().cursory * self.start_dpi_factor,
                );
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                nih_log!("up");
                if self.drag_active {
                    cx.release();
                    cx.set_active(false);

                    self.drag_active = false;
                }
            }
            WindowEvent::MouseMove(x, y) => {
                if self.drag_active {
                    // We need to convert our measurements into physical pixels relative to the
                    // initial drag to be able to keep a consistent ratio. This 'relative to the
                    // start' bit is important because otherwise we would be comparing the position
                    // to the same absoltue screen spotion.
                    // TODO: This may start doing fun things when the window grows so large that it
                    //       gets pushed upwards or leftwards
                    let (compensated_physical_x, compensated_physical_y) =
                        (x * self.start_dpi_factor, y * self.start_dpi_factor);
                    let (start_physical_x, start_physical_y) = self.start_physical_coordinates;
                    let new_scale_factor = (self.start_scale_factor
                        * (compensated_physical_x / start_physical_x)
                            .max(compensated_physical_y / start_physical_y)
                            as f64)
                        .max(0.5)
                        .min(4.0);

                    cx.set_user_scale_factor(new_scale_factor);
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let scale: f32 = cx.scale_factor() as f32;
        let bounds = cx.bounds();
        let highlighted: bool =
            self.drag_active || intersects_box(bounds, (cx.mouse().cursorx, cx.mouse().cursory));

        let mut container_path = vg::Path::new();
        container_path.rounded_rect(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            CORNER_RADIUS * scale,
        );
        container_path.close();

        // Fill with background color
        let paint = vg::Paint::color(if self.drag_active {
            TEXT_COLOR
        } else if highlighted {
            HIGHLIGHT_COLOR
        } else {
            BASE_COLOR
        });
        canvas.fill_path(&mut container_path, &paint);

        let icon_line_width: f32 = PADDING * scale;
        let icon_padding: f32 = PADDING * scale + icon_line_width * 0.5;
        let color = BACKGROUND_COLOR;
        let icon_paint = make_icon_stroke_paint(color, scale);
        let mut icon_path = vg::Path::new();
        // top right
        icon_path.move_to(bounds.x + bounds.w - icon_padding, bounds.y + icon_padding);
        // bottom right
        icon_path.line_to(
            bounds.x + bounds.w - icon_padding,
            bounds.y + bounds.h - icon_padding,
        );
        // bottom left
        icon_path.line_to(bounds.x + icon_padding, bounds.y + bounds.h - icon_padding);
        // top right
        icon_path.line_to(bounds.x + bounds.w - icon_padding, bounds.y + icon_padding);
        icon_path.close();

        canvas.stroke_path(&mut icon_path, &icon_paint);
    }
}
