use std::{cell::Cell, sync::Arc};

use thread_local::ThreadLocal;

use crate::world::{command::Command, core::Core};

#[derive(Clone)]
pub(crate) struct Mantle {
    pub(crate) core: Arc<Core>,
    pub(crate) commands: Arc<ThreadLocal<Cell<Vec<Command>>>>,
}
