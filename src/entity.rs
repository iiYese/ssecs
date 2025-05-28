use slotmap::{KeyData, new_key_type};

new_key_type! { pub struct Entity; }

use crate::{
    archetype::{ColumnReadGuard, FieldId},
    component::Component,
    query::AccessTuple,
    world::{command::Command, core::EntityLocation, mantle::Mantle},
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

pub struct View<'a> {
    pub(crate) entity: Entity,
    pub(crate) location: EntityLocation,
    pub(crate) mantle: &'a Mantle,
}

impl View<'_> {
    fn id(&self) -> Entity {
        self.entity
    }

    fn insert<C: Component>(&self, component: C) -> &Self {
        todo!()
    }

    fn remove<C: Component>(&self) -> &Self {
        let cell = self.mantle.commands.get().unwrap();
        let mut queue = cell.take();
        queue.push(Command::remove(C::id(), self.entity));
        cell.set(queue);
        self
    }

    pub fn has<Id: Into<FieldId>>(&self, field: Id) -> bool {
        self.mantle.core.archetype_has(field, self.location.archetype)
    }

    pub fn fields<Q: AccessTuple>(&self) -> Q::Out {
        todo!()
    }

    pub fn get_fields<Q: AccessTuple>(&self) -> Option<Q::Out> {
        todo!()
    }

    fn despawn(self) {
        todo!()
    }
}
