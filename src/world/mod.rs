use std::{
    cell::{Cell, UnsafeCell},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use thread_local::ThreadLocal;

use crate::{
    component::{COMPONENT_ENTRIES, ComponentInfo},
    entity::{Entity, View},
    query::Query,
    world::core::EntityLocation,
};

pub(crate) mod archetype;
pub(crate) mod command;
pub(crate) mod core;

use command::Command;
use core::Core;

pub struct World {
    pub(crate) crust: Arc<Crust>,
}

pub(crate) struct Crust {
    pub(crate) mantle: UnsafeCell<Mantle>,
    pub(crate) flush_guard: AtomicUsize, // nothing(0), flush(usize::MAX), blocked(1..usize::MAX)
}

pub(crate) struct Mantle {
    pub(crate) core: Core,
    pub(crate) commands: ThreadLocal<Cell<Vec<Command>>>,
}

impl Mantle {
    pub(crate) fn enqueue(&self, command: Command) {
        let cell = self.commands.get_or(|| Cell::new(Vec::default()));
        let mut queue = cell.take();
        queue.push(command);
        cell.set(queue);
    }

    pub(crate) fn flush(&mut self) {
        for cell in (&mut self.commands).iter_mut() {
            for command in cell.get_mut().drain(..) {
                command.apply(&mut self.core);
            }
        }
    }
}

impl Crust {
    pub(crate) fn begin_read(flush_guard: &AtomicUsize) {
        if let Err(_) = flush_guard.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
            (old < usize::MAX).then_some(old + 1)
        }) {
            panic!("Tried to read while structurally mutating");
        }
    }

    pub(crate) fn end_read(flush_guard: &AtomicUsize) {
        if let Err(_) = flush_guard.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
            (0 < old && old < usize::MAX).then_some(old - 1)
        }) {
            panic!("No read to end");
        }
    }

    pub(crate) fn begin_flush(flush_guard: &AtomicUsize) {
        if let Err(_) = flush_guard.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
            (0 == old).then_some(usize::MAX)
        }) {
            panic!("Tried to structurally mutate while reading");
        }
    }

    pub(crate) fn end_flush(flush_guard: &AtomicUsize) {
        if let Err(_) = flush_guard.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| {
            (old == usize::MAX).then_some(0)
        }) {
            panic!("No write to end");
        }
    }

    pub(crate) fn mantle<R>(&self, func: impl FnOnce(&Mantle) -> R) -> R {
        Self::begin_read(&self.flush_guard);
        let ret = func(unsafe { self.mantle.get().as_ref().unwrap() });
        Self::end_read(&self.flush_guard);
        ret
    }

    pub(crate) fn flush(&self) {
        Self::begin_flush(&self.flush_guard);
        unsafe { self.mantle.get().as_mut().unwrap().flush() };
        Self::end_flush(&self.flush_guard);
    }
}

impl World {
    pub fn new() -> Self {
        let mut world = Self {
            crust: Arc::new(Crust {
                flush_guard: AtomicUsize::new(0),
                mantle: UnsafeCell::new(Mantle { core: Core::new(), commands: Default::default() }),
            }),
        };

        for init in COMPONENT_ENTRIES {
            (init)(&mut world);
        }

        world.flush();

        world
    }

    pub fn entity(&self, entity: Entity) -> View {
        self.get_entity(entity).unwrap()
    }

    pub fn get_entity(&self, entity: Entity) -> Option<View> {
        self.crust.mantle(|mantle| {
            mantle.core.entity_location_locking(entity).map(|_| View { entity, world: &self })
        })
    }

    pub fn spawn(&self) -> View {
        self.crust.mantle(|mantle| {
            let entity = mantle.core.create_uninitialized_entity();
            mantle.enqueue(Command::spawn(entity));
            View { entity, world: &self }
        })
    }

    pub fn component_info(&self, component: Entity) -> Option<ComponentInfo> {
        self.crust.mantle(|mantle| mantle.core.component_info_locking(component))
    }

    pub fn query(&self) -> Query {
        Query::new(World { crust: self.crust.clone() })
    }

    /// Will panic if:
    /// - Attempted while something is reading (query, observer, system, etc.)
    /// - There are lingering column guards on locations being moved
    pub fn flush(&self) {
        self.crust.flush();
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
        let world = World::new();

        let e = world.spawn().insert(Player).id();
        world.flush();
        assert_eq!(true, world.entity(e).has(Player::id()));

        world.entity(e).remove(Player::id());
        world.flush();
        assert_eq!(false, world.entity(e).has(Player::id()));
    }

    #[test]
    fn set_remove() {
        let world = World::new();

        let e = world.spawn().insert(Foo(0));
        world.flush();
        assert_eq!(true, e.has(Foo::id()));
        assert_eq!(0, e.get::<Foo>().unwrap().0);

        e.insert(Bar(1));
        world.flush();
        assert_eq!(true, e.has(Foo::id()));
        assert_eq!(0, e.get::<Foo>().unwrap().0);
        assert_eq!(true, e.has(Bar::id()));
        assert_eq!(1, e.get::<Bar>().unwrap().0);

        e.remove(Foo::id());
        world.flush();
        assert_eq!(false, e.has(Foo::id()));
        assert!(e.get::<Foo>().is_none());
        assert_eq!(true, e.has(Bar::id()));
        assert_eq!(1, e.get::<Bar>().unwrap().0);
    }

    #[test]
    fn despawn() {
        let world = World::new();
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
        let world = World::new();

        let e = world.spawn().insert(RefCounted(val.clone()));
        world.flush();
        assert_eq!(2, Arc::strong_count(&val));
        assert_eq!(true, e.has(RefCounted::id()));

        e.remove(RefCounted::id());
        world.flush();
        assert_eq!(false, e.has(RefCounted::id()));
        assert_eq!(1, Arc::strong_count(&val));
    }

    #[test]
    fn flush() {
        let world = World::new();
        let e = world.spawn().insert(Foo(0));
        assert!(e.get::<Foo>().is_none());
        world.flush();
        assert!(e.get::<Foo>().is_some());
    }

    #[test]
    #[should_panic]
    fn flush_with_lock() {
        let world = World::new();
        let e = world.spawn().insert(Foo(0));
        world.flush();
        let _f = e.get::<Foo>().unwrap();
        e.insert(Bar(0));
        world.flush();
    }
}
