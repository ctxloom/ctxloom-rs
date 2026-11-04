//! [`Service`]: the ports the use cases run over, owned once, and the
//! composition of the use cases behind the trait a driving adapter calls
//! ([`SessionService`]).

use std::future::Future;

use angzarr_embedded::{OnRetryable, Registry};
use kernel_runner::KERNEL_RETRY;
use ports::{Clock, CommandIds, Delay, Jitter, ProjectIdentity};
use session_index::SessionIndexes;
use session_ports::{Liveness, ProjectStores, SessionFiles};
use sessions::{NameClaimAggregate, SessionAggregate};

use crate::read::Reading;
use crate::write::Writing;
use crate::{
    AppError, BindRequest, Bound, DeadLetter, KeepRequest, Kept, ListQuery, Removal, RemoveRequest,
    RenameRequest, Renamed, SearchQuery, ShowQuery, TranscriptQuery, TranscriptRow, View,
};

/// The use cases a driving adapter calls for a session. Their futures are
/// `Send`, so a server can spawn them.
pub trait SessionService {
    /// The sessions of the current project (or every project's), in `state`
    /// if one is asked, most recently started first.
    fn list(
        &mut self,
        query: ListQuery,
    ) -> impl Future<Output = Result<Vec<View>, AppError>> + Send;
    /// The listing's sessions that match every word.
    fn search(
        &mut self,
        query: SearchQuery,
    ) -> impl Future<Output = Result<Vec<View>, AppError>> + Send;
    /// One session, by its harp.
    fn show(&mut self, query: ShowQuery) -> impl Future<Output = Result<View, AppError>> + Send;
    /// Mark a session's transcript kept, or release it.
    fn keep(&mut self, request: KeepRequest)
        -> impl Future<Output = Result<Kept, AppError>> + Send;
    /// Remove a session: report first, apply on `yes`.
    fn remove(
        &mut self,
        request: RemoveRequest,
    ) -> impl Future<Output = Result<Removal, AppError>> + Send;
    /// Rename a session. Not built yet: it waits on the name claims.
    fn rename(
        &mut self,
        request: RenameRequest,
    ) -> impl Future<Output = Result<Renamed, AppError>> + Send;
    /// Which sessions have a captured transcript, and how large.
    fn transcripts(
        &mut self,
        query: TranscriptQuery,
    ) -> impl Future<Output = Result<Vec<TranscriptRow>, AppError>> + Send;
    /// A session's whole canonical transcript.
    fn transcript(&mut self, harp: &str) -> impl Future<Output = Result<String, AppError>> + Send;
    /// Bind the engine's session to a harp.
    fn bind(
        &mut self,
        request: BindRequest,
    ) -> impl Future<Output = Result<Bound, AppError>> + Send;
}

/// The registry the sessions domains run under: the session aggregate and the
/// name claims'.
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .aggregate::<SessionAggregate>(SessionAggregate::default)
        .and_then(|registry| registry.aggregate::<NameClaimAggregate>(NameClaimAggregate::default))
        .expect("the sessions and session-names domains, each once");
    registry
}

/// How long a session's transcript is kept after it ended, unless configured
/// (Go: `session_transcript_retention`, 30 days).
pub const DEFAULT_RETENTION_DAYS: u64 = 30;

/// The ports the use cases run over, owned once: `identity` resolves a
/// project, `stores` opens its store, `files` reads a session's files,
/// `liveness` asks its lock, `clock` stamps commands and mints their ids,
/// `delay` waits, with jitter, between retries.
pub struct Service<I, S, F, L, C, D> {
    pub identity: I,
    pub stores: S,
    pub files: F,
    pub liveness: L,
    pub clock: C,
    pub delay: D,
    /// How the kernel retries a command ([`KERNEL_RETRY`]).
    pub retry: OnRetryable,
    /// How many days a session's transcript is kept after it ended.
    pub retention_days: u64,
    pub(crate) registry: Registry,
    pub(crate) indexes: kernel_runner::ProjectLists<SessionIndexes>,
    /// The dead letters the use case's kernel calls answered, taken into its
    /// result, or its error.
    pub(crate) dead_letters: Vec<DeadLetter>,
}

