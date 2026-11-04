use super::*;

const URL: &str = "file:///tmp/acceptance-remote.git";

fn with(remotes: &[(&str, &str)]) -> Registry {
    let mut registry = Registry::empty();
    for (name, url) in remotes {
        registry.add(name, url, "git").expect("registers");
    }
    registry
}

fn forge_of(registry: &Registry, name: &str) -> String {
    registry.get(name).expect("a remote").forge
}

#[test]
fn test_an_empty_registry_is_stamped_and_lists_nothing() {
    let registry = Registry::empty();
    assert_eq!(registry.encode(), "schema_version: 1\n");
    assert!(registry.remotes().is_empty());
    assert_eq!(registry.default_name(), "");
    assert_eq!(registry.forge_labels(), vec!["git", "github"]);
}

// remote.feature: Registering a remote records it and it is listed back
#[test]
fn test_registering_a_remote_records_it_and_it_is_listed_back() {
    let mut registry = Registry::empty();
    let added = registry.add("origin", URL, "git").expect("registers");
    assert_eq!(
        added,
        Remote {
            name: "origin".to_string(),
            url: URL.to_string(),
            forge: "git".to_string()
        }
    );
    assert_eq!(
        registry.encode(),
        "remotes:\n  origin:\n    url: file:///tmp/acceptance-remote.git\n    forge: git\nschema_version: 1\n"
    );
    let again = Registry::decode(&registry.encode(), "remotes.yaml").expect("reads");
    assert_eq!(again.remotes(), vec![added]);
}

// remote.feature: Remotes are listed in name order
#[test]
fn test_remotes_are_listed_in_name_order() {
    let registry = with(&[("zeta", "file:///z.git"), ("alpha", "file:///a.git")]);
    let names: Vec<_> = registry.remotes().into_iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["alpha", "zeta"]);
}

// remote.feature: Registering an address that is already a remote is refused
#[test]
fn test_one_repository_is_one_remote() {
    let mut registry = with(&[("origin", URL)]);
    assert_eq!(
        registry.add("mirror", URL, "git"),
        Err(DepsError::SameRepository {
            existing: "origin".to_string()
        })
    );
    // Another spelling of the same repository is the same repository.
    assert_eq!(
        registry.add("mirror", "file:///tmp/acceptance-remote.git/", ""),
        Err(DepsError::SameRepository {
            existing: "origin".to_string()
        })
    );
    assert!(registry.get("mirror").is_none());
    assert_eq!(
        registry.add("origin", "file:///elsewhere.git", ""),
        Err(DepsError::RemoteExists {
            name: "origin".to_string()
        })
    );
}

// remote.feature: Registering a remote bound to an unknown forge is refused
#[test]
fn test_a_forge_that_nobody_configured_is_refused_and_nothing_is_registered() {
    let mut registry = Registry::empty();
    assert_eq!(
        registry.add("origin", URL, "no-such-forge"),
        Err(DepsError::UnknownForge {
            label: "no-such-forge".to_string()
        })
    );
    assert!(registry.remotes().is_empty());
    assert_eq!(registry.encode(), "schema_version: 1\n");
}

#[test]
fn test_an_address_that_names_no_repository_is_refused() {
    let mut registry = Registry::empty();
    let refused = registry
        .add("origin", "bundles", "")
        .expect_err("no repository");
    assert!(
        matches!(refused, DepsError::NoRepository { .. }),
        "{refused:?}"
    );
    assert!(registry.remotes().is_empty());
}

#[test]
fn test_a_remote_is_removed_by_name() {
    let mut registry = with(&[("origin", URL)]);
    assert_eq!(
        registry.remove("nope"),
        Err(DepsError::RemoteNotFound {
            name: "nope".to_string()
        })
    );
    let removed = registry.remove("origin").expect("removes");
    assert_eq!(removed.url, URL);
    assert!(registry.remotes().is_empty());
    assert_eq!(registry.encode(), "schema_version: 1\n");
}

// remote.feature: A remote can be made the default
#[test]
fn test_a_remote_can_be_made_the_default_and_the_default_cleared() {
    let mut registry = with(&[("origin", URL)]);
    registry.set_default(Some("origin")).expect("sets");
    assert_eq!(registry.default_name(), "origin");
    assert!(registry.encode().contains("default: origin\n"));
    assert_eq!(
        registry.set_default(Some("nope")),
        Err(DepsError::RemoteNotFound {
            name: "nope".to_string()
        })
    );
    assert_eq!(registry.default_name(), "origin");
    registry.set_default(None).expect("clears");
    assert_eq!(registry.default_name(), "");
    assert!(!registry.encode().contains("default"));
}

