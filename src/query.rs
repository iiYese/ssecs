use crate as ssecs;
use crate::{entity::Entity, world::World};
use ssecs_macros::*;

pub trait AccessTuple {
    type Out;
}

#[derive(Clone, Copy)]
enum Pattern {
    Read(u64),
    Write(u64),
    Match(u64),
    Exclude(u64),
}

#[derive(Clone, Copy)]
pub struct Term {
    src: u16,
    pattern: Pattern,
}

impl From<Entity> for Term {
    fn from(e: Entity) -> Term {
        Term { src: 0, pattern: Pattern::Match(e.raw()) }
    }
}

impl From<&'_ Entity> for Term {
    fn from(e: &Entity) -> Term {
        Term { src: 0, pattern: Pattern::Read(e.raw()) }
    }
}

impl From<&'_ mut Entity> for Term {
    fn from(e: &mut Entity) -> Term {
        Term { src: 0, pattern: Pattern::Write(e.raw()) }
    }
}

pub fn excl<T>(t: T) -> Term
where
    Term: From<T>,
{
    let mut term = Term::from(t);
    term.pattern = match term.pattern {
        Pattern::Read(id) => Pattern::Exclude(id),
        Pattern::Write(id) => Pattern::Exclude(id),
        Pattern::Match(id) => Pattern::Exclude(id),
        Pattern::Exclude(id) => Pattern::Exclude(id),
    };
    term
}

#[derive(Component)]
pub struct QueryState {
    // TODO
}

pub struct Query {
    world: World,
    cache: Entity,
    terms: Vec<Term>,
    variables: Vec<&'static str>,
}

trait QueryClosure {
    fn run(self, state: &QueryState);
}

impl Query {
    fn run<F: QueryClosure>(&self, func: F) {
        if self.cache.is_null() {
            let entity = self.world.entity(self.cache);
            let cache = entity.get::<QueryState>().unwrap();
            func.run(&cache);
        } else {
            let cache = QueryState {}; // TODO
            func.run(&cache);
        };
    }
}

impl Clone for Query {
    fn clone(&self) -> Self {
        Self {
            cache: self.cache,
            terms: self.terms.clone(),
            world: World { crust: self.world.crust.clone() },
            variables: self.variables.clone(),
        }
    }
}

pub struct QueryBuilder {
    query: Query,
}

impl QueryBuilder {
    pub(crate) fn new(world: World) -> Self {
        Self {
            query: Query {
                world,
                terms: Vec::new(),
                cache: Entity::null(), // Uncached by default
                variables: Vec::new(),
            },
        }
    }

    pub fn cached(mut self) -> Self {
        self.query.cache = self.query.world.spawn().id();
        self
    }

    pub fn term<T>(mut self, term: T) -> Self
    where
        Term: From<T>,
    {
        self.query.terms.push(term.into());
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

    #[test]
    fn query_compile() {
        let world = World::new();
        let query = world //
            .query()
            .term(Transform::id())
            .term(&Transform::id())
            .term(&mut Transform::id())
            .term(excl(Transform::id()))
            .build();
    }
}
