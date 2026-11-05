//! Run a `session` command against the application.

use std::io::{self, Write};

use angzarr_diagnostic::Diagnostic;
use std::time::{SystemTime, UNIX_EPOCH};

use angzarr_presentation::{write_json, Format};
use isolation_application::{IsolationError, WorktreeUse};
use reclaim_application::{parse_age, ReclaimError, SweepRequest};
use session_index::View;
use sessions_application::{
    dead_letter_line, AppError, DeadLetter, ImportRequest, ImportService, KeepRequest, ListQuery,
    RemoveRequest, RemovedNothing, RenameRequest, Scope, SearchQuery, SessionService, ShowQuery,
    State, Timestamp, TranscriptQuery, STATES,
};

use crate::render::{
    conversation_text, import_text, removal_text, rows_text, show_text, sweep_text,
    transcripts_text, worktrees_text, EditDoc, ImportDoc, KeepDoc, RemovalDoc, Row, ShowDoc,
    TranscriptsDoc,
};
use crate::{
    CliFault, EditArgs, ListArgs, RemoveArgs, Session, SessionVerb, SweepArgs, Transcript,
    TranscriptListArgs, TranscriptVerb, Worktrees, WorktreesVerb,
};

/// A command that completed: what it says on the diagnostic channel besides
/// its answer.
#[derive(Debug, Default)]
pub struct Done {
    /// Notices (a report-only run ends with one), each a warning.
    pub notices: Vec<Box<dyn Diagnostic>>,
    /// Dead letters the kernel answered, loud lines for stderr: each almost
    /// certainly marks a bug.
    pub lines: Vec<String>,
}

/// A command that did not complete: why, its exit code, and the dead-letter
/// lines to say beside it.
#[derive(Debug)]
pub struct Failure {
    pub error: Box<dyn Diagnostic>,
    pub exit: u8,
    pub lines: Vec<String>,
}

/// Run `session`'s command on `service`, writing its answer to `out`.
pub fn run<S: SessionService + ImportService + WorktreeUse>(
    session: &Session,
    service: &mut S,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|e| {
            fault(CliFault::Runtime {
                ext_err_txt: e.to_string(),
            })
        })?;
    runtime.block_on(execute(session, service, format, out))
}

/// A fault of the command line itself.
fn fault(fault: CliFault) -> Failure {
    Failure {
        exit: fault.exit_code(),
        error: Box::new(fault),
        lines: Vec::new(),
    }
}

/// A use case's failure: its diagnostic, its exit code, and the dead letters
/// its kernel calls answered.
fn failure(error: AppError) -> Failure {
    Failure {
        exit: error.exit_code(),
        lines: error
            .dead_letters()
            .iter()
            .map(|l| dead_letter_line(&l.letter))
            .collect(),
        error: Box::new(error),
    }
}

/// The answer written, or why it could not be.
fn wrote(written: io::Result<()>) -> Result<(), Failure> {
    written.map_err(|e| {
        fault(CliFault::Output {
            ext_err_txt: e.to_string(),
        })
    })
}

/// A `--state` word as a state; the word that is none is refused before
/// anything is asked.
fn state_of(word: Option<&String>) -> Result<Option<State>, Failure> {
    let Some(word) = word else { return Ok(None) };
    State::parse(word).map(Some).ok_or_else(|| {
        fault(CliFault::BadState {
            value: word.clone(),
            states: STATES.join(", "),
        })
    })
}

fn dead_letters(letters: &[DeadLetter]) -> Done {
    Done {
        notices: Vec::new(),
        lines: letters
            .iter()
            .map(|l| dead_letter_line(&l.letter))
            .collect(),
    }
}

/// A list of rows in `format`.
fn rows(views: &[View], format: Format, out: &mut dyn Write) -> Result<Done, Failure> {
    let written = match format {
        Format::Json => {
            let rows: Vec<Row> = views.iter().map(Row::of).collect();
            write_json(out, &rows)
        }
        Format::Text => rows_text(views, out),
    };
    wrote(written)?;
    Ok(Done::default())
}

