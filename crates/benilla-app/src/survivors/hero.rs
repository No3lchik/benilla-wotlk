//! The hero: a local unit the engine's own player controller drives, as it drives a logged-in
//! character. Tagged [`SelfPlayer`] with its guid in [`SelfGuid`], it is what `player::embody`
//! makes the [`crate::net::Embodied`] body and active mover; `player::controller::control` then
//! takes control from its transform, as at a login, holds it until the ground is resident, and
//! runs the 3.3.5a movement, camera and world collision on it. The keys reach the controller
//! through [`super::keys`].

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::units::{self, wow_to_bevy, UnitSpec};
use super::ARENA;
use crate::net::{SelfGuid, SelfPlayer};
use crate::player::Player;

#[derive(Component)]
pub(super) struct Hero;

/// A Stormwind warrior trainer's look (`CreatureDisplayInfo` 1300, the first of the reference's
/// Warrior `looks`): a dressed human body with no item fields to fill.
const HERO_DISPLAY: u32 = 1300;
/// Human (race 1), Warrior (class 1), male, rage.
const HERO_BYTES_0: u32 = 1 | 1 << 8 | 1 << 24;
const HERO_SERIAL: u64 = 1;

/// Where the run starts, in Bevy coordinates.
pub(super) fn arena_center() -> Vec3 {
    wow_to_bevy(ARENA.center)
}

/// Spawn the hero on the ground at the arena centre, as a server places a character on its floor.
/// Until that ground has streamed in, the avatar is a bodiless point held at the centre (`active`,
/// `settling`: the mover stands still, the stream and the camera aim there). At the spawn the
/// avatar goes back to unplaced, so the controller's take-control runs its login path: the settle
/// hold keeps the hero until the tile, colliders included, is resident.
pub(super) fn spawn_hero(
    mut commands: Commands,
    mut self_guid: ResMut<SelfGuid>,
    mut player: ResMut<Player>,
    world: WorldPoint,
    heroes: Query<(), With<Hero>>,
) {
    if !heroes.is_empty() {
        return;
    }
    let mut pos = arena_center();
    let Some(ground) = world.terrain_height_under(pos) else {
        player.active = true;
        player.detached = false;
        player.settling = true;
        player.pos = pos;
        return;
    };
    pos.y = ground;
    player.active = false;
    player.detached = false;
    let e = units::spawn_unit(
        &mut commands,
        &UnitSpec {
            serial: HERO_SERIAL,
            display: HERO_DISPLAY,
            scale: 1.0,
            level: 80,
            faction: units::FACTION_FRIENDLY,
            bytes0: HERO_BYTES_0,
        },
        pos,
        0.0,
    );
    commands.entity(e).insert((Hero, SelfPlayer));
    self_guid.0 = Some(units::guid_of(HERO_SERIAL));
    info!("survivors: hero spawned at {pos:?}");
}
