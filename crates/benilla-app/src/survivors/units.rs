//! Local units, built the way the net layer builds streamed ones: a [`Guid`], a [`NetEntity`] and
//! an [`ObjectStore`] are all the attach and animation systems need.
//!
//! The field indices are this engine's descriptor layout (the 1.12.1-shaped one in
//! `benilla-protocol` `messages/update_object/fields/mod.rs`), not 3.3.5a's wire layout; the test
//! below reads them back through the protocol's own accessors.

use benilla_protocol::ObjectFields;
use bevy::prelude::*;

use super::RunEntity;
use crate::net::{Guid, NetEntity, ObjectStore};

const FIELD_GUID_LOW: u16 = 0;
const FIELD_GUID_HIGH: u16 = 1;
const FIELD_TYPE: u16 = 2;
const FIELD_SCALE_X: u16 = 4;
const FIELD_HEALTH: u16 = 22;
const FIELD_MAX_HEALTH: u16 = 28;
const FIELD_LEVEL: u16 = 34;
const FIELD_FACTION: u16 = 35;
const FIELD_BYTES_0: u16 = 36;

/// `OBJECT_FIELD_TYPE` for a unit: `TYPEMASK_OBJECT | TYPEMASK_UNIT`.
const TYPE_UNIT: u32 = 0x9;

/// `FactionTemplate.dbc`: friendly to players.
pub(super) const FACTION_FRIENDLY: u32 = 35;

const HIGH_CREATURE: u64 = 0xF130 << 48;

pub(super) fn wow_to_bevy(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0])
}

/// The Bevy yaw that faces a model along `dir` (its XZ part).
pub(super) fn yaw_toward(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

pub(super) fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

pub(super) struct UnitSpec {
    pub serial: u64,
    /// `CreatureDisplayInfo.dbc` row.
    pub display: u32,
    pub scale: f32,
    pub level: u32,
    pub faction: u32,
    /// `UNIT_FIELD_BYTES_0`: race, class, gender, power type.
    pub bytes0: u32,
}

fn unit_fields(spec: &UnitSpec) -> ObjectFields {
    let guid = HIGH_CREATURE | spec.serial;
    ObjectFields::from_pairs(&[
        (FIELD_GUID_LOW, guid as u32),
        (FIELD_GUID_HIGH, (guid >> 32) as u32),
        (FIELD_TYPE, TYPE_UNIT),
        (FIELD_SCALE_X, spec.scale.to_bits()),
        (FIELD_HEALTH, 100),
        (FIELD_MAX_HEALTH, 100),
        (FIELD_LEVEL, spec.level),
        (FIELD_FACTION, spec.faction),
        (FIELD_BYTES_0, spec.bytes0),
    ])
}

/// Spawn a unit the attach builds a body for; the caller adds its own components.
pub(super) fn spawn_unit(commands: &mut Commands, spec: &UnitSpec, pos: Vec3, yaw: f32) -> Entity {
    commands
        .spawn((
            Guid(HIGH_CREATURE | spec.serial),
            NetEntity {
                kind: benilla_protocol::EntityKind::Unit,
                display_id: Some(spec.display),
                scale: spec.scale,
            },
            ObjectStore(unit_fields(spec)),
            crate::creature_anim::MovementState::default(),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            RunEntity,
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_fields_read_back_through_the_protocol() {
        let spec = UnitSpec {
            serial: 7,
            display: 1300,
            scale: 1.0,
            level: 60,
            faction: FACTION_FRIENDLY,
            bytes0: 1 | 1 << 8,
        };
        let store = ObjectStore(unit_fields(&spec));
        assert!(store.is_unit());
        let f = &store.0;
        assert_eq!(f.object_scale_x(), Some(1.0));
        assert_eq!(f.unit_health(), Some(100));
        assert_eq!(f.unit_max_health(), Some(100));
        assert_eq!(f.unit_level(), Some(60));
        assert_eq!(f.unit_faction_template(), Some(FACTION_FRIENDLY));
        assert_eq!(f.unit_race(), Some(1));
        assert_eq!(f.unit_class(), Some(1));
    }

    #[test]
    fn wow_and_bevy_axes_agree() {
        // WoW +X (north) is Bevy −Z, WoW +Y (west) is Bevy −X, WoW +Z is up.
        assert_eq!(wow_to_bevy([1.0, 0.0, 0.0]), Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(wow_to_bevy([0.0, 1.0, 0.0]), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(wow_to_bevy([0.0, 0.0, 1.0]), Vec3::Y);
        // A model faces −Z at yaw 0.
        assert!(yaw_toward(Vec3::NEG_Z).abs() < 1e-6);
    }
}
