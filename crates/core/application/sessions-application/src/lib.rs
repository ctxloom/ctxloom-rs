//! The sessions use cases, each over only the ports it uses, composed by one
//! [`Service`] behind the trait a driving adapter calls ([`SessionService`]):
//!
//! - read them: list, search, show, and the transcripts' list and show;
//! - keep one, or release it;
//! - remove one: its transcript and its index entry, reporting first;
//! - bind the engine's session to a harp (the SessionStart hook).
//!
//! Each reaches its project through the project identity port (taskloom's
//! `ports`), its store through [`session_ports::ProjectStores`], and a write
//! runs its commands through angzarr-embedded's kernel through
//! kernel-runner's `Runner`, which retries them. A read is the session index
//! caught up from the store. The ports are the only way out: this crate does
//! no I/O, keeps no clock and starts no runtime.
//!
//! A session's state is its lock's verdict ([`session_ports::Liveness`]),
//! never its end stamp; its title is the first prompt of its transcript.
//! Renaming waits on the session-name claims (D5 Q1) and the importer on the
//! stable ids: neither is here.

mod error;
mod go_sidecar;
mod import;
mod read;
mod service;
mod types;
mod write;

use angzarr_diagnostic::Report;

/// A dead letter, as the kernel answers it and the store keeps it. A use case
/// returns every one its kernel calls answered, beside its result or on its
/// error, for its driving adapter to say loudly: each almost certainly marks a
/// bug.
pub use angzarr_embedded::{dead_letter_line, DeadLetter};

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{AppError, Refusal, RemovedNothing, CATALOGS, EN_US};
pub use go_sidecar::{
    decode as decode_go_sidecar, GoEndReason, GoEntry, GoRotation, GoRunEnd, GoSidecarError,
};
pub use import::{
    imported_id, ImportReport, ImportRequest, ImportService, Imported, Importer, Left, Why,
};
pub use ports::{Project, Timestamp};
pub use service::{registry, Service, SessionService};
pub use session_index::{stamp, Captured, SessionRecord, State, View, NO_SUMMARY, STATES};
pub use types::{
    BindRequest, Bound, Class, FilePlan, KeepRequest, Kept, ListQuery, PlanItem, Removal,
    RemoveRequest, RenameRequest, Renamed, Scope, SearchQuery, ShowQuery, TranscriptQuery,
    TranscriptRow,
};

/// A store's failure, as the use cases fail with it.
pub(crate) fn failed(error: angzarr_embedded::StoreError) -> AppError {
    AppError::Failed(Report::of(&angzarr_embedded::KernelError::Store(error)))
}

/// A dead letter as the kernel answers one, from `component`: for a test that
/// hands a use case, or a driving adapter, a letter a kernel call answered.
#[cfg(any(test, feature = "test-support"))]
pub fn dead_letter_sample(component: &str) -> DeadLetter {
    use angzarr_embedded::v1::{AngzarrDeadLetter, Cover, Diagnostic};
    DeadLetter {
        stream: Cover {
            domain: sessions::SESSION_DOMAIN.to_string(),
            root: Some(angzarr_embedded::v1::Uuid::from_bytes([7; 16])),
            edition: None,
        },
        at: angzarr_embedded::DeadLetterAt::Sequence(0),
        letter: AngzarrDeadLetter {
            source_component: component.to_string(),
            reason: Some(Diagnostic {
                code: "ROUTER_REASON_UNHANDLED_HANDLER_ERROR".to_string(),
                args: Default::default(),
            }),
            ..AngzarrDeadLetter::default()
        },
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "fakes.test.rs"]
mod fakes;
