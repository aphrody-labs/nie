//! Input events from any smithay input backend (winit, libinput) into the seat.

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
    KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use crate::state::State;

/// Linux input code of the left mouse button.
const BTN_LEFT: u32 = 0x110;

impl State {
    /// Route one input event to the focused client; a click gives the pointed toplevel the focus.
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => {
                let Some(keyboard) = self.seat.get_keyboard() else { return };
                let serial = SERIAL_COUNTER.next_serial();
                keyboard.input::<(), _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    Event::time_msec(&event),
                    |_, _, _| FilterResult::Forward,
                );
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let size = self.output.current_mode().map(|m| m.size).unwrap_or_default();
                let pos = Point::from((event.x_transformed(size.w), event.y_transformed(size.h)));
                self.move_pointer(pos, Event::time_msec(&event));
            }
            InputEvent::PointerMotion { event } => {
                let delta = Point::from((event.delta_x(), event.delta_y()));
                self.move_pointer(self.pointer_location + delta, Event::time_msec(&event));
            }
            InputEvent::PointerButton { event } => {
                let Some(pointer) = self.seat.get_pointer() else { return };
                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let pressed = event.state() == ButtonState::Pressed;
                if pressed && button == BTN_LEFT {
                    self.click_focus();
                }
                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: if pressed { ButtonState::Pressed } else { ButtonState::Released },
                        serial,
                        time: Event::time_msec(&event),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event } => {
                let Some(pointer) = self.seat.get_pointer() else { return };
                let amount = |axis| {
                    event
                        .amount(axis)
                        .unwrap_or_else(|| event.amount_v120(axis).unwrap_or(0.0) * 15.0 / 120.0)
                };
                let mut frame = AxisFrame::new(Event::time_msec(&event)).source(event.source());
                for axis in [Axis::Horizontal, Axis::Vertical] {
                    let value = amount(axis);
                    if value != 0.0 {
                        frame = frame.value(axis, value);
                    } else if event.source() == AxisSource::Finger {
                        frame = frame.stop(axis);
                    }
                }
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }

    /// Move the pointer inside the output and deliver the motion to the surface under it.
    pub fn move_pointer(&mut self, pos: Point<f64, Logical>, time: u32) {
        let size = self.output.current_mode().map(|m| m.size).unwrap_or_default();
        let pos = Point::from((pos.x.clamp(0.0, f64::from(size.w - 1)), pos.y.clamp(0.0, f64::from(size.h - 1))));
        self.pointer_location = pos;
        let Some(pointer) = self.seat.get_pointer() else { return };
        let under = self.surface_under(pos);
        pointer.motion(self, under, &MotionEvent { location: pos, serial: SERIAL_COUNTER.next_serial(), time });
        pointer.frame(self);
    }

    fn click_focus(&mut self) {
        let Some((surface, _)) = self.surface_under(self.pointer_location) else { return };
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &surface))
            .cloned();
        if let Some(window) = window {
            self.space.raise_element(&window, true);
        }
        self.focus(Some(surface));
    }
}
