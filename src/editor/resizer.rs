use crate::editor::lattice;
use crate::editor::lattice::NODE_SIZE;
use crate::editor::width_to_grid_width;
use crate::editor::CONTAINER_CORNER_RADIUS;
use crate::GridParams;
use nih_plug::{nih_dbg, nih_log};
use nih_plug_vizia::vizia::cache::BoundingBox;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use nih_plug_vizia::widgets::GuiContextEvent;

use nih_plug::prelude::{GuiContext, Param, ParamPtr};

use crate::editor::*;

pub struct Resizer {
    drag_active: bool,
    last_vertical_resize_time: Instant,
    grid_params: Arc<GridParams>,
}

impl Resizer {
    pub fn new<LGridParams>(cx: &mut Context, grid_params: LGridParams) -> Handle<Self>
    where
        LGridParams: Lens<Target = Arc<GridParams>>,
    {
        // Styling is done in the style sheet
        Resizer {
            drag_active: false,
            last_vertical_resize_time: Instant::now(),
            grid_params: grid_params.get(cx),
        }
        .build(cx, |_| {})
    }
}

impl View for Resizer {
    fn element(&self) -> Option<&'static str> {
        Some("resizer")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match *window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                if intersects_box(
                    cx.cache.get_bounds(cx.current()),
                    (cx.mouse.cursorx, cx.mouse.cursory),
                ) {
                    cx.capture();
                    cx.set_active(true);

                    self.drag_active = true;

                    meta.consume();
                } else {
                    // TODO: The click should be forwarded to the element behind the triangle
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    cx.release();
                    cx.set_active(false);

                    self.drag_active = false;
                }
            }
            WindowEvent::MouseMove(x, y) => {
                cx.set_hover(intersects_box(cx.cache.get_bounds(cx.current()), (x, y)));

                if self.drag_active {
                    let (width, height) = (
                        width_to_grid_width(
                            (cx.mouse.cursorx / cx.style.dpi_factor as f32) + lattice::NODE_SIZE,
                        ),
                        height_to_grid_height(
                            (cx.mouse.cursory / cx.style.dpi_factor as f32) + lattice::NODE_SIZE,
                        ),
                    );

                    let (width_changed, height_changed) = (
                        self.grid_params.width.load(Ordering::Relaxed) != width,
                        // Limit frequency of height changes to reduce "flickering" on MacOS
                        self.grid_params.height.load(Ordering::Relaxed) != height
                            && Instant::now().duration_since(self.last_vertical_resize_time)
                                > Duration::from_millis(50),
                    );

                    if width_changed {
                        self.grid_params.width.store(width, Ordering::Relaxed);
                    }
                    if height_changed {
                        self.grid_params.height.store(height, Ordering::Relaxed);
                        self.last_vertical_resize_time = Instant::now();
                    }

                    if width_changed || height_changed {
                        cx.emit(GuiContextEvent::Resize);
                    }
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let scale: f32 = cx.style.dpi_factor as f32;

        // These basics are taken directly from the default implementation of this function
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let mut container_path = vg::Path::new();
        container_path.rounded_rect(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            CONTAINER_CORNER_RADIUS * scale,
        );
        container_path.close();

        // Fill with background color
        let paint = vg::Paint::color(CONTAINER_COLOR);
        canvas.fill_path(&mut container_path, &paint);

        let icon_line_width: f32 = CONTAINER_CORNER_RADIUS * scale;
        let icon_padding: f32 = CONTAINER_CORNER_RADIUS * scale + icon_line_width * 0.5;
        let color = match self.drag_active
            || intersects_box(bounds, (cx.mouse.cursorx, cx.mouse.cursory))
        {
            true => HIGHLIGHT_COLOR,
            false => NODE_COLOR,
        };
        let icon_paint = make_icon_paint(color, scale);
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
