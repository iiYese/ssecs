use crate::entity::Entity;

pub struct Access {}

impl From<Entity> for Access {
    fn from(_: Entity) -> Access {
        Access {}
    }
}

impl From<&'_ Entity> for Access {
    fn from(_: &Entity) -> Access {
        Access {}
    }
}

impl From<&'_ mut Entity> for Access {
    fn from(_: &mut Entity) -> Access {
        Access {}
    }
}

impl From<&'_ [Entity; 1]> for Access {
    fn from(_: &[Entity; 1]) -> Access {
        Access {}
    }
}

impl From<&'_ mut [Entity; 1]> for Access {
    fn from(_: &mut [Entity; 1]) -> Access {
        Access {}
    }
}

pub struct Query {}

impl Query {
    pub fn new() -> Self {
        Self {}
    }

    pub fn with<T>(self, _: T) -> Self
    where
        Access: From<T>,
    {
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::component::{Component, tests::*};

    #[test]
    fn query_compile() {
        Query::new()
            .with(Transform::id())
            .with(&Transform::id())
            .with(&[Transform::id()])
            .with(&mut Transform::id())
            .with(&mut [Transform::id()]);
    }
}
