//! The scheduler's wire types: prost messages generated from
//! `proto/io/angzarr/scheduler/v1` (the public time messages, the
//! private events and the clock's fact). The wire form only; domain code
//! works on domain types converted from these at the boundary.

/// The generated package, nested as its proto package is.
pub mod io {
    pub mod angzarr {
        pub mod scheduler {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/io.angzarr.scheduler.v1.rs"));
            }
        }
    }
}

/// `io.angzarr.scheduler.v1`.
pub use io::angzarr::scheduler::v1;

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
