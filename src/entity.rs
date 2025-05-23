use slotmap::{KeyData, new_key_type};

new_key_type! { pub struct Entity; }

impl Entity {
    pub fn null() -> Self {
        Self::default()
    }

    pub unsafe fn from_offset(val: u64) -> Self {
        // Slotmap IDs start from 1
        Self(KeyData::from_ffi(1 + val))
    }

    pub fn raw(self) -> u64 {
        self.0.as_ffi()
    }

    pub(crate) fn from_ffi(val: u64) -> Self {
        Self(KeyData::from_ffi(val))
    }
}
