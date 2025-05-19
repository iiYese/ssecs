use std::{
    collections::HashMap,
    mem::{MaybeUninit, align_of, forget, size_of},
    slice::from_raw_parts,
};

use derive_more::{Deref, DerefMut};
use parking_lot::{MappedRwLockReadGuard, Mutex, RwLock, RwLockReadGuard};
use slotmap::SlotMap;

use crate::{
    NonZstOrPanic,
    archetype::{
        Archetype, ArchetypeEdge, ArchetypeId, Column, ColumnIndex, FieldId, RowIndex, Signature,
    },
    component::{COMPONENT_ENTRIES, Component, ComponentInfo},
    entity::Entity,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct EntityLocation {
    archetype: ArchetypeId,
    row: RowIndex,
}

#[derive(Deref, DerefMut, Default, Debug)]
pub(crate) struct FieldLocations(HashMap<ArchetypeId, ColumnIndex>);

#[derive(Debug)]
pub struct World {
    // Add read_index: SlotMap<Entity, EntityLocation> (a copy of entity_index) if this is too slow
    entity_index: Mutex<SlotMap<Entity, EntityLocation>>,
    field_index: HashMap<FieldId, FieldLocations>,
    signature_index: HashMap<Signature, ArchetypeId>,
    archetypes: SlotMap<ArchetypeId, Archetype>,
}

impl World {
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
                assert_eq!(id, unsafe { Entity::from_offset(n as u64) });
                empty_archetype.entities.push(id);
            }
            // Add ComponentInfo edge
            let component_info_edge = &mut empty_archetype //
                .edges
                .entry(ComponentInfo::id().into())
                .or_default();
            component_info_edge.add = component_info_archetype_id;
        }

        // Mangually create ComponentInfo archetype
        let component_info_signature = Signature::new(&[ComponentInfo::id().into()]);
        archetypes[component_info_archetype_id] = Archetype {
            signature: component_info_signature.clone(),
            entities: Default::default(),
            columns: vec![RwLock::new(Column::new(
                align_of::<ComponentInfo>(),
                size_of::<ComponentInfo>(),
            ))],
            edges: HashMap::from([(
                ComponentInfo::id().into(),
                ArchetypeEdge {
                    remove: empty_archetype_id,
                    add: ArchetypeId::null(),
                },
            )]),
        };

        // Make world
        let mut world = Self {
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
        let old_location = entity_index[entity];
        if old_location.archetype == destination_id {
            return;
        }
        let [old_archetype, new_archetype] = self //
            .archetypes
            .get_disjoint_mut([old_location.archetype, destination_id])
            .unwrap();

        // Move bytes from old columns to new columns
        old_archetype.signature.each_shared(&new_archetype.signature, |n, m| {
            let mut old_column = old_archetype.columns[n].write();
            let mut new_column = new_archetype.columns[m].write();
            old_column.move_into(&mut new_column, old_location.row);
        });

        // Move entity entry from old archetype to new archetype
        old_archetype.entities.swap_remove(*old_location.row);
        new_archetype.entities.push(entity);

        // Update entity locations
        entity_index[entity] = EntityLocation {
            archetype: destination_id,
            row: RowIndex(new_archetype.entities.len() - 1),
        };
        if *old_location.row < old_archetype.entities.len() {
            entity_index[old_archetype.entities[*old_location.row]].row = old_location.row;
        }

        // Drop any unmoved bytes
        for column in old_archetype.columns.iter() {
            // TODO: call drop fns
            column.write().truncate(old_archetype.entities.len());
        }

        // Zero init any columns that didn't have a value moved into them
        for column in new_archetype.columns.iter() {
            column.write().zero_fill(new_archetype.entities.len());
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
                let info = self.component_info(field.as_entity().unwrap()).unwrap();
                new_archetype.columns.push(RwLock::new(Column::new(info.align, info.size)));
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
                let info = unsafe { std::ptr::read(bytes.as_ptr() as *const ComponentInfo) };
                Some(info)
            })
    }

    /// Get a component from an entity as type erased bytes
    pub fn get_bytes(
        &self,
        field: FieldId,
        entity: Entity,
    ) -> Option<MappedRwLockReadGuard<[MaybeUninit<u8>]>> {
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
        let _ = T::NON_ZST_OR_PANIC;
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
            row: RowIndex(empty_archetype.entities.len()),
        });
        empty_archetype.entities.push(new_entity);
        new_entity
    }

    pub unsafe fn set_bytes(
        &mut self,
        info: ComponentInfo,
        bytes: &[MaybeUninit<u8>],
        entity: Entity,
    ) {
        let Some(current_location) = self
            .entity_location(entity)
            .filter(|location| location.archetype != ArchetypeId::null())
        else {
            panic!("Entity does not exist");
        };
        let current_signature = self //
            .archetypes[current_location.archetype]
            .signature
            .clone();
        let archetype_id = if current_signature.contains(info.id.into()) {
            current_location.archetype
        } else if let Some(edge) = self
            .archetypes
            .get(current_location.archetype)
            .and_then(|archetype| archetype.edges.get(&info.id.into()))
            .map(|edge| edge.add)
            .filter(|archetype| *archetype != ArchetypeId::null())
        {
            // SAFETY: Columns are filled at end of call
            unsafe { self.move_entity(entity, edge) };
            edge
        } else {
            let new_archetyep_id = self.create_archetype(current_signature.with(info.id.into()));
            // SAFETY: Columns are filled at end of call
            unsafe { self.move_entity(entity, new_archetyep_id) };
            new_archetyep_id
        };

        // Set zero'd bytes
        let updated_location = self.entity_location(entity).unwrap();
        let column = self.field_index[&info.id.into()][&updated_location.archetype];
        let chunk = unsafe { from_raw_parts(bytes.as_ptr(), info.size) };
        self.archetypes[archetype_id] //
            .columns[*column]
            .write()
            .insert_chunk(updated_location.row, chunk);
    }

    pub fn set_component<C: Component>(&mut self, component: C, entity: Entity) {
        unsafe {
            let bytes = from_raw_parts(
                (&component as *const C) as *const MaybeUninit<u8>,
                size_of::<C>(),
            );
            self.set_bytes(C::info(), bytes, entity);
        }
        forget(component);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as ssecs;
    use crate::component::tests::*;
    use ssecs_macros::*;

    #[derive(Component)]
    struct Message(&'static str);

    #[test]
    fn component_info() {
        let world = World::new();
        assert_eq!(world.component_info(Player::id()), Some(Player::info()));
        assert_eq!(world.component_info(Health::id()), Some(Health::info()));
        assert_eq!(world.component_info(Message::id()), Some(Message::info()));
    }

    #[test]
    fn zsts() {
        let mut world = World::new();
        let e = world.new_entity();
        world.set_component(Player, e);
        assert!(world.has_component(Player::id(), e));
    }

    #[test]
    fn hello_world() {
        let mut world = World::new();
        let a = world.new_entity();
        let b = world.new_entity();
        world.set_component(Message("Hello"), a);
        world.set_component(Message("World"), b);
        assert!(world.has_component(Message::id(), a));
        assert!(world.has_component(Message::id(), b));
        assert_eq!("Hello", world.get::<Message>(a).unwrap().0);
        assert_eq!("World", world.get::<Message>(b).unwrap().0);
    }
}
