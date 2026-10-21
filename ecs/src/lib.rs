pub mod components;
pub mod entity;
pub mod reflection;
pub mod serialization;
pub mod world;

pub use components::*;
pub use entity::Entity;
pub use reflection::*;
pub use serialization::*;
pub use world::*;

pub use inventory;

#[doc(hidden)]
pub use flecs_ecs;
pub use flecs_ecs_derive::Component;
pub use ecs_macros::Inspectable;

pub mod prelude {
    pub use crate::components::*;
    pub use crate::entity::Entity;
    pub use crate::reflection::*;
    pub use crate::world::{create_world, World};
    pub use crate::Component;
    pub use crate::Inspectable;
}