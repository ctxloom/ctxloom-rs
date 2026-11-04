//! The runs and trees domains' wire types: prost messages generated from
//! `proto/ctxloom/runs/v1`. The wire form only; domain code works on domain
//! types converted from these at the boundary.

/// The generated package, nested as its proto package is.
pub mod ctxloom {
    pub mod runs {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/ctxloom.runs.v1.rs"));
        }
    }
}

/// `ctxloom.runs.v1`.
pub use ctxloom::runs::v1;

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
