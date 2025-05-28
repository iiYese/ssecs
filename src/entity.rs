use slotmap::{KeyData, new_key_type};

new_key_type! { pub struct Entity; }

use crate::{
    NonZstOrPanic,
    archetype::{ColumnReadGuard, FieldId, into_bytes},
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
    pub fn id(&self) -> Entity {
        self.entity
    }

    pub fn insert<C: Component>(&self, component: C) -> &Self {
        self.mantle.enqueue(Command::insert(
            C::info(),
            into_bytes(component),
            self.entity,
        ));
        self
    }

    pub fn remove<Id: Into<FieldId>>(&self, id: Id) -> &Self {
        self.mantle.enqueue(Command::remove(id.into(), self.entity));
        self
    }

    pub fn has<Id: Into<FieldId>>(&self, field: Id) -> bool {
        self.mantle.core.archetype_has(field.into(), self.location.archetype)
    }

    pub fn get<T: Component>(&self) -> Option<ColumnReadGuard<T>> {
        let _ = T::NON_ZST_OR_PANIC;
        self.mantle.core.get_bytes(T::id().into(), self.location).map(|bytes| {
            ColumnReadGuard::map(bytes, |bytes| {
                // SAFETY: Don't need to check TypeId because component's Entity id acts as TypeId
                unsafe { (bytes.as_ptr() as *const T).as_ref() }.unwrap()
            })
        })
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
