//! The library reading what remotes installed: their bundles join the
//! catalog under `<remote>/<bundle>`, a short reference is spelled out, and
//! a search reaches what the registered remotes publish.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use deps_ports::{Failure, InstalledBundle, RemoteHit, RemoteLibrary, RemoteSearch};

use crate::fakes::{plain, Setup, CONFIG, ENVELOPE};
use crate::{AssembleRequest, Library, Notice, SearchRequest};

const URL: &str = "file:///srv/origin.git";
const REFERENCE: &str = "ctxloom+file:///srv/origin.git//bundles/kit";

struct Remotes {
    installed: Vec<InstalledBundle>,
    search: Result<RemoteSearch, String>,
}

impl RemoteLibrary for Remotes {
    fn installed(&self) -> Result<Vec<InstalledBundle>, Failure> {
        Ok(self.installed.clone())
    }

    fn canonical(&self, reference: &str, local_exists: &dyn Fn(&str) -> bool) -> String {
        match reference.strip_prefix("origin/") {
            Some(bundle) if !local_exists(reference) => {
                format!("ctxloom+file:///srv/origin.git//bundles/{bundle}")
            }
            _ => reference.to_string(),
        }
    }

    fn within(&self, reference: &str, url: &str, sha: &str) -> String {
        if reference.starts_with("ctxloom+") {
            reference.to_string()
        } else {
            format!("ctxloom+{url}//bundles/{reference}@{sha}")
        }
    }

    fn search(&self, _query: &str) -> Result<RemoteSearch, Failure> {
        self.search
            .clone()
            .map_err(|text| Box::new(Notice::text(text)) as Failure)
    }
}

fn kit() -> InstalledBundle {
    let files: BTreeMap<String, Vec<u8>> = [
        ENVELOPE,
        (
            "fragments/kit-rules.md",
            "---\ntags: [kit]\n---\nKIT-RULES\n",
        ),
        ("profiles/dev.yaml", "bundles: [kit]\n"),
        (
            "profiles/bad.yaml",
            "bundles: [ctxloom+file:///srv/elsewhere.git//bundles/x]\n",
        ),
    ]
    .iter()
    .map(|(path, text)| ((*path).to_string(), text.as_bytes().to_vec()))
    .collect();
    InstalledBundle {
        reference: REFERENCE.to_string(),
        name: "origin/kit".to_string(),
        url: URL.to_string(),
        sha: "a".repeat(40),
        path: ".ctxloom/cache/bundles/kit".to_string(),
        files,
        executable: BTreeSet::new(),
    }
}

fn hit() -> RemoteHit {
    RemoteHit {
        remote: "origin".to_string(),
        name: "kit".to_string(),
        tags: vec!["go".to_string()],
        description: "a kit".to_string(),
        author: "me".to_string(),
        pull_ref: REFERENCE.to_string(),
    }
}

