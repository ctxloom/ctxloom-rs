//! Ported from Go's `containerprobe_test.go` (release/0.7, f36e8dfb21).

use std::collections::BTreeMap;

use super::{markers_from, Machine};

#[derive(Default)]
struct Fake {
    files: Vec<&'static str>,
    contents: BTreeMap<&'static str, &'static str>,
    env: BTreeMap<&'static str, &'static str>,
}

impl Machine for Fake {
    fn exists(&self, path: &str) -> bool {
        self.files.contains(&path)
    }

    fn read(&self, path: &str) -> Option<String> {
        self.contents.get(path).map(|c| c.to_string())
    }

    fn var(&self, name: &str) -> String {
        self.env
            .get(name)
            .map(|v| v.to_string())
            .unwrap_or_default()
    }
}

#[test]
fn no_signal_means_not_in_a_container() {
    assert!(markers_from(&Fake::default()).is_empty());
}

// Each probe answers on its own: a runtime exposing only one of them is the
// normal case.
#[test]
fn each_probe_answers_on_its_own() {
    let docker = Fake {
        files: vec!["/.dockerenv"],
        ..Fake::default()
    };
    assert_eq!(markers_from(&docker), vec!["/.dockerenv"]);

    let podman = Fake {
        files: vec!["/run/.containerenv"],
        ..Fake::default()
    };
    assert_eq!(markers_from(&podman), vec!["/run/.containerenv"]);

    let devcontainer = Fake {
        env: BTreeMap::from([("REMOTE_CONTAINERS", "true")]),
        ..Fake::default()
    };
    assert_eq!(markers_from(&devcontainer), vec!["$REMOTE_CONTAINERS"]);

    let pod = Fake {
        env: BTreeMap::from([("KUBERNETES_SERVICE_HOST", "10.0.0.1")]),
        ..Fake::default()
    };
    assert_eq!(markers_from(&pod), vec!["$KUBERNETES_SERVICE_HOST"]);

    let cgroup = Fake {
        contents: BTreeMap::from([("/proc/1/cgroup", "12:pids:/docker/abc123")]),
        ..Fake::default()
    };
    assert_eq!(markers_from(&cgroup), vec!["cgroup:docker"]);
}

// cgroup v2's bare "0::/" names no runtime and is never a signal.
#[test]
fn a_bare_cgroup_v2_line_is_no_signal() {
    let v2 = Fake {
        contents: BTreeMap::from([("/proc/1/cgroup", "0::/")]),
        ..Fake::default()
    };
    assert!(markers_from(&v2).is_empty());
}

#[test]
fn every_matching_marker_is_reported() {
    let all = Fake {
        files: vec!["/.dockerenv", "/run/.containerenv"],
        env: BTreeMap::from([("CODESPACES", "true")]),
        contents: BTreeMap::from([("/proc/1/cgroup", "kubepods/containerd")]),
    };
    let markers = markers_from(&all);
    for expected in [
        "/.dockerenv",
        "/run/.containerenv",
        "$CODESPACES",
        "cgroup:containerd",
        "cgroup:kubepods",
    ] {
        assert!(
            markers.iter().any(|m| m == expected),
            "{expected} in {markers:?}"
        );
    }
}
