//! Why a command to a tree was refused.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TreeRejection {
    /// A founding on a stream that already holds a tree.
    AlreadyFounded,
    /// A command to a stream holding no tree.
    NoTree,
}

impl fmt::Display for TreeRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for TreeRejection {
    fn code(&self) -> &str {
        match self {
            TreeRejection::AlreadyFounded => code!("already-founded"),
            TreeRejection::NoTree => code!("no-tree"),
        }
    }

    // Neither rejection has an arg: an empty list is the only right answer.
    #[cfg_attr(test, mutants::skip)]
    fn args(&self) -> Vec<(&str, Arg)> {
        Vec::new()
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every rejection, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(TreeRejection::AlreadyFounded),
        Box::new(TreeRejection::NoTree),
    ]
}
