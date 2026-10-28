//! The hero: its body, WASD movement over the terrain and the top-down camera. Controls as the
//! reference's `hero.rs`: locked, the hero faces the cursor and WASD moves him world-relative, so
//! he strafes and backpedals; `C` unlocks the camera to swing and tilt with a mouse drag, WASD then
//! camera-relative and the hero facing where he runs; Space hops; the wheel zooms.

use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::units::{self, flat, wow_to_bevy, yaw_toward, UnitSpec};
use super::ARENA;
use crate::creature_anim::{move_flags, MovementState};

#[derive(Component)]
pub(super) struct Hero {
    /// Vertical velocity in yards/second while airborne; 0 on the ground.
    vy: f32,
    /// Height above the ground while airborne. Kept here, not in the transform: the creature
    /// ground clamp re-seats every unit on the floor at the top of the frame, so the arc is
    /// re-applied over the ground each frame.
    air: f32,
}

/// A Stormwind warrior trainer's look (`CreatureDisplayInfo` 1300, the first of the reference's
/// Warrior `looks`): a dressed human body with no item fields to fill.
const HERO_DISPLAY: u32 = 1300;
/// Human (race 1), Warrior (class 1), male, rage.
const HERO_BYTES_0: u32 = 1 | 1 << 8 | 1 << 24;
/// The player run speed, yards per second.
const RUN_SPEED: f32 = 7.0;

/// The hop's initial upward speed and the gravity that brings it back down (the reference's
/// `CMovement` jump seed and gravity).
const JUMP_SPEED: f32 = 7.955_547;
const GRAVITY: f32 = 19.291_105;

/// Radians of orbit per raw mouse unit while dragging.
const ORBIT_RATE: f32 = 0.008;
/// Elevation clamp (radians): ~10°..~83° above the horizon, so the camera never dives underground.
const PITCH_MIN: f32 = 0.18;
const PITCH_MAX: f32 = 1.45;
/// The locked view's tilt.
const FIXED_PITCH: f32 = 61.0 * std::f32::consts::PI / 180.0;
/// Wheel zoom range, yards from the hero's shoulders.
const ZOOM_MIN: f32 = 1.5;
const ZOOM_MAX: f32 = 70.0;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(super) enum CamMode {
    /// Locked top-down: the reference's original feel.
    #[default]
    Fixed,
    /// Unlocked: a mouse drag swings and tilts it around the hero.
    Orbit,
}

#[derive(Resource)]
pub(super) struct CamRig {
    dist: f32,
    /// Orbit azimuth (radians) around the hero.
    yaw: f32,
    /// Elevation above the horizon (radians).
    pitch: f32,
    mode: CamMode,
}

impl Default for CamRig {
    fn default() -> Self {
        Self {
            dist: 30.0,
            yaw: 0.0,
            pitch: FIXED_PITCH,
            mode: CamMode::Fixed,
        }
    }
}

/// Where the cursor meets the terrain, if it does.
#[derive(Resource, Default)]
pub(super) struct AimPoint(Option<Vec3>);

impl AimPoint {
    pub(super) fn point(&self) -> Option<Vec3> {
        self.0
    }
}

/// Where the run starts, in Bevy coordinates.
pub(super) fn arena_center() -> Vec3 {
    wow_to_bevy(ARENA.center)
}

/// Spawn the hero once the ground under the arena has streamed in.
pub(super) fn spawn_hero(mut commands: Commands, world: WorldPoint, heroes: Query<(), With<Hero>>) {
    if !heroes.is_empty() {
        return;
    }
    let mut pos = arena_center();
    let Some(ground) = world.terrain_height_under(pos) else {
        return;
    };
    pos.y = ground;
    let e = units::spawn_unit(
        &mut commands,
        &UnitSpec {
            serial: 1,
            display: HERO_DISPLAY,
            scale: 1.0,
            level: 80,
            faction: units::FACTION_FRIENDLY,
            bytes0: HERO_BYTES_0,
        },
        pos,
        0.0,
    );
    commands.entity(e).insert(Hero { vy: 0.0, air: 0.0 });
    info!("survivors: hero spawned at {pos:?}");
}

