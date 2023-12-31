use crate::MidiLatticeParams;
use crate::Voices;

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use std::sync::{Arc, Mutex};
use triple_buffer::Output;

use crate::editor::{CORNER_RADIUS, PADDING};

use self::drag_region::DragRegion;
use self::grid::Grid;
use self::grid::NODE_SIZE;
use self::grid_resizer::GridResizer;

use super::color::COLOR_0;
use super::color::COLOR_3;
use super::intersects_box;
use crate::editor::color::COLOR_1;
mod drag_region;
pub mod grid;
pub mod grid_resizer;

pub struct Lattice {
    mouse_over: bool,
}

impl Lattice {
    pub fn new<LParams, LVoices>(
        cx: &mut Context,
        params: LParams,
        voices_output: LVoices,
    ) -> Handle<Self>
    where
        LParams: Lens<Target = Arc<MidiLatticeParams>> + Copy,
        LVoices: Lens<Target = Arc<Mutex<Output<Voices>>>>,
    {
        Self { mouse_over: false }.build(
            cx,
            // This is an otherwise empty element only used for custom drawing
            |cx| {
                Grid::new(cx, params, voices_output)
                    .position_type(PositionType::SelfDirected)
                    .bottom(Units::Pixels(0.0))
                    .left(Units::Pixels(0.0))
                    .top(Units::Pixels(0.0))
                    .right(Units::Pixels(0.0));

                DragRegion::new(cx, params.map(|p| p.grid_params.clone()))
                    .position_type(PositionType::ParentDirected)
                    .width(Units::Stretch(1.0))
                    .height(Units::Stretch(1.0));

                GridResizer::new(cx, params.map(|p| p.grid_params.clone()))
                    .position_type(PositionType::SelfDirected)
                    .bottom(Units::Pixels(PADDING))
                    .right(Units::Pixels(PADDING))
                    .left(Units::Stretch(1.0))
                    .top(Units::Stretch(1.0))
                    .width(Units::Pixels(NODE_SIZE * 1.3))
                    .height(Units::Pixels(NODE_SIZE * 1.3))
                    .visibility(Visibility::Hidden);
            },
        )
    }
}

/// Represents a mouse event over the lattice.
/// Necessary because children of the lattice have shared state:
/// - Children are visible when the mouse is over any part of the lattice
/// - If one child is being dragged, others don't display their mouse over visual state
enum LatticeEvent {
    MouseOver,
    MouseOut,
    MouseDown,
    MouseUpFromChild,
    MouseUpToChild,
}

impl View for Lattice {
    fn element(&self) -> Option<&'static str> {
        Some("lattice")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // Notify children when the mouse moves over or leaves the lattice
        event.map(|window_event, _meta| match *window_event {
            WindowEvent::MouseMove(x, y) => {
                // If the mouse entered or left the bounding box, notify the subtree
                if intersects_box(cx.bounds(), (x, y)) {
                    if !self.mouse_over {
                        cx.emit_custom(
                            Event::new(LatticeEvent::MouseOver).propagate(Propagation::Subtree),
                        );
                        self.mouse_over = true;
                    }
                } else {
                    if self.mouse_over {
                        cx.emit_custom(
                            Event::new(LatticeEvent::MouseOut).propagate(Propagation::Subtree),
                        );
                        self.mouse_over = false;
                    }
                }
            }
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.emit_custom(Event::new(LatticeEvent::MouseDown).propagate(Propagation::Subtree));
            }
            _ => {}
        });
        event.map(|lattice_event, _meta| match *lattice_event {
            // When a child tells us the mouse button was released, we notify all children.
            // We don't need to do this for MouseDown because the lattice directly receives
            // those events when a child receives one.
            LatticeEvent::MouseUpFromChild => {
                if self.mouse_over {
                    self.mouse_over = false;
                    if !intersects_box(cx.bounds(), (cx.mouse().cursorx, cx.mouse().cursory)) {
                        cx.emit_custom(
                            Event::new(LatticeEvent::MouseOut).propagate(Propagation::Subtree),
                        );
                    }
                }
                cx.emit_custom(
                    Event::new(LatticeEvent::MouseUpToChild).propagate(Propagation::Subtree),
                );
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {}
}
