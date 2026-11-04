//! ctxloom: the composition root. It builds the catalog from every crate's
//! `EN_US`, reads what it needs from the process (stdout's terminal, the
//! version, the lock protocol), and hands the command line the wiring. It
//! holds no logic of its own.

use std::io::{self, IsTerminal};
use std::process::ExitCode;

use ctxloom_cli::{Catalog, Context, Extension, Services};

/// The binary's en-US catalog: every crate's own.
fn catalog() -> Result<Catalog, angzarr_presentation::CatalogError> {
    Catalog::en_us(&[
        ctxloom_cli::EN_US,
        ctxloom_store_sqlite::EN_US,
        ctxloom_lib::EN_US,
        angzarr_yaml::EN_US,
        library::EN_US,
        library_application::EN_US,
        content_fs::EN_US,
        config_files::EN_US,
    ])
}

/// What the commands call, over the default wiring.
struct Wired {
    library: ctxloom_lib::LibraryCommands,
}

impl Services for Wired {
    fn extension(&mut self) -> &mut dyn Extension {
        &mut self.library
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
    fn diagnostic_samples(&self) -> Vec<Box<dyn angzarr_diagnostic::Diagnostic>> {
        let mut all = ctxloom_store_sqlite::diagnostic_samples();
        all.extend(ctxloom_lib::diagnostic_samples());
        all.extend(library::diagnostic_samples());
        all.extend(library_application::diagnostic_samples());
        all.extend(content_fs::diagnostic_samples());
        all.extend(config_files::diagnostic_samples());
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
        &mut Wired {
            library: ctxloom_lib::LibraryCommands::new(),
        },
        &mut stdout.lock(),
        &mut stderr.lock(),
    ))
}

#[cfg(test)]
#[path = "main.test.rs"]
mod tests;
