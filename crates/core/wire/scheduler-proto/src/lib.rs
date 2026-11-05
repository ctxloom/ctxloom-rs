//! The scheduler's wire types: prost messages generated from
//! `proto/angzarr/embedded/scheduler/v1` (the public time messages, the
//! private events and the clock's fact). The wire form only; domain code
//! works on domain types converted from these at the boundary.

/// The generated package, nested as its proto package is.
pub mod angzarr {
    pub mod embedded {
        pub mod scheduler {
            pub mod v1 {
                include!(concat!(
                    env!("OUT_DIR"),
                    "/angzarr.embedded.scheduler.v1.rs"
                ));
            }
        }
    }
}

/// `angzarr.embedded.scheduler.v1`.
pub use angzarr::embedded::scheduler::v1;

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
