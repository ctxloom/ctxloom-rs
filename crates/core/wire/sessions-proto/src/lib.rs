//! The sessions domain's wire types: prost messages generated from
//! `proto/ctxloom/sessions/v1`. They are the wire form only; domain code
//! works on domain types converted from these at the boundary.

/// The generated package, nested as its proto package is.
pub mod ctxloom {
    pub mod sessions {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/ctxloom.sessions.v1.rs"));
        }
    }
}

/// `ctxloom.sessions.v1`.
pub use ctxloom::sessions::v1;

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
