use std::{
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
};

use crate::{self as ssecs, entity::Entity, entity::View, world::World};
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
        struct Getter<T>(PhantomData<T>);
        impl<T: Clone> Getter<T> {
            #[allow(dead_code)]
            fn get() -> Option<unsafe fn(&[MaybeUninit<u8>]) -> &'static [MaybeUninit<u8>]> {
                Some(|bytes| unsafe {
                    let t = (bytes.as_ptr() as *const T).as_ref().unwrap();
                    let leaked = ManuallyDrop::new(t.clone());
                    std::slice::from_raw_parts((&raw const leaked).cast(), size_of::<Self>())
                })
            }
        }
        trait NoImpl<T> {
            fn get() -> Option<unsafe fn(&[MaybeUninit<u8>]) -> &'static [MaybeUninit<u8>]> {
                None
            }
        }
        impl<T> NoImpl<T> for Getter<T> {}
        Getter::<Self>::get()
    }

    fn get_erased_default() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
        struct Getter<T>(PhantomData<T>);
        impl<T: Default> Getter<T> {
            #[allow(dead_code)]
            fn get() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
                Some(|| {
                    let leaked = ManuallyDrop::new(T::default());
                    unsafe {
                        std::slice::from_raw_parts((&raw const leaked).cast(), size_of::<Self>())
                    }
                })
            }
        }
        trait NoImpl<T> {
            fn get() -> Option<fn() -> &'static [MaybeUninit<u8>]> {
                None
            }
        }
        impl<T> NoImpl<T> for Getter<T> {}
        Getter::<Self>::get()
    }

    #[allow(clippy::missing_safety_doc)]
    unsafe fn erased_drop(bytes: &mut [std::mem::MaybeUninit<u8>]) {
        unsafe { (bytes.as_ptr() as *mut Self).drop_in_place() }
    }

    fn get_on_insert() -> Option<fn(View<'_>)> {
        struct Getter<T>(PhantomData<T>);
        impl<T: OnInsert> Getter<T> {
            #[allow(dead_code)]
            fn get() -> Option<fn(View<'_>)> {
                Some(T::on_insert)
            }
        }
        trait NoImpl<T> {
            fn get() -> Option<fn(View<'_>)> {
                None
            }
        }
        impl<T> NoImpl<T> for Getter<T> {}
        Getter::<Self>::get()
    }

    fn get_on_remove() -> Option<fn(View<'_>)> {
        struct Getter<T>(PhantomData<T>);
        impl<T: OnRemove> Getter<T> {
            #[allow(dead_code)]
            fn get() -> Option<fn(View<'_>)> {
                Some(T::on_remove)
            }
        }
        trait NoImpl<T> {
            fn get() -> Option<fn(View<'_>)> {
                None
            }
        }
        impl<T> NoImpl<T> for Getter<T> {}
        Getter::<Self>::get()
    }
}

pub trait OnInsert {
    fn on_insert(entity: View<'_>);
}

pub trait OnRemove {
    fn on_remove(entity: View<'_>);
}

#[derive(Clone, Copy, Component, Debug)]
pub struct ComponentInfo {
    pub name: &'static str,
    pub align: usize,
    pub size: usize,
    pub id: Entity,
    pub clone: Option<unsafe fn(&[MaybeUninit<u8>]) -> &'static [MaybeUninit<u8>]>,
    pub default: Option<fn() -> &'static [MaybeUninit<u8>]>,
    pub drop: unsafe fn(&mut [MaybeUninit<u8>]),
    pub on_insert: Option<fn(View<'_>)>,
    pub on_remove: Option<fn(View<'_>)>,
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
