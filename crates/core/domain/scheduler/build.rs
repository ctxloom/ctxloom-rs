//! The scheduler domain layer (domain types, wire conversions, payload enums
//! and their `DomainMessage` impls: angzarr-build's markup layer), generated
//! into `OUT_DIR` from the protos. Generated code is never committed.

fn main() -> Result<(), angzarr_build::BuildError> {
    angzarr_build::markup_domain(
        &["io/angzarr/scheduler/v1/scheduler.proto"],
        &["../../../../proto"],
        &[("io.angzarr.scheduler.v1", "::scheduler_proto::v1")],
    )
}
