//! The runtimes on the machine.

use std::path::PathBuf;
use std::time::Duration;

use isolation_ports::{ContainerRuntimes, RuntimeFacts};

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

impl ContainerRuntimes for HostRuntimes {
    fn in_container(&self) -> Vec<String> {
        crate::markers_from(&crate::Real)
    }

    fn installed(&self) -> Vec<RuntimeFacts> {
        let _ = (&self.dirs, self.answer_within);
        todo!("S11 green")
    }

    fn image_present(&self, runtime: &str, image: &str) -> bool {
        let _ = (runtime, image);
        todo!("S11 green")
    }
}
