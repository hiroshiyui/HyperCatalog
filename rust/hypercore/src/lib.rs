//! hypercore — a platform-agnostic HyperCard-like document model and HyperTalk
//! interpreter. No Android or platform dependencies live here; hosts drive it through
//! the `session::Session` facade.

pub mod model;
pub mod script;
pub mod session;

pub use session::{
    DispatchResult, DrawCmd, HostEffect, ObjectProps, Prop, RenderList, Session, ViewNode, ViewTree,
};

#[cfg(test)]
mod tests;
