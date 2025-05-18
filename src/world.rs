use std::{collections::HashMap, mem::size_of};

use derive_more::{Deref, DerefMut};
use parking_lot::{MappedRwLockReadGuard, Mutex, RwLock, RwLockReadGuard};
use slotmap::SlotMap;

use crate::{
    archetype::{Archetype, ArchetypeEdge, ArchetypeId, Column, FieldId, Signature},
    component::{COMPONENT_ENTRIES, Component, ComponentInfo},
    entity::Entity,
};

pub struct World {
    // Add read_index: SlotMap<Entity, EntityLocation> (a copy of entity_index) if this is too slow
    entity_index: Mutex<SlotMap<Entity, EntityLocation>>,
    field_index: HashMap<FieldId, FieldLocations>,
    signature_index: HashMap<Signature, ArchetypeId>,
    archetypes: SlotMap<ArchetypeId, Archetype>,
}

impl World {
    pub fn new() -> Self {
        // Add empty archetype
        let mut archetypes = SlotMap::<ArchetypeId, Archetype>::with_key();
        let mut entity_index = SlotMap::<Entity, EntityLocation>::with_key();
        let empty_archetype_id = archetypes.insert(Archetype::default());
        assert_eq!(empty_archetype_id, ArchetypeId::empty_archetype());

        // Make sure all component entities are sawned before init
        // Needed if components add relationships (traits)
        if let Some(empty_archetype) = archetypes.get_mut(empty_archetype_id) {
            for n in 0..COMPONENT_ENTRIES.len() {
                let id = entity_index.insert(EntityLocation {
                    archetype: empty_archetype_id,
                    row: n,
                });
                assert_eq!(id, unsafe { Entity::from_offset(n as u64) });
                empty_archetype.entities.push(id);
            }
        }

        // Mangually create ComponentInfo archetype
        let component_info_signature = Signature::new(&[ComponentInfo::id().into()]);
        let component_info_archetype_id = archetypes.insert(Archetype {
            signature: component_info_signature.clone(),
            entities: Default::default(),
            columns: vec![RwLock::new(Column::new(size_of::<ComponentInfo>()))],
            edges: HashMap::from([(
                ComponentInfo::id().into(),
                ArchetypeEdge {
                    remove: empty_archetype_id,
                    add: ArchetypeId::null(),
                },
            )]),
        });

        // Make world
        let mut world = Self {
            archetypes,
            entity_index: Mutex::new(entity_index),
            field_index: Default::default(),
            signature_index: HashMap::from([
                (Signature::default(), empty_archetype_id),
                (component_info_signature, component_info_archetype_id),
            ]),
        };

        // Run component init
        for init in COMPONENT_ENTRIES {
            init(&mut world);
        }

        world
    }

    fn connect_edges(&mut self, signature: Signature, id: ArchetypeId) {
        // Iter adjacent archetypes & connect
        for field in signature.iter() {
            let without_field = signature.clone().without(*field);
            let Some(other) = self.signature_index.get(&without_field).copied() else {
                continue;
            };

            // Connect this to other
            self.archetypes[id].edges.entry(*field).or_default().remove = other;

            // Connect other to this
            self.archetypes[other].edges.entry(*field).or_default().add = id;
        }
    }

    /// Must ensure new columns have placeholder zero bytes written into with valid bytes
    unsafe fn move_entity(&mut self, entity: Entity, destination_id: ArchetypeId) {
        let mut entity_index = self.entity_index.lock();
        let location = &mut entity_index[entity];
        if location.archetype == destination_id {
            return;
        }
        let [old, new] = self //
            .archetypes
            .get_disjoint_mut([location.archetype, destination_id])
            .unwrap();

        // Move bytes from old columns to new columns
        old.signature.each_shared(&new.signature, |n, m| {
            let mut old_column = old.columns[n].write();
            let mut new_column = new.columns[m].write();
            new_column.extend_from_drained(old_column.remove_chunk(location.row));
        });

        // Move entity entry from old archetype to new archetype
        location.archetype = destination_id;
        old.entities.remove(location.row); // TODO: use indices to optimize
        location.row = new.entities.len();
        new.entities.push(entity);

        // Drop any unmoved bytes
        for column in old.columns.iter() {
            column.write().truncate(old.entities.len());
        }

        // Zero init any columns that didn't have a value moved into them
        for column in new.columns.iter() {
            column.write().zero_fill(new.entities.len());
        }
    }

