use crate::{entity::Entity, world::World};

use linkme;

pub(crate) type ComponentEntry = fn(world: &World);

#[linkme::distributed_slice]
pub(crate) static COMPONENT_INIT_FNS: [ComponentEntry];

pub trait Component {
    fn id() -> Entity;
    fn init(_: &World) {}
}

#[cfg(test)]
pub mod test {
    use super::*;

    pub struct Player;
    pub struct Transform;
    pub struct Health;

    use std::mem::size_of;

    impl Component for Player {
        fn id() -> Entity {
            #[linkme::distributed_slice(COMPONENT_INIT_FNS)]
            static ENTRY: ComponentEntry = Player::init;
            Entity(
                ((&raw const ENTRY as u64) - (COMPONENT_INIT_FNS[..].as_ptr() as u64))
                    / size_of::<ComponentEntry>() as u64,
            )
        }

        fn init(_: &World) {
            // world.component_with_id::<Player>(Player::id())
        }
    }

    impl Component for Transform {
        fn id() -> Entity {
            #[linkme::distributed_slice(COMPONENT_INIT_FNS)]
            static ENTRY: ComponentEntry = Transform::init;
            Entity(
                ((&raw const ENTRY as u64) - (COMPONENT_INIT_FNS[..].as_ptr() as u64))
                    / size_of::<ComponentEntry>() as u64,
            )
        }

        fn init(_: &World) {
            // world.component_with_id::<Transform>(Transform::id())
        }
    }

    impl Component for Health {
        fn id() -> Entity {
            #[linkme::distributed_slice(COMPONENT_INIT_FNS)]
            static ENTRY: ComponentEntry = Health::init;
            Entity(
                ((&raw const ENTRY as u64) - (COMPONENT_INIT_FNS[..].as_ptr() as u64))
                    / size_of::<ComponentEntry>() as u64,
            )
        }

        fn init(_: &World) {
            // world.component_with_id::<Health>(Health::id())
        }
    }

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