fn library(remotes: Remotes) -> Library {
    Setup {
        bundles: &[],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .library()
    .with_remotes(Rc::new(remotes))
}

fn installed() -> Remotes {
    Remotes {
        installed: vec![kit()],
        search: Ok(RemoteSearch::default()),
    }
}

#[test]
fn test_an_installed_remote_bundle_is_listed_under_its_remote_name() {
    let list = library(installed())
        .bundle_list(&plain())
        .result
        .expect("a list");
    let row = list
        .rows
        .iter()
        .find(|r| r.name == "origin/kit")
        .expect("kit");
    assert_eq!(row.reference, REFERENCE);
    assert_eq!(row.fragment_count, 1);
    assert_eq!(row.profile_count, 2);
}

#[test]
fn test_an_installed_remote_bundle_is_reachable_by_its_canonical_reference() {
    let lib = library(installed());
    let view = lib
        .bundle_view(&plain(), REFERENCE, false)
        .result
        .expect("a view");
    assert!(view.content.contains("KIT-RULES"), "{}", view.content);
    let by_name = lib
        .bundle_view(&plain(), "origin/kit", false)
        .result
        .expect("a view");
    assert!(by_name.content.contains("KIT-RULES"));
}

#[test]
fn test_a_library_without_remotes_lists_no_remote_bundle() {
    let lib = Setup {
        bundles: &[],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .library();
    let list = lib.bundle_list(&plain()).result.expect("a list");
    assert!(list.rows.iter().all(|r| r.name != "origin/kit"));
}

#[test]
fn test_a_remote_search_reports_what_the_remotes_publish() {
    let lib = library(Remotes {
        installed: Vec::new(),
        search: Ok(RemoteSearch {
            hits: vec![hit()],
            warnings: vec!["gone: fetch failed".to_string()],
        }),
    });
    let request = SearchRequest {
        query: "kit".to_string(),
        tags: Vec::new(),
        kind: String::new(),
        local: false,
        remote: true,
    };
    let out = lib.search(&plain(), &request);
    let found = out.result.expect("results");
    assert_eq!(found.remote.len(), 1);
    assert_eq!(found.remote[0].pull_ref, REFERENCE);
    assert_eq!(found.remote[0].kind, "bundle");
    assert!(
        out.notices
            .iter()
            .any(|n| matches!(n, Notice::Finding { text, .. } if text == "remote search: gone: fetch failed")),
        "{:?}",
        out.notices
    );
}

#[test]
fn test_a_remote_search_that_failed_says_so_and_goes_on() {
    let lib = library(Remotes {
        installed: Vec::new(),
        search: Err("boom".to_string()),
    });
    let request = SearchRequest {
        query: "kit".to_string(),
        tags: Vec::new(),
        kind: String::new(),
        local: false,
        remote: true,
    };
    let out = lib.search(&plain(), &request);
    assert!(out.result.expect("results").remote.is_empty());
    assert!(
        out.notices
            .iter()
            .any(|n| matches!(n, Notice::Finding { text, .. } if text.starts_with("remote search error: "))),
        "{:?}",
        out.notices
    );
}

#[test]
fn test_a_local_search_does_not_ask_the_remotes() {
    let lib = library(Remotes {
        installed: Vec::new(),
        search: Err("must not be asked".to_string()),
    });
    let request = SearchRequest {
        query: "kit".to_string(),
        tags: Vec::new(),
        kind: String::new(),
        local: true,
        remote: false,
    };
    let out = lib.search(&plain(), &request);
    assert!(out.result.expect("results").remote.is_empty());
    assert!(out.notices.is_empty(), "{:?}", out.notices);
}

fn ask(remote: bool, local: bool) -> SearchRequest {
    SearchRequest {
        query: "kit".to_string(),
        tags: Vec::new(),
        kind: String::new(),
        local,
        remote,
    }
}

#[test]
fn test_neither_flag_searches_the_remotes_too() {
    let lib = library(Remotes {
        installed: Vec::new(),
        search: Ok(RemoteSearch {
            hits: vec![hit()],
            warnings: Vec::new(),
        }),
    });
    for (remote, local, want) in [
        (false, false, 1),
        (true, true, 1),
        (true, false, 1),
        (false, true, 0),
    ] {
        let found = lib
            .search(&plain(), &ask(remote, local))
            .result
            .expect("results");
        assert_eq!(found.remote.len(), want, "remote {remote} local {local}");
    }
}

#[test]
fn test_a_remote_profile_may_name_only_its_own_repository() {
    let lib = library(installed());
    let assemble = |name: &str| {
        let request = AssembleRequest {
            profiles: vec![format!("{REFERENCE}#profiles/{name}")],
            ..AssembleRequest::default()
        };
        lib.assemble(&plain(), &request)
    };
    let refused = assemble("bad").result.err().map(|e| format!("{e:?}"));
    assert!(
        refused
            .as_deref()
            .is_some_and(|e| e.contains("own repository")),
        "{refused:?}"
    );
    let fine = assemble("dev");
    assert!(fine.result.is_ok(), "{:?}", fine.result.err());
}