    pub(crate) fn create_archetype(&mut self, signature: Signature) -> ArchetypeId {
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
                new_archetype.columns.push(RwLock::new(Column::new(
                    self.component_info(field.as_entity().unwrap()).unwrap().size,
                )))
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

    pub(crate) fn entity_location(&self, entity: Entity) -> Option<EntityLocation> {
        let entity_index = self.entity_index.lock();
        entity_index.get(entity).copied()
    }

    pub fn has_component(&self, component: Entity, entity: Entity) -> bool {
        self.entity_location(entity) //
            .zip(self.field_index.get(&component.into()))
            .is_some_and(|(entity_location, field_locations)| {
                field_locations.contains_key(&entity_location.archetype)
            })
    }

    /// Get metadata of a component
    pub fn component_info(&self, component: Entity) -> Option<ComponentInfo> {
        let entity_index = self.entity_index.lock();
        self.field_index
            .get(&ComponentInfo::id().into())
            .zip(entity_index.get_ignore_gen(component))
            .and_then(|(field_locations, component_location)| {
                let column = self
                    .archetypes
                    .get(component_location.archetype)?
                    .columns
                    .get(**field_locations.get(&component_location.archetype)?)?
                    .read();
                let bytes = &column.get_chunk(component_location.row);
                Some(unsafe { std::ptr::read(bytes.as_ptr() as *const ComponentInfo) })
            })
    }

    /// Get a component from an entity as type erased bytes
    pub fn get_bytes(&self, field: FieldId, entity: Entity) -> Option<MappedRwLockReadGuard<[u8]>> {
        self.entity_location(entity) //
            .zip(self.field_index.get(&field))
            .and_then(|(entity_location, field_locations)| {
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

    pub fn get<T: Component>(&self, entity: Entity) -> Option<MappedRwLockReadGuard<T>> {
        self.get_bytes(T::id().into(), entity).map(|bytes| {
            MappedRwLockReadGuard::map(bytes, |bytes| {
                // SAFETY: Don't need to check TypeId because component's Entity id acts as TypeId
                unsafe { (bytes.as_ptr() as *const T).as_ref() }.unwrap()
            })
        })
    }
}

// TODO: make everything pub(crate) & Replace with &self versions that enqueue commands
impl World {
    // TODO: Track entities temporarily & put them in the empty archetype before command flushes
    pub fn new_entity(&mut self) -> Entity {
        let mut entity_index = self.entity_index.lock();
        let empty_archetype = &mut self.archetypes[ArchetypeId::empty_archetype()];
        let new_entity = entity_index.insert(EntityLocation {
            archetype: ArchetypeId::empty_archetype(),
            row: empty_archetype.entities.len(),
        });
        empty_archetype.entities.push(new_entity);
        new_entity
    }

    pub fn set_component<C: Component>(&mut self, component: C, entity: Entity) {
        let Some(entity_location) = self
            .entity_location(entity)
            .filter(|location| location.archetype != ArchetypeId::null())
        else {
            panic!("Entity does not exist");
        };
        let current_signature = self //
            .archetypes[entity_location.archetype]
            .signature
            .clone();
        let archetype_id = if current_signature.contains(C::id().into()) {
            entity_location.archetype
        } else if let Some(edge) = self
            .archetypes
            .get(entity_location.archetype)
            .and_then(|archetype| archetype.edges.get(&C::id().into()))
            .map(|edge| edge.add)
            .filter(|archetype| *archetype != ArchetypeId::null())
        {
            // SAFETY: Columns are filled at end of call
            unsafe { self.move_entity(entity, edge) };
            edge
        } else {
            let new_archetyep_id = self.create_archetype(current_signature.with(C::id().into()));
            // SAFETY: Columns are filled at end of call
            unsafe { self.move_entity(entity, new_archetyep_id) };
            new_archetyep_id
        };

        // Set zero'd bytes
        let entity_location = self.entity_location(entity).unwrap();
        let column = self.field_index[&C::id().into()][&entity_location.archetype];
        self.archetypes[archetype_id] //
            .columns[*column]
            .write()
            .push_chunk(component.as_bytes());
    }
}

#[derive(Clone, Copy)]
pub(crate) struct EntityLocation {
    archetype: ArchetypeId,
    row: usize,
}

#[derive(Deref, DerefMut, Default)]
pub(crate) struct FieldLocations(HashMap<ArchetypeId, ColumnIndex>);

#[derive(Deref, DerefMut, Clone, Copy)]
pub(crate) struct ColumnIndex(pub usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_init() {
        dbg!(ArchetypeId::default());
        dbg!(ArchetypeId::null());
        dbg!(u32::MAX);
        World::new();
    }
}
