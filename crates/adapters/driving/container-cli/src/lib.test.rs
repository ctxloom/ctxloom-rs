//! The `container` commands on a fake application. Ported from Go's
//! cli/container.feature (release/0.7, f36e8dfb21); the plan's override is
//! named where one applies.

use angzarr_presentation::Format;
use isolation_application::{
    CheckReport, ContainerUse, Declaration, IsolationError, ScaffoldReport, ToolingReport,
};

use super::{commands, run, Call, Failure};

struct Fake {
    tooling: Vec<Declaration>,
}

fn report(backend: &str) -> CheckReport {
    CheckReport {
        backend: backend.to_string(),
        in_container: false,
        markers: Vec::new(),
        runtime: "none".to_string(),
        reachable: false,
        image: String::new(),
        image_present: false,
        shared_fs: "unprobed: no runtime".to_string(),
        guidance: vec!["no container runtime is reachable".to_string()],
    }
}

impl ContainerUse for Fake {
    fn check(&self, backend: Option<&str>) -> Result<CheckReport, IsolationError> {
        match backend {
            Some("bogus") => Err(IsolationError::UnknownBackend {
                backend: "bogus".to_string(),
                available: "claude-code, mock".to_string(),
            }),
            other => Ok(report(other.unwrap_or("claude-code"))),
        }
    }

    fn build(&self, backend: Option<&str>, _: Option<&str>) -> Result<(), IsolationError> {
        match backend {
            Some("bogus") => Err(IsolationError::UnknownBackend {
                backend: "bogus".to_string(),
                available: "claude-code, mock".to_string(),
            }),
            _ => Err(IsolationError::NoBuildRuntime),
        }
    }

    fn prune(&self, runtime: Option<&str>, _: bool) -> IsolationError {
        match runtime {
            Some("rkt") => IsolationError::UnknownRuntime {
                runtime: "rkt".to_string(),
                known: "docker|podman".to_string(),
            },
            _ => IsolationError::NoPruneRuntime,
        }
    }

    fn scaffold(&self) -> Result<ScaffoldReport, IsolationError> {
        Ok(ScaffoldReport {
            dir: ".devcontainer".to_string(),
        })
    }

    fn tooling(&self) -> Result<ToolingReport, IsolationError> {
        Ok(ToolingReport {
            instructions: "INSTRUCTIONS FOR THE LLM".to_string(),
            declarations: self.tooling.clone(),
        })
    }
}

fn fake() -> Fake {
    Fake {
        tooling: Vec::new(),
    }
}

/// Run `ctxloom <args>` on `uses`: the answer, or the failure.
fn go(uses: &Fake, args: &[&str], explicit: Option<Format>) -> (Result<u8, Failure>, String) {
    let matches = clap::Command::new("ctxloom")
        .subcommands(commands::nouns())
        .get_matches_from(std::iter::once("ctxloom").chain(args.iter().copied()));
    let call = Call::new(&matches, explicit, Format::Json);
    let mut out = Vec::new();
    let result = run(uses, &call, &mut out);
    (result, String::from_utf8(out).unwrap())
}

// "The bare noun lists what the namespace holds".
#[test]
fn the_bare_noun_prints_help_naming_what_it_holds() {
    let (result, out) = go(&fake(), &["container"], Some(Format::Text));
    assert_eq!(result.unwrap(), 0);
    for verb in ["scaffold", "tooling", "check", "build", "prune"] {
        assert!(out.contains(verb), "{verb} in {out}");
    }
}

// "A machine-readable format aimed at the namespace itself is refused".
#[test]
fn a_machine_readable_format_aimed_at_the_bare_noun_is_refused() {
    let (result, out) = go(&fake(), &["container"], Some(Format::Json));
    let failure = result.unwrap_err();
    assert_eq!(failure.cause.code(), "container-cli-help-not-payload");
    assert_eq!(failure.exit, 2);
    assert!(out.is_empty(), "no help is printed: {out}");
}

// "The capability check is diagnostic-only": json carries every axis.
#[test]
fn check_in_json_reports_every_axis() {
    let (result, out) = go(&fake(), &["container", "check", "mock"], None);
    assert_eq!(result.unwrap(), 0);
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["in_container"], false);
    assert_eq!(doc["runtime"], "none");
    assert_eq!(doc["reachable"], false);
    assert_eq!(doc["image_present"], false);
    assert!(doc["shared_fs"].as_str().unwrap().starts_with("unprobed"));
}

