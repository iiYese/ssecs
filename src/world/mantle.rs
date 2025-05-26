use std::sync::atomic::AtomicBool;

use crate::world::core::Core;

pub(crate) struct Mantle {
    pub(crate) core: Core,
    pub(crate) is_deferred: AtomicBool,
}

impl Mantle {
    pub(crate) fn new() -> Self {
        Mantle {
            core: Core::new(),
            is_deferred: AtomicBool::default(),
        }
    }
}
