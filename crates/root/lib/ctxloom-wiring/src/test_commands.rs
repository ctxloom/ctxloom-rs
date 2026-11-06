//! The hidden test-only commands (`ctxloom test ...`), mounted on the command
//! line as an [`Extension`](ctxloom_cli::Extension) noun beside the others,
//! so the command line's `Services` asks nothing of a build that has them and
//! a build without them alike. They exist only under `test-support`: the
//! exit ladder, the `--format` matrix, the error envelope, every diagnostic,
//! the id mints, the store open and the seed, reached through the binary
//! because no real command does them.

use std::io::Write;

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{render, write_json, Catalog, Render};
use clap::{Args, Command, FromArgMatches, Subcommand};
use ctxloom_cli::{shaped, Call, CliError, Failure};
use serde::Serialize;

use crate::{LibError, Seed, Wiring};

/// The noun these commands own.
pub(crate) const NOUN: &str = "test";

/// One value of every diagnostic the binary can raise besides the wiring's
/// and the command line's own: what the composition root hands in.
pub type Samples = fn() -> Vec<Box<dyn Diagnostic>>;

#[derive(Subcommand, Debug)]
enum TestVerb {
    /// Refuse (exit 2)
    Refuse,
    /// Report fatal findings (exit 3)
    Findings,
    /// Print a sample list
    Sample {
        /// Print the empty list
        #[arg(long)]
        empty: bool,
    },
    /// Print every diagnostic the binary can raise, as envelopes
    Diagnostics,
    /// Mint a session id
    SessionId,
    /// Print a run's id
    RunId {
        /// The session's id (a UUID)
        session: String,
        /// The incarnation's number
        incarnation: u64,
    },
    /// Open a project's store under the home
    OpenStore {
        /// The project's id
        project: String,
    },
    /// Plant a session in the cwd's project store, and the files a fixture
    /// writes beside it; print its stable id
    SeedSession(SeedArgs),
}

/// What `ctxloom test seed-session` plants (see the acceptance tests).
#[derive(Args, Debug, Clone, Default)]
struct SeedArgs {
    /// The session's name
    harp: String,
    /// Another project than the cwd's
    #[arg(long)]
    project: Option<String>,
    /// Seconds ago it was minted
    #[arg(long, default_value_t = 86_400)]
    started_ago: u64,
    /// Seconds ago its last run ended; left out, it never ended
    #[arg(long)]
    ended_ago: Option<u64>,
    /// `free` makes its lock file with nothing holding it; `none` makes none
    #[arg(long, default_value = "none")]
    lock: String,
    /// Record the canonical transcript Go's fixture records
    #[arg(long)]
    transcript: bool,
    /// Mark its transcript kept
    #[arg(long)]
    keep: bool,
    /// An internal one-shot's session
    #[arg(long)]
    oneshot: bool,
    /// The engine's session it is bound to
    #[arg(long)]
    native_id: Option<String>,
    /// The engine's transcript it is bound to
    #[arg(long)]
    native_transcript: Option<String>,
    /// An authored file in the session directory
    #[arg(long)]
    other_file: Vec<String>,
}

impl From<SeedArgs> for Seed {
    fn from(args: SeedArgs) -> Seed {
        let native = match (args.native_id, args.native_transcript) {
            (None, None) => None,
            (id, transcript) => Some((id.unwrap_or_default(), transcript.unwrap_or_default())),
        };
        Seed {
            harp: args.harp,
            project: args.project,
            started_ago: args.started_ago,
            ended_ago: args.ended_ago,
            lock_free: args.lock == "free",
            transcript: args.transcript,
            keep: args.keep,
            oneshot: args.oneshot,
            native,
            other_files: args.other_file,
        }
    }
}

/// The noun, shaped like the command line's own and hidden from its help.
pub(crate) fn noun() -> Command {
    shaped(TestVerb::augment_subcommands(
        Command::new(NOUN)
            .about("Commands for the tests only")
            .hide(true)
            .subcommand_required(true),
    ))
}

#[derive(Serialize)]
struct Row {
    name: &'static str,
    count: u64,
}

/// A list of rows: the sample document.
struct Sample(Vec<Row>);

impl Render for Sample {
    fn json(&self) -> impl Serialize + '_ {
        &self.0
    }

    fn text(&self, out: &mut dyn Write) -> std::io::Result<()> {
        if self.0.is_empty() {
            return writeln!(out, "(none)");
        }
        for row in &self.0 {
            writeln!(out, "{}  {}", row.name, row.count)?;
        }
        Ok(())
    }
}

/// A failure of the command line itself: exit 1.
fn failed(cause: impl Diagnostic + 'static) -> Failure {
    Failure::new(cause, 1)
}

fn output(written: std::io::Result<()>) -> Result<(), Failure> {
    written.map_err(|e| {
        failed(CliError::Output {
            detail: e.to_string(),
        })
    })
}

/// Run the `test` noun's verb.
pub(crate) fn run(
    wiring: impl FnOnce() -> Result<Wiring, LibError>,
    samples: Samples,
    call: &Call<'_>,
    catalog: &Catalog,
    out: &mut dyn Write,
) -> Result<u8, Failure> {
    let matches = call
        .matches
        .subcommand_matches(NOUN)
        .unwrap_or_else(|| unreachable!("the line named {NOUN}"));
    let verb = TestVerb::from_arg_matches(matches)
        .unwrap_or_else(|_| unreachable!("the command line parsed {NOUN}"));
    let format = call.format();
    match verb {
        TestVerb::Refuse => Err(Failure::new(
            LibError::TestRefused {
                verb: "refuse".to_string(),
            },
            2,
        )),
        TestVerb::Findings => Err(Failure::new(LibError::TestFindings { count: 1 }, 3)),
        TestVerb::Sample { empty } => output(render(out, format, &Sample(sample(empty)))),
        TestVerb::Diagnostics => output(diagnostics(samples, catalog, out)),
        TestVerb::SessionId => output(writeln!(out, "{}", crate::session_name())),
        TestVerb::RunId {
            session,
            incarnation,
        } => {
            let id = crate::run_id(&session, incarnation).map_err(failed)?;
            output(writeln!(out, "{id}"))
        }
        TestVerb::SeedSession(seed) => {
            let id = wiring()
                .and_then(|w| w.seed_session(&seed.into()))
                .map_err(failed)?;
            output(writeln!(out, "{id}"))
        }
        TestVerb::OpenStore { project } => {
            let path = wiring()
                .and_then(|w| w.open_store(&project))
                .map_err(failed)?;
            output(writeln!(out, "{}", path.display()))
        }
    }
    .map(|()| 0)
}

fn sample(empty: bool) -> Vec<Row> {
    if empty {
        return Vec::new();
    }
    vec![
        Row {
            name: "alpha",
            count: 1,
        },
        Row {
            name: "beta",
            count: 2,
        },
    ]
}

/// Every diagnostic's envelope, as one json array.
fn diagnostics(samples: Samples, catalog: &Catalog, out: &mut dyn Write) -> std::io::Result<()> {
    let mut all = ctxloom_cli::diagnostic_samples();
    all.extend(crate::diagnostic_samples());
    all.extend(samples());
    let envelopes: Vec<_> = all.iter().map(|d| catalog.envelope(&**d)).collect();
    write_json(out, &envelopes)
}

#[cfg(test)]
#[path = "test_commands.test.rs"]
mod tests;
