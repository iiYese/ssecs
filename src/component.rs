use std::mem::MaybeUninit;

use linkme;

use crate::{self as ssecs, entity::Entity, world::World};
use ssecs_macros::*;

pub type ComponentEntry = fn(world: &mut World);

#[linkme::distributed_slice]
pub static COMPONENT_ENTRIES: [ComponentEntry];

/// Should never be implemented manually
pub unsafe trait Component {
    fn id() -> Entity;
    fn init(_: &mut World); // TODO: Remove mut
    fn info() -> ComponentInfo;
    fn drop(bytes: &[MaybeUninit<u8>]);
}

#[derive(Clone, Copy, Component, Debug, PartialEq, Eq)]
pub struct ComponentInfo {
    pub(crate) name: &'static str,
    pub(crate) align: usize,
    pub(crate) size: usize,
    pub(crate) id: Entity,
    pub(crate) drop: fn(&[MaybeUninit<u8>]),
}

impl ComponentInfo {
    pub unsafe fn new(
        name: &'static str,
        align: usize,
        size: usize,
        id: Entity,
        drop: fn(&[MaybeUninit<u8>]),
    ) -> Self {
        Self {
            name,
            align,
            size,
            id,
            drop,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[derive(Component)]
    pub struct Player;

    #[derive(Component)]
    pub struct Transform;

    #[derive(Component)]
    pub struct Health;

    #[test]
    fn component_ids() {
        assert!(Player::id() != Transform::id());
        assert!(Transform::id() != Health::id());
        assert!(Health::id() != Player::id());
    }
}
