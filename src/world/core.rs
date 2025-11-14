use std::{collections::HashMap, mem::MaybeUninit};

use derive_more::{Deref, DerefMut};
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

use crate::{
    component::{COMPONENT_ENTRIES, Component, ComponentInfo},
    entity::Entity,
    slotmap::*,
    world::archetype::{
        Archetype, ArchetypeEdge, ArchetypeId, Column, ColumnIndex, FieldId, RowIndex, Signature,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EntityLocation {
    pub(crate) archetype: ArchetypeId,
    pub(crate) row: RowIndex,
}

impl EntityLocation {
    pub(crate) fn uninitialized() -> Self {
        Self { archetype: ArchetypeId::empty_archetype(), row: RowIndex(usize::MAX) }
    }
}

#[derive(Deref, DerefMut, Default, Debug)]
pub(crate) struct FieldLocations(HashMap<ArchetypeId, ColumnIndex>);

pub(crate) struct Core {
    // Add read_index: SlotMap<Entity, EntityLocation> (a copy of entity_index) if this is too slow
    entity_index: Mutex<SlotMap<Entity, EntityLocation>>,
    field_index: HashMap<FieldId, FieldLocations>,
    signature_index: HashMap<Signature, ArchetypeId>,
    archetypes: SlotMap<ArchetypeId, Archetype>,
}

impl Core {
    pub fn new() -> Self {
        // Add empty archetype & component info archetype
        let mut archetypes = SlotMap::<ArchetypeId, Archetype>::default();
        let mut entity_index = SlotMap::<Entity, EntityLocation>::default();
        let empty_archetype_id = archetypes.insert(Archetype::default());
        let component_info_archetype_id = archetypes.insert(Archetype::default());
        assert_eq!(empty_archetype_id, ArchetypeId::empty_archetype());

        if let Some(empty_archetype) = archetypes.get_mut(empty_archetype_id) {
            // Make sure all component entities are sawned before init
            // Needed if components add relationships (traits)
            for n in 0..COMPONENT_ENTRIES.len() {
                let id = entity_index
                    .insert(EntityLocation { archetype: empty_archetype_id, row: RowIndex(n) });
                empty_archetype.entities.push(id);
            }
            // Add ComponentInfo edge
            let component_info_edge = &mut empty_archetype //
                .edges
                .entry(ComponentInfo::id().into())
                .or_default();
            component_info_edge.add = Some(component_info_archetype_id);
        }

        // Mangually create ComponentInfo archetype
        let component_info_signature = Signature::new(&[ComponentInfo::id().into()]);
        archetypes[component_info_archetype_id] = Archetype {
            signature: component_info_signature.clone(),
            entities: Default::default(),
            columns: vec![RwLock::new(Column::new(ComponentInfo::info()))],
            edges: HashMap::from([(
                ComponentInfo::id().into(),
                ArchetypeEdge { remove: Some(empty_archetype_id), add: None },
            )]),
        };

        Self {
            archetypes,
            entity_index: Mutex::new(entity_index),
            field_index: HashMap::from([(
                ComponentInfo::id().into(),
                FieldLocations(HashMap::from([(
                    component_info_archetype_id,
                    ColumnIndex(0),
                )])),
            )]),
            signature_index: HashMap::from([
                (Signature::default(), empty_archetype_id),
                (component_info_signature, component_info_archetype_id),
            ]),
        }
    }

    /// Must ensure missing entries in columns for entity are filled
    unsafe fn move_entity(
        &mut self,
        old_location: EntityLocation,
        destination_id: ArchetypeId,
    ) -> EntityLocation {
        if old_location.archetype == destination_id {
            return old_location;
        }
        let entity_index = self.entity_index.get_mut();
        let [old_archetype, new_archetype] = self //
            .archetypes
            .disjoint([old_location.archetype, destination_id])
            .unwrap();

        // Move entity entry from old archetype to new archetype
        let entity = old_archetype.entities.swap_remove(*old_location.row);
        new_archetype.entities.push(entity);

        // Move bytes from old columns to new columns
        old_archetype.signature.each_shared(&new_archetype.signature, |n, m| {
            let old_column = old_archetype.columns[n].get_mut();
            let new_column = new_archetype.columns[m].get_mut();
            old_column.move_into(new_column, old_location.row);
        });

        // Update entity locations
        let updated_location = EntityLocation {
            archetype: destination_id,
            row: RowIndex(new_archetype.entities.len() - 1),
        };
        entity_index[entity] = updated_location;
        if *old_location.row < old_archetype.entities.len() {
            entity_index[old_archetype.entities[*old_location.row]].row = old_location.row;
        }

        // Drop any unmoved bytes
        for column in old_archetype.columns.iter() {
            column.write().shrink_to_fit(old_archetype.entities.len());
        }

        updated_location
    }

    fn connect_edges(&mut self, signature: Signature, id: ArchetypeId) {
        // Iter adjacent archetypes & connect
        for field in signature.iter() {
            let without_field = signature.clone().without(*field);
            let Some(other) = self.signature_index.get(&without_field).copied() else {
                continue;
            };

            // Connect this to other
            self.archetypes[id].edges.entry(*field).or_default().remove = Some(other);

            // Connect other to this
            self.archetypes[other].edges.entry(*field).or_default().add = Some(id);
        }
    }

    fn create_archetype(&mut self, signature: Signature) -> ArchetypeId {
        if let Some(id) = self.signature_index.get(&signature) {
            *id
        } else {
            let mut new_archetype = Archetype {
                signature: signature.clone(),
                entities: Default::default(),
                columns: Default::default(),
                edges: Default::default(),
            };

            // Crate columns & add type meta
            for field in signature.iter() {
                // TODO: Check for pairs
                let info = self.component_info(field.as_entity().unwrap()).unwrap();
                new_archetype.columns.push(RwLock::new(Column::new(info)));
            }

            // Create new archetype with signature
            let id = self.archetypes.insert(new_archetype);
            self.signature_index.insert(signature.clone(), id);

            // Populate field index with new archetype
            for (n, field) in signature.iter().enumerate() {
                self.field_index.entry(*field).or_default().insert(id, ColumnIndex(n));
            }

            // Add missing edge connections
            self.connect_edges(signature, id);

            id
        }
    }

    pub(crate) fn entity_location(&mut self, entity: Entity) -> Option<EntityLocation> {
        let entity_index = self.entity_index.get_mut();
        entity_index.get(entity).copied()
    }

    pub(crate) fn entity_location_locking(&self, entity: Entity) -> Option<EntityLocation> {
        let entity_index = self.entity_index.lock();
        entity_index.get(entity).copied()
    }

    fn get_component_info(
        entity_index: &SlotMap<Entity, EntityLocation>,
        field_index: &HashMap<FieldId, FieldLocations>,
        archetypes: &SlotMap<ArchetypeId, Archetype>,
        component: Entity,
    ) -> Option<ComponentInfo> {
        field_index
            .get(&ComponentInfo::id().into())
            .zip(entity_index.get_ignore_generation(component))
            .and_then(|(field_locations, component_location)| {
                let column = archetypes
                    .get(component_location.archetype)?
                    .columns
                    .get(**field_locations.get(&component_location.archetype)?)?
                    .read();
                let bytes = &column.get_chunk(component_location.row);
                let info = unsafe { std::ptr::read(bytes.as_ptr() as *const ComponentInfo) };
                Some(info)
            })
    }

    /// Get metadata of a component
    pub(crate) fn component_info(&mut self, component: Entity) -> Option<ComponentInfo> {
        let entity_index = self.entity_index.get_mut();
        let field_index = &self.field_index;
        let archetypes = &self.archetypes;
        Self::get_component_info(entity_index, field_index, archetypes, component)
    }

    pub(crate) fn component_info_locking(&self, component: Entity) -> Option<ComponentInfo> {
        let entity_index = self.entity_index.lock();
        let field_index = &self.field_index;
        let archetypes = &self.archetypes;
        Self::get_component_info(&entity_index, field_index, archetypes, component)
    }

    pub(crate) fn archetype_has(&self, field: FieldId, archetype: ArchetypeId) -> bool {
        self.field_index
            .get(&field)
            .is_some_and(|field_locations| field_locations.contains_key(&archetype))
    }

    /// Get a component from an entity as type erased bytes
    pub(crate) fn get_bytes<'a>(
        &'a self,
        field: FieldId,
        entity_location: EntityLocation,
    ) -> Option<MappedRwLockReadGuard<'a, [MaybeUninit<u8>]>> {
        self.field_index.get(&field).and_then(|field_locations| {
            let column = self
                .archetypes
                .get(entity_location.archetype)?
                .columns
                .get(**field_locations.get(&entity_location.archetype)?)?
                .read();
            Some(RwLockReadGuard::map(column, |column| {
                column.get_chunk(entity_location.row)
            }))
        })
    }

    /// Get a component from an entity as type erased bytes
    pub(crate) fn get_bytes_mut<'a>(
        &'a self,
        field: FieldId,
        entity_location: EntityLocation,
    ) -> Option<MappedRwLockWriteGuard<'a, [MaybeUninit<u8>]>> {
        self.field_index.get(&field).and_then(|field_locations| {
            let column = self
                .archetypes
                .get(entity_location.archetype)?
                .columns
                .get(**field_locations.get(&entity_location.archetype)?)?
                .write();
            Some(RwLockWriteGuard::map(column, |column| {
                column.get_chunk_mut(entity_location.row)
            }))
        })
    }

    pub(crate) fn create_uninitialized_entity(&self) -> Entity {
        let mut entity_index = self.entity_index.lock();
        entity_index.insert(EntityLocation::uninitialized())
    }

    pub(crate) fn initialize_entity_location(&mut self, entity: Entity) -> EntityLocation {
        let entity_index = self.entity_index.get_mut();
        let mut location = entity_index[entity];
        if location == EntityLocation::uninitialized() {
            let empty_archetype = &mut self.archetypes[ArchetypeId::empty_archetype()];
            location.row = RowIndex(empty_archetype.entities.len());
            empty_archetype.entities.push(entity);
            entity_index[entity] = location;
        }
        location
    }

    pub(crate) fn despawn(&mut self, entity: Entity) {
        if let Some(location) = self.entity_index.get_mut().remove(entity) {
            self.archetypes[location.archetype].drop(location.row);
        };
    }

    pub(crate) unsafe fn insert_bytes(
        &mut self,
        info: ComponentInfo,
        bytes: &[MaybeUninit<u8>],
        entity: Entity,
    ) -> EntityLocation {
        assert_eq!(info.size, bytes.len());
        let Some(current_location) = self.entity_location(entity) else {
            panic!("Entity does not exist");
        };
        let current_archetype = &self.archetypes[current_location.archetype];
        let entity = current_archetype.entities[*current_location.row];

        // Find destination archetype
        let destination = if current_archetype.signature.contains(info.id.into()) {
            current_location.archetype
        } else if let Some(edge) = current_archetype //
            .edges
            .get(&info.id.into())
            .and_then(|edge| edge.add)
        {
            edge
        } else {
            self.create_archetype(current_archetype.signature.clone().with(info.id.into()))
        };

        // SAFETY: New chunk is immediately created for entity
        unsafe { self.move_entity(current_location, destination) };

        // SAFETY:
        //  - component info should match column component info
        //  - chunk corresponding to row if we moved to a new archetype is created
        //  - write_into will call drop fn on old component value if we didn't move archetype
        let updated_location = self.entity_location(entity).unwrap();
        unsafe {
            let column = self.field_index[&info.id.into()][&updated_location.archetype];
            self.archetypes[destination] //
                .columns[*column]
                .get_mut()
                .write_into(updated_location.row, bytes);
        }
        updated_location
    }

    pub(crate) fn remove_field(&mut self, field: FieldId, entity: Entity) -> EntityLocation {
        let Some(current_location) = self.entity_location(entity) else {
            panic!("Entity does not exist");
        };
        let current_archetype = &self.archetypes[current_location.archetype];

        // Find destination
        let destination = if let Some(edge) = current_archetype //
            .edges
            .get(&field)
            .and_then(|edge| edge.remove)
        {
            edge
        } else {
            self.create_archetype(current_archetype.signature.clone().without(field))
        };

        // SAFETY: Should only ever drop components
        unsafe { self.move_entity(current_location, destination) }
    }
}
