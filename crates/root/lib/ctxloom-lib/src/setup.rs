//! The setup commands (S12) wired: the cache, the doctor's files and the
//! scaffold over the process's filesystem, and the use cases over them.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use cache_files::CacheFiles;
use config_files::{find_app_dir, ConfigFilesFs, ScaffoldFs, Surroundings};
use content_fs::UserFilesFs;
use deps_application::Deps;
use doctor_checks::Doctor;
use init_application::Init;
use reclaim_application::Cleaner;

use crate::library::PathSearch;
use crate::{LibError, Wiring};

/// The nouns these commands own.
pub(crate) const NOUNS: [&str; 3] = ["clean", "doctor", "init"];

/// The setup use cases, built together.
pub(crate) struct SetupUseCases {
    pub(crate) cleaner: Cleaner,
    pub(crate) doctor: Doctor,
    pub(crate) init: Init,
}

impl SetupUseCases {
    pub(crate) fn build(deps: Rc<Deps>) -> Result<SetupUseCases, LibError> {
        let wiring = Wiring::from_process()?;
        let fs = wiring.fs();
        let home = wiring.home().to_path_buf();
        let around = Surroundings {
            root_env: std::env::var("CTXLOOM_ROOT").ok(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            home: home.clone(),
            temp_root: std::env::temp_dir(),
        };
        let app = find_app_dir(fs.as_ref(), &around);
        let user = Arc::new(UserFilesFs::new(Arc::clone(&fs)));
        let dirs = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        let binaries = PathSearch {
            fs: Arc::clone(&fs),
            dirs,
        };
        let cleaner = Cleaner::new(Arc::new(CacheFiles::new(Arc::clone(&fs), &app.path)));
        let config = ConfigFilesFs::new(Arc::clone(&fs), app, &home);
        let doctor = Doctor::new(
            Arc::new(config),
            user.clone(),
            Arc::new(binaries),
            &home.display().to_string(),
            env!("CARGO_PKG_VERSION"),
        );
        let init = Init::new(
            Arc::new(ScaffoldFs::new(Arc::clone(&fs), &around.cwd)),
            user,
            deps,
        );
        Ok(SetupUseCases {
            cleaner,
            doctor,
            init,
        })
    }
}
