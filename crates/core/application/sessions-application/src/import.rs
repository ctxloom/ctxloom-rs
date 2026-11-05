//! `session import`: Go's `~/.ctxloom/sessions/*/session.yaml` into session
//! streams, once (D14, plan 9). One-shot and idempotent: a session's stable
//! id is UUIDv5(project, harp) (D5 Q1), so a rerun finds each stream there and
//! adds nothing. Go's directories are only read, never written.
//!
//! A session of a project is found by its Go `project_dir`: the work root of
//! the registered project it equals; one registered nowhere is left, named.

use std::fmt;
use std::future::Future;

use angzarr_diagnostic::{code, Arg, Diagnostic, Report};
use angzarr_embedded::v1::{CommandBook, Cover, Uuid};
use angzarr_embedded::{CommandBookExt, Handled, OnRetryable, Registry};
use kernel_runner::Runner;
use ports::{Clock, CommandIds, Delay, Jitter, Project, ProjectIdentity};
use session_ports::{GoSession, GoSessions, ProjectStores};
use sessions::{
    claim_root, AdoptRotations, BindEngineSession, ClaimName, MintSession, NameClaimAggregate,
    NameClaimCommand, Origin, RecordEnd, RecordPurge, Rotation, SessionAggregate, SessionCommand,
    SessionRejection, SetKept, NAMES_DOMAIN, SESSION_DOMAIN,
};

use crate::{decode_go_sidecar, AppError, DeadLetter, GoEntry, GoSidecarError};

/// Import Go's sessions.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ImportRequest {}

/// A session imported: where, and its stable id.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Imported {
    pub harp: String,
    pub project: String,
    pub id: String,
}

