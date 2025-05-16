mod archetype;
mod component;
mod entity;
mod meta_tags;
mod query;
mod world;

pub mod prelude {
    pub use crate::component::Component;
    pub use crate::entity::Entity;
    pub use crate::query::Query;
    pub use crate::world::World;
}
