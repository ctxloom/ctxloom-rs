//! The trees domain layer, generated into `OUT_DIR` from the protos.
//! Generated code is never committed.

fn main() -> Result<(), angzarr_build::BuildError> {
    angzarr_build::markup_domain(
        &["ctxloom/runs/v1/trees.proto"],
        &["../../../../proto"],
        &[("ctxloom.runs.v1", "::runs_proto::v1")],
    )
}
