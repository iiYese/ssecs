use std::{
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
};

use crate::{self as ssecs, entity::Entity, world::World};
use ssecs_macros::*;

pub type ComponentEntry = fn(world: &World);

#[linkme::distributed_slice]
pub static COMPONENT_ENTRIES: [ComponentEntry];

/// # Safety
/// Should never be implemented manually
pub unsafe trait Component: Sized {
    fn id() -> Entity;
    fn init(_: &World);
    fn info() -> ComponentInfo;

    fn get_erased_clone() -> Option<unsafe fn(&[MaybeUninit<u8>]) -> &'static [MaybeUninit<u8>]> {
        struct CloneGetter<T>(PhantomData<T>);
        impl<T: Clone> CloneGetter<T> {
            #[allow(dead_code)]
            fn get_clone() -> Option<unsafe fn(&[MaybeUninit<u8>]) -> &'static [MaybeUninit<u8>]> {
                Some(|bytes| unsafe {
                    let t = (bytes.as_ptr() as *const T).as_ref().unwrap();
                    let leaked = ManuallyDrop::new(t.clone());
                    std::slice::from_raw_parts((&raw const leaked).cast(), size_of::<Self>())
                })
            }
        }
        trait NoClone<T> {
            fn get_clone() -> Option<unsafe fn(&[MaybeUninit<u8>]) -> &'static [MaybeUninit<u8>]> {
                None
            }
        }
        impl<T> NoClone<T> for CloneGetter<T> {}
        CloneGetter::<Self>::get_clone()
    }

    fn get_erased_default() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
        struct DefaultGetter<T>(PhantomData<T>);
        impl<T: Default> DefaultGetter<T> {
            #[allow(dead_code)]
            fn get_default() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
                Some(|| {
                    let leaked = ManuallyDrop::new(T::default());
                    unsafe {
                        std::slice::from_raw_parts((&raw const leaked).cast(), size_of::<Self>())
                    }
                })
            }
        }
        trait NoDefault<T> {
            fn get_default() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
                None
            }
        }
        impl<T> NoDefault<T> for DefaultGetter<T> {}
        DefaultGetter::<Self>::get_default()
    }

    #[allow(clippy::missing_safety_doc)]
    unsafe fn erased_drop(bytes: &mut [std::mem::MaybeUninit<u8>]) {
        unsafe { (bytes.as_ptr() as *mut Self).drop_in_place() }
    }
}

#[derive(Clone, Copy, Component, Debug, PartialEq, Eq)]
pub struct ComponentInfo {
    pub(crate) name: &'static str,
    pub(crate) align: usize,
    pub(crate) size: usize,
    pub(crate) id: Entity,
    pub(crate) clone: Option<unsafe fn(&[MaybeUninit<u8>]) -> &'static [MaybeUninit<u8>]>,
    pub(crate) default: Option<fn() -> &'static [MaybeUninit<u8>]>,
    pub(crate) drop: unsafe fn(&mut [MaybeUninit<u8>]),
}

impl ComponentInfo {
    pub unsafe fn new(
        name: &'static str,
        align: usize,
        size: usize,
        id: Entity,
        clone: Option<unsafe fn(&[MaybeUninit<u8>]) -> &'static [MaybeUninit<u8>]>,
        default: Option<fn() -> &'static [MaybeUninit<u8>]>,
        drop: unsafe fn(&mut [MaybeUninit<u8>]),
    ) -> Self {
        Self { name, align, size, id, clone, default, drop }
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
