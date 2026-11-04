//! The program's commands as one extension of the command line: the
//! library's (S1) and the remote and dependency ones (S3). They share a noun,
//! `bundle` (the library inspects bundles; `push` and `move` publish and
//! relocate them), and the library reads the remotes through the dependency
//! use cases, so both are built together.

use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use angzarr_presentation::Catalog;
use clap::Command;
use config_files::{find_app_dir, Surroundings};
use content_fs::ContentFs;
use ctxloom_cli::{Call, Extension, Failure};
use deps_application::Deps;
use deps_files::ProjectFiles;
use forge_github::GitHub;
use remote_git::GitRemotes;

use crate::setup::{SetupUseCases, NOUNS as SETUP_NOUNS};
use crate::{LibError, LibraryCommands, Wiring};

/// The nouns the dependency commands own outright.
const OWN_NOUNS: [&str; 2] = ["remote", "deps"];

/// The dependency use cases over the process's home, working directory and
/// project.
fn build_deps() -> Result<Deps, LibError> {
    let wiring = Wiring::from_process()?;
    let fs = wiring.fs();
    let around = Surroundings {
        root_env: std::env::var("CTXLOOM_ROOT").ok(),
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        home: wiring.home().to_path_buf(),
        temp_root: std::env::temp_dir(),
    };
    let app = find_app_dir(fs.as_ref(), &around);
    Ok(Deps::new(
        Arc::new(GitRemotes::new(
            Arc::clone(&fs),
            app.path.join("cache/repos"),
        )),
        Arc::new(ProjectFiles::new(Arc::clone(&fs), app.path.clone())),
        Arc::new(ContentFs::new(fs, &app.path)),
    )
    .with_forge(Arc::new(GitHub::from_process())))
}

/// Every command of the program, wired when one first needs the home.
#[derive(Default)]
pub struct Commands {
    wired: Option<Wired>,
}

struct Wired {
    deps: Rc<Deps>,
    library: LibraryCommands,
    setup: SetupUseCases,
}

impl Commands {
    pub fn new() -> Commands {
        Commands::default()
    }

    fn wired(&mut self) -> Result<&mut Wired, LibError> {
        if self.wired.is_none() {
            let deps = Rc::new(build_deps()?);
            let library = LibraryCommands::new().with_remotes(deps.clone());
            let setup = SetupUseCases::build(deps.clone())?;
            self.wired = Some(Wired {
                deps,
                library,
                setup,
            });
        }
        Ok(self
            .wired
            .as_mut()
            .unwrap_or_else(|| unreachable!("just built")))
    }
}

impl Commands {
    /// A setup command: `clean`, `doctor` or `init`.
    fn setup(
        &mut self,
        noun: &str,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        let wired = self.wired().map_err(|e| Failure {
            cause: Box::new(e),
            exit: 1,
        })?;
        let uses = setup_cli::UseCases {
            cleaner: &wired.setup.cleaner,
            doctor: &wired.setup.doctor,
            init: &wired.setup.init,
        };
        let inner = setup_cli::Call {
            matches: call.matches,
            explicit_format: call.explicit_format,
            default_format: call.default_format,
        };
        setup_cli::run(&uses, noun, &inner, catalog, out, err).map_err(|f| Failure {
            cause: f.cause,
            exit: f.exit,
        })
    }
}

impl Extension for Commands {
    fn nouns(&self) -> Vec<Command> {
        let mut nouns: Vec<Command> = library_cli::nouns()
            .into_iter()
            .map(|noun| {
                if noun.get_name() != "bundle" {
                    return noun;
                }
                deps_cli::bundle_verbs()
                    .into_iter()
                    .fold(noun, |noun, verb| noun.subcommand(verb))
            })
            .collect();
        nouns.extend(deps_cli::nouns());
        nouns.extend(setup_cli::nouns());
        nouns
    }

    fn run(
        &mut self,
        noun: &str,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        if SETUP_NOUNS.contains(&noun) {
            return self.setup(noun, call, catalog, out, err);
        }
        let verb = call
            .matches
            .subcommand_matches(noun)
            .and_then(|m| m.subcommand_name())
            .unwrap_or_default()
            .to_string();
        let wired = self.wired().map_err(|e| Failure {
            cause: Box::new(e),
            exit: 1,
        })?;
        let ours =
            OWN_NOUNS.contains(&noun) || (noun == "bundle" && deps_cli::is_bundle_verb(&verb));
        if !ours {
            return wired.library.run(noun, call, catalog, out, err);
        }
        let inner = deps_cli::Call {
            matches: call.matches,
            explicit_format: call.explicit_format,
            default_format: call.default_format,
            stdout_is_terminal: call.stdout_is_terminal,
        };
        deps_cli::run(&wired.deps, noun, &inner, catalog, out, err).map_err(|f| Failure {
            cause: f.cause,
            exit: f.exit,
        })
    }
}

#[cfg(test)]
#[path = "commands.test.rs"]
mod tests;