/// Let go of the mouse buttons when the window loses focus or the pointer leaves it: a release
/// that lands elsewhere never reaches the game, and the unlocked camera would swing with every
/// move until both buttons were clicked again.
pub(super) fn drop_stray_buttons(
    mut focus: MessageReader<bevy::window::WindowFocused>,
    mut left: MessageReader<bevy::window::CursorLeft>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    let lost = focus.read().any(|f| !f.focused);
    if lost | (left.read().count() > 0) {
        mouse.release_all();
    }
}

/// Cast the cursor onto the terrain: the camera ray meets the plane at the hero's height, then the
/// terrain height there, a few times over, which settles on hills within a frame.
pub(super) fn aim_cursor(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    world: WorldPoint,
    mut aim: ResMut<AimPoint>,
) {
    let point = (|| {
        let cursor = windows.single().ok()?.cursor_position()?;
        let (cam, tf) = cams.single().ok()?;
        let ray = cam
            .viewport_to_world(&GlobalTransform::from(*tf), cursor)
            .ok()?;
        let mut y = hero.single().ok()?.translation.y;
        let mut at = None;
        for _ in 0..4 {
            if ray.direction.y.abs() < 1e-4 {
                break;
            }
            let t = (y - ray.origin.y) / ray.direction.y;
            if t <= 0.0 {
                break;
            }
            let p = ray.origin + *ray.direction * t;
            at = Some(p);
            match world.terrain_height_under(p) {
                Some(h) if (h - y).abs() > 0.05 => y = h,
                _ => break,
            }
        }
        at
    })();
    if aim.0 != point {
        aim.0 = point;
    }
}

/// WASD over the terrain within the arena's leash, and the streaming focus on the hero.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    rig: Res<CamRig>,
    aim: Res<AimPoint>,
    world: WorldPoint,
    player: Option<ResMut<crate::player::Player>>,
    mut hero: Query<(&mut Transform, &mut MovementState, &mut Hero)>,
) {
    let focus = match hero.single_mut() {
        Ok((mut tf, mut mv, mut hm)) => {
            let was_flags = mv.flags;
            let dt = time.delta_secs();
            let mut dir = Vec3::ZERO;
            if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
                dir.z -= 1.0;
            }
            if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
                dir.z += 1.0;
            }
            if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
                dir.x -= 1.0;
            }
            if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
                dir.x += 1.0;
            }
            // In orbit mode WASD is camera-relative; in fixed mode it stays world-relative.
            if rig.mode == CamMode::Orbit {
                let (sy, cy) = rig.yaw.sin_cos();
                dir = Vec3::new(dir.x * cy + dir.z * sy, 0.0, dir.z * cy - dir.x * sy);
            }
            let mut pos = tf.translation;
            // Locked, the hero faces the cursor and WASD moves him under it, so he backpedals and
            // strafes as a player does; unlocked, he faces where he runs.
            let face = (rig.mode == CamMode::Fixed)
                .then(|| aim.0.map(|a| flat(a - pos)))
                .flatten()
                .filter(|d| d.length() > 0.5)
                .map(|d| d.normalize());
            if let Some(look) = face {
                let target = Quat::from_rotation_y(yaw_toward(look));
                tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
            }
            if dir != Vec3::ZERO && dt > 0.0 {
                let dir = dir.normalize();
                pos += dir * RUN_SPEED * dt;
                mv.speed = RUN_SPEED;
                mv.flags = match face {
                    Some(look) => gait(look, dir),
                    None => {
                        let target = Quat::from_rotation_y(yaw_toward(dir));
                        tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
                        move_flags::FORWARD
                    }
                };
            } else if mv.speed != 0.0 || mv.flags != 0 {
                mv.speed = 0.0;
                mv.flags = 0;
            }
            let center = arena_center();
            let off = flat(pos - center);
            if off.length() > ARENA.leash {
                let held = center + off.normalize() * ARENA.leash;
                pos.x = held.x;
                pos.z = held.z;
            }
            let ground = world.terrain_height_under(pos).unwrap_or(pos.y);
            // Space hops: lift off the terrain, then gravity brings the hero back down.
            if keys.just_pressed(KeyCode::Space) && hm.vy == 0.0 && hm.air == 0.0 {
                hm.vy = JUMP_SPEED;
            }
            if hm.vy != 0.0 || hm.air > 0.0 {
                hm.vy -= GRAVITY * dt;
                hm.air += hm.vy * dt;
                if hm.air <= 0.0 {
                    hm.air = 0.0;
                    hm.vy = 0.0;
                }
            }
            pos.y = ground + hm.air;
            // `vertical_speed`'s sign and the `FALLING` flag select the Jump / Fall animations.
            mv.vertical_speed = hm.vy;
            if hm.vy != 0.0 {
                mv.flags |= move_flags::FALLING;
            }
            if tf.translation != pos {
                tf.translation = pos;
            }
            if mv.flags != was_flags {
                let (yaw, _, _) = tf.rotation.to_euler(EulerRot::YXZ);
                debug!(
                    "survivors: gait flags {:#x}, facing {:.0}°",
                    mv.flags,
                    yaw.to_degrees()
                );
            }
            pos
        }
        Err(_) => arena_center(),
    };
    // The terrain streams and the loading screen waits on the ground under this.
    if let Some(mut player) = player {
        player.active = true;
        player.detached = false;
        player.settling = false;
        player.pos = focus;
    }
}

