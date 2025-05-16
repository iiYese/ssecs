use std::{collections::HashMap, mem::size_of};

use derive_more::{Deref, DerefMut};
use parking_lot::{MappedRwLockReadGuard, RwLockReadGuard};
use slotmap::SlotMap;

use crate::{
    archetype::{Archetype, ArchetypeId, FieldId},
    component::{COMPONENT_ENTRIES, Component, ComponentInfo},
    entity::Entity,
};

pub struct World {
    entity_index: SlotMap<Entity, EntityLocation>,
    field_index: HashMap<FieldId, FieldLocations>,
    archetypes: SlotMap<ArchetypeId, Archetype>,
}

impl World {
    pub fn new() -> Self {
        // Add empty archetype
        let mut archetypes = SlotMap::<ArchetypeId, Archetype>::with_key();
        let empty_archetype = archetypes.insert(Archetype::default());

        let world = Self {
            entity_index: Default::default(),
            field_index: Default::default(),
            archetypes,
        };

        // Make sure all component entities are sawned before init
        // Needed if components add relationships (traits)
        for n in 0..COMPONENT_ENTRIES.len() {}

        // init components
        for init in COMPONENT_ENTRIES {
            init(&world);
        }

        world
    }

    pub(crate) fn has_component(&self, entity: Entity, component: Entity) -> bool {
        self.entity_index
            .get(entity)
            .zip(self.field_index.get(&component.into()))
            .is_some_and(|(entity_location, field_locations)| {
                field_locations.contains_key(&entity_location.archetype)
            })
    }

    pub(crate) fn entity_location(&self, entity: Entity) -> Option<EntityLocation> {
        self.entity_index
            .contains_key(entity)
            .then(|| unsafe { self.entity_index.get_unchecked(entity) })
            .copied()
    }

    /// Get metadata of a component
    pub fn component_info(&self, component: Entity) -> Option<ComponentInfo> {
        self.entity_location(component)
            .zip(self.field_index.get(&ComponentInfo::id().into()))
            .and_then(|(component_location, field_locations)| {
                let column = self
                    .archetypes
                    .get(component_location.archetype)?
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
        component_info: ComponentInfo,
        entity: Entity,
    ) -> Option<MappedRwLockReadGuard<[u8]>> {
        self.entity_location(entity)
            .zip(self.field_index.get(&component_info.id.into()))
            .and_then(|(entity_location, field_locations)| {
                let column = self
                    .archetypes
                    .get(entity_location.archetype)?
                    .columns
                    .get(**field_locations.get(&entity_location.archetype)?)?
                    .read();
                Some(RwLockReadGuard::map(column, |column| {
                    &column[entity_location.row * component_info.size..][..component_info.size]
                }))
            })
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<MappedRwLockReadGuard<T>> {
        self.get_bytes(T::info(), entity).map(|bytes| {
            MappedRwLockReadGuard::map(bytes, |bytes| {
                // SAFETY: Don't need to check TypeId because component's Entity id acts as TypeId
                unsafe { (bytes.as_ptr() as *const T).as_ref() }.unwrap()
            })
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct EntityLocation {
    archetype: ArchetypeId,
    row: usize,
}

#[derive(Deref, DerefMut)]
pub(crate) struct FieldLocations(HashMap<ArchetypeId, ColumnIndex>);

#[derive(Deref, DerefMut)]
pub(crate) struct ColumnIndex(pub usize);
