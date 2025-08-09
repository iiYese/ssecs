use crate::{entity::Entity, world::World};

pub trait AccessTuple {
    type Out;
}

pub struct Access {}

impl From<Entity> for Access {
    fn from(_: Entity) -> Access {
        Access {}
    }
}

impl From<&'_ Entity> for Access {
    fn from(_: &Entity) -> Access {
        Access {}
    }
}

impl From<&'_ mut Entity> for Access {
    fn from(_: &mut Entity) -> Access {
        Access {}
    }
}

pub struct Query {
    entity: Entity,
    world: World,
}

impl Clone for Query {
    fn clone(&self) -> Self {
        Self { entity: self.entity, world: World { crust: self.world.crust.clone() } }
    }
}

impl Query {
    pub(crate) fn new(world: World) -> Self {
        Self {
            world,
            entity: Entity::null(), // TODO
        }
    }

    pub fn with<T>(self, _: T) -> Self
    where
        Access: From<T>,
    {
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::component::{Component, tests::*};

    #[test]
    fn query_compile() {
        let world = World::new();
        let query = world //
            .query()
            .with(Transform::id())
            .with(&Transform::id())
            .with(&mut Transform::id());
    }
}
