use crate::{
    entity::Entity,
    world::{World, mantle::Mantle},
};

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

#[derive(Clone)]
pub struct Query {
    entity: Entity,
    mantle: Mantle,
}

impl Query {
    pub(crate) fn new(mantle: Mantle) -> Self {
        Self {
            mantle,
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
