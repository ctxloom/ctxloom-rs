//! The runtimes on the machine.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use file_system::{Fs, Kind};
use isolation_ports::{ContainerRuntimes, RuntimeFacts, RUNTIMES};
use process_env::EnvSnapshot;

/// Runtimes found on a `PATH`.
pub struct HostRuntimes {
    fs: Arc<dyn Fs>,
    env: Arc<EnvSnapshot>,
    pub(crate) dirs: Vec<PathBuf>,
    pub(crate) answer_within: Duration,
}

impl HostRuntimes {
    /// The runtimes on `dirs`, each given `answer_within` to answer `info`;
    /// `env` is where the in-container markers are read.
    pub fn new(
        fs: Arc<dyn Fs>,
        env: Arc<EnvSnapshot>,
        dirs: Vec<PathBuf>,
        answer_within: Duration,
    ) -> HostRuntimes {
        HostRuntimes {
            fs,
            env,
            dirs,
            answer_within,
        }
    }

    /// `env`'s `PATH`, with ten seconds to answer.
    pub fn from_env(fs: Arc<dyn Fs>, env: Arc<EnvSnapshot>) -> HostRuntimes {
        let dirs = env.search_path();
        HostRuntimes::new(fs, env, dirs, Duration::from_secs(10))
    }
}

impl HostRuntimes {
    /// The executable `name` on the searched directories.
    fn find(&self, name: &str) -> Option<PathBuf> {
        let found = self.dirs.iter().map(|dir| dir.join(name)).find(|path| {
            self.fs
                .metadata(path)
                .is_ok_and(|meta| meta.kind == Kind::File && meta.executable)
        })?;
        // The command runs on the host's own path, not the caller's.
        self.fs.real_path(&found).ok()
    }

    /// Run `bin args`, bounded: whether it exited 0 in time. A runtime that
    /// never answers is killed, not waited on.
    fn answers(&self, bin: &Path, args: &[&str]) -> bool {
        let Ok(mut child) = Command::new(bin)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let deadline = Instant::now() + self.answer_within;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => return false,
            }
        }
    }
}

impl ContainerRuntimes for HostRuntimes {
    // The real machine's markers: what they are depends on where the tests run
    // (a clean host has none), so no hermetic test can pin the result.
    #[cfg_attr(test, mutants::skip)]
    fn in_container(&self) -> Vec<String> {
        crate::markers_from(&crate::Real::new(
            Arc::clone(&self.fs),
            Arc::clone(&self.env),
        ))
    }

    fn installed(&self) -> Vec<RuntimeFacts> {
        RUNTIMES
            .iter()
            .filter_map(|name| {
                let bin = self.find(name)?;
                Some(RuntimeFacts {
                    name: (*name).to_string(),
                    reachable: self.answers(&bin, &["info"]),
                })
            })
            .collect()
    }

    fn image_present(&self, runtime: &str, image: &str) -> bool {
        RUNTIMES.contains(&runtime)
            && self
                .find(runtime)
                .is_some_and(|bin| self.answers(&bin, &["image", "inspect", image]))
    }
}
