use crate::archetype::FieldId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Entity(pub(crate) u64);

impl Entity {
    pub const NULL: Self = Self(u64::MAX);

    pub fn raw(self) -> u64 {
        self.0
    }
}
