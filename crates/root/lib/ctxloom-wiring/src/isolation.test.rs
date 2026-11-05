//! The registered companions' tooling: only a registered companion is run,
//! and what it declares is what `container` reads.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use isolation_ports::Companions;
use library_application::Flags;
use process_env::EnvSnapshot;

use super::Registered;
use crate::fixtures::{mkdirs, put_mode, root, unix};

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
        put_mode(
            bin.join(format!("ctxloom-companion-{name}")),
            LOADOUT,
            0o755,
        );
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