// remote.feature: Correcting a remote's URL records the new address
#[test]
fn test_an_edit_changes_the_url_and_reports_what_moved() {
    let mut registry = with(&[("origin", URL)]);
    let edited = registry
        .edit(
            "origin",
            &Edit {
                url: Some("file:///tmp/acceptance-moved.git".to_string()),
                ..Edit::default()
            },
        )
        .expect("edits");
    assert_eq!(edited.changed, vec!["url"]);
    assert_eq!(edited.before.url, URL);
    assert_eq!(edited.after.url, "file:///tmp/acceptance-moved.git");
    assert!(!edited.default_followed);
    assert!(!registry.encode().contains("acceptance-remote.git"));
}

// remote.feature: An edit restating a remote's own name and address is accepted
#[test]
fn test_an_edit_restating_a_remote_s_own_name_and_address_changes_nothing() {
    let mut registry = with(&[("origin", URL)]);
    let before = registry.encode();
    let edited = registry
        .edit(
            "origin",
            &Edit {
                name: Some("origin".to_string()),
                url: Some(URL.to_string()),
                forge: None,
            },
        )
        .expect("edits");
    assert!(edited.changed.is_empty());
    assert_eq!(registry.encode(), before);
}

// remote.feature: Moving a remote onto another remote's address is refused
#[test]
fn test_an_edit_onto_another_remote_s_address_is_refused_and_changes_nothing() {
    let mut registry = with(&[
        ("origin", URL),
        ("mirror", "file:///tmp/acceptance-mirror.git"),
    ]);
    let before = registry.encode();
    assert_eq!(
        registry.edit(
            "mirror",
            &Edit {
                url: Some(URL.to_string()),
                ..Edit::default()
            }
        ),
        Err(DepsError::SameRepository {
            existing: "origin".to_string()
        })
    );
    assert_eq!(registry.encode(), before);
}

// remote.feature: Rebinding a remote to an unknown forge is refused / records
// the new binding
#[test]
fn test_a_forge_is_rebound_to_a_known_label_only() {
    let mut registry = with(&[("origin", URL)]);
    let refused = registry.edit(
        "origin",
        &Edit {
            forge: Some("no-such-forge".to_string()),
            ..Edit::default()
        },
    );
    assert_eq!(
        refused,
        Err(DepsError::UnknownForge {
            label: "no-such-forge".to_string()
        })
    );
    assert_eq!(forge_of(&registry, "origin"), "git");
    let edited = registry
        .edit(
            "origin",
            &Edit {
                forge: Some("github".to_string()),
                ..Edit::default()
            },
        )
        .expect("rebinds");
    assert_eq!(edited.changed, vec!["forge"]);
    assert_eq!(forge_of(&registry, "origin"), "github");
    // An empty label is a value: resolve by the URL's host.
    registry
        .edit(
            "origin",
            &Edit {
                forge: Some(String::new()),
                ..Edit::default()
            },
        )
        .expect("clears the binding");
    assert_eq!(forge_of(&registry, "origin"), "");
    assert!(!registry.encode().contains("forge"));
}

// remote.feature: Renaming the default remote carries the default with it
#[test]
fn test_renaming_the_default_remote_carries_the_default() {
    let mut registry = with(&[("origin", URL), ("other", "file:///o.git")]);
    registry.set_default(Some("origin")).expect("sets");
    let edited = registry
        .edit(
            "origin",
            &Edit {
                name: Some("upstream".to_string()),
                ..Edit::default()
            },
        )
        .expect("renames");
    assert!(edited.default_followed);
    assert_eq!(edited.changed, vec!["name"]);
    assert_eq!(registry.default_name(), "upstream");
    assert!(registry.get("origin").is_none());
    assert_eq!(registry.get("upstream").expect("renamed").url, URL);
    // Renaming one that is not the default leaves the default alone.
    let edited = registry
        .edit(
            "other",
            &Edit {
                name: Some("renamed".to_string()),
                ..Edit::default()
            },
        )
        .expect("renames");
    assert!(!edited.default_followed);
    assert_eq!(registry.default_name(), "upstream");
}

