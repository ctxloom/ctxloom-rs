//! The tasks domain layer (domain types, wire conversions, payload enums and
//! the state's annals impl), generated into `OUT_DIR` from the protos.
//! Generated code is never committed.

fn main() -> Result<(), angzarr_build::BuildError> {
    angzarr_build::domain(
        &["ctxloom/tasks/v1/tasks.proto"],
        &["../../../../proto", tagma_proto::PROTO_INCLUDE],
        &[("ctxloom.tasks.v1", "::tasks_proto::v1")],
    )
}
