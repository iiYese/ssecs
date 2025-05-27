use std::sync::Arc;

use crate::{
    entity::Entity,
    world::{World, core::Core},
};

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

impl From<&'_ [Entity; 1]> for Access {
    fn from(_: &[Entity; 1]) -> Access {
        Access {}
    }
}

impl From<&'_ mut [Entity; 1]> for Access {
    fn from(_: &mut [Entity; 1]) -> Access {
        Access {}
    }
}

#[derive(Clone)]
pub struct Query {
    entity: Entity,
    core: *const Core,
}

impl Query {
    pub(crate) fn new(core: &Core) -> Self {
        core.incr_ref_count();
        Self {
            core,
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

impl Drop for Query {
    fn drop(&mut self) {
        unsafe { self.core.as_ref().unwrap().decr_ref_count() };
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::component::{Component, tests::*};

    #[test]
    fn query_compile() {
        let world = World::new();
        let query = world
            .query()
            .with(Transform::id())
            .with(&Transform::id())
            .with(&[Transform::id()])
            .with(&mut Transform::id())
            .with(&mut [Transform::id()]);
    }
}