impl<I, S, F, L, C, D> Service<I, S, F, L, C, D> {
    pub fn new(identity: I, stores: S, files: F, liveness: L, clock: C, delay: D) -> Self {
        Service {
            identity,
            stores,
            files,
            liveness,
            clock,
            delay,
            retry: KERNEL_RETRY,
            retention_days: DEFAULT_RETENTION_DAYS,
            registry: registry(),
            indexes: kernel_runner::ProjectLists::new(session_index::SessionIndex::catch_up),
            dead_letters: Vec::new(),
        }
    }
}

impl<I, S, F, L, C, D> Service<I, S, F, L, C, D> {
    /// The ports a read uses, lent for one call.
    fn reading(&mut self) -> Reading<'_, I, S, F, L> {
        Reading {
            identity: &mut self.identity,
            stores: &mut self.stores,
            files: &mut self.files,
            liveness: &mut self.liveness,
            registry: &self.registry,
            indexes: &mut self.indexes,
        }
    }

    /// The ports a write uses, lent for one call.
    fn writing(&mut self) -> Writing<'_, I, S, F, L, C, D> {
        Writing {
            read: Reading {
                identity: &mut self.identity,
                stores: &mut self.stores,
                files: &mut self.files,
                liveness: &mut self.liveness,
                registry: &self.registry,
                indexes: &mut self.indexes,
            },
            clock: &mut self.clock,
            delay: &mut self.delay,
            retry: self.retry,
            retention_days: self.retention_days,
            dead_letters: &mut self.dead_letters,
        }
    }

    /// `result`, an error carrying the dead letters the call's kernel calls
    /// answered.
    fn with_dead_letters<T>(&mut self, result: Result<T, AppError>) -> Result<T, AppError> {
        result.map_err(|error| {
            let letters = std::mem::take(&mut self.dead_letters);
            match letters.is_empty() {
                true => error,
                false => AppError::DeadLettered {
                    error: Box::new(error),
                    dead_letters: letters,
                },
            }
        })
    }
}

impl<I, S, F, L, C, D> SessionService for Service<I, S, F, L, C, D>
where
    I: ProjectIdentity + Send,
    S: ProjectStores + Send,
    F: SessionFiles + Send,
    L: Liveness + Send,
    C: Clock + CommandIds + Send,
    D: Delay + Jitter + Send,
{
    async fn list(&mut self, query: ListQuery) -> Result<Vec<View>, AppError> {
        self.reading().list(query).await
    }

    async fn search(&mut self, query: SearchQuery) -> Result<Vec<View>, AppError> {
        self.reading().search(query).await
    }

    async fn show(&mut self, query: ShowQuery) -> Result<View, AppError> {
        self.reading().show(query).await
    }

    async fn keep(&mut self, request: KeepRequest) -> Result<Kept, AppError> {
        let result = self.writing().keep(request).await;
        self.with_dead_letters(result)
    }

    async fn remove(&mut self, request: RemoveRequest) -> Result<Removal, AppError> {
        let result = self.writing().remove(request).await;
        self.with_dead_letters(result)
    }

    async fn rename(&mut self, request: RenameRequest) -> Result<Renamed, AppError> {
        let result = self.writing().rename(request).await;
        self.with_dead_letters(result)
    }

    async fn transcripts(
        &mut self,
        query: TranscriptQuery,
    ) -> Result<Vec<TranscriptRow>, AppError> {
        self.reading().transcripts(query).await
    }

    async fn transcript(&mut self, harp: &str) -> Result<String, AppError> {
        self.reading().transcript(harp).await
    }

    async fn bind(&mut self, request: BindRequest) -> Result<Bound, AppError> {
        let result = self.writing().bind(request).await;
        self.with_dead_letters(result)
    }
}