async fn execute<S: SessionService + ImportService + WorktreeUse>(
    session: &Session,
    service: &mut S,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    match &session.verb {
        None => list(service, &ListArgs::default(), format, out).await,
        Some(SessionVerb::List(args)) => list(service, args, format, out).await,
        Some(SessionVerb::Search(args)) => {
            let query = SearchQuery {
                list: ListQuery {
                    project: None,
                    all: args.all,
                    state: state_of(args.state.as_ref())?,
                },
                words: args.words.clone(),
            };
            let views = service.search(query).await.map_err(failure)?;
            rows(&views, format, out)
        }
        Some(SessionVerb::Show(args)) => {
            let view = service
                .show(ShowQuery {
                    project: None,
                    harp: args.harp.clone(),
                })
                .await
                .map_err(failure)?;
            wrote(match format {
                Format::Json => write_json(out, &ShowDoc::of(&view)),
                Format::Text => show_text(&view, out),
            })?;
            Ok(Done::default())
        }
        Some(SessionVerb::Edit(args)) => edit(service, args, format, out).await,
        Some(SessionVerb::Keep(args)) => {
            let kept = service
                .keep(KeepRequest {
                    project: None,
                    harp: args.harp.clone(),
                    keep: !args.undo,
                })
                .await
                .map_err(failure)?;
            wrote(match format {
                Format::Json => write_json(
                    out,
                    &KeepDoc {
                        harp: kept.harp.clone(),
                        kept: kept.kept,
                    },
                ),
                Format::Text if kept.kept => writeln!(
                    out,
                    "{} is marked keep: its transcript is kept at any age",
                    kept.harp
                ),
                Format::Text => writeln!(
                    out,
                    "{} is no longer marked keep: its transcript is reclaimable once session_transcript_retention has passed since it ended",
                    kept.harp
                ),
            })?;
            Ok(dead_letters(&kept.dead_letters))
        }
        Some(SessionVerb::Remove(args)) => remove(service, args, Scope::Remove, format, out).await,
        Some(SessionVerb::Purge(args)) => remove(service, args, Scope::Purge, format, out).await,
        Some(SessionVerb::Sweep(args)) => sweep(service, args, format, out).await,
        Some(SessionVerb::Import) => {
            let report = service.import(ImportRequest {}).await.map_err(failure)?;
            wrote(match format {
                Format::Json => write_json(out, &ImportDoc::of(&report)),
                Format::Text => import_text(&report, out),
            })?;
            Ok(dead_letters(&report.dead_letters))
        }
        Some(SessionVerb::Transcript(transcript)) => {
            transcripts(service, transcript, format, out).await
        }
        Some(SessionVerb::Worktrees(worktrees)) => worktree_verb(service, worktrees, format, out),
    }
}

/// An isolation use case's failure: its diagnostic and exit code.
fn isolation_failure(error: IsolationError) -> Failure {
    Failure {
        exit: error.exit_code(),
        error: Box::new(error),
        lines: Vec::new(),
    }
}

/// `session worktrees ...`: the scratch checkouts, listed or purged. A purge
/// reports first, and an action that changed nothing refuses (exit 2) after
/// the report is out, so an unattended run that found nothing safe cannot be
/// mistaken for one that cleaned up.
fn worktree_verb<S: WorktreeUse>(
    service: &mut S,
    worktrees: &Worktrees,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    let (harp, apply) = match &worktrees.verb {
        None => (None, false),
        Some(WorktreesVerb::List(args)) => (args.harp.as_deref(), false),
        Some(WorktreesVerb::Purge(args)) => (Some(args.harp.as_str()), args.yes),
    };
    let report = service.worktrees(harp, apply).map_err(isolation_failure)?;
    wrote(match format {
        Format::Json => write_json(out, &report),
        Format::Text => worktrees_text(&report, out),
    })?;
    let mut done = Done::default();
    let Some(WorktreesVerb::Purge(args)) = &worktrees.verb else {
        return Ok(done);
    };
    if !args.yes {
        done.notices.push(Box::new(RemovedNothing {
            command: format!("ctxloom session worktrees purge {} --yes", args.harp),
        }));
    } else if report.reaped == 0 {
        return Err(isolation_failure(IsolationError::NothingRemoved {
            harp: args.harp.clone(),
            spared: report.spared,
            skipped: report.skipped,
        }));
    }
    Ok(done)
}

async fn list<S: SessionService>(
    service: &mut S,
    args: &ListArgs,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    let query = ListQuery {
        project: None,
        all: args.all,
        state: state_of(args.state.as_ref())?,
    };
    let views = service.list(query).await.map_err(failure)?;
    rows(&views, format, out)
}

async fn edit<S: SessionService>(
    service: &mut S,
    args: &EditArgs,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    // Read through `Option`, not emptiness: `--name ""` is a caller asking for
    // an invalid name and must reach the validator.
    let Some(to) = &args.name else {
        return Err(fault(CliFault::EditNothing {
            harp: args.harp.clone(),
        }));
    };
    let renamed = service
        .rename(RenameRequest {
            project: None,
            harp: args.harp.clone(),
            to: to.clone(),
        })
        .await
        .map_err(failure)?;
    wrote(match format {
        Format::Json => write_json(out, &EditDoc::of(&renamed)),
        Format::Text => writeln!(out, "renamed {} \u{2192} {}", renamed.from, renamed.to),
    })?;
    Ok(dead_letters(&renamed.dead_letters))
}

