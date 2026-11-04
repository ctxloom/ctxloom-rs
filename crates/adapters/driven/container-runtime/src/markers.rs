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
    let mut markers = Vec::new();
    for file in ["/.dockerenv", "/run/.containerenv"] {
        if machine.exists(file) {
            markers.push(file.to_string());
        }
    }
    for var in [
        "REMOTE_CONTAINERS",
        "CODESPACES",
        "DEVCONTAINER",
        "KUBERNETES_SERVICE_HOST",
    ] {
        if !machine.var(var).is_empty() {
            markers.push(format!("${var}"));
        }
    }
    // cgroup v1 runtime signatures: BEST-EFFORT ONLY. cgroup v2 exposes a
    // bare "0::/" with no runtime marker, which is why the sentinel files and
    // the environment lead and this never stands alone as a negative signal.
    if let Some(cgroup) = machine.read("/proc/1/cgroup") {
        for marker in ["docker", "containerd", "kubepods", "/lxc/"] {
            if cgroup.contains(marker) {
                markers.push(format!("cgroup:{marker}"));
            }
        }
    }
    markers
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
