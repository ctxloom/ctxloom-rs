//! The use cases over fake configurations: which agents a call touches,
//! what each is asked, and what is answered.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use annals_diagnostic::{Diagnostic, Report};
use ports::{AgentConfigs, McpServer, Rewritten, Scope};

use super::{
    taskloom_server, Install, RegistrationError, Registrations, State, Uninstall, TASKLOOM,
};

/// One register call, as asked.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Asked {
    engine: String,
    path: PathBuf,
    name: String,
    owner: String,
    server: Option<McpServer>,
    dry_run: bool,
}

/// Two agents, "alpha" and "beta"; each scope's configuration at
/// `<scope>/<engine>.json`.
#[derive(Default)]
struct Fake {
    present: Vec<(String, Scope)>,
    /// What `installed` answers, by path; absent is no configuration.
    installed: BTreeMap<PathBuf, Result<bool, Report>>,
    /// Paths with no configuration file to name.
    pathless: Vec<(String, Scope)>,
    /// Engines whose register fails.
    failing: Vec<String>,
    resolvable: bool,
    asked: Vec<Asked>,
    resolved: Vec<String>,
}

fn path(engine: &str, scope: &Scope) -> PathBuf {
    match scope {
        Scope::User => PathBuf::from(format!("/home/{engine}.json")),
        Scope::Project(dir) => dir.join(format!("{engine}.json")),
    }
}

fn why() -> Report {
    Report::new("agent-configs-corrupt", "it is not JSON")
}

impl AgentConfigs for Fake {
    fn engines(&self) -> Vec<String> {
        vec!["alpha".to_string(), "beta".to_string()]
    }

    fn present(&mut self, engine: &str, scope: &Scope) -> bool {
        self.present.iter().any(|(e, s)| e == engine && s == scope)
    }

    fn config_path(&mut self, engine: &str, scope: &Scope) -> Result<PathBuf, Report> {
        if self.pathless.iter().any(|(e, s)| e == engine && s == scope) {
            return Err(why());
        }
        Ok(path(engine, scope))
    }

    fn register(
        &mut self,
        engine: &str,
        path: &Path,
        name: &str,
        owner: &str,
        server: Option<&McpServer>,
        dry_run: bool,
    ) -> Result<Rewritten, Report> {
        self.asked.push(Asked {
            engine: engine.to_string(),
            path: path.to_path_buf(),
            name: name.to_string(),
            owner: owner.to_string(),
            server: server.cloned(),
            dry_run,
        });
        if self.failing.iter().any(|e| e == engine) {
            return Err(why());
        }
        Ok(Rewritten {
            changed: server.is_some(),
            after: format!("{engine} after"),
        })
    }

    fn installed(&mut self, _: &str, path: &Path, name: &str) -> Result<Option<bool>, Report> {
        assert_eq!(name, TASKLOOM);
        self.installed.get(path).cloned().transpose()
    }

    fn resolvable(&mut self, command: &str) -> bool {
        self.resolved.push(command.to_string());
        self.resolvable
    }
}

fn registrations(fake: Fake) -> Registrations<Fake> {
    Registrations::taskloom(fake)
}

fn install(engine: Option<&str>, scope: Scope) -> Install {
    Install {
        engine: engine.map(str::to_string),
        scope,
        print_only: false,
    }
}

fn project() -> Scope {
    Scope::Project(PathBuf::from("/work"))
}

#[test]
fn test_taskloom_registers_taskloom_mcp_under_its_name() {
    assert_eq!(
        taskloom_server(),
        McpServer {
            command: "taskloom".to_string(),
            args: vec!["mcp".to_string()]
        }
    );
    assert_eq!(TASKLOOM, "taskloom");
}

#[test]
fn test_an_install_writes_every_agent_in_use_at_the_scope_and_only_those() {
    let mut r = registrations(Fake {
        present: vec![
            ("beta".to_string(), Scope::User),
            ("alpha".to_string(), project()),
        ],
        resolvable: true,
        ..Fake::default()
    });
    let installed = r.install(&install(None, Scope::User)).expect("installed");
    assert!(installed.resolvable);
    assert_eq!(r.configs.resolved, ["taskloom"]);
    assert_eq!(
        r.configs.asked,
        [Asked {
            engine: "beta".to_string(),
            path: PathBuf::from("/home/beta.json"),
            name: "taskloom".to_string(),
            owner: "taskloom".to_string(),
            server: Some(taskloom_server()),
            dry_run: false,
        }]
    );
    assert_eq!(installed.outcomes.len(), 1);
    assert_eq!(installed.outcomes[0].engine, "beta");
    assert_eq!(
        installed.outcomes[0].config,
        PathBuf::from("/home/beta.json")
    );
    assert_eq!(installed.outcomes[0].rewritten.after, "beta after");
}

