#![allow(clippy::type_complexity)]
#![allow(clippy::let_unit_value)]

pub mod component;
pub mod entity;
pub mod query;
mod slotmap;
pub mod world;

trait NonZstOrPanic: Sized {
    #[allow(missing_docs)]
    const NON_ZST_OR_PANIC: () = {
        if std::mem::size_of::<Self>() == 0 {
            panic!("ZSTs are not allowed for this API");
        }
    };
}

impl<T> NonZstOrPanic for T {}

pub mod prelude {
    pub use crate::component::Component;
    pub use crate::entity::Entity;
    pub use crate::query::Query;
    pub use crate::world::World;
}
