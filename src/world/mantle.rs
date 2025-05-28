use std::{cell::Cell, sync::Arc};

use thread_local::ThreadLocal;

use crate::world::{command::Command, core::Core};

#[derive(Clone)]
pub(crate) struct Mantle {
    pub(crate) core: Arc<Core>,
    pub(crate) commands: Arc<ThreadLocal<Cell<Vec<Command>>>>,
}

impl Mantle {
    pub(crate) fn enqueue(&self, command: Command) {
        let cell = self.commands.get_or(|| Cell::new(Vec::default()));
        let mut queue = cell.take();
        queue.push(command);
        cell.set(queue);
    }
}