#[test]
fn test_an_install_with_no_agent_in_use_refuses_naming_the_engines() {
    let mut r = registrations(Fake::default());
    let refused = r.install(&install(None, Scope::User)).unwrap_err();
    assert_eq!(
        refused,
        RegistrationError::NoBackends {
            known: "alpha, beta".to_string()
        }
    );
    assert_eq!(
        refused.to_string(),
        "no agent backends detected; name one with --engine (alpha, beta)"
    );
    assert!(r.configs.asked.is_empty());
}

#[test]
fn test_an_agent_named_is_written_whether_in_use_or_not_and_print_only_writes_none() {
    let mut r = registrations(Fake::default());
    let request = Install {
        print_only: true,
        ..install(Some("alpha"), project())
    };
    r.install(&request).expect("installed");
    assert_eq!(r.configs.asked.len(), 1);
    assert_eq!(r.configs.asked[0].path, PathBuf::from("/work/alpha.json"));
    assert!(r.configs.asked[0].dry_run);
}

#[test]
fn test_an_engine_no_one_knows_is_refused_by_name() {
    let mut r = registrations(Fake::default());
    let refused = r.install(&install(Some("foo"), Scope::User)).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "unknown engine \"foo\"; known engines: alpha, beta"
    );
    let refused = r
        .uninstall(&Uninstall {
            engine: Some("Alpha".to_string()),
            scope: Scope::User,
        })
        .unwrap_err();
    assert_eq!(refused.code(), "registration-application-unknown-engine");
}

#[test]
fn test_an_agent_that_fails_stops_the_call_naming_the_agent() {
    let mut r = registrations(Fake {
        present: vec![
            ("alpha".to_string(), Scope::User),
            ("beta".to_string(), Scope::User),
        ],
        failing: vec!["alpha".to_string()],
        ..Fake::default()
    });
    let failed = r.install(&install(None, Scope::User)).unwrap_err();
    assert_eq!(failed.to_string(), "alpha: it is not JSON");
    assert_eq!(r.configs.asked.len(), 1, "beta is not reached");

    let mut r = registrations(Fake {
        pathless: vec![("alpha".to_string(), Scope::User)],
        ..Fake::default()
    });
    let failed = r.install(&install(Some("alpha"), Scope::User)).unwrap_err();
    assert!(
        matches!(failed, RegistrationError::EngineFailed { ref engine, .. } if engine == "alpha")
    );
}

#[test]
fn test_an_uninstall_takes_the_server_out_and_with_no_agent_in_use_does_nothing() {
    let mut r = registrations(Fake {
        present: vec![("alpha".to_string(), project())],
        ..Fake::default()
    });
    let removed = r
        .uninstall(&Uninstall {
            engine: None,
            scope: project(),
        })
        .expect("uninstalled");
    assert_eq!(removed.len(), 1);
    assert_eq!(r.configs.asked[0].server, None, "no server: the removal");
    assert!(!r.configs.asked[0].dry_run);

    let mut r = registrations(Fake::default());
    let removed = r
        .uninstall(&Uninstall {
            engine: None,
            scope: Scope::User,
        })
        .expect("nothing to do is no error");
    assert!(removed.is_empty());
}

#[test]
fn test_a_check_reports_each_configuration_there_is_at_both_scopes() {
    let mut installed = BTreeMap::new();
    installed.insert(PathBuf::from("/home/alpha.json"), Ok(true));
    installed.insert(PathBuf::from("/work/alpha.json"), Ok(false));
    installed.insert(PathBuf::from("/home/beta.json"), Err(why()));
    let mut r = registrations(Fake {
        installed,
        pathless: vec![("beta".to_string(), project())],
        ..Fake::default()
    });
    let checked = r.check(Path::new("/work"));
    assert!(!checked.resolvable);
    let found: Vec<(&str, &str, &Path, &State)> = checked
        .found
        .iter()
        .map(|f| (f.engine.as_str(), f.scope, f.path.as_path(), &f.state))
        .collect();
    assert_eq!(
        found,
        [
            (
                "alpha",
                "user",
                Path::new("/home/alpha.json"),
                &State::Registered
            ),
            (
                "alpha",
                "project",
                Path::new("/work/alpha.json"),
                &State::NotRegistered
            ),
            (
                "beta",
                "user",
                Path::new("/home/beta.json"),
                &State::Unreadable(why())
            ),
        ]
    );
}
