//! `manage` over fake configurations: Go's flags, Go's words on stderr,
//! Go's check table, and the JSON documents.

use std::path::{Path, PathBuf};

use annals_diagnostic::Report;
use ports::{AgentConfigs, McpServer, Rewritten, Scope};
use presentation::{Catalog, Format};
use registration_application::Registrations;
use serde_json::Value;

use super::{run, Settings};

/// One agent, "claude-code"; its files under /home and the project dir.
#[derive(Default)]
struct Fake {
    present: bool,
    resolvable: bool,
    /// Whether a rewrite changes the file.
    changes: bool,
    fails: bool,
    /// What `installed` answers.
    installed: Option<Result<bool, Report>>,
    scopes: Vec<Scope>,
    dry_runs: Vec<bool>,
}

impl AgentConfigs for Fake {
    fn engines(&self) -> Vec<String> {
        vec!["claude-code".to_string()]
    }

    fn present(&mut self, _: &str, scope: &Scope) -> bool {
        self.scopes.push(scope.clone());
        self.present
    }

    fn config_path(&mut self, _: &str, scope: &Scope) -> Result<PathBuf, Report> {
        Ok(match scope {
            Scope::User => PathBuf::from("/home/.claude.json"),
            Scope::Project(dir) => dir.join(".mcp.json"),
        })
    }

    fn register(
        &mut self,
        _: &str,
        _: &Path,
        _: &str,
        _: &str,
        server: Option<&McpServer>,
        dry_run: bool,
    ) -> Result<Rewritten, Report> {
        self.dry_runs.push(dry_run);
        if self.fails {
            return Err(Report::new("agent-configs-read", "cannot read it"));
        }
        Ok(Rewritten {
            changed: self.changes,
            after: if server.is_some() {
                "{ new }\n"
            } else {
                "{}\n"
            }
            .to_string(),
        })
    }

    fn installed(&mut self, _: &str, path: &Path, _: &str) -> Result<Option<bool>, Report> {
        if path.starts_with("/home") {
            return self.installed.clone().transpose();
        }
        Ok(None)
    }

    fn resolvable(&mut self, _: &str) -> bool {
        self.resolvable
    }
}

struct Ran {
    code: u8,
    out: String,
    err: String,
}

fn catalog() -> Catalog {
    Catalog::en_us(&[crate::EN_US, registration_application::EN_US]).expect("the catalog loads")
}

