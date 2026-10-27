//! Warcraft Survivors: a top-down survival mode on the benilla world, started by
//! [`crate::run_survivors`]. There is no server: the hero is a local entity dressed by the same
//! attach and animation systems a streamed unit gets, with the player controller and FrameXML off
//! ([`crate::run_mode::CaptureMode`]).
//!
//! Milestone 0: one hero on one arena, WASD over the terrain and a top-down camera. Ported from
//! azeroth-survivors-re `crates/benilla-app/src/survivors/` (`72048f4`: `mod.rs`, `hero.rs`,
//! `units.rs`), re-checked against this engine.

mod hero;
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

/// One arena: where the hero starts (WoW coordinates) and how far from it he may walk.
pub(crate) struct Arena {
    pub center: [f32; 3],
    pub leash: f32,
}

/// The plains south of the Crossroads, as the reference's `data::MAPS[0]`.
pub(crate) const ARENA: Arena = Arena {
    center: [-950.0, -2700.0, 92.0],
    leash: 150.0,
};

/// Everything a run spawns, so a restart can clear it.
#[derive(Component)]
pub(crate) struct RunEntity;

pub(crate) struct SurvivorsPlugin;

impl Plugin for SurvivorsPlugin {
    fn build(&self, app: &mut App) {
        // Present before `OnEnter(InWorld)`, where the UI lifecycle decides not to load FrameXML,
        // and it gates the player controller and its camera off.
        app.insert_resource(crate::run_mode::CaptureMode)
            .init_resource::<hero::CamRig>()
            .add_systems(
                Update,
                (hero::spawn_hero, hero::drive_hero, hero::place_camera)
                    .chain()
                    .in_set(WorldStage::Input),
            );
    }
}
