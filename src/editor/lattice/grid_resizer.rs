use crate::editor::color::*;
use crate::editor::lattice::grid;
use crate::editor::lattice::LatticeEvent;
use crate::editor::width_to_grid_width;
use crate::editor::*;
use crate::GridParams;

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::GuiContextEvent;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct GridResizer {
    drag_active: bool,
    grid_params: Arc<GridParams>,
    mouse_over: bool,
    lattice_mouse_down: bool,
}

impl GridResizer {
    pub fn new<LGridParams>(cx: &mut Context, grid_params: LGridParams) -> Handle<Self>
    where
        LGridParams: Lens<Target = Arc<GridParams>>,
    {
        // Styling is done in the style sheet
        GridResizer {
            drag_active: false,
            grid_params: grid_params.get(cx),
            mouse_over: false,
            lattice_mouse_down: false,
        }
        .build(cx, |_| {})
    }
}

impl View for GridResizer {
    fn element(&self) -> Option<&'static str> {
        Some("resizer")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|lattice_event, _meta| match *lattice_event {
            LatticeEvent::MouseOver => cx.set_visibility(Visibility::Visible),
            LatticeEvent::MouseOut => cx.set_visibility(Visibility::Hidden),
            LatticeEvent::MouseDown => {
                self.lattice_mouse_down = true;
            }
            LatticeEvent::MouseUpToChild => {
                //meta.consume();
                self.lattice_mouse_down = false;
            }
            _ => {}
        });
        event.map(|window_event, _meta| match *window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.capture();
                self.drag_active = true;
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                cx.emit(LatticeEvent::MouseUpFromChild);
                if self.drag_active {
                    cx.release();
                    self.drag_active = false;
                }
            }
            WindowEvent::MouseOver => {
                self.mouse_over = true;
            }
            WindowEvent::MouseOut => {
                self.mouse_over = false;
            }
            WindowEvent::MouseMove(_x, _y) => {
                if self.drag_active {
                    let (width, height) = (
                        width_to_grid_width(
                            (cx.mouse().cursorx / cx.scale_factor() as f32 + RIGHT_REGION_WIDTH)
                                + grid::NODE_SIZE,
                        ),
                        height_to_grid_height(
                            (cx.mouse().cursory / cx.scale_factor() as f32 + BOTTOM_REGION_HEIGHT)
                                + grid::NODE_SIZE,
                        ),
                    );

                    self.grid_params.width.store(width, Ordering::Relaxed);
                    self.grid_params.height.store(height, Ordering::Relaxed);

                    cx.emit(GuiContextEvent::Resize);
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let scale: f32 = cx.scale_factor() as f32;
        let bounds = cx.bounds();

        let icon_padding: f32 = PADDING * 1.75 * scale;

        let color = if self.drag_active {
            OVERLAY_COLOR_2
        } else if self.mouse_over && !self.lattice_mouse_down {
            OVERLAY_COLOR_1
        } else {
            OVERLAY_COLOR_0
        };
        let icon_paint = &make_icon_paint(color, PADDING * 2.5 * scale);
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
