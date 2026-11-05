//! Writing sessions: keep, bind and remove. A write finds its session as a
//! read does, then runs one command through angzarr-embedded's kernel
//! (angzarr-embedded-runner's `Runner`), which retries it.

use angzarr_diagnostic::{Diagnostic, Report};
use angzarr_embedded::v1::{CommandBook, Cover, Uuid};
use angzarr_embedded::{CommandBookExt, Handled, OnRetryable};
use angzarr_embedded_runner::Runner;

use crate::bridge::Port;
use ports::{Clock, CommandIds, Delay, Jitter, Project, ProjectIdentity};
use session_index::{Retained, Retention, SessionRecord};
use session_ports::{Liveness, LockState, ProjectStores, SessionFiles};
use sessions::{
    claim_root, validate_rename, BindEngineSession, ClaimName, ForgetSession, NameClaimAggregate,
    NameClaimCommand, RecordPurge, ReleaseName, RenameSession, SessionAggregate, SessionCommand,
    SessionRejection, SetKept, NAMES_DOMAIN, SESSION_DOMAIN,
};

use crate::read::Reading;
use crate::{
    failed, AppError, BindRequest, Bound, Class, DeadLetter, FilePlan, KeepRequest, Kept, PlanItem,
    Refusal, Removal, RemoveRequest, RenameRequest, Renamed, Scope,
};

/// A write's way to its sessions and the kernel, borrowed from the service
/// for one call.
pub(crate) struct Writing<'s, I, S, F, L, C, D> {
    pub read: Reading<'s, I, S, F, L>,
    pub clock: &'s mut C,
    pub delay: &'s mut D,
    pub retry: OnRetryable,
    pub retention_days: u64,
    pub dead_letters: &'s mut Vec<DeadLetter>,
}

/// The stream of the session with stable id `id` (a UUID's text).
pub(crate) fn session_cover(id: &str) -> Result<Cover, AppError> {
    let bytes = uuid::Uuid::parse_str(id)
        .map_err(|e| {
            AppError::Failed(
                angzarr_diagnostic::Report::new("sessions-application-bad-id")
                    .arg("ext_err_txt", e.to_string()),
            )
        })?
        .into_bytes();
    Ok(Cover {
        domain: SESSION_DOMAIN.to_string(),
        root: Some(Uuid::from_bytes(bytes)),
        edition: None,
    })
}

