//! What every driving CLI adapter shares, so none writes it again: the
//! [`Call`] a command is run with, the [`Failure`] it ends in, the notices
//! printer, and the helpers that read arguments and write answers.
//!
//! It is a presentation crate: domain-free, and linking no use case.

mod args;
mod call;
mod notices;

pub use args::{emit, flag, many, one};
pub use call::{Call, Failure};
pub use notices::{say_notices, say_warning, FixOf};

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
