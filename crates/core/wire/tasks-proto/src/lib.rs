//! The tasks domain's wire types: prost messages generated from
//! `proto/ctxloom/tasks/v1/tasks.proto`. They are the wire form only; domain
//! code works on domain types converted from these at the boundary.

/// `ctxloom.tasks.v1`. One module, not one per package segment: the package
/// refers to no other package's types except angzarr's, which prost reaches
/// by absolute path (`extern_path`).
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/ctxloom.tasks.v1.rs"));
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
