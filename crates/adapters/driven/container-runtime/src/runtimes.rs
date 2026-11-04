//! The runtimes on the machine.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use isolation_ports::{ContainerRuntimes, RuntimeFacts, RUNTIMES};

/// Runtimes found on a `PATH`.
pub struct HostRuntimes {
    dirs: Vec<PathBuf>,
    answer_within: Duration,
}

impl HostRuntimes {
    /// The runtimes on `dirs`, each given `answer_within` to answer `info`.
    pub fn new(dirs: Vec<PathBuf>, answer_within: Duration) -> HostRuntimes {
        HostRuntimes {
            dirs,
            answer_within,
        }
    }

    /// The process's `PATH`, with ten seconds to answer.
    pub fn from_process() -> HostRuntimes {
        let dirs = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        HostRuntimes::new(dirs, Duration::from_secs(10))
    }
}

impl HostRuntimes {
    /// The executable `name` on the searched directories.
    fn find(&self, name: &str) -> Option<PathBuf> {
        use std::os::unix::fs::PermissionsExt;
        self.dirs.iter().map(|dir| dir.join(name)).find(|path| {
            path.metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
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
    fn in_container(&self) -> Vec<String> {
        crate::markers_from(&crate::Real)
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
