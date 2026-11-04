//! `companion.feature`'s write scenarios at the application port.

use std::sync::Arc;

use ctxloom_ports::{Scope, Unanswered};

use crate::fakes::{plain, Mem, Setup, CONFIG};
use crate::write::views::Removal;
use crate::Library;

const HOME: &str = "schema_version: 8\neditor:\n  command: 'true'\n";

fn world() -> (Library, Arc<Mem>) {
    Setup {
        layers: &[HOME, CONFIG],
        path: &[
            ("ctxloom-companion-acme", "/bin/ctxloom-companion-acme"),
            ("ctxloom-companion-sick", "/bin/ctxloom-companion-sick"),
            ("ctxloom-companion-mute", "/bin/ctxloom-companion-mute"),
            ("ltk", "/bin/ltk"),
        ],
        ..Setup::default()
    }
    .fixture()
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

// companion.feature: A registered companion runs, and removing it stops it (the registration)
#[test]
fn test_add_records_the_name_in_the_home_config_and_never_the_path() {
    let (library, mem) = world();
    let done = library.companion_add(&plain(), "acme").result.unwrap();
    assert_eq!(
        (
            done.name.as_str(),
            done.bin.as_str(),
            done.path.as_str(),
            done.added
        ),
        (
            "acme",
            "ctxloom-companion-acme",
            "/bin/ctxloom-companion-acme",
            true
        )
    );
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        format!("{HOME}companions:\n  - acme\n")
    );
    assert_eq!(mem.config(Scope::Project).unwrap(), CONFIG);
}

#[test]
fn test_a_second_add_registers_nothing_twice_and_writes_nothing() {
    let (library, mem) = world();
    library.companion_add(&plain(), "acme").result.unwrap();
    let writes = mem.writes();
    let again = library.companion_add(&plain(), "acme").result.unwrap();
    assert!(!again.added);
    assert_eq!(mem.writes(), writes);
    library.companion_add(&plain(), "ltk").result.unwrap();
    assert!(mem
        .config(Scope::Home)
        .unwrap()
        .ends_with("companions:\n  - acme\n  - ltk\n"));
}

#[test]
fn test_first_party_companions_are_their_own_binaries() {
    let (library, _mem) = world();
    let done = library.companion_add(&plain(), "ltk").result.unwrap();
    assert_eq!((done.bin.as_str(), done.path.as_str()), ("ltk", "/bin/ltk"));
}

// companion.feature: A name with nothing on PATH is refused, naming the binary it looked for
#[test]
fn test_add_refuses_a_name_with_nothing_on_the_path_a_bad_name_and_a_silent_binary() {
    let (library, mem) = world();
    assert!(err(library.companion_add(&plain(), "nowhere")).contains("CompanionNotOnPath"));
    for bad in ["", "-x", "a b", "../x", "a/b", ".x"] {
        assert!(
            err(library.companion_add(&plain(), bad)).contains("CompanionName"),
            "{bad:?}"
        );
    }
    mem.set_probe(|path| {
        if path.ends_with("sick") {
            Err(Unanswered::Failed("exit status: 3".to_string()))
        } else {
            Err(Unanswered::Silent)
        }
    });
    assert!(err(library.companion_add(&plain(), "sick")).contains("NotACompanion"));
    assert!(err(library.companion_add(&plain(), "mute")).contains("CompanionSilent"));
    assert_eq!(mem.writes(), 0);
    assert_eq!(mem.config(Scope::Home).unwrap(), HOME);
}

#[test]
fn test_add_makes_the_home_config_where_there_is_none() {
    let (library, mem) = Setup {
        layers: &[CONFIG],
        path: &[("ctxloom-companion-acme", "/bin/a")],
        ..Setup::default()
    }
    .fixture();
    library.companion_add(&plain(), "acme").result.unwrap();
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        "schema_version: 8\ncompanions:\n  - acme\n"
    );
}

// companion.feature: Remove without --yes names the apply command and changes nothing
#[test]
fn test_remove_previews_then_unregisters_exactly_that_name() {
    let (library, mem) = world();
    library.companion_add(&plain(), "acme").result.unwrap();
    library.companion_add(&plain(), "ltk").result.unwrap();
    let before = mem.config(Scope::Home);
    let Removal::Preview(p) = library
        .companion_remove(&plain(), "acme", false)
        .result
        .unwrap()
    else {
        panic!("a preview")
    };
    assert_eq!(
        (p.target.as_str(), p.apply.as_str()),
        ("companion \"acme\"", "ctxloom companion remove acme --yes")
    );
    assert_eq!(mem.config(Scope::Home), before);
    let Removal::Done(done) = library
        .companion_remove(&plain(), "acme", true)
        .result
        .unwrap()
    else {
        panic!("done")
    };
    assert_eq!((done.name.as_str(), done.removed), ("acme", true));
    assert!(mem
        .config(Scope::Home)
        .unwrap()
        .ends_with("companions:\n  - ltk\n"));
    library
        .companion_remove(&plain(), "ltk", true)
        .result
        .unwrap();
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        HOME,
        "the empty list goes with its last name"
    );
}

#[test]
fn test_remove_of_a_name_not_registered_is_refused_with_and_without_yes() {
    let (library, mem) = world();
    for yes in [false, true] {
        assert!(
            err(library.companion_remove(&plain(), "acme", yes)).contains("CompanionNotRegistered")
        );
    }
    assert_eq!(mem.writes(), 0);
}

// companion.feature: a project may add companions, never remove home's
#[test]
fn test_a_name_only_the_project_registers_cannot_be_removed_from_home() {
    let (library, mem) = Setup {
        layers: &[HOME, "schema_version: 8\ncompanions:\n  - beta\n"],
        ..Setup::default()
    }
    .fixture();
    assert!(
        err(library.companion_remove(&plain(), "beta", true)).contains("CompanionNotRegistered")
    );
    assert_eq!(mem.writes(), 0);
}
