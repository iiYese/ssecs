use std::{
    ops::{Deref, DerefMut},
    sync::atomic::AtomicUsize,
};

use parking_lot::MappedRwLockReadGuard;
use slotmap::{KeyData, new_key_type};

new_key_type! { pub struct Entity; }

use crate::{
    NonZstOrPanic,
    archetype::FieldId,
    component::Component,
    query::AccessTuple,
    world::{Crust, Mantle, World, command::Command},
};

impl Entity {
    pub fn null() -> Self {
        Self::default()
    }

    pub unsafe fn from_offset(val: u64) -> Self {
        // Slotmap IDs start from 1
        Self(KeyData::from_ffi(1 + val))
    }

    pub fn raw(self) -> u64 {
        self.0.as_ffi()
    }

    pub(crate) fn from_ffi(val: u64) -> Self {
        Self(KeyData::from_ffi(val))
    }
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub(crate) entity: Entity,
    pub(crate) world: &'a World,
}

impl View<'_> {
    pub fn id(&self) -> Entity {
        self.entity
    }

    pub fn insert<C: Component>(self, component: C) -> Self {
        self.world.crust.mantle(|mantle| {
            mantle.enqueue(Command::insert(component, self.entity));
        });
        self
    }

    pub fn remove<Id: Into<FieldId>>(self, id: Id) -> Self {
        self.world.crust.mantle(|mantle| {
            mantle.enqueue(Command::remove(id.into(), self.entity));
        });
        self
    }

    pub fn has<Id: Into<FieldId> + Copy>(&self, field: Id) -> bool {
        self.world.crust.mantle(|Mantle { core, .. }| {
            core.entity_location_locking(self.entity)
                .filter(|location| core.archetype_has(field.into(), location.archetype))
                .is_some()
        })
    }

    /// Will panic if called in the middle of a flush
    pub fn get<T: Component>(&self) -> Option<ColumnReadGuard<T>> {
        let _ = T::NON_ZST_OR_PANIC;
        Crust::begin_read(&self.world.crust.flush_guard);
        // SAFETY: World aliasing is temporary
        let core = unsafe { &self.world.crust.mantle.get().as_ref().unwrap().core };
        let location = core.entity_location_locking(self.entity).unwrap();
        let out = core.get_bytes(T::id().into(), location).map(|bytes| {
            ColumnReadGuard::new(
                MappedRwLockReadGuard::map(bytes, |bytes| {
                    // SAFETY: Don't TypeId check not needed because Entity id acts as TypeId
                    unsafe { (bytes.as_ptr() as *const T).as_ref() }.unwrap()
                }),
                &self.world.crust.flush_guard,
            )
        });
        Crust::end_read(&self.world.crust.flush_guard);
        out
    }

    pub fn fields<Q: AccessTuple>(&self) -> Q::Out {
        todo!()
    }

    pub fn get_fields<Q: AccessTuple>(&self) -> Option<Q::Out> {
        todo!()
    }

    pub fn duplicate(&self, options: DupeOpts) -> View<'_> {
        let destination = self.world.spawn();
        self.duplicate_into(options, destination);
        destination
    }

    pub fn duplicate_into(&self, options: DupeOpts, destination: View<'_>) {
        todo!();
    }

    pub fn despawn(self) {
        self.world.crust.mantle(|mantle| mantle.enqueue(Command::despawn(self.entity)));
    }
}

pub struct ColumnReadGuard<'a, T> {
    mapped_guard: MappedRwLockReadGuard<'a, T>,
    flush_guard: *const AtomicUsize,
}

impl<'a, T> ColumnReadGuard<'a, T> {
    pub(crate) fn new(
        mapped_guard: MappedRwLockReadGuard<'a, T>,
        flush_guard: &AtomicUsize,
    ) -> Self {
        Crust::begin_read(flush_guard);
        Self { mapped_guard, flush_guard }
    }
}

impl<T> Deref for ColumnReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &*self.mapped_guard
    }
}

impl<T> Drop for ColumnReadGuard<'_, T> {
    fn drop(&mut self) {
        // SAFETY: Always safe because atomic
        Crust::end_read(unsafe { self.flush_guard.as_ref().unwrap() });
    }
}

pub enum DupeOpts {
    OrDefault,
    OrPanic,
}
