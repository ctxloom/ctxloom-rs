//! The registered companions' tooling: only a registered companion is run,
//! and what it declares is what `container` reads.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use isolation_ports::Companions;
use library_application::Flags;
use process_env::EnvSnapshot;

use isolation_ports::Backends;

use super::{Engines, Registered};
use crate::fixtures::{mkdirs, put_mode, put_program, root, unix};

const LOADOUT: &str =
    "#!/bin/sh\ncase \"$1\" in\n  loadout) printf 'init:\\n  tooling: \"acme-tools\"\\n';;\nesac\n";

/// A home and a `bin` directory holding the companions `names`, on the PATH
/// of the environment the companions run in.
fn registered(names: &[&str], register: &[&str]) -> (file_system::Rooted, Registered) {
    let temp = root();
    let dir: PathBuf = temp.root().to_path_buf();
    let (home, work, bin) = (dir.join("home"), dir.join("work"), dir.join("bin"));
    for d in [&home, &work, &bin] {
        mkdirs(d);
    }
    for name in names {
        put_program(bin.join(format!("ctxloom-companion-{name}")), LOADOUT);
    }
    let base = EnvSnapshot::from_process();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let env = Arc::new(EnvSnapshot::from_parts(
        base.flavor(),
        Ok(work),
        base.temp_dir().to_path_buf(),
        vec![
            ("HOME".into(), home.into_os_string()),
            ("PATH".into(), path.into()),
        ],
    ));
    let wiring = crate::Wiring::new(Arc::clone(&env)).unwrap();
    let library = Rc::new(crate::library::build(&wiring, None).unwrap());
    for name in register {
        library
            .companion_add(&Flags::default(), name)
            .result
            .unwrap();
    }
    let companions = Registered {
        library,
        flags: Flags::default(),
        env,
    };
    (temp, companions)
}

#[test]
fn a_registered_companion_declares_its_tooling() {
    if !unix() {
        return;
    }
    let (_temp, companions) = registered(&["acme"], &["acme"]);
    let tooling = companions.tooling().unwrap();
    assert_eq!(tooling.len(), 1);
    assert_eq!(tooling[0].source, "acme");
    assert_eq!(tooling[0].content, "acme-tools");
}

#[test]
fn a_companion_nobody_registered_declares_nothing() {
    if !unix() {
        return;
    }
    let (_temp, companions) = registered(&["acme"], &[]);
    assert!(companions.tooling().unwrap().is_empty());
}

/// Engines over a home and a project of the test's own, whose config is
/// `config` when given.
fn engines_of(names: &[&str], config: Option<&str>) -> (file_system::Rooted, Engines) {
    let temp = root();
    let dir: PathBuf = temp.root().to_path_buf();
    let (home, work) = (dir.join("home"), dir.join("work"));
    mkdirs(&home);
    mkdirs(work.join(".ctxloom"));
    if let Some(config) = config {
        put_mode(work.join(".ctxloom/config.yaml"), config, 0o644);
    }
    let base = EnvSnapshot::from_process();
    let env = Arc::new(EnvSnapshot::from_parts(
        base.flavor(),
        Ok(work),
        base.temp_dir().to_path_buf(),
        vec![("HOME".into(), home.into_os_string())],
    ));
    let wiring = crate::Wiring::new(env).unwrap();
    let library = Rc::new(crate::library::build(&wiring, None).unwrap());
    let engines = Engines {
        names: names.iter().map(ToString::to_string).collect(),
        library,
        flags: Flags::default(),
    };
    (temp, engines)
}

#[test]
fn the_engines_asked_about_are_the_ones_given() {
    let (_temp, engines) = engines_of(&["claude", "mock"], None);
    assert_eq!(engines.names(), vec!["claude", "mock"]);
}

#[test]
fn with_no_configured_default_a_bare_command_means_the_first_engine() {
    let (_temp, engines) = engines_of(&["claude", "mock"], None);
    assert_eq!(engines.default_backend().as_deref(), Some("claude"));
}

#[test]
fn with_no_engines_there_is_no_default() {
    let (_temp, engines) = engines_of(&[], None);
    assert_eq!(engines.default_backend(), None);
}

#[test]
fn the_configured_default_is_the_one_a_bare_command_means_when_it_is_an_engine() {
    let config =
        "schema_version: 8\nllm:\n  configs:\n    mock:\n      type: mock\n  defaults:\n    primary: mock\n";
    let (_temp, engines) = engines_of(&["claude", "mock"], Some(config));
    assert_eq!(engines.default_backend().as_deref(), Some("mock"));
    let (_temp, other) = engines_of(&["claude"], Some(config));
    assert_eq!(
        other.default_backend().as_deref(),
        Some("claude"),
        "a default that is no engine here is passed over"
    );
}
