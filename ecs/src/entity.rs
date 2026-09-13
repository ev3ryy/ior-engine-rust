use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Entity(pub u64);

impl Entity {
    pub const NULL: Self = Self(0);

    #[inline]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl From<u64> for Entity {
    #[inline]
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<Entity> for u64 {
    #[inline]
    fn from(entity: Entity) -> Self {
        entity.0
    }
}

impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entity({})", self.0)
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}