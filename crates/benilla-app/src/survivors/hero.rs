//! The hero: its body, WASD movement over the terrain and the top-down camera.

use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::units::{self, flat, wow_to_bevy, yaw_toward, UnitSpec};
use super::ARENA;
use crate::creature_anim::{move_flags, MovementState};

#[derive(Component)]
pub(super) struct Hero;

/// A Stormwind warrior trainer's look (`CreatureDisplayInfo` 1300, the first of the reference's
/// Warrior `looks`): a dressed human body with no item fields to fill.
const HERO_DISPLAY: u32 = 1300;
/// Human (race 1), Warrior (class 1), male, rage.
const HERO_BYTES_0: u32 = 1 | 1 << 8 | 1 << 24;
/// The player run speed, yards per second.
const RUN_SPEED: f32 = 7.0;

/// The locked view's tilt.
const FIXED_PITCH: f32 = 61.0 * std::f32::consts::PI / 180.0;
/// Wheel zoom range, yards from the hero's shoulders.
const ZOOM_MIN: f32 = 1.5;
const ZOOM_MAX: f32 = 70.0;

#[derive(Resource)]
pub(super) struct CamRig {
    pub dist: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        Self { dist: 30.0 }
    }
}

/// Where the run starts, in Bevy coordinates.
fn arena_center() -> Vec3 {
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
    commands.entity(e).insert(Hero);
    info!("survivors: hero spawned at {pos:?}");
}

/// WASD moves the hero world-relative (W is screen-up), facing where he runs, held to the arena
/// and seated on the terrain. Without a hero yet, the arena centre is the focus the terrain streams
/// around.
pub(super) fn drive_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    world: WorldPoint,
    player: Option<ResMut<crate::player::Player>>,
    mut hero: Query<(&mut Transform, &mut MovementState), With<Hero>>,
) {
    let focus = match hero.single_mut() {
        Ok((mut tf, mut mv)) => {
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
            let mut pos = tf.translation;
            if dir != Vec3::ZERO && dt > 0.0 {
                let dir = dir.normalize();
                pos += dir * RUN_SPEED * dt;
                let target = Quat::from_rotation_y(yaw_toward(dir));
                tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
                mv.speed = RUN_SPEED;
                mv.flags = move_flags::FORWARD;
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
            pos.y = world.terrain_height_under(pos).unwrap_or(pos.y);
            if tf.translation != pos {
                tf.translation = pos;
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

/// A locked top-down camera over the hero; the wheel zooms.
pub(super) fn place_camera(
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    world: WorldPoint,
    player: Option<Res<crate::player::Player>>,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    let focus = hero
        .single()
        .map(|t| t.translation)
        .unwrap_or_else(|_| player.map_or_else(arena_center, |p| p.pos));
    // Multiplicative, so a notch moves as far by eye close in as far out.
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.12 * notches).clamp(0.5, 1.5)).clamp(ZOOM_MIN, ZOOM_MAX);
    }
    let (sp, cp) = FIXED_PITCH.sin_cos();
    let look = focus + Vec3::Y * 1.4;
    let mut eye = look + Vec3::new(0.0, sp, cp) * rig.dist;
    // Never under the ground: a slope behind the hero would bury the lens.
    if let Some(h) = world.terrain_height_under(eye) {
        eye.y = eye.y.max(h + 0.6);
    }
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}
