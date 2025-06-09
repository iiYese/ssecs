use std::{
    cell::{Cell, RefCell, UnsafeCell},
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
    },
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
    pub(crate) mantle: Arc<Mantle>,
}

pub(crate) struct Mantle {
    pub(crate) core: UnsafeCell<Core>,
    pub(crate) commands: RefCell<ThreadLocal<Cell<Vec<Command>>>>,
    pub(crate) read_write: AtomicIsize,
}

impl Mantle {
    pub(crate) fn enqueue(&self, command: Command) {
        let commands = self.commands.borrow();
        let cell = commands.get_or(|| Cell::new(Vec::default()));
        let mut queue = cell.take();
        queue.push(command);
        cell.set(queue);
    }

    pub(crate) fn begin_read(&self) {
        use Ordering::SeqCst;
        if let Err(_) = self //
            .read_write
            .fetch_update(SeqCst, SeqCst, |old| (-1 < old).then_some(old + 1))
        {
            panic!("Tried to read while structurally mutating");
        }
    }

    pub(crate) fn end_read(&self) {
        use Ordering::SeqCst;
        if let Err(_) = self //
            .read_write
            .fetch_update(SeqCst, SeqCst, |old| (0 < old).then_some(old - 1))
        {
            panic!("No read to end");
        }
    }

    pub(crate) fn core<R>(&self, mut func: impl FnMut(&Core) -> R) -> R {
        self.begin_read();
        let ret = func(unsafe { self.core.get().as_ref().unwrap() });
        self.end_read();
        ret
    }

    pub(crate) fn flush(&self) {
        use Ordering::SeqCst;
        if let Err(_) = self //
            .read_write
            .fetch_update(SeqCst, SeqCst, |old| (0 == old).then_some(old - 1))
        {
            panic!("Tried to structurally mutate while reading");
        }

        // SAFETY: When `read_write == 0` nothing should be aliasing core
        let core = unsafe { self.core.get().as_mut().unwrap() };
        let mut commands = self.commands.borrow_mut();

        for cell in (&mut *commands).iter_mut() {
            for command in cell.get_mut().drain(..) {
                command.apply(core)
            }
        }

        self.read_write.fetch_update(SeqCst, SeqCst, |_| Some(0)).unwrap();
    }
}

impl World {
    pub fn new() -> Self {
        let mut world = Self {
            mantle: Arc::new(Mantle {
                core: UnsafeCell::new(Core::new()),
                commands: Default::default(),
                read_write: AtomicIsize::new(0),
            }),
        };

        for init in COMPONENT_ENTRIES {
            (init)(&mut world);
        }

        world.flush();

        world
    }

    pub fn entity(&self, entity: Entity) -> View<'_> {
        self.get_entity(entity).unwrap()
    }

    pub fn get_entity(&self, entity: Entity) -> Option<View<'_>> {
        self.mantle
            .core(|core| core.entity_location_locking(entity))
            .map(|_| View { entity, world: &self })
    }

    pub fn spawn(&self) -> View<'_> {
        let entity = self.mantle.core(|core| core.create_uninitialized_entity());
        self.mantle.enqueue(Command::spawn(entity));
        View { entity, world: &self }
    }

    pub fn component_info(&self, component: Entity) -> Option<ComponentInfo> {
        self.mantle.core(|core| core.component_info_locking(component))
    }

    pub fn query(&self) -> Query {
        Query::new(World { mantle: self.mantle.clone() })
    }

    /// Will panic if:
    /// - Attempted while something is reading (query, observer, system, etc.)
    /// - There are lingering column guards on locations being moved
    pub fn flush(&self) {
        self.mantle.flush();
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
}
