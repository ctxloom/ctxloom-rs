//! The tasks domain's wire types: prost messages generated from
//! `proto/ctxloom/tasks/v1/tasks.proto`. They are the wire form only; domain
//! code works on domain types converted from these at the boundary.

/// The generated package, nested as its proto package is: prost reaches
/// another package's types by relative path (`super::super::super::tagma`),
/// and angzarr's settings map only angzarr's own by absolute path
/// (`extern_path`). tagma's `Tag` is therefore re-exported at
/// [`tagma`], where that relative path lands.
pub mod ctxloom {
    pub mod tasks {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/ctxloom.tasks.v1.rs"));

            /// tagma's `Tag`, where the tasks domain's generated layer
            /// looks for every message its package reaches.
            pub use tagma_proto::v1::Tag;
        }
    }
}

/// `tagma.v1`, the package `tasks.proto` imports `Tag` from: tagma-proto's.
pub mod tagma {
    pub use tagma_proto::v1;
}

/// `ctxloom.tasks.v1`.
pub use ctxloom::tasks::v1;

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
