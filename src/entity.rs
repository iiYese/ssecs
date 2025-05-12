#[derive(Clone, Copy, Debug)]
pub struct Entity(pub(crate) u64);

impl Entity {
    pub const NULL: Self = Self(u64::MAX);

    pub fn raw(self) -> u64 {
        self.0
    }
}