impl<I, S, F, L, C, D> Writing<'_, I, S, F, L, C, D>
where
    I: ProjectIdentity,
    S: ProjectStores,
    F: SessionFiles,
    L: Liveness,
    C: Clock + CommandIds,
    D: Delay + Jitter + Send,
{
    /// Run `command` on `record`'s stream in `project`'s store, under a fresh
    /// command id, retried by the kernel; whether it recorded anything.
    async fn commit(
        &mut self,
        project: &Project,
        record: &SessionRecord,
        command: impl Into<SessionCommand>,
    ) -> Result<bool, AppError> {
        let command = command.into();
        let book = CommandBook::command_of(session_cover(&record.id)?, &command);
        self.commit_book(project, record, book).await
    }

    /// Send `book` (a command, or a fact such as the bind) to the session.
    async fn commit_book(
        &mut self,
        project: &Project,
        record: &SessionRecord,
        book: CommandBook,
    ) -> Result<bool, AppError> {
        let _ = record;
        let store = self.read.stores.open(project).await.map_err(failed)?;
        let command_id = self.clock.command_id();
        let mut runner = Runner {
            registry: self.read.registry,
            clock: &mut Port(&mut *self.clock),
            delay: &mut Port(&mut *self.delay),
            retry: self.retry,
            dead_letters: &mut *self.dead_letters,
        };
        let handled = runner
            .run::<SessionAggregate, _, _>(store, book, &command_id)
            .await??;
        Ok(recorded(&handled))
    }

    pub(crate) async fn keep(&mut self, request: KeepRequest) -> Result<Kept, AppError> {
        let (project, record) = self
            .read
            .find(request.project.as_deref(), &request.harp)
            .await?;
        let changed = self
            .commit(&project, &record, SetKept { kept: request.keep })
            .await?;
        Ok(Kept {
            harp: record.harp,
            kept: request.keep,
            changed,
            dead_letters: std::mem::take(self.dead_letters),
        })
    }

    pub(crate) async fn bind(&mut self, request: BindRequest) -> Result<Bound, AppError> {
        let found = self
            .read
            .find(request.project.as_deref(), &request.harp)
            .await;
        let (project, record) = match found {
            Ok(found) => found,
            Err(AppError::NoSession { .. }) => return Ok(Bound::NoSuchSession),
            Err(other) => return Err(other),
        };
        // The bind is a fact (owner ruling 2026-11-04, S5 review item 8).
        let book = sessions::bind_book(
            session_cover(&record.id)?,
            &BindEngineSession {
                native_id: request.native_id,
                transcript_path: request.transcript_path,
                engine_version: None,
            },
        );
        let changed = self.commit_book(&project, &record, book).await?;
        Ok(match changed {
            true => Bound::Recorded {
                dead_letters: std::mem::take(self.dead_letters),
            },
            false => Bound::Unchanged,
        })
    }

    /// Run `command` on the claim stream of `harp` in `project`'s store: the
    /// kernel's answer, a refusal typed.
    async fn claim_command(
        &mut self,
        project: &Project,
        harp: &str,
        command: impl Into<NameClaimCommand>,
    ) -> Result<(), AppError> {
        let command = command.into();
        let cover = Cover {
            domain: NAMES_DOMAIN.to_string(),
            root: Some(claim_root(&project.id, harp)),
            edition: None,
        };
        let book = CommandBook::command_of(cover, &command);
        let store = self.read.stores.open(project).await.map_err(failed)?;
        let command_id = self.clock.command_id();
        let mut runner = Runner {
            registry: self.read.registry,
            clock: &mut Port(&mut *self.clock),
            delay: &mut Port(&mut *self.delay),
            retry: self.retry,
            dead_letters: &mut *self.dead_letters,
        };
        runner
            .run::<NameClaimAggregate, SessionRejection, _>(store, book, &command_id)
            .await??;
        Ok(())
    }

    /// Give `session`'s hold on `harp` back, best effort: a session minted
    /// before claims existed holds none, and that is not a failure.
    async fn release(
        &mut self,
        project: &Project,
        harp: &str,
        session: &str,
    ) -> Result<(), AppError> {
        let released = self
            .claim_command(
                project,
                harp,
                ReleaseName {
                    session: session.to_string(),
                },
            )
            .await;
        match released {
            Err(AppError::Rejected(cause))
                if [
                    SessionRejection::NoClaim.code(),
                    (SessionRejection::NotHolder {
                        harp: String::new(),
                    })
                    .code(),
                ]
                .contains(&cause.code()) =>
            {
                Ok(())
            }
            other => other,
        }
    }

    /// Rename a session (plan 3.1): claim the new name, move the output
    /// folder beside itself, record the name, free the old one; each step
    /// undone if a later one fails, so a refusal leaves everything where it
    /// was.
    pub(crate) async fn rename(&mut self, request: RenameRequest) -> Result<Renamed, AppError> {
        let (project, record) = self
            .read
            .find(request.project.as_deref(), &request.harp)
            .await?;
        let to = request.to;
        validate_rename(&to).map_err(|r| AppError::Rejected(Report::of(&r)))?;
        if to == record.harp {
            return Err(AppError::Rejected(Report::of(
                &SessionRejection::NamesTaken { harp: to },
            )));
        }
        self.claim_command(
            &project,
            &to,
            ClaimName {
                project_id: project.id.clone(),
                harp: to.clone(),
                session: record.id.clone(),
            },
        )
        .await?;
        let new_output = beside(&record.output_dir, &to);
        let moved = match self.move_output(&record.output_dir, &new_output) {
            Ok(moved) => moved,
            Err(error) => {
                self.release(&project, &to, &record.id).await?;
                return Err(error);
            }
        };
        let recorded = self
            .commit(
                &project,
                &record,
                RenameSession {
                    harp: to.clone(),
                    output_dir: new_output.clone(),
                },
            )
            .await;
        if let Err(error) = recorded {
            if moved {
                let _ = self.read.files.move_output(&new_output, &record.output_dir);
            }
            self.release(&project, &to, &record.id).await?;
            return Err(error);
        }
        self.release(&project, &record.harp, &record.id).await?;
        Ok(Renamed {
            from: record.harp,
            to,
            output_dir: new_output,
            dead_letters: std::mem::take(self.dead_letters),
        })
    }

    fn move_output(&mut self, from: &str, to: &str) -> Result<bool, AppError> {
        if from.is_empty() {
            return Ok(false);
        }
        self.read
            .files
            .move_output(from, to)
            .map_err(AppError::Files)
    }

    pub(crate) async fn remove(&mut self, request: RemoveRequest) -> Result<Removal, AppError> {
        let (project, record) = self
            .read
            .find(request.project.as_deref(), &request.harp)
            .await?;
        let harp = record.harp.clone();
        let plan = self.plan(&harp, request.scope)?;
        let refusal = self.refusal(&record, request.even_if_live, request.scope)?;
        let mut removal = Removal {
            scope: request.scope,
            harp,
            applied: false,
            index_entry_removed: false,
            files: FilePlan {
                bytes_freed: 0,
                ..plan
            },
            refusal,
            dead_letters: Vec::new(),
        };
        if removal.refusal.is_some() || !request.yes {
            return Ok(removal);
        }
        // The purge is recorded BEFORE any file is unlinked: if this dies
        // between the two, the index already reads "purged" over its
        // now-missing transcript, not a live session that lost it.
        let destroy: Vec<String> = removal
            .files
            .destroy
            .iter()
            .map(|i| i.rel.clone())
            .collect();
        if !destroy.is_empty() {
            let bytes = removal.files.destroy.iter().map(|i| i.bytes).sum();
            self.commit(
                &project,
                &record,
                RecordPurge {
                    files: destroy.len() as u64,
                    bytes,
                    at: None,
                },
            )
            .await?;
            let freed = self
                .read
                .files
                .destroy(&record.harp, &destroy)
                .map_err(AppError::Files)?;
            removal.files.bytes_freed = freed;
        }
        removal.applied = true;
        if request.scope == Scope::Remove {
            self.commit(&project, &record, ForgetSession {}).await?;
            removal.index_entry_removed = true;
        }
        removal.dead_letters = std::mem::take(self.dead_letters);
        Ok(removal)
    }

    /// What removing `harp` would do to its files: the machine-written
    /// conversation destroyed, everything else named and left.
    fn plan(&mut self, harp: &str, scope: Scope) -> Result<FilePlan, AppError> {
        let survey = self.read.files.survey(harp).map_err(AppError::Files)?;
        let mut plan = FilePlan::default();
        for file in survey {
            let transcript = file.rel.starts_with("transcripts/");
            let machine = transcript || file.rel.starts_with("native/");
            let (class, action, reason, into) =
                if machine && (transcript || scope != Scope::Transcript) {
                    (Class::Machine, "destroy", None, &mut plan.destroy)
                } else if machine {
                    (
                        Class::Machine,
                        "keep",
                        Some("not a transcript: `session purge` takes it".to_string()),
                        &mut plan.keep,
                    )
                } else {
                    (
                        Class::Authored,
                        "keep",
                        Some("authored: never destroyed".to_string()),
                        &mut plan.keep,
                    )
                };
            into.push(PlanItem {
                path: file.path,
                rel: file.rel,
                class,
                bytes: file.bytes,
                action,
                reason,
            });
        }
        Ok(plan)
    }

    /// Why `record` may not be removed, if it may not: its lock does not prove
    /// its owner dead (unless `even_if_live`), then its transcript's
    /// retention.
    fn refusal(
        &mut self,
        record: &SessionRecord,
        even_if_live: bool,
        scope: Scope,
    ) -> Result<Option<Refusal>, AppError> {
        let harp = record.harp.clone();
        let lock = self.read.liveness.lock_state(&harp);
        if lock != LockState::Free && !even_if_live {
            let reason = match lock {
                LockState::Held => "its lock is held: the owner is alive and running",
                _ => "it has no lock to prove the owner ended",
            };
            return Ok(Some(Refusal::NotProvenDead {
                harp,
                reason: reason.to_string(),
                verb: scope.verb().to_string(),
            }));
        }
        let last_active = self
            .read
            .files
            .last_active(&harp)
            .map_err(AppError::Files)?;
        let now = self.clock.now();
        let retention = Retention {
            cutoff: Some(session_index::stamp::days_before(now, self.retention_days)),
        };
        Ok(match retention.retains(Some(record), last_active) {
            None => None,
            Some(Retained::Kept) => Some(Refusal::Kept { harp }),
            Some(Retained::Recent { ended }) => Some(Refusal::Retained {
                harp,
                detail: match ended {
                    Some(at) => format!("it ended {}", session_index::stamp::rfc3339(at)),
                    None => "nothing records when it ended".to_string(),
                },
            }),
            Some(Retained::NoBound) => Some(Refusal::Retained {
                harp,
                detail: "no retention is configured".to_string(),
            }),
        })
    }
}

/// Whether `handled` recorded any event: a decision with none (a bind that
/// changes nothing) is applied and empty.
fn recorded(handled: &Handled) -> bool {
    // A fact answered DUPLICATE (a repeat bind, owner ruling 2026-11-04, S5
    // review item 8) recorded nothing new, though it re-reads its events.
    matches!(handled, Handled::Applied(_))
        && handled.books().iter().any(|book| !book.pages.is_empty())
}

/// Where a session's output folder goes under the name `to`: beside itself.
/// A session recording none has none.
fn beside(output_dir: &str, to: &str) -> String {
    match std::path::Path::new(output_dir).parent() {
        Some(parent) if !output_dir.is_empty() => parent.join(to).display().to_string(),
        _ => String::new(),
    }
}
