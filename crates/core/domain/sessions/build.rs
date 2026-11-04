//! The sessions domain layer (domain types, wire conversions, payload enums
//! and their `DomainMessage` impls: angzarr-build's markup layer), generated
//! into `OUT_DIR` from the protos. Generated code is never committed.

fn main() -> Result<(), angzarr_build::BuildError> {
    angzarr_build::markup_domain(
        &[
            "ctxloom/sessions/v1/sessions.proto",
            "ctxloom/sessions/v1/names.proto",
        ],
        &["../../../../proto"],
        &[("ctxloom.sessions.v1", "::sessions_proto::v1")],
    )
}
