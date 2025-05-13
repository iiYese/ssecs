use std::collections::HashMap;

use derive_more::{Deref, DerefMut};

use crate::{
    archetype::{Archetype, ArchetypeId, FieldId},
    component::COMPONENT_INIT_FNS,
    entity::Entity,
};

pub struct World {
    entity_index: HashMap<Entity, EntityLocation>, // TODO: Replace with sparse map
    field_index: HashMap<FieldId, FieldLocations>,
}

impl World {
    pub fn new() -> Self {
        let world = Self {
            entity_index: Default::default(),
            field_index: Default::default(),
        };
        for func in COMPONENT_INIT_FNS {
            func(&world);
        }
        world
    }
}

pub(crate) struct EntityLocation {
    table: *const Archetype,
    row: usize,
}

#[derive(Deref, DerefMut)]
pub(crate) struct FieldLocations(HashMap<ArchetypeId, ColumnIndex>);

#[derive(Deref, DerefMut)]
pub(crate) struct ColumnIndex(pub usize);
