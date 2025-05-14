use linkme;

use crate::{self as ssecs, entity::Entity, world::World};
use ssecs_macros::*;

pub type ComponentEntry = fn(world: &World);

#[linkme::distributed_slice]
pub static COMPONENT_ENTRIES: [ComponentEntry];

pub trait Component {
    fn id() -> Entity;
    fn init(_: &World) {}
}

#[derive(Clone, Copy, Component)]
pub struct ComponentInfo {
    pub size: usize,
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
        let mut ids = vec![
            Player::id().raw(),
            Transform::id().raw(),
            Health::id().raw(),
        ];
        ids.sort();
        assert_eq!(ids, vec![0, 1, 2]);
    }
}
