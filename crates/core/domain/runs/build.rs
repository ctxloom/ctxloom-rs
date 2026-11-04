//! The runs domain layer (domain types, wire conversions, payload enums and
//! their `DomainMessage` impls: angzarr-build's markup layer), generated into
//! `OUT_DIR` from the protos. Generated code is never committed.

fn main() -> Result<(), angzarr_build::BuildError> {
    angzarr_build::markup_domain(
        &["ctxloom/runs/v1/runs.proto"],
        &["../../../../proto"],
        &[("ctxloom.runs.v1", "::runs_proto::v1")],
    )
}
