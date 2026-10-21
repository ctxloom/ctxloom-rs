//! The tasks domain's prost types, generated into `OUT_DIR` with angzarr's
//! settings. Generated code is never committed.

fn main() -> Result<(), angzarr_build::BuildError> {
    angzarr_build::wire(&["ctxloom/tasks/v1/tasks.proto"], &["../../../../proto"])
}
