//! Whether this process is inside a container.

/// What the markers are read through, so a test never touches the real
/// `/proc`, the sentinel files or the process environment (CI itself runs
/// inside containers).
pub trait Machine {
    fn exists(&self, path: &str) -> bool;
    fn read(&self, path: &str) -> Option<String>;
    fn var(&self, name: &str) -> String;
}

/// Every in-container marker that matched, named for diagnostics; empty means
/// none did. A HEURISTIC: it feeds diagnostics, and the can-containers-launch
/// decision is behavior-based, never this alone.
pub fn markers_from(machine: &dyn Machine) -> Vec<String> {
    let _ = machine;
    todo!("S11 green")
}

/// The real machine: the filesystem and the process environment.
pub struct Real;

impl Machine for Real {
    fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn var(&self, name: &str) -> String {
        std::env::var(name).unwrap_or_default()
    }
}
