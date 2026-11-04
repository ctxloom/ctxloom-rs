//! ctxloom: the composition root. It builds the catalog from every crate's
//! `EN_US`, reads what it needs from the process (stdout's terminal, the
//! version, the lock protocol), and hands the command line the wiring. It
//! holds no logic of its own.

use std::io::{self, IsTerminal};
use std::process::ExitCode;

use ctxloom_cli::{Catalog, Context, Extension, Services};

/// The binary's en-US catalog: every crate's own.
fn catalog() -> Result<Catalog, angzarr_presentation::CatalogError> {
    let mut sources = ctxloom_lib::catalog_sources();
    sources.extend([
        ctxloom_cli::EN_US,
        session_cli::EN_US,
        hook_cli::EN_US,
        angzarr_yaml::EN_US,
        library::EN_US,
        library_application::EN_US,
        deps::EN_US,
        deps_application::EN_US,
        content_fs::EN_US,
        config_files::EN_US,
        editor_process::EN_US,
        surface_ports::EN_US,
        setup_application::EN_US,
    ]);
    Catalog::en_us(&sources)
}

/// What the commands call, over the default wiring.
#[derive(Default)]
struct Wired {
    /// Every command of the program, wired when one first needs the home.
    library: ctxloom_lib::Commands,
    /// The sessions use cases, built when a command first needs them.
    sessions: Option<ctxloom_lib::Sessions>,
}

impl Services for Wired {
    type Sessions = ctxloom_lib::Sessions;

    fn extension(&mut self) -> &mut dyn Extension {
        &mut self.library
    }

    fn sessions(
        &mut self,
    ) -> Result<&mut ctxloom_lib::Sessions, Box<dyn angzarr_diagnostic::Diagnostic>> {
        if self.sessions.is_none() {
            let wiring = ctxloom_lib::Wiring::from_process().map_err(|e| Box::new(e) as _)?;
            self.sessions = Some(wiring.sessions());
        }
        match self.sessions.as_mut() {
            Some(sessions) => Ok(sessions),
            None => unreachable!("built just above"),
        }
    }

    fn hooks(
        &mut self,
    ) -> Result<Option<Box<dyn hook_cli::HookService + '_>>, Box<dyn angzarr_diagnostic::Diagnostic>>
    {
        let hooks = self.library.hooks().map_err(|e| Box::new(e) as _)?;
        Ok(Some(Box::new(hooks)))
    }

    #[cfg(feature = "test-support")]
    fn session_name(&mut self) -> String {
        ctxloom_lib::session_name()
    }

    #[cfg(feature = "test-support")]
    fn run_id(
        &mut self,
        session: &str,
        incarnation: u64,
    ) -> Result<String, Box<dyn angzarr_diagnostic::Diagnostic>> {
        match ctxloom_lib::run_id(session, incarnation) {
            Ok(id) => Ok(id.to_string()),
            Err(e) => Err(Box::new(e)),
        }
    }

    #[cfg(feature = "test-support")]
    fn open_store(
        &mut self,
        project: &str,
    ) -> Result<String, Box<dyn angzarr_diagnostic::Diagnostic>> {
        let opened = ctxloom_lib::Wiring::from_process().and_then(|w| w.open_store(project));
        match opened {
            Ok(path) => Ok(path.display().to_string()),
            Err(e) => Err(Box::new(e)),
        }
    }

    #[cfg(feature = "test-support")]
    fn seed_session(
        &mut self,
        seed: &ctxloom_cli::SeedArgs,
    ) -> Result<String, Box<dyn angzarr_diagnostic::Diagnostic>> {
        let native = match (&seed.native_id, &seed.native_transcript) {
            (None, None) => None,
            (id, transcript) => Some((
                id.clone().unwrap_or_default(),
                transcript.clone().unwrap_or_default(),
            )),
        };
        let planted = ctxloom_lib::Wiring::from_process().and_then(|w| {
            w.seed_session(&ctxloom_lib::Seed {
                harp: seed.harp.clone(),
                project: seed.project.clone(),
                started_ago: seed.started_ago,
                ended_ago: seed.ended_ago,
                lock_free: seed.lock == "free",
                transcript: seed.transcript,
                keep: seed.keep,
                oneshot: seed.oneshot,
                native,
                other_files: seed.other_file.clone(),
            })
        });
        planted.map_err(|e| Box::new(e) as _)
    }

    #[cfg(feature = "test-support")]
    fn diagnostic_samples(&self) -> Vec<Box<dyn angzarr_diagnostic::Diagnostic>> {
        let mut all = ctxloom_store_sqlite::diagnostic_samples();
        all.extend(ctxloom_lib::diagnostic_samples());
        all.extend(library::diagnostic_samples());
        all.extend(library_application::diagnostic_samples());
        all.extend(deps::diagnostic_samples());
        all.extend(deps_application::diagnostic_samples());
        all.extend(content_fs::diagnostic_samples());
        all.extend(config_files::diagnostic_samples());
        all.extend(editor_process::diagnostic_samples());
        all.extend(surface_ports::diagnostic_samples());
        all.extend(setup_application::diagnostic_samples());
        all.extend(session_cli::diagnostic_samples());
        all.extend(hook_cli::diagnostic_samples());
        all.extend(session_files::diagnostic_samples());
        all.extend(sessions_application::diagnostic_samples());
        all.extend(sessions::diagnostic_samples());
        all
    }
}

fn main() -> ExitCode {
    let catalog = match catalog() {
        Ok(catalog) => catalog,
        Err(e) => {
            eprintln!("{}: {e}", ctxloom_cli::PROG);
            return ExitCode::FAILURE;
        }
    };
    let context = Context {
        version: env!("CARGO_PKG_VERSION").to_string(),
        lock_protocol: file_lock::LOCK_PROTOCOL,
        stdout_is_terminal: io::stdout().is_terminal(),
    };
    let (stdout, stderr) = (io::stdout(), io::stderr());
    ExitCode::from(ctxloom_cli::run(
        std::env::args_os(),
        &context,
        &catalog,
        &mut Wired::default(),
        &mut stdout.lock(),
        &mut stderr.lock(),
    ))
}

#[cfg(test)]
#[path = "main.test.rs"]
mod tests;
