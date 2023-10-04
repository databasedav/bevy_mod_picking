//! A raycasting backend for [`bevy_sprite`](bevy::sprite).

#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![deny(missing_docs)]

use std::cmp::Ordering;

use bevy::{prelude::*, window::PrimaryWindow};
use bevy_picking_core::backend::prelude::*;

/// Commonly used imports for the [`bevy_picking_sprite`](crate) crate.
pub mod prelude {
    pub use crate::SpriteBackend;
}

/// Adds picking support for [`bevy_sprite`](bevy::sprite)
#[derive(Clone)]
pub struct SpriteBackend;
impl Plugin for SpriteBackend {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, sprite_picking.in_set(PickSet::Backend));
    }
}

/// Checks if any sprite entities are under each pointer
pub fn sprite_picking(
    pointers: Query<(&PointerId, &PointerLocation)>,
    cameras: Query<(Entity, &Camera, &GlobalTransform)>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    images: Res<Assets<Image>>,
    sprite_query: Query<(
        Entity,
        &Sprite,
        &Handle<Image>,
        &GlobalTransform,
        &ComputedVisibility,
        Option<&Pickable>,
    )>,
    mut output: EventWriter<PointerHits>,
) {
    let mut sorted_sprites: Vec<_> = sprite_query.iter().collect();
    sorted_sprites.sort_by(|a, b| {
        (b.3.translation().z)
            .partial_cmp(&a.3.translation().z)
            .unwrap_or(Ordering::Equal)
    });

    for (pointer, location) in pointers.iter().filter_map(|(pointer, pointer_location)| {
        pointer_location.location().map(|loc| (pointer, loc))
    }) {
        let mut blocked = false;
        let Some((cam_entity, camera, cam_transform)) = cameras.iter().find(|(_, camera, _)| {
            camera
                .target
                .normalize(Some(primary_window.single()))
                .unwrap()
                == location.target
        }) else {
            continue;
        };

        let Some(cursor_pos_world) = camera.viewport_to_world_2d(cam_transform, location.position)
        else {
            continue;
        };

        let picks: Vec<(Entity, HitData)> = sorted_sprites
            .iter()
            .copied()
            .filter_map(
                |(entity, sprite, image, sprite_transform, visibility, sprite_focus)| {
                    if blocked || !visibility.is_visible() {
                        return None;
                    }
                    let (scale, _rot, position) = sprite_transform.to_scale_rotation_translation();
                    let extents = sprite
                        .custom_size
                        .or_else(|| images.get(image).map(|f| f.size()))?
                        * scale.truncate();
                    let center = position.truncate() - (sprite.anchor.as_vec() * extents);
                    let rect = Rect::from_center_half_size(center, extents / 2.0);

                    let is_cursor_in_sprite = rect.contains(cursor_pos_world);
                    blocked = is_cursor_in_sprite
                        && sprite_focus.map(|p| p.should_block_lower) != Some(false);

                    is_cursor_in_sprite
                        .then_some((entity, HitData::new(cam_entity, position.z, None, None)))
                },
            )
            .collect();

        let order = camera.order as f32;
        output.send(PointerHits::new(*pointer, picks, order))
    }
}
