use std::{mem::MaybeUninit, sync::Arc};

use crate::{
    NonZstOrPanic,
    archetype::{ColumnReadGuard, FieldId},
    component::{COMPONENT_ENTRIES, Component, ComponentInfo},
    entity::{Entity, View},
    query::Query,
};

pub(crate) mod command;
pub(crate) mod core;
pub(crate) mod mantle;

use core::Core;
use mantle::Mantle;

pub struct World {
    mantle: Mantle,
}

// TODO: Commands
impl World {
    pub fn new() -> Self {
        let mut world = Self {
            mantle: Mantle { core: Arc::new(Core::new()), commands: Arc::new(Default::default()) },
        };

        for init in COMPONENT_ENTRIES {
            (init)(&mut world);
        }

        world.flush();

        world
    }

    fn core_mut(&mut self) -> &mut Core {
        Arc::get_mut(&mut self.mantle.core).unwrap()
    }

    fn entity(&self, entity: Entity) -> View<'_> {
        let location = self.mantle.core.entity_location_locking(entity).unwrap();
        View { mantle: &self.mantle, entity, location }
    }

    pub fn spawn(&mut self) -> Entity {
        self.core_mut().spawn().0
    }

    pub fn component_info(&self, component: Entity) -> Option<ComponentInfo> {
        self.mantle.core.component_info_locking(component)
    }

    pub fn has<Id: Into<FieldId>>(&self, field: Id, entity: Entity) -> bool {
        self.mantle.core.has(field, entity)
    }

    pub fn remove<C: Component>(&mut self, entity: Entity) {
        self.core_mut().remove_field(C::id(), entity);
    }

    pub unsafe fn insert_bytes(
        &mut self,
        info: ComponentInfo,
        bytes: &[MaybeUninit<u8>],
        entity: Entity,
    ) {
        let Some(location) = self.core_mut().entity_location(entity) else {
            panic!("Entity does not exist");
        };
        unsafe { self.core_mut().insert_bytes(info, bytes, location) };
    }

    pub fn insert<C: Component>(&mut self, component: C, entity: Entity) {
        // SAFETY: This is always safe because we are providing static type info
        unsafe {
            let bytes = std::slice::from_raw_parts(
                (&component as *const C) as *const MaybeUninit<u8>,
                size_of::<C>(),
            );
            self.insert_bytes(C::info(), bytes, entity);
        }
        std::mem::forget(component);
    }

    pub fn get_bytes(
        &self,
        field: FieldId,
        entity: Entity,
    ) -> Option<ColumnReadGuard<[MaybeUninit<u8>]>> {
        let Some(location) = self.mantle.core.entity_location_locking(entity) else {
            panic!("Entity does not exist");
        };
        self.mantle.core.get_bytes(field, location)
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<ColumnReadGuard<T>> {
        let _ = T::NON_ZST_OR_PANIC;
        let Some(location) = self.mantle.core.entity_location_locking(entity) else {
            panic!("Entity does not exist");
        };
        self.mantle.core.get_bytes(T::id().into(), location).map(|bytes| {
            ColumnReadGuard::map(bytes, |bytes| {
                // SAFETY: Don't need to check TypeId because component's Entity id acts as TypeId
                unsafe { (bytes.as_ptr() as *const T).as_ref() }.unwrap()
            })
        })
    }

    pub fn query(&self) -> Query {
        Query::new(self.mantle.clone())
    }

    pub(crate) fn flush(&mut self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as ssecs;
    use crate::component::tests::*;
    use ssecs_macros::*;
    use std::sync::Arc;

    #[derive(Component)]
    #[allow(dead_code)]
    pub struct RefCounted(Arc<u8>);

    #[derive(Component)]
    struct Foo(u8);

    #[derive(Component)]
    struct Bar(u8);

    #[test]
    fn component_info() {
        let world = World::new();
        for info in [
            ComponentInfo::info(),
            Player::info(),
            Health::info(),
            Transform::info(),
            Foo::info(),
            Bar::info(),
        ] {
            assert_eq!(world.component_info(info.id), Some(info));
        }
    }

    #[test]
    fn zsts() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(Player, e);
        assert_eq!(true, world.has(Player::id(), e));
        world.remove::<Player>(e);
        assert_eq!(false, world.has(Player::id(), e));
    }

    #[test]
    fn set_remove() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(Foo(0), e);
        assert_eq!(true, world.has(Foo::id(), e));
        assert_eq!(0, world.get::<Foo>(e).unwrap().0);

        world.insert(Bar(1), e);
        assert_eq!(true, world.has(Foo::id(), e));
        assert_eq!(0, world.get::<Foo>(e).unwrap().0);
        assert_eq!(true, world.has(Bar::id(), e));
        assert_eq!(1, world.get::<Bar>(e).unwrap().0);

        world.remove::<Foo>(e);
        assert_eq!(false, world.has(Foo::id(), e));
        assert!(world.get::<Foo>(e).is_none());
        assert_eq!(true, world.has(Bar::id(), e));
        assert_eq!(1, world.get::<Bar>(e).unwrap().0);
    }

    #[test]
    fn drop() {
        let val = Arc::new(0_u8);
        let mut world = World::new();
        let e = world.spawn();
        world.insert(RefCounted(val.clone()), e);
        assert_eq!(2, Arc::strong_count(&val));
        assert_eq!(true, world.has(RefCounted::id(), e));
        world.remove::<RefCounted>(e);
        assert_eq!(false, world.has(RefCounted::id(), e));
        assert_eq!(1, Arc::strong_count(&val));
    }
}
