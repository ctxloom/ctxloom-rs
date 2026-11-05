//! The library commands (S1) wired: the content and config files over the
//! process's filesystem, the application over them, and the driving adapter
//! joined to the command line as an extension.

use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use angzarr_presentation::Catalog;
use companion_probe::ProcessProbe;
use config_files::{ConfigFilesFs, ScaffoldFs};
use content_fs::{ContentFs, UserFilesFs};
use ctxloom_cli::{Call, Extension, Failure};
use ctxloom_ports::Executables;
use deps_ports::RemoteLibrary;
use editor_process::ProcessEditor;
use file_system::{Fs, Kind};
use library_application::{Library, WritePorts};
use process_env::EnvSnapshot;

use crate::{LibError, Wiring};

/// Where a program is on the `PATH`, by the filesystem seam.
pub(crate) struct PathSearch {
    pub(crate) fs: Arc<dyn Fs>,
    pub(crate) dirs: Vec<PathBuf>,
}

impl Executables for PathSearch {
    fn find(&self, name: &str) -> Option<String> {
        self.dirs.iter().find_map(|dir| {
            let candidate = dir.join(name);
            let meta = self.fs.metadata(&candidate).ok()?;
            (meta.kind == Kind::File && meta.executable).then(|| candidate.display().to_string())
        })
    }
}

/// The library's commands as an extension of the command line. The library is
/// wired when a command first needs it, so a command that needs nothing of the
/// home (`version`) does not.
pub struct LibraryCommands {
    /// The environment the process was launched in.
    env: Arc<EnvSnapshot>,
    library: Option<Library>,
    /// What the library reads of the remotes, where the program has them.
    remotes: Option<Rc<dyn RemoteLibrary>>,
}

impl LibraryCommands {
    pub fn new(env: Arc<EnvSnapshot>) -> LibraryCommands {
        LibraryCommands {
            env,
            library: None,
            remotes: None,
        }
    }

    /// The library reading the remotes too.
    pub fn with_remotes(mut self, remotes: Rc<dyn RemoteLibrary>) -> LibraryCommands {
        self.remotes = Some(remotes);
        self
    }

    fn wired(&mut self) -> Result<&Library, LibError> {
        if self.library.is_none() {
            let wiring = Wiring::new(Arc::clone(&self.env))?;
            self.library = Some(build(&wiring, self.remotes.clone())?);
        }
        Ok(self
            .library
            .as_ref()
            .unwrap_or_else(|| unreachable!("just built")))
    }
}

pub(crate) fn build(
    wiring: &Wiring,
    remotes: Option<Rc<dyn RemoteLibrary>>,
) -> Result<Library, LibError> {
    let fs = wiring.fs();
    let home = wiring.home().to_path_buf();
    let (around, app) = wiring.locate();
    let content = ContentFs::new(Arc::clone(&fs), &app.path);
    let files = ConfigFilesFs::new(Arc::clone(&fs), app, &home);
    let binaries = wiring.path_search();
    // Overrides are text: a variable that is not UTF-8 is none (it is never
    // an error, as `std::env::vars` made it).
    let environ = wiring.env().vars_utf8().into_iter().collect();
    let ports = WritePorts {
        user: Arc::new(UserFilesFs::new(Arc::clone(&fs))),
        scaffold: Arc::new(ScaffoldFs::new(Arc::clone(&fs), &around.cwd)),
        editor: Arc::new(ProcessEditor::new(Arc::clone(&fs), &around.temp_root)),
        probe: Arc::new(ProcessProbe::standard(Arc::clone(wiring.env()))),
    };
    let library = Library::new(
        Arc::new(content),
        Arc::new(files),
        Arc::new(binaries),
        environ,
    )
    .with_write_ports(ports);
    Ok(match remotes {
        Some(remotes) => library.with_remotes(remotes),
        None => library,
    })
}

impl Extension for LibraryCommands {
    fn nouns(&self) -> Vec<clap::Command> {
        library_cli::nouns()
    }

    fn run(
        &mut self,
        noun: &str,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        let library = self.wired().map_err(|e| Failure::new(e, 1))?;
        library_cli::run(library, noun, call, catalog, out, err)
    }
}

#[cfg(test)]
#[path = "library.test.rs"]
mod tests;
