//! The `session` and `hook` commands mounted on the command line: the
//! sessions use cases over their default adapters, and the engines'
//! callbacks, run as the extension's nouns.

use std::io::Write;
use std::sync::Arc;

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{Catalog, Format};
use clap::{ArgMatches, Command, FromArgMatches};
use cli_support::say_warning;
use ctxloom_cli::{shaped, Call, CliError, Failure, PROG};
use hook_cli::HookService;
use sessions_application::{BindRequest, SessionService};

use crate::{Commands, Sessions};

/// The nouns these commands own.
pub(crate) const NOUNS: [&str; 2] = ["session", "hook"];

/// The environment variable naming the session the hooks run in.
const SESSION_HARP: &str = "CTXLOOM_SESSION_HARP";

/// The two nouns, shaped like the command line's own.
pub(crate) fn nouns() -> Vec<Command> {
    vec![shaped(session_cli::noun()), shaped(hook_cli::noun())]
}

/// `noun`'s own matches.
fn matches_of<'a>(call: &Call<'a>, noun: &str) -> &'a ArgMatches {
    call.matches
        .subcommand_matches(noun)
        .unwrap_or_else(|| unreachable!("the line named {noun}"))
}

/// A failure of the command line itself: exit 1.
fn failed(cause: Box<dyn Diagnostic>) -> Failure {
    Failure { cause, exit: 1 }
}

impl Commands {
    /// The sessions use cases, built when a command first needs them: a
    /// command that needs none (the statusline hook) works where there is no
    /// home.
    fn sessions(&mut self) -> Result<&mut Sessions, Box<dyn Diagnostic>> {
        if self.sessions.is_none() {
            let wiring = crate::Wiring::new(Arc::clone(&self.env)).map_err(|e| Box::new(e) as _)?;
            self.sessions = Some(wiring.sessions());
        }
        match self.sessions.as_mut() {
            Some(sessions) => Ok(sessions),
            None => unreachable!("built just above"),
        }
    }

    /// `session ...`: the command run on the sessions use cases, its notices
    /// and dead letters said beside its answer.
    pub(crate) fn session(
        &mut self,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        let command = session_cli::Session::from_arg_matches(matches_of(call, "session"))
            .unwrap_or_else(|e| unreachable!("clap parsed it: {e}"));
        let format = call.format();
        let service = self.sessions().map_err(failed)?;
        match session_cli::run(&command, service, format, out) {
            Ok(done) => {
                for line in &done.lines {
                    let _ = writeln!(err, "{PROG}: {line}");
                }
                for notice in &done.notices {
                    say_warning(&**notice, PROG, format, catalog, err);
                }
                Ok(0)
            }
            Err(failure) => {
                for line in &failure.lines {
                    let _ = writeln!(err, "{PROG}: {line}");
                }
                Err(Failure {
                    cause: failure.failure.cause,
                    exit: failure.failure.exit,
                })
            }
        }
    }

    /// `hook ...`: an engine's callback. Stdout is the engine's input and
    /// only that; what a hook can foresee going wrong it warns of on the
    /// diagnostic channel and still succeeds. `input` is its payload.
    pub(crate) fn hook(
        &mut self,
        input: &str,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        let hook = hook_cli::Hook::from_arg_matches(matches_of(call, "hook"))
            .unwrap_or_else(|e| unreachable!("clap parsed it: {e}"));
        let format: Format = call.format();
        let harp = self.env.utf8(SESSION_HARP).unwrap_or_default().to_string();
        let written = match hook.verb {
            hook_cli::HookVerb::Hud => out.write_all(hook_cli::hud(input, &harp, 0).as_bytes()),
            hook_cli::HookVerb::SessionBind => {
                let outcome =
                    hook_cli::session_bind(hook.engine.as_deref(), input, &harp, &mut Bind(self))
                        .map_err(failed)?;
                for warning in &outcome.warnings {
                    say_warning(warning, PROG, format, catalog, err);
                }
                out.write_all(outcome.stdout.as_bytes())
            }
            hook_cli::HookVerb::SessionStart => {
                let engine = hook.engine.as_deref().unwrap_or(hook_cli::CLAUDE_CODE);
                let hooks = self.hooks().map_err(|e| failed(Box::new(e)))?;
                let answer = hooks.session_start(engine, input).map_err(failed)?;
                out.write_all(answer.as_bytes())
            }
            hook_cli::HookVerb::SkillMates => {
                let engine = hook.engine.as_deref().unwrap_or(hook_cli::CLAUDE_CODE);
                let hooks = self.hooks().map_err(|e| failed(Box::new(e)))?;
                let answer = hooks.skill_mates(engine, input).map_err(failed)?;
                out.write_all(answer.as_bytes())
            }
        };
        written.map_err(|e| {
            Failure::new(
                CliError::Output {
                    detail: e.to_string(),
                },
                1,
            )
        })?;
        Ok(0)
    }
}

/// What binds a session for the hook: the sessions use case, blocked on.
struct Bind<'a>(&'a mut Commands);

impl hook_cli::Binding for Bind<'_> {
    fn bind(
        &mut self,
        harp: &str,
        native_id: &str,
        transcript_path: &str,
    ) -> Result<(), Box<dyn Diagnostic>> {
        let service = self.0.sessions()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .map_err(|e| -> Box<dyn Diagnostic> {
                Box::new(CliError::Output {
                    detail: e.to_string(),
                })
            })?;
        runtime
            .block_on(service.bind(BindRequest {
                project: None,
                harp: harp.to_string(),
                native_id: native_id.to_string(),
                transcript_path: transcript_path.to_string(),
            }))
            .map(drop)
            .map_err(|e| -> Box<dyn Diagnostic> { Box::new(e) })
    }
}

#[cfg(test)]
#[path = "sessions.test.rs"]
mod tests;
