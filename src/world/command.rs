use std::mem::{ManuallyDrop, MaybeUninit};

use crate::{
    archetype::FieldId,
    component::{Component, ComponentInfo},
    entity::Entity,
    world::core::Core,
};

// TODO: Batching
// - Despawn is last: Ignore all other ops on entity
// - Inserrt/Remove is last: Ignore all other inserts for component
// - Iter reverse for less work
#[derive(Debug)]
enum Operation {
    Noop,
    Spawn(Entity),
    Despawn(Entity),
    Insert {
        info: ComponentInfo,
        bytes: Box<[MaybeUninit<u8>]>,
        entity: Entity,
    },
    Remove {
        field: FieldId,
        entity: Entity,
    },
}

#[derive(Debug)]
pub(crate) struct Command {
    operation: Operation,
    jump: usize,
}

unsafe impl Send for Command {}

impl Default for Command {
    fn default() -> Self {
        Self { operation: Operation::Noop, jump: 1 }
    }
}

impl Command {
    pub(crate) fn apply(self, core: &mut Core) {
        use Operation::*;
        match self.operation {
            Noop => {
                return;
            }
            Spawn(entity) => {
                core.initialize_entity_location(entity);
            }
            Despawn(entity) => {
                core.despawn(entity);
            }
            Insert { info, bytes, entity } => {
                unsafe { core.insert_bytes(info, &bytes, entity) };
            }
            Remove { field, entity } => {
                core.remove_field(field, entity);
            }
        }
    }

    pub(crate) fn spawn(entity: Entity) -> Self {
        Self { jump: 1, operation: Operation::Spawn(entity) }
    }

    pub(crate) fn despawn(entity: Entity) -> Self {
        Self { jump: 1, operation: Operation::Despawn(entity) }
    }

    pub(crate) fn insert<C: Component>(val: C, entity: Entity) -> Self {
        let leaked = ManuallyDrop::new(val);
        let bytes: &[MaybeUninit<u8>] = unsafe {
            std::slice::from_raw_parts((&raw const leaked).cast(), size_of::<C>()) //
        };
        // SAFETY: Safe because this is using static type info
        unsafe { Self::insert_bytes(C::info(), bytes.into(), entity) }
    }

    pub(crate) unsafe fn insert_bytes(
        info: ComponentInfo,
        bytes: Box<[MaybeUninit<u8>]>,
        entity: Entity,
    ) -> Self {
        Self { jump: 1, operation: Operation::Insert { info, bytes, entity } }
    }

    pub(crate) fn remove<Id: Into<FieldId>>(field: Id, entity: Entity) -> Self {
        Self { jump: 1, operation: Operation::Remove { field: field.into(), entity } }
    }
}
