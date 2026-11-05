//! The fixture's wire types and domain layer, generated into `OUT_DIR` from
//! its proto. A test-only crate, so both calls are here rather than in a wire
//! crate of its own. Generated code is never committed.

fn main() -> Result<(), angzarr_build::BuildError> {
    let protos = ["angzarr/embedded/scheduler/fixture/v1/fixture.proto"];
    let includes = ["../../../../proto"];
    angzarr_build::wire(&protos, &includes)?;
    angzarr_build::markup_domain(
        &protos,
        &includes,
        &[("angzarr.embedded.scheduler.fixture.v1", "crate::wire")],
    )
}
