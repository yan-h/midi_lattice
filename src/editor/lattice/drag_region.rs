use crate::editor::lattice::grid::NODE_SIZE;
use crate::editor::lattice::LatticeEvent;
use crate::editor::*;
use crate::GridParams;
use crate::GRID_MAX_DISTANCE;

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::ParamEvent;
use std::sync::Arc;

/// Draggable region on the lattice. When moused over, shows a visual indicator that it's draggable.
pub struct DragRegion {
    grid_params: Arc<GridParams>,

    // Whether something else is being dragged on the lattice.
    lattice_mouse_down: bool,

    // Whether the mouse is over this region. Controls whether the icon is partially highlighted.
    mouse_over: bool,

    // Whether this is being dragged. Controls wherther the icon is fully highlighted, and
    // whether mouse motion drags the grid.
    drag_active: bool,

    // State used to calculate grid position during drag
    start_physical_coordinates: (f32, f32),
    start_grid_coordinates: (f32, f32),
}

impl DragRegion {
    pub fn new<L>(cx: &mut Context, grid_params: L) -> Handle<Self>
    where
        L: Lens<Target = Arc<GridParams>> + Clone,
    {
        // Styling is done in the style sheet
        DragRegion {
            grid_params: grid_params.get(cx),
            lattice_mouse_down: false,
            mouse_over: false,
            drag_active: false,
            start_physical_coordinates: (0.0, 0.0),
            start_grid_coordinates: (0.0, 0.0),
        }
        .build(cx, |_| {})
    }
}

fn normalize_grid_position(pos: f32) -> f32 {
    (pos + GRID_MAX_DISTANCE) / (GRID_MAX_DISTANCE * 2.0)
}

impl View for DragRegion {
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
                // cx.set_active(true);

                self.drag_active = true;
                self.start_physical_coordinates = (
                    cx.mouse().cursorx, // * cx.scale_factor(),
                    cx.mouse().cursory, // * cx.scale_factor(),
                );
                self.start_grid_coordinates =
                    (self.grid_params.x.value(), self.grid_params.y.value());
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                cx.emit(LatticeEvent::MouseUpFromChild);

                if self.drag_active {
                    cx.release();
                    self.drag_active = false;

                    cx.emit(ParamEvent::BeginSetParameter(&self.grid_params.x).upcast());
                    cx.emit(
                        ParamEvent::SetParameter(
                            &self.grid_params.x,
                            self.grid_params.x.value().round(),
                        )
                        .upcast(),
                    );
                    cx.emit(ParamEvent::EndSetParameter(&self.grid_params.x).upcast());

                    cx.emit(ParamEvent::BeginSetParameter(&self.grid_params.y).upcast());
                    cx.emit(
                        ParamEvent::SetParameter(
                            &self.grid_params.y,
                            self.grid_params.y.value().round(),
                        )
                        .upcast(),
                    );
                    cx.emit(ParamEvent::EndSetParameter(&self.grid_params.y).upcast());
                }
            }
            WindowEvent::MouseOver => {
                self.mouse_over = true;
            }
            WindowEvent::MouseOut => {
                self.mouse_over = false;
            }
            WindowEvent::MouseMove(mouse_x, mouse_y) => {
                let (start_physical_coordinates_x, start_physical_coordinates_y) =
                    self.start_physical_coordinates;
                let (start_grid_x, start_grid_y) = self.start_grid_coordinates;

                if self.drag_active {
                    // Move the grid according to how far the mouse moved from the start drag location
                    let grid_x_offset = (mouse_x - start_physical_coordinates_x)
                        / (cx.scale_factor() * (NODE_SIZE + CONTAINER_PADDING));

                    let grid_y_offset = (mouse_y - start_physical_coordinates_y)
                        / (cx.scale_factor() * (NODE_SIZE + CONTAINER_PADDING));

                    cx.emit(ParamEvent::BeginSetParameter(&self.grid_params.x).upcast());
                    cx.emit(
                        ParamEvent::SetParameter(&self.grid_params.x, start_grid_x - grid_x_offset)
                            .upcast(),
                    );
                    cx.emit(ParamEvent::EndSetParameter(&self.grid_params.x).upcast());

                    cx.emit(ParamEvent::BeginSetParameter(&self.grid_params.y).upcast());
                    cx.emit(
                        ParamEvent::SetParameter(&self.grid_params.y, start_grid_y + grid_y_offset)
                            .upcast(),
                    );
                    cx.emit(ParamEvent::EndSetParameter(&self.grid_params.y).upcast());
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();

        if cx.visibility() == Some(Visibility::Visible) {
            // Draw "draggable" icon in center
            let icon_radius: f32 = NODE_SIZE * 1.4 * cx.scale_factor();
            let arrow_size: f32 = NODE_SIZE * 0.4 * cx.scale_factor();
            let (center_x, center_y) = (bounds.x + bounds.w * 0.5, bounds.y + bounds.h * 0.5);
            let (left_x, top_y, right_x, bottom_y) = (
                center_x - icon_radius,
                center_y - icon_radius,
                center_x + icon_radius,
                center_y + icon_radius,
            );
            let mut icon_path = vg::Path::new();

            // center lines
            icon_path.move_to(left_x, center_y);
            icon_path.line_to(right_x, center_y);
            icon_path.move_to(center_x, top_y);
            icon_path.line_to(center_x, bottom_y);

            // top arrow
            icon_path.move_to(center_x, top_y);
            icon_path.line_to(center_x - arrow_size, top_y + arrow_size);
            icon_path.move_to(center_x, top_y);
            icon_path.line_to(center_x + arrow_size, top_y + arrow_size);

            // bottom_arrow
            icon_path.move_to(center_x, bottom_y);
            icon_path.line_to(center_x - arrow_size, bottom_y - arrow_size);
            icon_path.move_to(center_x, bottom_y);
            icon_path.line_to(center_x + arrow_size, bottom_y - arrow_size);

            // left arrow
            icon_path.move_to(left_x, center_y);
            icon_path.line_to(left_x + arrow_size, center_y - arrow_size);
            icon_path.move_to(left_x, center_y);
            icon_path.line_to(left_x + arrow_size, center_y + arrow_size);

            // right arrow
            icon_path.move_to(right_x, center_y);
            icon_path.line_to(right_x - arrow_size, center_y - arrow_size);
            icon_path.move_to(right_x, center_y);
            icon_path.line_to(right_x - arrow_size, center_y + arrow_size);

            icon_path.close();

            let color = if self.drag_active {
                OVERLAY_COLOR_2
            } else if self.mouse_over && !self.lattice_mouse_down {
                OVERLAY_COLOR_1
            } else {
                OVERLAY_COLOR_0
            };

            canvas.stroke_path(
                &mut icon_path,
                &make_icon_paint(color, CONTAINER_PADDING * 3.0 * cx.scale_factor()),
            );
        }
    }
}
