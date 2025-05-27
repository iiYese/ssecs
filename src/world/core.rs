use std::{
    collections::HashMap,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

use derive_more::{Deref, DerefMut};
use parking_lot::{Mutex, RwLock, RwLockReadGuard};
use slotmap::SlotMap;
use thread_local::ThreadLocal;

use crate::{
    archetype::{
        Archetype, ArchetypeEdge, ArchetypeId, Column, ColumnIndex, ColumnReadGuard, FieldId,
        RowIndex, Signature,
    },
    component::{COMPONENT_ENTRIES, Component, ComponentInfo},
    entity::Entity,
    world::command::Command,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct EntityLocation {
    archetype: ArchetypeId,
    row: RowIndex,
}

#[derive(Deref, DerefMut, Default, Debug)]
pub(crate) struct FieldLocations(HashMap<ArchetypeId, ColumnIndex>);

#[derive(Debug)]
pub(crate) struct Core {
    // Add read_index: SlotMap<Entity, EntityLocation> (a copy of entity_index) if this is too slow
    entity_index: Mutex<SlotMap<Entity, EntityLocation>>,
    field_index: HashMap<FieldId, FieldLocations>,
    signature_index: HashMap<Signature, ArchetypeId>,
    archetypes: SlotMap<ArchetypeId, Archetype>,
    commands: ThreadLocal<Command>,
    // No. of queries referencing core
    ref_count: AtomicUsize,
}

impl Core {
    pub fn new() -> Self {
        // Add empty archetype & component info archetype
        let mut archetypes = SlotMap::<ArchetypeId, Archetype>::with_key();
        let mut entity_index = SlotMap::<Entity, EntityLocation>::with_key();
        let empty_archetype_id = archetypes.insert(Archetype::default());
        let component_info_archetype_id = archetypes.insert(Archetype::default());
        assert_eq!(empty_archetype_id, ArchetypeId::empty_archetype());

        if let Some(empty_archetype) = archetypes.get_mut(empty_archetype_id) {
            // Make sure all component entities are sawned before init
            // Needed if components add relationships (traits)
            for n in 0..COMPONENT_ENTRIES.len() {
                let id = entity_index.insert(EntityLocation {
                    archetype: empty_archetype_id,
                    row: RowIndex(n),
                });
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
                ArchetypeEdge {
                    remove: Some(empty_archetype_id),
                    add: None,
                },
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
            commands: ThreadLocal::default(),
            ref_count: AtomicUsize::new(0),
        }
    }

    pub(crate) fn incr_ref_count(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn decr_ref_count(&self) {
        self.ref_count.fetch_sub(1, Ordering::Relaxed);
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
            .get_disjoint_mut([old_location.archetype, destination_id])
            .unwrap();

        // Move entity entry from old archetype to new archetype
        let entity = old_archetype.entities.swap_remove(*old_location.row);
        new_archetype.entities.push(entity);

        // Move bytes from old columns to new columns
        old_archetype.signature.each_shared(&new_archetype.signature, |n, m| {
            let mut old_column = old_archetype.columns[n].write();
            let mut new_column = new_archetype.columns[m].write();
            old_column.move_into(&mut new_column, old_location.row);
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
            .zip(entity_index.get_ignore_gen(component))
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

    pub(crate) fn has_component(&self, component: Entity, entity: Entity) -> bool {
        self.entity_location_locking(entity) //
            .zip(self.field_index.get(&component.into()))
            .is_some_and(|(entity_location, field_locations)| {
                field_locations.contains_key(&entity_location.archetype)
            })
    }

    /// Get a component from an entity as type erased bytes
    pub(crate) fn get_bytes(
        &self,
        field: FieldId,
        entity_location: EntityLocation,
    ) -> Option<ColumnReadGuard<[MaybeUninit<u8>]>> {
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

    // TODO: Track entities temporarily & put them in the empty archetype before command flushes
    pub(crate) fn new_entity(&mut self) -> (Entity, EntityLocation) {
        let entity_index = self.entity_index.get_mut();
        let empty_archetype = &mut self.archetypes[ArchetypeId::empty_archetype()];
        let entity_location = EntityLocation {
            archetype: ArchetypeId::empty_archetype(),
            row: RowIndex(empty_archetype.entities.len()),
        };
        let entity_id = entity_index.insert(entity_location);
        empty_archetype.entities.push(entity_id);
        (entity_id, entity_location)
    }

    pub(crate) unsafe fn set_bytes(
        &mut self,
        info: ComponentInfo,
        bytes: &[MaybeUninit<u8>],
        current_location: EntityLocation,
    ) -> EntityLocation {
        assert_eq!(info.size, bytes.len());
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
                .write()
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
