//! The keyboard and wheel for the stock player controller. In a logged-in client a key runs its
//! `Bindings.xml` body on the FrameXML VM, whose calls (`MoveForwardStart()`, `TurnLeftStop()`,
//! `CameraZoomIn(1.0)` …) land in [`BindingsState`]; this mode loads no FrameXML, so it makes the
//! same calls straight from a fixed key map, the stock `DefaultBindings.wtf` set. The mouse needs
//! nothing here: the controller reads the buttons off `camera::latch_world_mouse`, not bindings.
//!
//! Runs after `bindings::latch_and_dispatch` (in `UiInput`), which clears last frame's edges, and
//! before the controller.

use benilla_ui::script::{BindingInput, FiredInput, HeldInput};
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use crate::bindings::BindingsState;

/// What a key does: hold a movement bit while down, or fire a one-shot on the press.
#[derive(Clone, Copy)]
enum Bind {
    Held(HeldInput),
    Fire(FiredInput),
    /// `JUMP`: `JumpOrAscendStart()` fires the jump and holds the ascend half; `AscendStop()`
    /// on the release.
    Jump,
}

/// The stock 3.3.5a defaults for movement (`DefaultBindings.wtf`).
const KEY_MAP: &[(KeyCode, Bind)] = &[
    (KeyCode::KeyW, Bind::Held(HeldInput::MoveForward)),
    (KeyCode::ArrowUp, Bind::Held(HeldInput::MoveForward)),
    (KeyCode::KeyS, Bind::Held(HeldInput::MoveBackward)),
    (KeyCode::ArrowDown, Bind::Held(HeldInput::MoveBackward)),
    (KeyCode::KeyA, Bind::Held(HeldInput::TurnLeft)),
    (KeyCode::ArrowLeft, Bind::Held(HeldInput::TurnLeft)),
    (KeyCode::KeyD, Bind::Held(HeldInput::TurnRight)),
    (KeyCode::ArrowRight, Bind::Held(HeldInput::TurnRight)),
    (KeyCode::KeyQ, Bind::Held(HeldInput::StrafeLeft)),
    (KeyCode::KeyE, Bind::Held(HeldInput::StrafeRight)),
    (KeyCode::Space, Bind::Jump),
    (KeyCode::NumLock, Bind::Fire(FiredInput::ToggleAutoRun)),
    (KeyCode::NumpadDivide, Bind::Fire(FiredInput::ToggleRun)),
    (KeyCode::KeyZ, Bind::Fire(FiredInput::ToggleSheath)),
];

/// The calls a key's press or release makes.
fn calls(bind: Bind, pressed: bool) -> Vec<BindingInput> {
    match (bind, pressed) {
        (Bind::Held(h), true) => vec![BindingInput::Start(h)],
        (Bind::Held(h), false) => vec![BindingInput::Stop(h, false)],
        (Bind::Fire(f), true) => vec![BindingInput::Fire(f)],
        (Bind::Fire(_), false) => Vec::new(),
        (Bind::Jump, true) => vec![
            BindingInput::Fire(FiredInput::Jump),
            BindingInput::Start(HeldInput::Ascend),
        ],
        (Bind::Jump, false) => vec![BindingInput::Stop(HeldInput::Ascend, false)],
    }
}

/// Whole wheel notches out of this frame's travel, the fraction carried; `MOUSEWHEELUP` zooms in
/// a yard a notch (`CameraZoomIn(1.0)`), down zooms out. A reversal drops the carried fraction, as
/// `bindings::WheelNotches` does: it is a new gesture.
fn wheel_calls(carry: &mut f32, lines: f32) -> Vec<BindingInput> {
    if *carry * lines < 0.0 {
        *carry = 0.0;
    }
    *carry += lines;
    let whole = carry.trunc();
    *carry -= whole;
    let fire = if whole > 0.0 {
        FiredInput::CameraZoomIn(1.0)
    } else {
        FiredInput::CameraZoomOut(1.0)
    };
    (0..whole.abs() as u32)
        .map(|_| BindingInput::Fire(fire))
        .collect()
}

pub(super) fn feed_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    mut carry: Local<f32>,
    mut binds: ResMut<BindingsState>,
) {
    let mut out = Vec::new();
    for &(key, bind) in KEY_MAP {
        if keys.just_pressed(key) {
            out.extend(calls(bind, true));
        }
        if keys.just_released(key) {
            out.extend(calls(bind, false));
        }
    }
    let lines: f32 = wheel
        .read()
        .map(|w| crate::bindings::wheel_lines(w.unit, w.y))
        .sum();
    if lines != 0.0 {
        out.extend(wheel_calls(&mut carry, lines));
    }
    if !out.is_empty() {
        binds.apply_script_input(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_key_starts_on_press_and_stops_on_release() {
        assert_eq!(
            calls(Bind::Held(HeldInput::TurnLeft), true),
            vec![BindingInput::Start(HeldInput::TurnLeft)]
        );
        assert_eq!(
            calls(Bind::Held(HeldInput::TurnLeft), false),
            vec![BindingInput::Stop(HeldInput::TurnLeft, false)]
        );
        assert!(calls(Bind::Fire(FiredInput::ToggleAutoRun), false).is_empty());
    }

    #[test]
    fn space_jumps_and_holds_ascend_until_released() {
        assert_eq!(
            calls(Bind::Jump, true),
            vec![
                BindingInput::Fire(FiredInput::Jump),
                BindingInput::Start(HeldInput::Ascend)
            ]
        );
        assert_eq!(
            calls(Bind::Jump, false),
            vec![BindingInput::Stop(HeldInput::Ascend, false)]
        );
    }

    #[test]
    fn the_wheel_fires_one_zoom_per_whole_notch_and_carries_the_rest() {
        let mut carry = 0.0;
        assert_eq!(wheel_calls(&mut carry, 2.0).len(), 2);
        assert!(wheel_calls(&mut carry, 0.6).is_empty());
        assert_eq!(
            wheel_calls(&mut carry, 0.6),
            vec![BindingInput::Fire(FiredInput::CameraZoomIn(1.0))]
        );
        assert_eq!(
            wheel_calls(&mut carry, -1.0),
            vec![BindingInput::Fire(FiredInput::CameraZoomOut(1.0))]
        );
    }

    #[test]
    fn every_movement_command_has_a_key() {
        for h in [
            HeldInput::MoveForward,
            HeldInput::MoveBackward,
            HeldInput::TurnLeft,
            HeldInput::TurnRight,
            HeldInput::StrafeLeft,
            HeldInput::StrafeRight,
        ] {
            assert!(
                KEY_MAP
                    .iter()
                    .any(|(_, b)| matches!(b, Bind::Held(x) if *x == h)),
                "{h:?} unbound"
            );
        }
    }
}