#[test]
fn test_an_edit_that_cannot_land_changes_nothing() {
    let mut registry = with(&[("origin", URL), ("other", "file:///o.git")]);
    let before = registry.encode();
    for (name, edit, refused) in [
        (
            "nope",
            Edit {
                url: Some(URL.to_string()),
                ..Edit::default()
            },
            DepsError::RemoteNotFound {
                name: "nope".to_string(),
            },
        ),
        ("origin", Edit::default(), DepsError::NothingToEdit),
        (
            "origin",
            Edit {
                name: Some(String::new()),
                ..Edit::default()
            },
            DepsError::EmptyName,
        ),
        (
            "origin",
            Edit {
                name: Some("other".to_string()),
                ..Edit::default()
            },
            DepsError::RemoteExists {
                name: "other".to_string(),
            },
        ),
    ] {
        assert_eq!(registry.edit(name, &edit), Err(refused), "{name} {edit:?}");
        assert_eq!(registry.encode(), before);
    }
    // One bad field spoils the whole edit: nothing half-lands.
    let spoiled = registry.edit(
        "origin",
        &Edit {
            name: Some("renamed".to_string()),
            url: None,
            forge: Some("no-such-forge".to_string()),
        },
    );
    assert!(spoiled.is_err());
    assert_eq!(registry.encode(), before);
}

// remote.feature: A hand-configured forge can be bound, and survives the
// rewrite
#[test]
fn test_a_registry_written_by_hand_is_read_as_written_and_rewritten_without_loss() {
    let text = "schema_version: 1
forges:
  corp:
    type: git
remotes:
  origin:
    url: file:///tmp/acceptance-remote.git
    owner: the platform team
note: written by hand
";
    let mut registry = Registry::decode(text, "remotes.yaml").expect("reads");
    assert_eq!(registry.forge_labels(), vec!["corp", "git", "github"]);
    registry
        .add("mirror", "file:///tmp/acceptance-mirror.git", "corp")
        .expect("binds the configured forge");
    let written = registry.encode();
    assert!(
        written.contains("forges:\n  corp:\n    type: git\n"),
        "{written}"
    );
    assert!(written.contains("note: written by hand\n"), "{written}");
    assert!(
        written.contains("    owner: the platform team\n"),
        "{written}"
    );
    assert_eq!(forge_of(&registry, "mirror"), "corp");
    // The keys are written sorted, the stamp among them.
    let keys: Vec<&str> = written
        .lines()
        .filter(|l| !l.starts_with(' '))
        .map(|l| l.split(':').next().unwrap_or(l))
        .collect();
    assert_eq!(keys, vec!["forges", "note", "remotes", "schema_version"]);
}

// Go writes `name: ""` in each remote; the entry is read, kept as it was
// written, and the name is the key's.
#[test]
fn test_a_registry_go_wrote_is_read() {
    let text = "default: origin
remotes:
  origin:
    name: \"\"
    url: file:///tmp/r.git
    forge: git
schema_version: 1
";
    let registry = Registry::decode(text, "remotes.yaml").expect("reads");
    assert_eq!(
        registry.get("origin"),
        Some(Remote {
            name: "origin".to_string(),
            url: "file:///tmp/r.git".to_string(),
            forge: "git".to_string()
        })
    );
    assert_eq!(registry.default_name(), "origin");
    assert_eq!(registry.encode(), text);
}

// remote.feature: A registry that declares no schema generation is refused
#[test]
fn test_a_registry_of_another_generation_is_refused_naming_it() {
    let unstamped = Registry::decode(
        "remotes:\n  origin:\n    url: file:///r.git\n",
        "remotes.yaml",
    )
    .expect_err("unstamped");
    assert_eq!(
        unstamped,
        DepsError::TooOld {
            path: "remotes.yaml".to_string(),
            kind: "remotes".to_string(),
            found: 0,
            oldest: 1
        }
    );
    let newer = Registry::decode("schema_version: 2\n", "remotes.yaml").expect_err("newer");
    assert_eq!(
        newer,
        DepsError::TooNew {
            path: "remotes.yaml".to_string(),
            kind: "remotes".to_string(),
            found: 2,
            current: 1
        }
    );
}

