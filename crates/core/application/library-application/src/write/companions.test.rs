//! `companion.feature`'s write scenarios at the application port.

use std::sync::Arc;

use ctxloom_ports::{Scope, Unanswered};

use crate::fakes::{plain, Mem, Setup, CONFIG};
use crate::write::views::Removal;
use crate::Library;

const HOME: &str = "schema_version: 9\neditor:\n  command: 'true'\n";

fn world() -> (Library, Arc<Mem>) {
    Setup {
        layers: &[HOME, CONFIG],
        ..Setup::default()
    }
    .fixture()
}

fn cmd(of: &[&str]) -> Vec<String> {
    of.iter().map(ToString::to_string).collect()
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

// companion.feature: A registered companion runs, and removing it stops it (the registration)
#[test]
fn test_add_records_the_name_and_its_command_in_the_home_config() {
    let (library, mem) = world();
    let command = cmd(&["/opt/acme/bin/acme", "--as", "acme"]);
    let done = library
        .companion_add(&plain(), "acme", &command)
        .result
        .unwrap();
    assert_eq!(
        (done.name.as_str(), &done.command, done.added),
        ("acme", &command, true)
    );
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        format!("{HOME}companions:\n  acme:\n    command:\n      - /opt/acme/bin/acme\n      - --as\n      - acme\n")
    );
    assert_eq!(mem.config(Scope::Project).unwrap(), CONFIG);
}

#[test]
fn test_add_probes_the_command_once_and_before_it_records() {
    let (library, mem) = world();
    let command = cmd(&["acme", "--flag"]);
    library
        .companion_add(&plain(), "acme", &command)
        .result
        .unwrap();
    assert_eq!(mem.probed_commands(), [command]);
}

#[test]
fn test_a_second_identical_add_writes_nothing_and_a_changed_one_replaces_the_command() {
    let (library, mem) = world();
    library
        .companion_add(&plain(), "acme", &cmd(&["a"]))
        .result
        .unwrap();
    let writes = mem.writes();
    let again = library
        .companion_add(&plain(), "acme", &cmd(&["a"]))
        .result
        .unwrap();
    assert!(!again.added);
    assert_eq!(mem.writes(), writes);
    let changed = library
        .companion_add(&plain(), "acme", &cmd(&["b", "--x"]))
        .result
        .unwrap();
    assert!(changed.added);
    library
        .companion_add(&plain(), "ltk", &cmd(&["ltk"]))
        .result
        .unwrap();
    let home = mem.config(Scope::Home).unwrap();
    assert!(
        home.ends_with(
            "companions:\n  acme:\n    command:\n      - b\n      - --x\n  ltk:\n    command:\n      - ltk\n"
        ),
        "{home}"
    );
}

// companion.feature: a command that does not answer is refused, and nothing is recorded
#[test]
fn test_add_refuses_a_missing_program_a_bad_name_no_command_and_a_command_that_does_not_answer() {
    let (library, mem) = world();
    mem.set_probe(|command| match command[0].as_str() {
        "gone" => Err(Unanswered::Missing("No such file".to_string())),
        "sick" => Err(Unanswered::Exited("exit status: 3".to_string())),
        "broken" => Err(Unanswered::Failed(
            "did not answer within 3000ms".to_string(),
        )),
        _ => Err(Unanswered::Silent),
    });
    assert!(err(library.companion_add(&plain(), "x", &cmd(&["gone"])))
        .contains("CompanionProgramMissing"));
    for bad in ["", "-x", "a b", "../x", "a/b", ".x"] {
        assert!(
            err(library.companion_add(&plain(), bad, &cmd(&["a"]))).contains("CompanionName"),
            "{bad:?}"
        );
    }
    for none in [cmd(&[]), cmd(&[""])] {
        assert!(
            err(library.companion_add(&plain(), "x", &none)).contains("CompanionNoCommand"),
            "{none:?}"
        );
    }
    assert!(err(library.companion_add(&plain(), "x", &cmd(&["sick"]))).contains("NotACompanion"));
    assert!(
        err(library.companion_add(&plain(), "x", &cmd(&["broken", "-v"]))).contains("broken -v")
    );
    assert!(err(library.companion_add(&plain(), "x", &cmd(&["mute"]))).contains("CompanionSilent"));
    assert_eq!(mem.writes(), 0);
    assert_eq!(mem.config(Scope::Home).unwrap(), HOME);
}

#[test]
fn test_add_makes_the_home_config_where_there_is_none() {
    let (library, mem) = Setup {
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    library
        .companion_add(&plain(), "acme", &cmd(&["acme"]))
        .result
        .unwrap();
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        "schema_version: 9\ncompanions:\n  acme:\n    command:\n      - acme\n"
    );
}

// companion.feature: Remove without --yes names the apply command and changes nothing
#[test]
fn test_remove_previews_then_unregisters_exactly_that_name() {
    let (library, mem) = world();
    library
        .companion_add(&plain(), "acme", &cmd(&["acme"]))
        .result
        .unwrap();
    library
        .companion_add(&plain(), "ltk", &cmd(&["ltk"]))
        .result
        .unwrap();
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
        .ends_with("companions:\n  ltk:\n    command:\n      - ltk\n"));
    library
        .companion_remove(&plain(), "ltk", true)
        .result
        .unwrap();
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        HOME,
        "the empty map goes with its last name"
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
        layers: &[
            HOME,
            "schema_version: 9\ncompanions:\n  beta:\n    command: [beta]\n",
        ],
        ..Setup::default()
    }
    .fixture();
    assert!(
        err(library.companion_remove(&plain(), "beta", true)).contains("CompanionNotRegistered")
    );
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_remove_of_another_name_than_the_registered_one_is_refused() {
    let (library, mem) = world();
    library
        .companion_add(&plain(), "acme", &cmd(&["acme"]))
        .result
        .unwrap();
    let writes = mem.writes();
    for yes in [false, true] {
        assert!(
            err(library.companion_remove(&plain(), "ltk", yes)).contains("CompanionNotRegistered")
        );
    }
    assert_eq!(mem.writes(), writes);
}
