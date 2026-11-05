//! The `ContainerRuntimes` port over the docker and podman command lines
//! (S11). Go: `shared/containerprobe` and `isolation/runtime*.go`
//! (release/0.7).
//!
//! A runtime is installed when its binary is an executable file on the
//! searched `PATH`, and reachable when `<bin> info` answers within a bound.
//! The search directories are the caller's, so a test (or a hermetic lane)
//! can hide the machine's runtimes by giving none.

mod markers;
mod runtimes;

pub use markers::{markers_from, Machine, Real};
pub use runtimes::HostRuntimes;

#[cfg(test)]
#[path = "markers.test.rs"]
mod markers_tests;

#[cfg(test)]
#[path = "runtimes.test.rs"]
mod runtimes_tests;
