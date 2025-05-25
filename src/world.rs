use std::{
    collections::HashMap,
    mem::{MaybeUninit, forget, size_of},
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
    /// `location.archetype` should never be `Archetype::null()`
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

    /// Must ensure missing entries in columns for entity are filled
    unsafe fn move_entity(&mut self, old_location: EntityLocation, destination_id: ArchetypeId) {
        if old_location.archetype == destination_id {
            return;
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
        entity_index[entity] = EntityLocation {
            archetype: destination_id,
            row: RowIndex(new_archetype.entities.len() - 1),
        };
        if *old_location.row < old_archetype.entities.len() {
            entity_index[old_archetype.entities[*old_location.row]].row = old_location.row;
        }

        // Drop any unmoved bytes
        for column in old_archetype.columns.iter() {
            column.write().shrink_to_fit(old_archetype.entities.len());
        }
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
                let info = self.component_info_non_locking(field.as_entity().unwrap()).unwrap();
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

    fn entity_location(&self, entity: Entity) -> Option<EntityLocation> {
        let entity_index = self.entity_index.lock();
        entity_index.get(entity).copied()
    }

    fn entity_location_non_locking(&mut self, entity: Entity) -> Option<EntityLocation> {
        let entity_index = self.entity_index.get_mut();
        entity_index.get(entity).copied()
    }

    pub fn has_component(&self, component: Entity, entity: Entity) -> bool {
        self.entity_location(entity) //
            .zip(self.field_index.get(&component.into()))
            .is_some_and(|(entity_location, field_locations)| {
                field_locations.contains_key(&entity_location.archetype)
            })
    }

    fn component_info_non_locking(&mut self, component: Entity) -> Option<ComponentInfo> {
        let entity_index = self.entity_index.get_mut();
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
        let entity_index = self.entity_index.get_mut();
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
        let Some(current_location) = self.entity_location_non_locking(entity) else {
            panic!("Entity does not exist");
        };
        assert_eq!(info.size, bytes.len());
        let current_archetype = &self.archetypes[current_location.archetype];

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
        unsafe {
            let updated_location = self.entity_location_non_locking(entity).unwrap();
            let column = self.field_index[&info.id.into()][&updated_location.archetype];
            self.archetypes[destination] //
                .columns[*column]
                .write()
                .write_into(updated_location.row, bytes);
        }
    }

    pub fn set_component<C: Component>(&mut self, component: C, entity: Entity) {
        // SAFETY: This is always safe because we are providing static type info
        unsafe {
            let bytes = from_raw_parts(
                (&component as *const C) as *const MaybeUninit<u8>,
                size_of::<C>(),
            );
            self.set_bytes(C::info(), bytes, entity);
        }
        forget(component);
    }

    pub fn remove_field(&mut self, field: FieldId, entity: Entity) {
        let Some(current_location) = self.entity_location_non_locking(entity) else {
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
        unsafe {
            self.move_entity(current_location, destination);
        }
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        self.remove_field(C::id().into(), entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as ssecs;
    use crate::component::tests::*;
    use ssecs_macros::*;
    use std::sync::Arc;

    #[derive(Component)]
    #[allow(dead_code)]
    pub struct RefCounted(Arc<u8>);

    #[derive(Component)]
    struct Foo(u8);

    #[derive(Component)]
    struct Bar(u8);

    #[test]
    fn component_info() {
        let world = World::new();
        for info in [
            ComponentInfo::info(),
            Player::info(),
            Health::info(),
            Transform::info(),
            Foo::info(),
            Bar::info(),
        ] {
            assert_eq!(world.component_info(info.id), Some(info));
        }
    }

    #[test]
    fn zsts() {
        let mut world = World::new();
        let e = world.new_entity();
        world.set_component(Player, e);
        assert_eq!(true, world.has_component(Player::id(), e));
        world.remove_component::<Player>(e);
        assert_eq!(false, world.has_component(Player::id(), e));
    }

    #[test]
    fn set_remove() {
        let mut world = World::new();
        let e = world.new_entity();
        world.set_component(Foo(0), e);
        assert_eq!(true, world.has_component(Foo::id(), e));
        assert_eq!(0, world.get::<Foo>(e).unwrap().0);

        world.set_component(Bar(1), e);
        assert_eq!(true, world.has_component(Foo::id(), e));
        assert_eq!(0, world.get::<Foo>(e).unwrap().0);
        assert_eq!(true, world.has_component(Bar::id(), e));
        assert_eq!(1, world.get::<Bar>(e).unwrap().0);

        world.remove_component::<Foo>(e);
        assert_eq!(false, world.has_component(Foo::id(), e));
        assert!(world.get::<Foo>(e).is_none());
        assert_eq!(true, world.has_component(Bar::id(), e));
        assert_eq!(1, world.get::<Bar>(e).unwrap().0);
    }

    #[test]
    fn drop() {
        let val = Arc::new(0_u8);
        let mut world = World::new();
        let e = world.new_entity();
        world.set_component(RefCounted(val.clone()), e);
        assert_eq!(2, Arc::strong_count(&val));
        assert_eq!(true, world.has_component(RefCounted::id(), e));
        world.remove_component::<RefCounted>(e);
        assert_eq!(false, world.has_component(RefCounted::id(), e));
        assert_eq!(1, Arc::strong_count(&val));
    }
}
