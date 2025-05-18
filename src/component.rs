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
}

#[derive(Clone, Copy, Component, Debug)]
pub struct ComponentInfo {
    pub name: &'static str,
    pub align: usize,
    pub size: usize,
    pub id: Entity,
}

#[cfg(test)]
pub(crate) mod test {
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
