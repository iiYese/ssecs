use crate::{entity::Entity, world::World};

use linkme;

type ComponentEntry = fn(world: &World);

#[linkme::distributed_slice]
static COMPONENT_INIT_FNS: [ComponentEntry];

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
            println!("player");
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
            println!("transform");
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
            println!("health");
            // world.component_with_id::<Health>(Health::id())
        }
    }

    #[test]
    fn component_test() {
        dbg!(Player::id());
        dbg!(Transform::id());
        dbg!(Health::id());

        for func in COMPONENT_INIT_FNS {
            func(&World {});
        }
    }
}
