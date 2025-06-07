use std::{
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
};

use linkme;

use crate::{self as ssecs, entity::Entity, world::World};
use ssecs_macros::*;

pub type ComponentEntry = fn(world: &World);

#[linkme::distributed_slice]
pub static COMPONENT_ENTRIES: [ComponentEntry];

/// Should never be implemented manually
pub unsafe trait Component: Sized {
    fn id() -> Entity;
    fn init(_: &World);
    fn info() -> ComponentInfo;
    fn drop(bytes: &mut [MaybeUninit<u8>]);
    fn default() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
        struct DefaultGetter<T>(PhantomData<T>);

        trait NoDefault<T> {
            fn get_default() -> Option<fn() -> &'static [MaybeUninit<u8>]>;
        }

        #[allow(dead_code)]
        impl<T: Default> DefaultGetter<T> {
            fn get_default() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
                Some(|| {
                    let leaked = ManuallyDrop::new(T::default());
                    unsafe {
                        std::slice::from_raw_parts((&raw const leaked).cast(), size_of::<Self>())
                    }
                })
            }
        }

        impl<T> NoDefault<T> for DefaultGetter<T> {
            fn get_default() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
                None
            }
        }

        DefaultGetter::<Self>::get_default()
    }
}

#[derive(Clone, Copy, Component, Debug, PartialEq, Eq)]
pub struct ComponentInfo {
    pub(crate) name: &'static str,
    pub(crate) align: usize,
    pub(crate) size: usize,
    pub(crate) id: Entity,
    pub(crate) drop: fn(&mut [MaybeUninit<u8>]),
}

impl ComponentInfo {
    pub unsafe fn new(
        name: &'static str,
        align: usize,
        size: usize,
        id: Entity,
        drop: fn(&mut [MaybeUninit<u8>]),
    ) -> Self {
        Self { name, align, size, id, drop }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[derive(Component)]
    pub struct Player;

    #[derive(Component)]
    pub struct Transform;

    #[derive(Component)]
    pub struct Health;

    #[test]
    fn component_ids() {
        assert!(Player::id() != Transform::id());
        assert!(Transform::id() != Health::id());
        assert!(Health::id() != Player::id());
    }
}
