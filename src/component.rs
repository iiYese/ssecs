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
    fn default() -> &'static [MaybeUninit<u8>] {
        struct DefaultOrPanic<T>(PhantomData<T>);
        trait NoDefault<T> {
            fn default() -> T;
        }

        #[allow(dead_code)]
        impl<T: Default> DefaultOrPanic<T> {
            fn default() -> T {
                T::default()
            }
        }

        impl<T> NoDefault<T> for DefaultOrPanic<T> {
            fn default() -> T {
                panic!("Type does not implement Default");
            }
        }

        let leaked = ManuallyDrop::new(DefaultOrPanic::<Self>::default());
        unsafe { std::slice::from_raw_parts((&raw const leaked).cast(), size_of::<Self>()) }
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
