//! The ports of the run core (S5): the credential [`Token`] and its
//! [`TokenHash`] (D29), the [`TokenSource`] that mints one, the [`Edge`] the
//! core is served through (the exposure seam, `App::serve_in`, as a port). RED (S5): stubs; the tests propose the shapes.
//!
//! `Arrival` and `Unserved` are this crate's own, not angzarr-embedded-host's:
//! the host brings a gRPC stack and process supervision, which the core must
//! not link. The driven adapter `embedded-edge` converts (review question 6).

mod edge;
mod token;

pub use edge::{Arrival, Edge, Unserved};
#[cfg(any(test, feature = "test-support"))]
pub use token::diagnostic_samples;
pub use token::{Token, TokenError, TokenHash, TokenSource, EN_US};
