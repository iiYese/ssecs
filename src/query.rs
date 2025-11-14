use crate as ssecs;
use crate::{
    component::Component,
    entity::{Entity, View},
    world::World,
};
use ssecs_macros::*;

pub trait AccessTuple {
    type Out;
}

#[derive(Clone, Copy, Default)]
pub enum Access {
    #[default]
    Noop,
    Include,
    Exclude,
    Read,
    Write,
}

impl Access {
    fn is_noop(self) -> bool {
        matches!(self, Self::Noop)
    }
}

#[derive(Clone)]
struct Term {
    field: u64,
    access: Access,
}

impl Default for Term {
    fn default() -> Self {
        Self { field: 0, access: Access::Noop }
    }
}

#[derive(Component)]
struct QueryState {
    // TODO
}

pub struct Query {
    world: World,
    terms: Vec<Term>,
}

trait QueryClosure {
    fn run(self, query: &Query, state: &QueryState);
}

impl<F: FnMut(View<'_>)> QueryClosure for F {
    fn run(self, query: &Query, state: &QueryState) {}
}

impl Query {
    fn run<F: QueryClosure>(&self, func: F) {
        let cache = QueryState {}; // TODO
        func.run(self, &cache);
    }
}

impl Clone for Query {
    fn clone(&self) -> Self {
        Self { terms: self.terms.clone(), world: World { crust: self.world.crust.clone() } }
    }
}

pub struct QueryBuilder {
    query: Query,
    cursor: usize,
}

impl QueryBuilder {
    pub(crate) fn new(world: World) -> Self {
        Self { cursor: 0, query: Query { world, terms: Vec::new() } }
    }

    pub fn term(mut self) -> Self {
        self.query.terms.push(Term::default());
        self
    }

    pub fn incl(mut self, component: Entity) -> Self {
        let Some(term) = self.query.terms.last_mut() else {
            panic!("Must create term before calling `incl`");
        };
        term.access = Access::Include;
        term.field = component.raw();
        self
    }

    pub fn excl(mut self, component: Entity) -> Self {
        let Some(term) = self.query.terms.last_mut() else {
            panic!("Must create term before calling `excl`");
        };
        term.access = Access::Exclude;
        term.field = component.raw();
        self
    }

    pub fn read(mut self, component: Entity) -> Self {
        let Some(term) = self.query.terms.last_mut() else {
            panic!("Must create term before calling `read`");
        };
        term.access = Access::Read;
        term.field = component.raw();
        self
    }

    pub fn write(mut self, component: Entity) -> Self {
        let Some(term) = self.query.terms.last_mut() else {
            panic!("Must create term before calling `write`");
        };
        term.access = Access::Write;
        term.field = component.raw();
        self
    }

    pub fn build(self) -> Query {
        self.query
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::component::{Component, tests::*};

    #[derive(Component)]
    struct Byte(u8);

    #[derive(Component)]
    struct A;

    #[derive(Component)]
    struct B;

    #[test]
    #[rustfmt::skip]
    fn basic_queries() {
        let world = World::new();

        world.spawn().insert(Byte(0));
        world.spawn().insert(Byte(0)).insert(A);
        world.spawn().insert(Byte(0)).insert(A);
        world.spawn().insert(Byte(0)).insert(B);
        world.spawn().insert(Byte(0)).insert(B);
        world.spawn().insert(Byte(0)).insert(B);

        world.flush();

        let query = world
            .query()
            .term().incl(Byte::id())
            .build()
            .run(|view: View<'_>| {
                view.get_mut::<Byte>().unwrap().0 += 1;
            });
    }
}
