use std::{collections::HashMap, mem::size_of};

use derive_more::{Deref, DerefMut};
use parking_lot::{MappedRwLockReadGuard, RwLockReadGuard};

use crate::{
    archetype::{Archetype, ArchetypeId, FieldId},
    component::{COMPONENT_ENTRIES, Component, ComponentInfo},
    entity::Entity,
};

pub struct World {
    entity_index: HashMap<Entity, EntityLocation>, // TODO: Replace with sparse map
    field_index: HashMap<FieldId, FieldLocations>,
    archetypes: HashMap<ArchetypeId, Archetype>,
}

impl World {
    pub fn new() -> Self {
        let world = Self {
            entity_index: Default::default(),
            field_index: Default::default(),
            archetypes: Default::default(),
        };
        for func in COMPONENT_ENTRIES {
            func(&world);
        }
        world
    }

    pub(crate) fn has_component(&self, entity: Entity, component: Entity) -> bool {
        self.entity_index
            .get(&entity)
            .zip(self.field_index.get(&component.into()))
            .is_some_and(|(entity_location, field_locations)| {
                field_locations.contains_key(&entity_location.archetype)
            })
    }

    /// Get metadata of a component
    pub fn component_info(&self, component: Entity) -> Option<ComponentInfo> {
        self.entity_index
            .get(&component)
            .zip(self.field_index.get(&ComponentInfo::id().into()))
            .and_then(|(component_location, field_locations)| {
                let column = self
                    .archetypes
                    .get(&component_location.archetype)?
                    .columns
                    .get(**field_locations.get(&component_location.archetype)?)?
                    .read();
                let bytes = &column[component_location.row * size_of::<ComponentInfo>()..];
                Some(unsafe { std::ptr::read(bytes.as_ptr() as *const ComponentInfo) })
            })
    }

    /// Get a component from an entity as type erased bytes
    pub fn get_bytes(
        &self,
        component: Entity,
        entity: Entity,
    ) -> Option<MappedRwLockReadGuard<[u8]>> {
        let Some(component_info) = self.component_info(component) else {
            panic!("Component entity does not have ComponentInfo. Might not be a component.");
        };

        self.entity_index
            .get(&entity)
            .zip(self.field_index.get(&component.into()))
            .and_then(|(entity_location, field_locations)| {
                let column = self
                    .archetypes
                    .get(&entity_location.archetype)?
                    .columns
                    .get(**field_locations.get(&entity_location.archetype)?)?
                    .read();
                Some(RwLockReadGuard::map(column, |column| {
                    &column[entity_location.row * component_info.size..][..component_info.size]
                }))
            })
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<MappedRwLockReadGuard<T>> {
        self.get_bytes(T::id(), entity).map(|bytes| {
            MappedRwLockReadGuard::map(bytes, |bytes| {
                // SAFETY: Don't need to check TypeId because component's Entity id acts as TypeId
                unsafe { (bytes.as_ptr() as *const T).as_ref() }.unwrap()
            })
        })
    }
}

pub(crate) struct EntityLocation {
    archetype: ArchetypeId,
    row: usize,
}

#[derive(Deref, DerefMut)]
pub(crate) struct FieldLocations(HashMap<ArchetypeId, ColumnIndex>);

#[derive(Deref, DerefMut)]
pub(crate) struct ColumnIndex(pub usize);
