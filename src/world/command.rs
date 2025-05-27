use std::{
    mem::MaybeUninit,
    sync::{Arc, atomic::AtomicBool},
};

use crate::{
    component::{Component, ComponentInfo},
    entity::Entity,
    world::core::Core,
};

#[derive(Debug)]
pub(crate) struct Command {
    op: Operation,
    info: ComponentInfo,
    bytes: *const MaybeUninit<u8>,
    target: Entity,
    jump: usize,
}

impl Command {
    fn apply(self, core: &mut Core) {
        todo!()
    }
}

unsafe impl Send for Command {}

impl Default for Command {
    fn default() -> Self {
        Self {
            op: Operation::Noop,
            info: ComponentInfo::info(),
            bytes: std::ptr::null(),
            target: Entity::null(),
            jump: 1,
        }
    }
}

// TOD: Batching
// - Despawn is last: Ignore all other ops on entity
// - Inserrt/Remove is last: Ignore all other inserts for component
// - Iter reverse for less work
#[derive(Debug)]
enum Operation {
    Noop,
    Insert,
    Remove,
    Spawn,
    Despawn,
}