/// Why a Go session was left where it is.
#[derive(Clone, PartialEq, Debug)]
pub enum Why {
    /// Its `project_dir` is the work root of no project the registry holds:
    /// there is no store to put it in.
    NoProject { project_dir: String },
    /// Its sidecar is of a generation this build does not read, or malformed.
    Sidecar(GoSidecarError),
    /// Its sidecar could not be read from disk.
    Unreadable(Report),
    /// A session of the project already holds its name.
    NameTaken { harp: String },
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for Why {
    fn code(&self) -> &str {
        match self {
            Why::NoProject { .. } => code!("left-no-project"),
            Why::Sidecar(cause) => cause.code(),
            Why::Unreadable(cause) => cause.code(),
            Why::NameTaken { .. } => code!("left-name-taken"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            Why::NoProject { project_dir } => vec![("project_dir", project_dir.as_str().into())],
            Why::Sidecar(cause) => cause.args(),
            Why::Unreadable(cause) => cause.args(),
            Why::NameTaken { harp } => vec![("harp", harp.as_str().into())],
        }
    }
}

/// A Go session the import left, and why.
#[derive(Clone, PartialEq, Debug)]
pub struct Left {
    pub harp: String,
    pub why: Why,
}

/// What an import came to.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ImportReport {
    pub imported: Vec<Imported>,
    /// Sessions a former import already holds, by harp: nothing added.
    pub already: Vec<String>,
    pub left: Vec<Left>,
    pub dead_letters: Vec<DeadLetter>,
}

/// The stable id of the session Go named `harp` in project `project_id`:
/// UUIDv5 of the harp text in the project's namespace
/// ([`sessions::project_namespace`]), so the same session imported twice is
/// the same stream (D5 Q1). OWNER-APPROVED and FIXED: these ids are the
/// streams' roots, and changing the derivation orphans every imported
/// session.
pub fn imported_id(project_id: &str, harp: &str) -> String {
    uuid::Uuid::new_v5(&sessions::project_namespace(project_id), harp.as_bytes()).to_string()
}

/// The import a driving adapter calls.
pub trait ImportService {
    fn import(
        &mut self,
        request: ImportRequest,
    ) -> impl Future<Output = Result<ImportReport, AppError>> + Send;
}

/// The importer: the only holder of Go's session directories. A service of its
/// own, as taskloom's importer is the only holder of its JSONL logs.
pub struct Importer<I, S, G, C, D> {
    pub identity: I,
    pub stores: S,
    pub go: G,
    pub clock: C,
    pub delay: D,
    pub retry: OnRetryable,
    pub(crate) registry: Registry,
    pub(crate) dead_letters: Vec<DeadLetter>,
}

impl<I, S, G, C, D> Importer<I, S, G, C, D> {
    pub fn new(identity: I, stores: S, go: G, clock: C, delay: D) -> Self {
        Importer {
            identity,
            stores,
            go,
            clock,
            delay,
            retry: kernel_runner::KERNEL_RETRY,
            registry: crate::registry(),
            dead_letters: Vec::new(),
        }
    }
}

impl<I, S, G, C, D> ImportService for Importer<I, S, G, C, D>
where
    I: ProjectIdentity + Send,
    S: ProjectStores + Send,
    G: GoSessions + Send,
    C: Clock + CommandIds + Send,
    D: Delay + Jitter + Send,
{
    async fn import(&mut self, request: ImportRequest) -> Result<ImportReport, AppError> {
        let ImportRequest {} = request;
        let mut sessions = self.go.go_sessions().map_err(AppError::Files)?;
        // A directory whose name is no usable harp is not a session.
        sessions.retain(|go| sessions::validate(&go.harp).is_ok());
        let projects = self.identity.every_project().map_err(AppError::Identity)?;
        let mut report = ImportReport::default();
        for go in sessions {
            let result = self.one(&projects, go, &mut report).await;
            if let Err(error) = result {
                let letters = std::mem::take(&mut self.dead_letters);
                return Err(match letters.is_empty() {
                    true => error,
                    false => AppError::DeadLettered {
                        error: Box::new(error),
                        dead_letters: letters,
                    },
                });
            }
        }
        report.dead_letters = std::mem::take(&mut self.dead_letters);
        Ok(report)
    }
}

/// How an import step came out for one Go session.
enum Step {
    /// Imported; `already` when a former import held it (its mint was a
    /// duplicate).
    Done {
        already: bool,
    },
    Left(Why),
}

impl<I, S, G, C, D> Importer<I, S, G, C, D>
where
    S: ProjectStores,
    C: Clock + CommandIds,
    D: Delay + Jitter + Send,
{
    /// Import one Go session, or say why it was left.
    async fn one(
        &mut self,
        projects: &[Project],
        go: GoSession,
        report: &mut ImportReport,
    ) -> Result<(), AppError> {
        let harp = go.harp.clone();
        let left = |why| Left {
            harp: harp.clone(),
            why,
        };
        let text = match go.sidecar {
            Ok(text) => text,
            Err(cause) => {
                report.left.push(left(Why::Unreadable(cause)));
                return Ok(());
            }
        };
        let entry = match decode_go_sidecar(&text) {
            Ok(entry) => entry,
            Err(error) => {
                report.left.push(left(Why::Sidecar(error)));
                return Ok(());
            }
        };
        let Some(project) = projects
            .iter()
            .find(|p| p.root.as_deref() == Some(entry.project_dir.as_str()))
        else {
            report.left.push(left(Why::NoProject {
                project_dir: entry.project_dir,
            }));
            return Ok(());
        };
        let id = imported_id(&project.id, &harp);
        match self
            .session(project, &id, &harp, &entry, go.keep_marker)
            .await?
        {
            Step::Left(why) => report.left.push(left(why)),
            Step::Done { already: true } => report.already.push(harp),
            Step::Done { already: false } => report.imported.push(Imported {
                harp,
                project: project.id.clone(),
                id,
            }),
        }
        Ok(())
    }

    /// The commands that make the Go session `harp` a session of `project`,
    /// each under a command id derived from the session's id and its step, so
    /// a rerun (or one that resumes a crashed one) is answered DUPLICATE and
    /// adds nothing.
    ///
    /// Go's `run_ends[]` and `end_reason` are NOT imported: owner ruling,
    /// 2026-11-03 ("Do NOT import run_ends[] in S4; they return with S10").
    async fn session(
        &mut self,
        project: &Project,
        id: &str,
        harp: &str,
        entry: &GoEntry,
        keep_marker: bool,
    ) -> Result<Step, AppError> {
        let root = uuid::Uuid::parse_str(id).map_err(|e| {
            AppError::Failed(
                Report::new("sessions-application-bad-id").arg("ext_err_txt", e.to_string()),
            )
        })?;
        let session_cover = Cover {
            domain: SESSION_DOMAIN.to_string(),
            root: Some(Uuid::from_bytes(*root.as_bytes())),
            edition: None,
        };
        let claim_cover = Cover {
            domain: NAMES_DOMAIN.to_string(),
            root: Some(claim_root(&project.id, harp)),
            edition: None,
        };
        let step_id =
            |step: &str| Uuid::from_bytes(*uuid::Uuid::new_v5(&root, step.as_bytes()).as_bytes());
        let store = self.stores.open(project).await.map_err(crate::failed)?;
        let mut runner = Runner {
            registry: &self.registry,
            clock: &mut self.clock,
            delay: &mut self.delay,
            retry: self.retry,
            dead_letters: &mut self.dead_letters,
        };
        // The name first: a name another session holds is not taken.
        let claim = NameClaimCommand::from(ClaimName {
            project_id: project.id.clone(),
            harp: harp.to_string(),
            session: id.to_string(),
        });
        let claimed = runner
            .run::<NameClaimAggregate, SessionRejection, _>(
                store,
                CommandBook::command_of(claim_cover, &claim),
                &step_id("claim"),
            )
            .await?;
        match claimed {
            Ok(_) => {}
            Err(refusal)
                if matches!(refusal.rejection, Some(SessionRejection::NamesTaken { .. })) =>
            {
                return Ok(Step::Left(Why::NameTaken {
                    harp: harp.to_string(),
                }))
            }
            Err(refusal) => return Err(refusal.into()),
        }
        let commands = commands(&project.id, harp, entry, keep_marker);
        let commands: Vec<(&'static str, CommandBook)> = commands
            .into_iter()
            .map(|(step, input)| {
                let book = match input {
                    Input::Command(command) => {
                        CommandBook::command_of(session_cover.clone(), &command)
                    }
                    Input::Bind(bind) => sessions::bind_book(session_cover.clone(), &bind),
                };
                (step, book)
            })
            .collect();
        let mut already = false;
        for (step, command) in commands {
            let handled = runner
                .run::<SessionAggregate, SessionRejection, _>(store, command, &step_id(step))
                .await??;
            if step == "mint" {
                already = matches!(handled, Handled::Duplicate(..));
            }
        }
        Ok(Step::Done { already })
    }
}

/// The commands that make a Go session a session, in order, each named for
/// its command id.
/// One step of an import: a command, or the bind, which is a fact (owner
/// ruling 2026-11-04, S5 review item 8).
enum Input {
    Command(SessionCommand),
    Bind(BindEngineSession),
}

trait IntoInput {
    fn into_input(self) -> Input;
}

impl<T: Into<SessionCommand>> IntoInput for T {
    fn into_input(self) -> Input {
        Input::Command(self.into())
    }
}

fn commands(
    project_id: &str,
    harp: &str,
    entry: &GoEntry,
    keep_marker: bool,
) -> Vec<(&'static str, Input)> {
    let origin = match entry.origin.as_str() {
        "agent" => Origin::Agent,
        "oneshot" => Origin::Oneshot,
        _ => Origin::Session,
    };
    let mut commands: Vec<(&'static str, Input)> = vec![(
        "mint",
        MintSession {
            harp: harp.to_string(),
            project_id: project_id.to_string(),
            project_dir: entry.project_dir.clone(),
            output_dir: entry.output_dir.clone(),
            engine: entry.backend.clone(),
            origin,
            started_at: Some(entry.started_at),
            depth: 0,
        }
        .into_input(),
    )];
    if !entry.session_id.is_empty() || !entry.transcript_path.is_empty() {
        commands.push((
            "bind",
            Input::Bind(BindEngineSession {
                native_id: entry.session_id.clone(),
                transcript_path: entry.transcript_path.clone(),
                engine_version: Some(entry.engine_version.clone()).filter(|v| !v.is_empty()),
            }),
        ));
    }
    if !entry.rotations.is_empty() {
        commands.push((
            "rotations",
            AdoptRotations {
                rotations: entry
                    .rotations
                    .iter()
                    .map(|r| Rotation {
                        native_id: r.session_id.clone(),
                        transcript_path: r.transcript_path.clone(),
                        rotated_at: Some(r.rotated_at),
                    })
                    .collect(),
            }
            .into_input(),
        ));
    }
    if entry.keep || keep_marker {
        commands.push(("keep", SetKept { kept: true }.into_input()));
    }
    if let Some(at) = entry.purged_at {
        commands.push((
            "purge",
            RecordPurge {
                files: 0,
                bytes: 0,
                at: Some(at),
            }
            .into_input(),
        ));
    }
    if let Some(at) = entry.ended_at {
        commands.push(("end", RecordEnd { at }.into_input()));
    }
    commands
}

#[cfg(test)]
#[path = "import.test.rs"]
mod tests;
