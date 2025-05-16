use slotmap::{KeyData, new_key_type};

use crate::archetype::FieldId;

new_key_type! { pub struct Entity; }

impl Entity {
    pub fn null() -> Self {
        Self(KeyData::from_ffi(u64::MAX))
    }

    pub unsafe fn from_offset(val: u64) -> Self {
        Self(KeyData::from_ffi(val))
    }

    pub fn raw(self) -> u64 {
        self.0.as_ffi()
    }
}
