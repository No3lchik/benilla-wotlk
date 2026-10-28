//! The test ground's markers, for checking movement by eye and in frame captures: a post every
//! [`STEP`] yards around the arena centre and the hero's facing as an arrow at his feet. Plain unlit
//! meshes, as `raid_marks.rs` draws its marks.
//!
//! Colours: the centre post is white; the north–south line (WoW +X, Bevy −Z) blue, the east–west
//! line red, the rest grey. The posts at the north end of the blue line stand twice as tall, so
//! the compass reads at any camera yaw.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::hero::Hero;
use super::RunEntity;

/// Yards between neighbouring posts.
const STEP: f32 = 10.0;
/// Posts on each side of the centre along each axis.
const REACH: i32 = 4;
const POST_HEIGHT: f32 = 3.0;

/// The arrow under the hero; a child of his body, so it turns with him.
#[derive(Component)]
pub(super) struct FacingArrow;

fn unlit(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        unlit: true,
        cull_mode: None,
        ..default()
    })
}

/// Plant the grid once the hero stands and the ground under every post has streamed in.
pub(super) fn spawn_markers(
    mut commands: Commands,
    mut done: Local<bool>,
    world: WorldPoint,
    hero: Query<Entity, With<Hero>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if *done {
        return;
    }
    let Ok(hero) = hero.single() else {
        return;
    };
    let center = super::hero::arena_center();
    let mut posts = Vec::new();
    for i in -REACH..=REACH {
        for j in -REACH..=REACH {
            // i east (+X), j north (−Z).
            let mut at = center + Vec3::new(i as f32 * STEP, 0.0, -(j as f32) * STEP);
            let Some(ground) = world.terrain_height_under(at) else {
                return; // not streamed yet: try again next frame
            };
            at.y = ground;
            posts.push((i, j, at));
        }
    }
    *done = true;

    let white = unlit(&mut materials, Color::WHITE);
    let blue = unlit(&mut materials, Color::srgb(0.15, 0.35, 1.0));
    let red = unlit(&mut materials, Color::srgb(1.0, 0.15, 0.1));
    let grey = unlit(&mut materials, Color::srgb(0.6, 0.6, 0.6));
    let post = meshes.add(Cuboid::new(0.3, POST_HEIGHT, 0.3));
    let tall = meshes.add(Cuboid::new(0.4, POST_HEIGHT * 2.0, 0.4));
    for (i, j, at) in posts {
        let (mesh, height, material) = match (i, j) {
            (0, 0) => (&post, POST_HEIGHT, &white),
            (0, j) if j > 0 => (&tall, POST_HEIGHT * 2.0, &blue),
            (0, _) => (&post, POST_HEIGHT, &blue),
            (_, 0) => (&post, POST_HEIGHT, &red),
            _ => (&post, POST_HEIGHT, &grey),
        };
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at + Vec3::Y * height * 0.5),
            RunEntity,
        ));
    }

    // Facing: a yellow bar from the feet forward (a model faces −Z at yaw 0), with a tip.
    let yellow = unlit(&mut materials, Color::srgb(1.0, 0.9, 0.1));
    let arrow = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(0.15, 0.1, 2.0))),
            MeshMaterial3d(yellow.clone()),
            Transform::from_xyz(0.0, 0.15, -1.0),
            FacingArrow,
        ))
        .with_child((
            Mesh3d(meshes.add(Cuboid::new(0.6, 0.1, 0.3))),
            MeshMaterial3d(yellow),
            Transform::from_xyz(0.0, 0.0, -1.0),
        ))
        .id();
    commands.entity(hero).add_child(arrow);

    info!(
        "survivors: markers planted, {} posts every {STEP} yd",
        (2 * REACH + 1).pow(2)
    );
}