async fn remove<S: SessionService + WorktreeUse>(
    service: &mut S,
    args: &RemoveArgs,
    scope: Scope,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    let removal = service
        .remove(RemoveRequest {
            scope,
            project: None,
            harp: args.harp.clone(),
            yes: args.yes,
            even_if_live: args.even_if_live,
        })
        .await
        .map_err(failure)?;
    // Emptying a session covers the scratch worktrees living inside it, and
    // reaches exactly the verdicts the worktree leaf reaches on its own (Go:
    // `session purge` drove isolation.ReapWorktrees). A refused purge touches
    // nothing; a session with no directory has none.
    let worktrees = if scope == Scope::Purge && removal.refusal.is_none() {
        match service.worktrees(Some(&args.harp), args.yes) {
            Ok(report) => Some(report).filter(|r| !r.worktrees.is_empty()),
            Err(IsolationError::NoSession { .. }) => None,
            Err(error) => return Err(isolation_failure(error)),
        }
    } else {
        None
    };
    wrote(match format {
        Format::Json => write_json(out, &RemovalDoc::of(&removal, worktrees.as_ref())),
        Format::Text => removal_text(&removal, out).and_then(|()| match &worktrees {
            Some(report) => worktrees_text(report, out),
            None => Ok(()),
        }),
    })?;
    let mut done = dead_letters(&removal.dead_letters);
    if let Some(refusal) = removal.refusal {
        return Err(Failure {
            error: Box::new(refusal),
            exit: 2,
            lines: done.lines,
        });
    }
    if !args.yes {
        let flag = if args.even_if_live {
            " --even-if-live"
        } else {
            ""
        };
        done.notices.push(Box::new(RemovedNothing {
            command: format!("ctxloom {} {} --yes{flag}", scope.verb(), args.harp),
        }));
    }
    Ok(done)
}

/// `session sweep`: report every session's fate, and with `--yes` act on
/// exactly that.
async fn sweep<S: SessionService + WorktreeUse>(
    service: &mut S,
    args: &SweepArgs,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    let now = Timestamp {
        seconds: now,
        nanos: 0,
    };
    let purge_cutoff = args
        .purge_older_than
        .as_deref()
        .map(|age| parse_age("--purge-older-than", age, now))
        .transpose()
        .map_err(reclaim_failure)?;
    let report = reclaim_application::sweep(
        service,
        &SweepRequest {
            all_projects: args.all_projects,
            purge_cutoff,
            yes: args.yes,
        },
    )
    .await
    .map_err(reclaim_failure)?;
    wrote(match format {
        Format::Json => write_json(out, &report),
        Format::Text => sweep_text(&report, out),
    })?;
    let mut done = Done::default();
    let failed = report.counts.get("failed").copied().unwrap_or(0);
    if args.yes && failed > 0 {
        return Err(fault(CliFault::SweepFailed { count: failed }));
    }
    if !args.yes {
        let age = args
            .purge_older_than
            .as_ref()
            .map(|age| format!(" --purge-older-than {age}"))
            .unwrap_or_default();
        let all = if args.all_projects {
            " --all-projects"
        } else {
            ""
        };
        done.notices.push(Box::new(RemovedNothing {
            command: format!("ctxloom session sweep{all}{age} --yes"),
        }));
    }
    Ok(done)
}

/// A reclaim use case's failure: its diagnostic and exit code.
fn reclaim_failure(error: ReclaimError) -> Failure {
    Failure {
        exit: error.exit_code(),
        error: Box::new(error),
        lines: Vec::new(),
    }
}

async fn transcripts<S: SessionService + WorktreeUse>(
    service: &mut S,
    transcript: &Transcript,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    match &transcript.verb {
        None => transcript_list(service, &TranscriptListArgs::default(), format, out).await,
        Some(TranscriptVerb::List(args)) => transcript_list(service, args, format, out).await,
        Some(TranscriptVerb::Purge(args)) => {
            remove(service, args, Scope::Transcript, format, out).await
        }
        Some(TranscriptVerb::Show(args)) => {
            let text = service.transcript(&args.harp).await.map_err(failure)?;
            wrote(match format {
                Format::Json if text.ends_with('\n') => write!(out, "{text}"),
                Format::Json => writeln!(out, "{text}"),
                Format::Text => conversation_text(&text, out),
            })?;
            Ok(Done::default())
        }
    }
}

async fn transcript_list<S: SessionService>(
    service: &mut S,
    args: &TranscriptListArgs,
    format: Format,
    out: &mut dyn Write,
) -> Result<Done, Failure> {
    let rows = service
        .transcripts(TranscriptQuery {
            project: None,
            all: args.all,
            harp: args.harp.clone(),
        })
        .await
        .map_err(failure)?;
    wrote(match format {
        Format::Json => write_json(out, &TranscriptsDoc::of(&rows)),
        Format::Text => transcripts_text(&rows, out),
    })?;
    Ok(Done::default())
}

#[cfg(test)]
#[path = "run.test.rs"]
mod tests;
