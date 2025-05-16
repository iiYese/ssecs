use linkme;

use crate::{self as ssecs, entity::Entity, world::World};
use ssecs_macros::*;

pub type ComponentEntry = fn(world: &World);

#[linkme::distributed_slice]
pub static COMPONENT_ENTRIES: [ComponentEntry];

pub trait Component {
    fn id() -> Entity;
    fn init(_: &World);
    fn info() -> ComponentInfo;
}

#[derive(Clone, Copy, Component)]
pub struct ComponentInfo {
    pub size: usize,
    pub id: Entity,
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[derive(Component)]
    pub struct Player;

    #[derive(Component)]
    pub struct Transform;

    #[derive(Component)]
    pub struct Health;

    #[test]
    fn component_ids() {
        let mut ids = [Player::id(), Transform::id(), Health::id()];
        ids.sort();
        assert_eq!(ids, [0, 1, 2].map(|n| unsafe { Entity::from_offset(n) }));
    }
}
