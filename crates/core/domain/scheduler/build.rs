//! The scheduler domain layer (domain types, wire conversions, payload enums
//! and their `DomainMessage` impls: angzarr-build's markup layer), generated
//! into `OUT_DIR` from the protos. Generated code is never committed.

fn main() -> Result<(), angzarr_build::BuildError> {
    angzarr_build::markup_domain(
        &["angzarr/embedded/scheduler/v1/scheduler.proto"],
        &["../../../../proto"],
        &[("angzarr.embedded.scheduler.v1", "::scheduler_proto::v1")],
    )
}