/// Top-down until `C` unlocks it to swing and tilt; the wheel zooms in both.
#[allow(clippy::too_many_arguments)]
pub(super) fn place_camera(
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    world: WorldPoint,
    player: Option<Res<crate::player::Player>>,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    // Locking again snaps back to the top-down view, so world-relative WASD lines up again.
    if keys.just_pressed(KeyCode::KeyC) {
        rig.mode = match rig.mode {
            CamMode::Fixed => CamMode::Orbit,
            CamMode::Orbit => {
                rig.yaw = 0.0;
                rig.pitch = FIXED_PITCH;
                CamMode::Fixed
            }
        };
        info!("survivors: camera {:?}", rig.mode);
    }
    let focus = hero
        .single()
        .map(|t| t.translation)
        .unwrap_or_else(|_| player.map_or_else(arena_center, |p| p.pos));
    // Multiplicative, so a notch moves as far by eye close in as far out.
    let before = rig.dist;
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.12 * notches).clamp(0.5, 1.5)).clamp(ZOOM_MIN, ZOOM_MAX);
    }
    if rig.dist != before {
        debug!("survivors: zoom {before:.1} -> {:.1} yd", rig.dist);
    }
    if rig.mode == CamMode::Orbit && mouse.any_pressed([MouseButton::Left, MouseButton::Right]) {
        rig.yaw -= motion.delta.x * ORBIT_RATE;
        rig.pitch = (rig.pitch + motion.delta.y * ORBIT_RATE).clamp(PITCH_MIN, PITCH_MAX);
    }
    let (sy, cy) = rig.yaw.sin_cos();
    let (sp, cp) = rig.pitch.sin_cos();
    let look = focus + Vec3::Y * 1.4;
    let mut eye = look + Vec3::new(cp * sy, sp, cp * cy) * rig.dist;
    // Never under the ground: a low tilt on a slope would bury the lens.
    if let Some(h) = world.terrain_height_under(eye) {
        eye.y = eye.y.max(h + 0.6);
    }
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}

/// The movement flags for running `dir` while facing `look`: forward, backpedal or strafe, by
/// which quarter the run falls in, so the run, backpedal and strafe animations play.
fn gait(look: Vec3, dir: Vec3) -> u32 {
    let ahead = look.dot(dir);
    // Positive when `dir` is to the hero's left: WoW faces −Z with +X on the right.
    let left = look.cross(dir).y;
    let quarter = std::f32::consts::FRAC_1_SQRT_2;
    if ahead >= quarter {
        move_flags::FORWARD
    } else if ahead <= -quarter {
        move_flags::BACKWARD
    } else if left > 0.0 {
        move_flags::STRAFE_LEFT
    } else {
        move_flags::STRAFE_RIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gait_picks_the_quarter_the_run_falls_in() {
        let north = Vec3::NEG_Z;
        assert_eq!(gait(north, Vec3::NEG_Z), move_flags::FORWARD);
        assert_eq!(gait(north, Vec3::Z), move_flags::BACKWARD);
        assert_eq!(gait(north, Vec3::NEG_X), move_flags::STRAFE_LEFT);
        assert_eq!(gait(north, Vec3::X), move_flags::STRAFE_RIGHT);
        // Diagonals ahead still run forward.
        assert_eq!(
            gait(north, Vec3::new(0.5, 0.0, -1.0).normalize()),
            move_flags::FORWARD
        );
    }
}