// "The text report labels every axis, and writes nothing".
#[test]
fn check_in_text_labels_every_axis() {
    let (result, out) = go(&fake(), &["container", "check", "mock"], Some(Format::Text));
    assert_eq!(result.unwrap(), 0);
    assert!(
        out.contains("Container capability (backend: mock)"),
        "{out}"
    );
    assert!(out.contains("in a container:"), "{out}");
    assert!(out.contains("runtime:"), "{out}");
    assert!(out.contains("shared fs:"), "{out}");
    assert!(
        out.contains("-> no container runtime is reachable"),
        "{out}"
    );
}

#[test]
fn check_in_text_names_the_container_the_image_and_the_fault() {
    let mut full = report("mock");
    full.in_container = true;
    full.markers = vec!["/.dockerenv".to_string(), "$CODESPACES".to_string()];
    full.runtime = "docker".to_string();
    full.reachable = true;
    full.image = "ctxloom-agent-mock".to_string();
    full.image_present = true;
    let mut out = Vec::new();
    super::text::check(&full, &mut out).unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(
        out.contains("in a container:  yes (/.dockerenv, $CODESPACES)"),
        "{out}"
    );
    assert!(out.contains("runtime:         docker (reachable)"), "{out}");
    assert!(
        out.contains("agent image:     ctxloom-agent-mock (present)"),
        "{out}"
    );
}

// "An unknown backend is a usage error".
#[test]
fn an_unknown_backend_fails_check_and_build_alike() {
    for verb in ["check", "build"] {
        let (result, _) = go(&fake(), &["container", verb, "bogus"], None);
        let failure = result.unwrap_err();
        assert_eq!(
            failure.cause.code(),
            "isolation-application-unknown-backend"
        );
        assert_eq!(failure.exit, 2);
    }
}

// "With no container runtime on this machine, prune exits 3".
#[test]
fn prune_refusals_carry_their_exit_codes() {
    let (result, out) = go(&fake(), &["container", "prune"], None);
    let failure = result.unwrap_err();
    assert_eq!(
        failure.cause.code(),
        "isolation-application-no-prune-runtime"
    );
    assert_eq!(failure.exit, 3);
    assert!(out.is_empty());
    let (result, _) = go(&fake(), &["container", "prune", "--runtime", "rkt"], None);
    assert_eq!(result.unwrap_err().exit, 2);
}

#[test]
fn scaffold_says_where_it_wrote_and_what_to_do_next() {
    let (result, out) = go(&fake(), &["container", "scaffold"], None);
    assert_eq!(result.unwrap(), 0);
    assert!(out.contains("Devcontainer: .devcontainer"), "{out}");
    assert!(out.contains("ctxloom container build"), "{out}");
}

// "The machine-readable form carries the instructions and the declarations".
#[test]
fn tooling_in_json_carries_the_instructions_and_the_declarations() {
    let uses = Fake {
        tooling: vec![Declaration {
            source: "tooled".to_string(),
            content: "TOOLING-DECL-TOOLED".to_string(),
        }],
    };
    let (result, out) = go(&uses, &["container", "tooling", "list"], Some(Format::Json));
    assert_eq!(result.unwrap(), 0);
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["instructions"], "INSTRUCTIONS FOR THE LLM");
    assert_eq!(doc["declarations"][0]["content"], "TOOLING-DECL-TOOLED");
}

// "Bare container tooling lists the declarations" (text).
#[test]
fn bare_tooling_lists_and_the_text_form_attributes_each_declaration() {
    let uses = Fake {
        tooling: vec![Declaration {
            source: "tooled".to_string(),
            content: "TOOLING-DECL-TOOLED".to_string(),
        }],
    };
    let (result, out) = go(&uses, &["container", "tooling"], Some(Format::Text));
    assert_eq!(result.unwrap(), 0);
    assert!(out.contains("INSTRUCTIONS FOR THE LLM"), "{out}");
    assert!(out.contains("### tooled"), "{out}");
    assert!(out.contains("TOOLING-DECL-TOOLED"), "{out}");
    assert!(!out.contains("Commands:"), "{out}");
}

// "An unregistered companion contributes nothing": the empty declarations
// are an empty list in json, and a sentence in text.
#[test]
fn tooling_with_nothing_declared_says_so() {
    let (result, out) = go(
        &fake(),
        &["container", "tooling", "list"],
        Some(Format::Text),
    );
    assert_eq!(result.unwrap(), 0);
    assert!(
        out.contains("No registered companion declares container tooling"),
        "{out}"
    );
    let (result, out) = go(
        &fake(),
        &["container", "tooling", "list"],
        Some(Format::Json),
    );
    assert_eq!(result.unwrap(), 0);
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["declarations"], serde_json::json!([]));
}