fn manage(fake: &mut Registrations<Fake>, format: Format, words: &[&str]) -> Ran {
    let settings = Settings {
        format,
        stdout_is_terminal: false,
        prog: "taskloom",
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let words = words.iter().map(|w| w.to_string()).collect();
    let code = run(words, &settings, fake, &catalog(), &mut out, &mut err);
    Ran {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn fake(fake: Fake) -> Registrations<Fake> {
    Registrations::taskloom(fake)
}

const WARNING: &str = "taskloom: warning: \"taskloom\" does not resolve on this process's PATH: exec: \"taskloom\": executable file not found in $PATH\n  the registered MCP entry runs \"taskloom\", which the agent resolves against ITS OWN PATH at startup; install taskloom somewhere on that PATH or the server will not start\n";

#[test]
fn test_an_install_says_what_go_says_and_writes_nothing_on_stdout() {
    let mut r = fake(Fake {
        present: true,
        changes: true,
        ..Fake::default()
    });
    let ran = manage(&mut r, Format::Text, &["install"]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(ran.out, "");
    assert_eq!(
        ran.err,
        format!("{WARNING}taskloom: registered MCP server for claude-code\n  config: /home/.claude.json\n")
    );
    assert_eq!(r.configs.scopes, [Scope::User]);
    assert_eq!(r.configs.dry_runs, [false]);

    let mut r = fake(Fake {
        present: true,
        resolvable: true,
        ..Fake::default()
    });
    let ran = manage(&mut r, Format::Text, &["install"]);
    assert!(!ran.err.contains("warning"), "{}", ran.err);
}

#[test]
fn test_project_is_a_switch_for_the_file_under_dir() {
    let mut r = fake(Fake::default());
    let ran = manage(
        &mut r,
        Format::Text,
        &[
            "install",
            "--engine",
            "claude-code",
            "--project",
            "--dir",
            "/work",
        ],
    );
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(
        ran.err.ends_with("  config: /work/.mcp.json\n"),
        "{}",
        ran.err
    );
    let mut r = fake(Fake {
        present: true,
        ..Fake::default()
    });
    manage(&mut r, Format::Text, &["uninstall", "--project"]);
    assert_eq!(r.configs.scopes, [Scope::Project(PathBuf::from("."))]);
}

#[test]
fn test_print_only_prints_each_file_as_it_would_be_and_writes_none() {
    let mut r = fake(Fake {
        present: true,
        resolvable: true,
        changes: true,
        ..Fake::default()
    });
    let ran = manage(&mut r, Format::Text, &["install", "--print-only"]);
    assert_eq!(
        ran.err,
        "# claude-code \u{2192} /home/.claude.json\n{ new }\n"
    );
    assert_eq!(r.configs.dry_runs, [true]);
    let ran = manage(&mut r, Format::Json, &["install", "--print-only"]);
    let doc: Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(doc["configs"][0]["changed"], true);
    assert_eq!(doc["configs"][0]["written"], false);
}

#[test]
fn test_in_json_an_install_answers_one_document() {
    let mut r = fake(Fake {
        present: true,
        changes: true,
        ..Fake::default()
    });
    let ran = manage(&mut r, Format::Json, &["install"]);
    let doc: Value = serde_json::from_str(&ran.out).expect("one document");
    assert_eq!(
        doc,
        serde_json::json!({
            "command_resolves": false,
            "configs": [{
                "engine": "claude-code",
                "config": "/home/.claude.json",
                "changed": true,
                "written": true,
                "after": "{ new }\n",
            }]
        })
    );
    let mut r = fake(Fake::default());
    let ran = manage(
        &mut r,
        Format::Text,
        &["--json", "install", "--engine", "claude-code"],
    );
    assert!(ran.out.starts_with('{'), "its own --json: {}", ran.out);
    let ran = manage(
        &mut r,
        Format::Json,
        &["install", "--engine", "claude-code", "--format", "text"],
    );
    assert_eq!(ran.out, "", "its own --format");
}

#[test]
fn test_an_install_with_nothing_to_install_into_fails_in_gos_words() {
    let mut r = fake(Fake::default());
    let ran = manage(&mut r, Format::Text, &["install"]);
    assert_eq!(ran.code, 1);
    assert_eq!(
        ran.err,
        "taskloom: no agent backends detected; name one with --engine (claude-code)\n"
    );
    let ran = manage(&mut r, Format::Json, &["install", "--engine", "foo"]);
    assert_eq!(ran.code, 1);
    assert_eq!(ran.out, "");
    let envelope: Value = serde_json::from_str(&ran.err).unwrap();
    assert_eq!(envelope["code"], "registration-application-unknown-engine");

    let mut r = fake(Fake {
        present: true,
        resolvable: true,
        fails: true,
        ..Fake::default()
    });
    let ran = manage(&mut r, Format::Text, &["install"]);
    assert_eq!(ran.err, "taskloom: claude-code: cannot read it\n");
}

#[test]
fn test_an_uninstall_says_whether_it_removed_anything() {
    let mut r = fake(Fake::default());
    let ran = manage(&mut r, Format::Text, &["uninstall"]);
    assert_eq!(ran.code, 0);
    assert_eq!(
        ran.err,
        "taskloom: nothing to remove (no agent backends detected; name one with --engine)\n"
    );
    let mut r = fake(Fake {
        present: true,
        ..Fake::default()
    });
    let ran = manage(&mut r, Format::Text, &["uninstall"]);
    assert_eq!(
        ran.err,
        "taskloom: not registered with claude-code, nothing to remove\n  config: /home/.claude.json\n"
    );
    r.configs.changes = true;
    let ran = manage(&mut r, Format::Text, &["uninstall"]);
    assert_eq!(
        ran.err,
        "taskloom: removed MCP server from claude-code\n  config: /home/.claude.json\n"
    );
    let ran = manage(&mut r, Format::Json, &["uninstall"]);
    let doc: Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(doc["command_resolves"], Value::Null);
    assert_eq!(doc["configs"][0]["written"], true);
}

#[test]
fn test_a_check_is_gos_table() {
    let mut r = fake(Fake {
        installed: Some(Ok(true)),
        ..Fake::default()
    });
    let ran = manage(&mut r, Format::Text, &["check"]);
    assert_eq!(ran.code, 0);
    assert_eq!(
        ran.out,
        "binary: taskloom NOT on PATH\nclaude-code  user     registered (/home/.claude.json)\n"
    );
    r.configs.resolvable = true;
    r.configs.installed = Some(Ok(false));
    let ran = manage(&mut r, Format::Text, &["check", "--dir", "/work"]);
    assert_eq!(
        ran.out,
        "binary: taskloom on PATH\nclaude-code  user     not registered (/home/.claude.json)\n"
    );
    r.configs.installed = Some(Err(Report::new("agent-configs-parse", "parse config: bad")));
    let ran = manage(&mut r, Format::Text, &["check"]);
    assert!(
        ran.out.ends_with(
            "claude-code  user     unreadable: parse config: bad (/home/.claude.json)\n"
        ),
        "{}",
        ran.out
    );
    let ran = manage(&mut r, Format::Json, &["check"]);
    let doc: Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(doc["binary_on_path"], true);
    assert_eq!(doc["configs"][0]["state"], "unreadable");
    assert_eq!(doc["configs"][0]["scope"], "user");
    assert_eq!(doc["configs"][0]["error"]["code"], "agent-configs-parse");
    r.configs.installed = Some(Ok(true));
    let ran = manage(&mut r, Format::Json, &["check"]);
    let doc: Value = serde_json::from_str(&ran.out).unwrap();
    assert_eq!(doc["configs"][0]["error"], Value::Null);
    assert_eq!(doc["configs"][0]["path"], "/home/.claude.json");
}

#[test]
fn test_help_is_an_answer_and_a_word_it_does_not_know_an_error() {
    let mut r = fake(Fake::default());
    let ran = manage(&mut r, Format::Text, &["--help"]);
    assert_eq!(ran.code, 0);
    assert!(ran.out.contains("install"), "{}", ran.out);
    let ran = manage(&mut r, Format::Text, &["install", "--help"]);
    assert!(ran.out.contains("--print-only"), "{}", ran.out);
    let ran = manage(&mut r, Format::Json, &["frob"]);
    assert_eq!(ran.code, 1);
    let envelope: Value = serde_json::from_str(&ran.err).unwrap();
    assert_eq!(envelope["code"], "registration-cli-usage");
    let ran = manage(&mut r, Format::Text, &[]);
    assert_eq!(ran.code, 1);
    assert!(ran.err.contains("Usage"), "{}", ran.err);
}