#[test]
fn test_a_registry_with_a_bad_part_is_refused() {
    for text in [
        "schema_version: 1\nremotes: [origin]\n",
        "schema_version: 1\nremotes:\n  origin: {}\n",
        "schema_version: 1\nremotes:\n  origin:\n    url: [a]\n",
        "schema_version: 1\nforges:\n  corp:\n    type: svn\n",
        "schema_version: 1\ndefault: [a]\n",
        "schema_version: 1\nremotes: [unterminated\n",
    ] {
        let refused = Registry::decode(text, "remotes.yaml").expect_err(text);
        assert!(
            matches!(
                refused,
                DepsError::Unreadable { .. }
                    | DepsError::BadShape { .. }
                    | DepsError::ForgeType { .. }
            ),
            "{text}: {refused:?}"
        );
    }
    assert_eq!(
        Registry::decode(
            "schema_version: 1\nforges:\n  corp:\n    type: svn\n",
            "r.yaml"
        ),
        Err(DepsError::ForgeType {
            forge: "corp".to_string(),
            found: "svn".to_string()
        })
    );
}

#[test]
fn test_the_remote_registered_for_a_repository_is_found_by_any_spelling() {
    let registry = with(&[
        ("origin", "https://github.com/o/r"),
        ("local", "file:///srv/x.git"),
    ]);
    for spelling in [
        "git@github.com:o/r.git",
        "https://GitHub.com/o/r/",
        "https://github.com/o/r.git",
    ] {
        let found = registry.lookup_url(spelling).map(|r| r.name);
        assert_eq!(found.as_deref(), Some("origin"), "{spelling}");
    }
    assert_eq!(
        registry
            .lookup_url("file:///srv/x.git")
            .map(|r| r.name)
            .as_deref(),
        Some("local")
    );
    assert!(registry.lookup_url("https://github.com/o/other").is_none());
    assert!(registry.lookup_url("bundles").is_none());
}

#[test]
fn test_a_registry_stamped_below_the_oldest_generation_is_refused_naming_it() {
    assert_eq!(
        Registry::decode("schema_version: 0\n", "remotes.yaml").expect_err("old"),
        DepsError::TooOld {
            path: "remotes.yaml".to_string(),
            kind: "remotes".to_string(),
            found: 0,
            oldest: 1
        }
    );
}

#[test]
fn test_an_empty_default_is_not_written_back() {
    let registry = Registry::decode(
        "schema_version: 1\ndefault: ''\nremotes:\n  a:\n    url: file:///a.git\n",
        "remotes.yaml",
    )
    .expect("reads");
    assert!(
        !registry.encode().contains("default"),
        "{}",
        registry.encode()
    );
}

#[test]
fn test_removing_another_remote_keeps_the_default() {
    let mut registry = with(&[("a", "file:///a.git"), ("b", "file:///b.git")]);
    registry.set_default(Some("a")).expect("default");
    registry.remove("b").expect("removes");
    assert!(
        registry.encode().contains("default: a"),
        "{}",
        registry.encode()
    );
    registry.remove("a").expect("removes");
    assert!(
        !registry.encode().contains("default"),
        "{}",
        registry.encode()
    );
}

#[test]
fn test_remotes_and_forges_are_written_in_name_order_whatever_order_they_were_read_in() {
    let registry = Registry::decode(
        "schema_version: 1\nforges:\n  zz:\n    type: git\n  aa:\n    type: git\nremotes:\n  z:\n    url: file:///z.git\n  a:\n    url: file:///a.git\n",
        "remotes.yaml",
    )
    .expect("reads");
    let text = registry.encode();
    assert!(
        text.find("  a:").expect("a") < text.find("  z:").expect("z"),
        "{text}"
    );
    assert!(
        text.find("  aa:").expect("aa") < text.find("  zz:").expect("zz"),
        "{text}"
    );
}

#[test]
fn test_an_edit_keeps_what_the_author_wrote_inside_the_remote() {
    let mut registry = Registry::decode(
        "schema_version: 1\nremotes:\n  origin:\n    note: mine\n    url: file:///a.git\n",
        "remotes.yaml",
    )
    .expect("reads");
    registry
        .edit(
            "origin",
            &Edit {
                url: Some("file:///b.git".to_string()),
                ..Edit::default()
            },
        )
        .expect("edits");
    let text = registry.encode();
    assert!(
        text.contains("note: mine") && text.contains("file:///b.git"),
        "{text}"
    );
}

#[test]
fn test_a_remote_with_an_empty_address_is_refused() {
    let refused = Registry::decode(
        "schema_version: 1\nremotes:\n  origin:\n    url: ''\n",
        "remotes.yaml",
    )
    .expect_err("empty url");
    assert!(
        matches!(&refused, DepsError::BadShape { key, .. } if key == "remotes.origin.url"),
        "{refused:?}"
    );
}
