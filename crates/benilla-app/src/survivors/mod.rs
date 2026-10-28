//! Warcraft Survivors: a survival mode on the benilla world, started by
//! [`crate::run_survivors`]. There is no server: the hero is a local entity dressed by the same
//! attach and animation systems a streamed unit gets, and driven by the engine's own player
//! controller and camera as a logged-in character is ([`hero`], [`keys`]). FrameXML stays off
//! (`ui_script::lifecycle::ui_wanted`).
//!
//! Milestone 0: one hero on one arena with WoW controls. The world boot and units are ported from
//! azeroth-survivors-re `crates/benilla-app/src/survivors/` (`72048f4`: `mod.rs`, `units.rs`),
//! re-checked against this engine; the reference's top-down controls were replaced by the stock
//! controller (`docs/decisions/wow-controls.md` in the game repo).

mod hero;
mod keys;
mod markers;
mod units;

use std::sync::atomic::{AtomicBool, Ordering};

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;

/// Set by [`crate::run_survivors`] before the app builds.
pub(crate) static ACTIVE: AtomicBool = AtomicBool::new(false);

pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// The arena's `Map.dbc` row: Kalimdor, for The Barrens (the reference's first map).
pub(crate) const START_MAP: u32 = 1;

/// One arena: where the hero starts (WoW coordinates). The reference's 150 yd leash is not
/// enforced yet: the stock controller owns the hero's position.
pub(crate) struct Arena {
    pub center: [f32; 3],
}

/// The plains south of the Crossroads, as the reference's `data::MAPS[0]`.
pub(crate) const ARENA: Arena = Arena {
    center: [-950.0, -2700.0, 92.0],
};

/// Everything a run spawns, so a restart can clear it.
#[derive(Component)]
pub(crate) struct RunEntity;

pub(crate) struct SurvivorsPlugin;

impl Plugin for SurvivorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (hero::spawn_hero, keys::feed_keys, markers::spawn_markers)
                .chain()
                .in_set(WorldStage::Input)
                .after(crate::ui_script::UiInput)
                .before(crate::player::PlayerControlSet),
        );
    }
}
