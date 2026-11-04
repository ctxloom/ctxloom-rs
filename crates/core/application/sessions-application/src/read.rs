//! Reading sessions: the listing, the search, one session, and the
//! transcripts. A read opens a project's store only if it exists
//! ([`ProjectStores::open_existing`]) and creates nothing: reading the record
//! touches nothing (session.feature).

use angzarr_embedded::Registry;
use kernel_runner::ProjectLists;
use ports::{Project, ProjectIdentity};
use session_index::{only_state, title_of, Captured, SessionIndexes, SessionRecord, State, View};
use session_ports::{Liveness, LockState, ProjectStores, SessionFiles};

use crate::{failed, AppError, ListQuery, SearchQuery, ShowQuery, TranscriptQuery, TranscriptRow};

/// A read's way to its projects, stores, files and locks, borrowed from the
/// service for one call.
pub(crate) struct Reading<'s, I, S, F, L> {
    pub identity: &'s mut I,
    pub stores: &'s mut S,
    pub files: &'s mut F,
    pub liveness: &'s mut L,
    pub registry: &'s Registry,
    pub indexes: &'s mut ProjectLists<SessionIndexes>,
}

impl<I, S, F, L> Reading<'_, I, S, F, L>
where
    I: ProjectIdentity,
    S: ProjectStores,
    F: SessionFiles,
    L: Liveness,
{
    /// The projects a call spans: every project's under `all`, else the one
    /// `pinned` (or the work root) names, none where there is none.
    pub(crate) fn scope(
        &mut self,
        pinned: Option<&str>,
        all: bool,
    ) -> Result<Vec<Project>, AppError> {
        if all {
            return self.identity.every_project().map_err(AppError::Identity);
        }
        let project = self.identity.for_read(pinned).map_err(AppError::Identity)?;
        Ok(project.into_iter().collect())
    }

    /// `project`'s sessions as its store holds them now, most recently
    /// started first; none where it has no store yet.
    pub(crate) async fn records(
        &mut self,
        project: &Project,
    ) -> Result<Vec<SessionRecord>, AppError> {
        let Some(store) = self.stores.open_existing(project).await.map_err(failed)? else {
            return Ok(Vec::new());
        };
        let index = self
            .indexes
            .caught_up(project, store, self.registry)
            .await
            .map_err(AppError::Failed)?;
        let records = index.index().records();
        Ok(records)
    }

    /// The session named `harp`, and the project holding it: the one
    /// `pinned` (or the work root) names first, then any other (Go finds a
    /// harp wherever it was recorded).
    pub(crate) async fn find(
        &mut self,
        pinned: Option<&str>,
        harp: &str,
    ) -> Result<(Project, SessionRecord), AppError> {
        let here = self.scope(pinned, false)?;
        let mut searched = Vec::new();
        for project in here {
            if let Some(found) = self.find_in(&project, harp).await? {
                return Ok((project, found));
            }
            searched.push(project.id);
        }
        for project in self.scope(None, true)? {
            if searched.contains(&project.id) {
                continue;
            }
            if let Some(found) = self.find_in(&project, harp).await? {
                return Ok((project, found));
            }
        }
        Err(AppError::NoSession {
            harp: harp.to_string(),
        })
    }

    async fn find_in(
        &mut self,
        project: &Project,
        harp: &str,
    ) -> Result<Option<SessionRecord>, AppError> {
        Ok(self
            .records(project)
            .await?
            .into_iter()
            .find(|record| record.harp == harp))
    }

    /// `record` as a listing shows it: its state from its lock (never its end
    /// stamp), its title from its transcript, its transcript on disk. A
    /// transcript that cannot be read is no title, as in Go.
    pub(crate) fn view(&mut self, record: SessionRecord) -> View {
        let state = match self.liveness.lock_state(&record.harp) {
            LockState::Held => State::Live,
            LockState::Free => State::Ended,
            LockState::Unproven => State::Unknown,
        };
        let title = self
            .files
            .read_transcript(&record.harp)
            .ok()
            .flatten()
            .and_then(|text| title_of(&text));
        let captured = self
            .files
            .transcript(&record.harp)
            .ok()
            .flatten()
            .map(|t| Captured {
                path: t.path,
                bytes: t.bytes,
            });
        View {
            record,
            state,
            title,
            captured,
        }
    }

    /// Every session of `query`'s scope, most recently started first.
    async fn sessions(&mut self, query: &ListQuery) -> Result<Vec<View>, AppError> {
        let mut records = Vec::new();
        for project in self.scope(query.project.as_deref(), query.all)? {
            records.extend(self.records(&project).await?);
        }
        records.sort_by(|a, b| {
            let started = |r: &SessionRecord| (r.started_at.seconds, r.started_at.nanos);
            started(b)
                .cmp(&started(a))
                .then_with(|| a.harp.cmp(&b.harp))
        });
        let views = records
            .into_iter()
            .map(|record| self.view(record))
            .collect();
        Ok(only_state(views, query.state))
    }

    pub(crate) async fn list(&mut self, query: ListQuery) -> Result<Vec<View>, AppError> {
        self.sessions(&query).await
    }

    pub(crate) async fn search(&mut self, query: SearchQuery) -> Result<Vec<View>, AppError> {
        let views = self.sessions(&query.list).await?;
        Ok(views
            .into_iter()
            .filter(|view| view.matches(&query.words))
            .collect())
    }

    pub(crate) async fn show(&mut self, query: ShowQuery) -> Result<View, AppError> {
        let (_, record) = self.find(query.project.as_deref(), &query.harp).await?;
        Ok(self.view(record))
    }

    pub(crate) async fn transcripts(
        &mut self,
        query: TranscriptQuery,
    ) -> Result<Vec<TranscriptRow>, AppError> {
        let records = match &query.harp {
            Some(harp) => vec![self.find(query.project.as_deref(), harp).await?.1],
            None => {
                let list = ListQuery {
                    project: query.project.clone(),
                    all: query.all,
                    state: None,
                };
                self.sessions(&list)
                    .await?
                    .into_iter()
                    .map(|view| view.record)
                    .collect()
            }
        };
        let mut rows = Vec::with_capacity(records.len());
        for record in records {
            let captured = self.files.transcript(&record.harp).ok().flatten();
            rows.push(TranscriptRow {
                harp: record.harp,
                captured: captured.is_some(),
                bytes: captured.as_ref().map_or(0, |t| t.bytes),
                path: captured.map(|t| t.path),
            });
        }
        Ok(rows)
    }

    pub(crate) async fn transcript(&mut self, harp: &str) -> Result<String, AppError> {
        let (_, record) = self.find(None, harp).await?;
        self.files
            .read_transcript(&record.harp)
            .map_err(AppError::Files)?
            .ok_or(AppError::NoCapturedTranscript { harp: record.harp })
    }
}
