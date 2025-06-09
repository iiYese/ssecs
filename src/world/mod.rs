use std::{
    cell::{Cell, UnsafeCell},
    sync::Arc,
};

use thread_local::ThreadLocal;

use crate::{
    component::{COMPONENT_ENTRIES, ComponentInfo},
    entity::{Entity, View},
    query::Query,
};

pub(crate) mod command;
pub(crate) mod core;

use command::Command;
use core::{Core, EntityLocation};

pub struct World {
    pub(crate) mantle: Arc<UnsafeCell<Mantle>>,
}

pub(crate) struct Mantle {
    pub(crate) core: Core,
    pub(crate) commands: ThreadLocal<Cell<Vec<Command>>>,
}

impl World {
    pub fn new() -> Self {
        let mut world = Self {
            mantle: Arc::new(UnsafeCell::new(Mantle {
                core: Core::new(),
                commands: Default::default(),
            })),
        };

        for init in COMPONENT_ENTRIES {
            (init)(&mut world);
        }

        world.flush();

        world
    }

    pub fn entity(&self, entity: Entity) -> View<'_> {
        let mantle = unsafe { self.mantle() };
        let location = mantle.core.entity_location_locking(entity).unwrap();
        View { world: &self, entity, location }
    }

    pub fn get_entity(&self, entity: Entity) -> Option<View<'_>> {
        let mantle = unsafe { self.mantle() };
        mantle.core.entity_location_locking(entity).map(|location| View {
            entity,
            world: &self,
            location,
        })
    }

    pub fn spawn(&self) -> View<'_> {
        let mantle = unsafe { self.mantle() };
        let entity = mantle.core.create_uninitialized_entity();
        self.enqueue(Command::spawn(entity));
        View { entity, world: &self, location: EntityLocation::uninitialized() }
    }

    pub fn component_info(&self, component: Entity) -> Option<ComponentInfo> {
        let mantle = unsafe { self.mantle() };
        mantle.core.component_info_locking(component)
    }

    pub fn query(&self) -> Query {
        Query::new(World { mantle: self.mantle.clone() })
    }

    pub fn flush(&mut self) {
        let mantle = unsafe { self.mantle_mut() };
        for cell in (&mut mantle.commands).into_iter() {
            for command in cell.get_mut().drain(..) {
                command.apply(&mut mantle.core)
            }
        }
    }

    pub(crate) unsafe fn mantle(&self) -> &Mantle {
        unsafe { self.mantle.get().as_ref().unwrap() }
    }

    pub(crate) unsafe fn mantle_mut(&self) -> &mut Mantle {
        unsafe { self.mantle.get().as_mut().unwrap() }
    }

    pub(crate) fn enqueue(&self, command: Command) {
        let mantle = unsafe { self.mantle() };
        let cell = mantle.commands.get_or(|| Cell::new(Vec::default()));
        let mut queue = cell.take();
        queue.push(command);
        cell.set(queue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as ssecs;
    use crate::component::{Component, tests::*};
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
        let e = world.spawn().insert(Player).id();
        world.flush();
        assert_eq!(true, world.entity(e).has(Player::id()));

        world.entity(e).remove(Player::id());
        world.flush();
        assert_eq!(false, world.entity(e).has(Player::id()));
    }

    #[test]
    fn set_remove() {
        let mut world = World::new();
        let e = world.spawn().insert(Foo(0)).id();
        world.flush();

        {
            let e = world.entity(e);
            assert_eq!(true, e.has(Foo::id()));
            assert_eq!(0, e.get::<Foo>().unwrap().0);
            e.insert(Bar(1));
        }
        world.flush();

        {
            let e = world.entity(e);
            assert_eq!(true, e.has(Foo::id()));
            assert_eq!(0, e.get::<Foo>().unwrap().0);
            assert_eq!(true, e.has(Bar::id()));
            assert_eq!(1, e.get::<Bar>().unwrap().0);
            e.remove(Foo::id());
        }
        world.flush();

        {
            let e = world.entity(e);
            assert_eq!(false, e.has(Foo::id()));
            assert!(e.get::<Foo>().is_none());
            assert_eq!(true, e.has(Bar::id()));
            assert_eq!(1, e.get::<Bar>().unwrap().0);
        }
    }

    #[test]
    fn despawn() {
        let mut world = World::new();
        let e = world.spawn().id();
        world.flush();
        assert!(world.get_entity(e).is_some());
        world.entity(e).despawn();
        world.flush();
        assert!(world.get_entity(e).is_none());
    }

    #[test]
    fn drop() {
        let val = Arc::new(0_u8);
        let mut world = World::new();
        let e = world.spawn().insert(RefCounted(val.clone())).id();
        world.flush();

        {
            let e = world.entity(e);
            assert_eq!(2, Arc::strong_count(&val));
            assert_eq!(true, e.has(RefCounted::id()));
            e.remove(RefCounted::id());
        }
        world.flush();

        {
            let e = world.entity(e);
            assert_eq!(false, e.has(RefCounted::id()));
            assert_eq!(1, Arc::strong_count(&val));
        }
    }
}
