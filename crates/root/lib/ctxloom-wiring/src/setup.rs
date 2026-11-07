//! The setup commands (S12) wired: the cache, the doctor's files and the
//! scaffold over the process's filesystem, and the use cases over them.

use std::rc::Rc;
use std::sync::Arc;

use auth_application::Auth;
use cache_files::CacheFiles;
use config_files::{ConfigFilesFs, ScaffoldFs};
use content_fs::UserFilesFs;
use deps_application::Deps;
use doctor_checks::Doctor;
use init_application::{Create, Failure, Init};
use library_application::Library;
use reclaim_application::Cleaner;

use crate::{LibError, ProcessEnv, Wiring};

/// The nouns these commands own.
pub(crate) const NOUNS: [&str; 4] = ["auth", "clean", "doctor", "init"];

/// `config create`, as init's scaffold of a new project.
struct ConfigCreate(Library);

impl Create for ConfigCreate {
    fn create(&self) -> Result<(), Failure> {
        let created = self.0.config_create("");
        created
            .result
            .map(|_| ())
            .map_err(|e| Box::new(e) as Failure)
    }
}

/// The setup use cases, built together.
pub(crate) struct SetupUseCases {
    pub(crate) auth: Auth,
    pub(crate) cleaner: Cleaner,
    pub(crate) doctor: Doctor,
    pub(crate) init: Init,
}

impl SetupUseCases {
    pub(crate) fn build(wiring: &Wiring, deps: Rc<Deps>) -> Result<SetupUseCases, LibError> {
        let fs = wiring.fs();
        let home = wiring.home().to_path_buf();
        let (around, app) = wiring.locate();
        let user = Arc::new(UserFilesFs::new(Arc::clone(&fs)));
        let binaries = wiring.path_search();
        let cleaner = Cleaner::new(Arc::new(CacheFiles::new(Arc::clone(&fs), &app.path)));
        let config = ConfigFilesFs::new(Arc::clone(&fs), app, &home);
        let doctor = Doctor::new(
            Arc::new(config),
            user.clone(),
            Arc::new(binaries),
            Auth::new(Arc::new(ProcessEnv::from_snapshot(wiring.env()))),
            &home.display().to_string(),
            env!("CARGO_PKG_VERSION"),
        );
        let init = Init::new(
            Arc::new(ScaffoldFs::new(Arc::clone(&fs), &around.cwd)),
            user,
            deps,
            Box::new(ConfigCreate(crate::library::build(wiring, None)?)),
        );
        Ok(SetupUseCases {
            auth: Auth::new(Arc::new(ProcessEnv::from_snapshot(wiring.env()))),
            cleaner,
            doctor,
            init,
        })
    }
}

#[cfg(test)]
#[path = "setup.test.rs"]
mod tests;
