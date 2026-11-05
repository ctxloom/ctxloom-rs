//! Gherkin features (S9, the surface half), run with cucumber-rs over the real
//! binary: Go ctxloom's own `.feature` text (revision f36e8dfb21, release/0.7;
//! each file's header names what was left out) with new Rust step definitions.
//!
//! plan supersedes Go, where a step differs:
//! - "the session index records harp ... on engine ...": a no-op. The engine
//!   whose codec reads the payload is the one `--engine` names (plan 8.4), not
//!   one looked up from the session index.
//! - "the project already has the bundle ...:" takes Go's inline bundle
//!   document and writes the tree this port reads (an envelope with inline
//!   items is refused by the tree reader).
//! - a skill's execute bit comes from the sidecar's `executable:` (plan 8.2),
//!   not from the file's mode.
//! - the engines are the ones this port registers: `claude-code`, `mock` and
//!   `mock-launch`.

use std::fmt;
use std::path::Path;

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use serde_json::Value;

use super::gherkin_common::{common_steps, doc, Common, CommonWorld};
use super::library::{at, reports_as, Project};
use super::surfaces::{additional_context, hook_lines};

const CONTEXT_MARKER: &str = "J000400-CONTEXT-MARKER-3f8a91";
const COMMAND_MARKER: &str = "J000400-COMMAND-BODY-MARKER-7c2d64";
const MCP_COMMAND: &str = "j000400-mcp-tool-9e1b52";
const HOOK_COMMAND: &str = "echo J000400-HOOK-COMMAND-4a6f18";
const HAND_AUTHORED: &str = "# Team conventions\n\n<!-- Alice wrote this by hand; ctxloom must never touch it -->\nJ000400-HAND-AUTHORED-MARKER-6d2e73: always use tabs, never spaces\n\n## House style\n\n- tabs, not spaces\n- no trailing whitespace\n";

/// Go's `steps_j000600_doctor.go`: the skill Alice authors.
const DOCTOR_SKILL: &str = "---\nname: ctxloom-doctor\ndescription: \"DOCTOR-SKILL-MARKER-7d4e21 - Validate a ctxloom setup: dependencies, agents, version currency, hooks and MCP; run before relying on isolation.\"\n---\n\n# ctxloom doctor\n\n## DOCTOR-CHECK-DEPS-a1: required binaries on PATH\n\n## DOCTOR-CHECK-AGENTS-b2: agents resolve cleanly\n\n## DOCTOR-CHECK-VERSION-c3: version currency\n\n## DOCTOR-CHECK-HOOKS-TRUST-d4: hooks and MCP\n";

/// Go's `steps_turn_transcript.go`: one prompt, one assistant message.
fn transcript(closing: &str) -> String {
    let text = Value::String(closing.to_string()).to_string();
    let user = r#"{"type":"user","isSidechain":false,"cwd":"/repo","sessionId":"s","version":"2.1.44","message":{"role":"user","content":[{"type":"text","text":"go"}]},"uuid":"u1","timestamp":"2026-08-22T10:00:00.000Z"}"#;
    let assistant = format!(
        r#"{{"type":"assistant","isSidechain":false,"cwd":"/repo","sessionId":"s","version":"2.1.44","message":{{"model":"m","id":"msg_1","type":"message","role":"assistant","content":[{{"type":"text","text":{text}}}],"stop_reason":"end_turn","usage":{{"input_tokens":1,"output_tokens":1}}}},"uuid":"a1","timestamp":"2026-08-22T10:00:02.000Z"}}"#
    );
    format!("{user}\n{assistant}\n")
}

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct World {
    common: Common,
    /// The `-<engine>` target of a j000400 materialize.
    hand_authored: Option<(String, String)>,
    /// The engine and target of the last team materialize.
    team: Option<(String, String)>,
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "World")
    }
}

impl CommonWorld for World {
    fn common(&self) -> &Common {
        &self.common
    }

    fn common_mut(&mut self) -> &mut Common {
        &mut self.common
    }
}

impl World {
    fn new() -> World {
        World {
            common: Common::new(),
            hand_authored: None,
            team: None,
        }
    }

    fn tree_has(&self, dir: &str, needle: &str) -> bool {
        self.common
            .project
            .tree(dir)
            .values()
            .any(|bytes| String::from_utf8_lossy(bytes).contains(needle))
    }
}

// ------------------------------------------------------------------ given

#[given(expr = "a bundle {string} exists")]
fn bundle(w: &mut World, name: String) {
    w.common.project.bundle(&name);
}

#[given(expr = "a fragment {string} in bundle {string} exists")]
fn fragment(w: &mut World, name: String, bundle: String) {
    w.common.project.fragment(&bundle, &name);
}

#[given(expr = "a command {string} in bundle {string} exists")]
fn command(w: &mut World, name: String, bundle: String) {
    w.common.project.command(&bundle, &name);
}

#[given(expr = "a profile {string} with bundle {string}")]
fn profile(w: &mut World, name: String, bundle: String) {
    w.common.project.profile(&name, &bundle);
}

#[given(expr = "the session harp is {string}")]
fn session_harp(w: &mut World, name: String) {
    w.common
        .env
        .push(("CTXLOOM_SESSION_HARP".to_string(), name));
}

#[given(expr = "the session index records harp {string} on engine {string}")]
fn index_records(_w: &mut World, _harp: String, _engine: String) {
    // plan 8.4: `--engine` names the codec; nothing is looked up.
}

/// Go's inline bundle document, as the tree this port reads: the envelope, a
/// hook per entry (its `order:` in a sidecar), a skill's tags in its sidecar.
#[given(expr = "the project already has the bundle {string}:")]
fn project_bundle(w: &mut World, step: &Step, name: String) {
    let parsed = angzarr_yaml::parse(&doc(step)).expect("a bundle document");
    let root = format!(".ctxloom/content/bundles/v2/{name}");
    let text = |key: &str| {
        parsed
            .get(key)
            .and_then(angzarr_yaml::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let version = if text("version").is_empty() {
        "1.0.0".to_string()
    } else {
        text("version")
    };
    let mut envelope = format!("schema_version: 4\nversion: {version}\n");
    if !text("description").is_empty() {
        envelope.push_str(&format!("description: {}\n", text("description")));
    }
    w.common
        .project
        .write(&format!("{root}/bundle.yaml"), &envelope);
    for (event, hooks) in parsed
        .get("hooks")
        .and_then(angzarr_yaml::Value::as_map)
        .unwrap_or_default()
    {
        for (i, hook) in hooks.as_seq().unwrap_or_default().iter().enumerate() {
            let field = |k: &str| {
                hook.get(k)
                    .and_then(angzarr_yaml::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            w.common.project.write(
                &format!("{root}/hooks/{event}/h{i}.yaml"),
                &format!("type: {}\ncommand: {}\n", field("type"), field("command")),
            );
            if !field("order").is_empty() {
                w.common.project.write(
                    &format!("{root}/hooks/{event}/.h{i}.meta.yaml"),
                    &format!("order: {}\n", field("order")),
                );
            }
        }
    }
    for (skill, body) in parsed
        .get("skills")
        .and_then(angzarr_yaml::Value::as_map)
        .unwrap_or_default()
    {
        let tags: Vec<String> = body
            .get("tags")
            .and_then(angzarr_yaml::Value::as_seq)
            .unwrap_or_default()
            .iter()
            .filter_map(|t| t.as_str().map(str::to_string))
            .collect();
        w.common.project.write(
            &format!("{root}/skills/.{skill}.meta.yaml"),
            &format!("tags:\n  - {}\n", tags.join("\n  - ")),
        );
    }
}

#[given(expr = "a {string} transcript at {string} whose turn ends with:")]
fn transcript_at(w: &mut World, step: &Step, _engine: String, rel: String) {
    w.common.project.write(&rel, &transcript(&doc(step)));
}

#[given("ctxloom is already wired into her project")]
fn wired(w: &mut World) {
    w.common.run_lines(
        "ctxloom manage install --engine claude-code --no-pull\nctxloom materialize --yes --degraded",
        "",
    );
    w.ran().succeeds();
}

#[given(expr = "Carol's team profile carries a shared fragment, command, MCP server, and hook")]
fn team(w: &mut World) {
    let project = Project::initialized();
    let root = ".ctxloom/content/bundles/v2/team";
    project.write(
        &format!("{root}/bundle.yaml"),
        "schema_version: 4\nversion: 1.0.0\ndescription: J000400 shared team bundle\n",
    );
    project.write(
        &format!("{root}/fragments/onboarding-context.md"),
        CONTEXT_MARKER,
    );
    project.write(
        &format!("{root}/prompts/onboarding.md"),
        &format!("---\ndescription: J000400 onboarding command\n---\n{COMMAND_MARKER}"),
    );
    project.write(
        &format!("{root}/mcp/toolserver.yaml"),
        &format!("command: {MCP_COMMAND}\nargs:\n  - \"--flag\"\n"),
    );
    project.write(
        &format!("{root}/hooks/session_start/team-hook.yaml"),
        &format!("type: command\ncommand: {HOOK_COMMAND:?}\n"),
    );
    project.profile("team", "team");
    w.common.project = project;
}

#[given(expr = "Alice's team already hand-authored {word} for {word} with their own conventions")]
fn hand_authored(w: &mut World, file: String, engine: String) {
    w.common
        .project
        .write(&format!("out-{engine}/{file}"), HAND_AUTHORED);
    w.hand_authored = Some((engine, file));
}

#[given(expr = "Alice's project has a directory-form bundle {string}")]
fn directory_bundle(w: &mut World, name: String) {
    w.common.project = Project::initialized();
    w.common.project.write(
        &format!(".ctxloom/content/bundles/v2/{name}/bundle.yaml"),
        "schema_version: 4\nversion: 1.0.0\n",
    );
}

#[given(expr = "Alice starts a ctxloom-doctor skill in {string} described as {string}")]
fn start_skill(w: &mut World, bundle: String, described: String) {
    w.common.run_lines(
        &format!("ctxloom skill create {bundle} ctxloom-doctor -d {described}"),
        "",
    );
    w.ran().succeeds();
}

fn skill_dir(reference: &str) -> String {
    let (bundle, rest) = reference.split_once('#').expect("a skill reference");
    let name = rest.trim_start_matches("skills/");
    format!(".ctxloom/content/bundles/v2/{bundle}/skills/{name}")
}

#[given(expr = "Alice authors the ctxloom-doctor skill's full body in {string}")]
fn author_skill(w: &mut World, reference: String) {
    w.common
        .project
        .write(&format!("{}/SKILL.md", skill_dir(&reference)), DOCTOR_SKILL);
}

#[given(
    regex = r#"^Alice adds an executable scripts/run.sh carrying the marker "([^"]*)" to the skill "([^"]*)"$"#
)]
fn add_script(w: &mut World, marker: String, reference: String) {
    let dir = skill_dir(&reference);
    w.common.project.write(
        &format!("{dir}/scripts/run.sh"),
        &format!("#!/bin/sh\necho {marker}\n"),
    );
    let sidecar = dir.replace("/skills/", "/skills/.") + ".meta.yaml";
    let held = w.common.project.read(&sidecar).unwrap_or_default();
    w.common.project.write(
        &sidecar,
        &format!("{held}executable:\n  - scripts/run.sh\n"),
    );
}

#[given(expr = "profile {string} curates skill {string}")]
fn curate(w: &mut World, name: String, reference: String) {
    let rel = format!(".ctxloom/content/bundles/v2/project/profiles/{name}.yaml");
    let held = w.common.project.read(&rel).expect("the profile");
    w.common
        .project
        .write(&rel, &format!("{held}skills:\n  - {reference}\n"));
}

// ------------------------------------------------------------------- when

#[when(regex = r"^Alice materializes the team profile for (\S+)$")]
fn materialize_team(w: &mut World, engine: String) {
    let target = format!("out-{engine}");
    w.common.run_lines(
        &format!("ctxloom materialize team --target {target} --backend {engine}"),
        "",
    );
    w.ran().succeeds();
    w.team = Some((engine, target));
}

#[when(regex = r#"^Carol materializes the "([^"]*)" profile for (\S+)$"#)]
fn materialize_profile(w: &mut World, profile: String, engine: String) {
    let target = format!("out-{engine}");
    w.common.run_lines(
        &format!("ctxloom materialize {profile} --target {target} --backend {engine}"),
        "",
    );
    w.ran().succeeds();
    w.team = Some((engine, target));
}

// ------------------------------------------------------------------- then

/// A pattern of `\A`, `\z`, `\n`, `\n?`, `[\s\S]*` and literal text, which is
/// all Go's scenarios use.
fn matches(pattern: &str, text: &str) -> bool {
    fn go(p: &str, t: &str, at_start: bool) -> bool {
        if let Some(rest) = p.strip_prefix("\\A") {
            return at_start && go(rest, t, at_start);
        }
        if p == "\\z" {
            return t.is_empty();
        }
        if let Some(rest) = p.strip_prefix("[\\s\\S]*") {
            return (0..=t.len())
                .filter(|i| t.is_char_boundary(*i))
                .any(|i| go(rest, &t[i..], false));
        }
        if let Some(rest) = p.strip_prefix("\\n?") {
            return t.strip_prefix('\n').is_some_and(|t| go(rest, t, false)) || go(rest, t, false);
        }
        if let Some(rest) = p.strip_prefix("\\n") {
            return t.strip_prefix('\n').is_some_and(|t| go(rest, t, false));
        }
        match (p.chars().next(), t.chars().next()) {
            (None, _) => true,
            (Some(a), Some(b)) if a == b => go(&p[a.len_utf8()..], &t[b.len_utf8()..], false),
            _ => false,
        }
    }
    // An unanchored pattern may start anywhere.
    if pattern.starts_with("\\A") {
        go(pattern, text, true)
    } else {
        (0..=text.len())
            .filter(|i| text.is_char_boundary(*i))
            .any(|i| go(pattern, &text[i..], false))
    }
}

#[then(expr = "the output matches {string}")]
fn output_matches(w: &mut World, pattern: String) {
    assert!(
        matches(&pattern, &w.ran().output()),
        "{pattern}: {}",
        w.ran().said()
    );
}

#[then(expr = "the file {string} matches {string}")]
fn file_matches(w: &mut World, rel: String, pattern: String) {
    let held = w.common.project.read(&rel).expect("a file");
    assert!(matches(&pattern, &held), "{rel}: {held:?}");
}

#[then(expr = "the file {string} is valid YAML")]
fn valid_yaml(w: &mut World, rel: String) {
    let held = w.common.project.read(&rel).expect("a file");
    angzarr_yaml::parse(&held).unwrap_or_else(|e| panic!("{rel}: {e}"));
}

#[then(expr = "the file {string} registers a SessionStart hook whose command contains {string}")]
fn registers_hook(w: &mut World, rel: String, text: String) {
    assert!(
        w.common.project.registers_hook(&rel, "SessionStart", &text),
        "{rel}"
    );
}

#[then(expr = "the file {string} registers no SessionStart hook whose command contains {string}")]
fn registers_no_hook(w: &mut World, rel: String, text: String) {
    assert!(
        !w.common.project.registers_hook(&rel, "SessionStart", &text),
        "{rel}"
    );
}

#[then(expr = "the file {string} registers no MCP server named {string}")]
fn registers_no_mcp(w: &mut World, rel: String, name: String) {
    assert!(
        !w.common.project.registers_mcp(&rel, "mcpServers", &name),
        "{rel}"
    );
}

#[then(expr = "the JSON output array {string} contains an object whose {string} is {string}")]
fn array_has(w: &mut World, path: String, key: String, value: String) {
    let doc = w.json();
    let array = at(&doc, &path)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("no array at {path}: {doc}"));
    assert!(array.iter().any(|o| o[&key] == value.as_str()), "{doc}");
}

#[then(expr = "every object in the JSON output array {string} has a non-empty {string}")]
fn every_non_empty(w: &mut World, path: String, key: String) {
    let doc = w.json();
    let array = at(&doc, &path)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("no array at {path}: {doc}"));
    assert!(array
        .iter()
        .all(|o| o[&key].as_str().is_some_and(|s| !s.is_empty())));
}

#[then(expr = "the hook's additionalContext contains {string}")]
fn context_contains(w: &mut World, text: String) {
    let context = additional_context(w.ran());
    assert!(context.contains(&text), "{}", w.ran().said());
}

#[then("the hook's additionalContext is empty")]
fn context_empty(w: &mut World) {
    assert_eq!(additional_context(w.ran()), "", "{}", w.ran().said());
}

// j000100

#[given("ctxloom's session-start hook is wired into her assistant")]
#[then("ctxloom's session-start hook is wired into her assistant")]
fn session_start_wired(w: &mut World) {
    assert!(w.common.project.registers_hook(
        ".claude/settings.json",
        "SessionStart",
        "hook session-start"
    ));
}

#[then("ctxloom's own working state is kept out of source control")]
fn state_ignored(w: &mut World) {
    w.common
        .project
        .file_contains(".ctxloom/.gitignore", "/cache");
}

#[then("ctxloom reports the engine it wired her project for")]
fn reports_engine(w: &mut World) {
    w.ran().contains("claude-code");
    assert_eq!(
        reports_as(&w.json(), "backends[backend=claude-code].hooks_present"),
        "true"
    );
}

#[then("nothing ctxloom wired into her assistant is left behind")]
fn nothing_left(w: &mut World) {
    w.common.project.file_is_absent(".claude/settings.json");
    w.common.project.file_is_absent("CLAUDE.md");
}

#[then("the context she authored is still hers")]
fn still_hers(w: &mut World) {
    assert!(w.common.project.exists(".ctxloom/config.yaml"));
    assert!(w
        .common
        .project
        .exists(".ctxloom/content/bundles/v2/project/profiles/default.yaml"));
}

// j000400

fn team_target(w: &World, engine: &str) -> String {
    match &w.team {
        Some((e, target)) if e == engine => target.clone(),
        _ => format!("out-{engine}"),
    }
}

#[then(
    regex = r"^the materialized (\S+) context carries the shared fragment's marker, in its own native shape$"
)]
fn team_context(w: &mut World, engine: String) {
    let file = if engine == "claude-code" {
        "CLAUDE.md"
    } else {
        "MOCK_CONTEXT.md"
    };
    let target = team_target(w, &engine);
    w.common
        .project
        .file_contains(&format!("{target}/{file}"), CONTEXT_MARKER);
}

#[then(
    regex = r"^the materialized (\S+) MCP configuration carries the shared server's command, in its own native shape$"
)]
fn team_mcp(w: &mut World, engine: String) {
    let target = team_target(w, &engine);
    let mcp = w.common.project.json_file(&format!("{target}/.mcp.json"));
    assert_eq!(mcp["mcpServers"]["toolserver"]["command"], MCP_COMMAND);
}

#[then(
    regex = r"^the materialized (\S+) MCP configuration registers no server under ctxloom's own name$"
)]
fn team_mcp_no_ctxloom(w: &mut World, engine: String) {
    let target = team_target(w, &engine);
    let mcp = w.common.project.json_file(&format!("{target}/.mcp.json"));
    let servers = mcp["mcpServers"]
        .as_object()
        .unwrap_or_else(|| panic!("{engine}: no mcpServers table: {mcp}"));
    assert!(!servers.contains_key("ctxloom"), "{engine}: {mcp}");
}

#[then(
    regex = r"^the materialized (\S+) hook configuration carries the shared hook's command, in its own native shape$"
)]
fn team_hook(w: &mut World, engine: String) {
    let target = team_target(w, &engine);
    let doc = w
        .common
        .project
        .json_file(&format!("{target}/.claude/settings.json"));
    assert!(
        hook_lines(&doc, "SessionStart")
            .iter()
            .any(|l| l.contains(HOOK_COMMAND)),
        "{doc}"
    );
}

#[then(
    regex = r"^the materialized (\S+) command file carries the shared command's body, in its own native shape$"
)]
fn team_command(w: &mut World, engine: String) {
    let target = team_target(w, &engine);
    w.common.project.file_contains(
        &format!("{target}/.claude/commands/team-onboarding.md"),
        COMMAND_MARKER,
    );
}

#[then(
    regex = r"^no (\S+) surface anywhere in the materialized tree carries the shared (hook's command|MCP server's command|command's body)$"
)]
fn nowhere(w: &mut World, engine: String, what: String) {
    let needle = match what.as_str() {
        "hook's command" => HOOK_COMMAND,
        "MCP server's command" => MCP_COMMAND,
        _ => COMMAND_MARKER,
    };
    assert!(
        !w.tree_has(&format!("out-{engine}"), needle),
        "{engine}: {what}"
    );
}

#[then(regex = r"^the materialize report names each surface (\S+) does not carry, with a reason$")]
fn report_names(w: &mut World, engine: String) {
    if w.common.text_asked() {
        // Go's text arm: the report says "NOT carried" and names each surface.
        let said = w.ran().output();
        assert!(said.contains("NOT carried"), "{engine}: {said}");
        for surface in ["mcp", "hooks", "commands"] {
            assert!(said.contains(surface), "{surface}: {said}");
        }
        return;
    }
    let doc = w.json();
    let carried = doc["engines"][0]["not_carried"].to_string();
    for surface in ["mcp", "hooks", "commands"] {
        assert!(carried.contains(surface), "{surface}: {carried}");
    }
    assert!(carried.contains("reason"), "{carried}");
}

#[then(regex = r"^(\S+) still carries Alice's hand-authored conventions, byte-for-byte$")]
fn hand_authored_kept(w: &mut World, file: String) {
    let (engine, _) = w.hand_authored.clone().expect("a hand-authored file");
    let held = w
        .common
        .project
        .read(&format!("out-{engine}/{file}"))
        .expect("the file");
    let authored = HAND_AUTHORED.trim_end_matches('\n');
    assert!(held.starts_with(authored), "{file}: {held:?}");
    assert!(
        !held[authored.len()..].trim().is_empty(),
        "nothing alongside"
    );
}

// j000600

#[then(expr = "the claude-code skill surface carries the doctor skill's marker {string}")]
fn skill_marker(w: &mut World, marker: String) {
    w.common.project.file_contains(
        "out-claude-code/.claude/skills/ctxloom-doctor/SKILL.md",
        &marker,
    );
}

#[then(expr = "{string} is executable")]
fn is_executable(w: &mut World, rel: String) {
    assert!(w.common.project.is_executable(&rel), "{rel}");
}

#[then(expr = "{string} carries the marker {string}")]
fn carries_marker(w: &mut World, rel: String, marker: String) {
    w.common.project.file_contains(&rel, &marker);
}

// ------------------------------------------------- j002000: engine switch day

/// Go's `j002000Marker`: the team's guidance.
const J002000_MARKER: &str = "J002000-TEAM-GUIDANCE-MARKER";

/// Go's `j002000Setup`: a project declaring both engines (the new one is
/// `mock`, this port's stand-in for Go's `mock-lossy`), an agent `dev` bound
/// to the old one, and the team's guidance composed into the profile it names.
#[given("Alice's team has standardised on one engine and is moving to another")]
fn j002000_setup(w: &mut World) {
    w.common.project = Project::initialized();
    w.common.project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\nllm:\n  configs:\n    claude-code:\n      type: claude-code\n    mock:\n      type: mock\n  defaults:\n    primary: claude-code\n    fast: claude-code\nagents:\n  dev:\n    llm: claude-code\n    profiles:\n      - default\ndefault_agent: dev\n",
    );
    let root = ".ctxloom/content/bundles/v2/house";
    w.common.project.write(
        &format!("{root}/bundle.yaml"),
        "schema_version: 4\nversion: 1.0.0\n",
    );
    w.common.project.write(
        &format!("{root}/fragments/house-guidance.md"),
        J002000_MARKER,
    );
    w.common.project.write(
        &format!("{root}/hooks/session_end/h0.yaml"),
        "type: command\ncommand: echo j002000-session-end\n",
    );
    // Go's scaffold: a seed bundle and a "default" profile including it.
    w.common.run_lines(
        "ctxloom bundle create seed -d \"J000200 seed bundle\"\nctxloom profile create default --include seed -d \"J000200 default profile\"\nctxloom profile modify default --add-bundle house",
        "",
    );
    w.ran().succeeds();
}

/// Go's `ctxloom refuses and names the engines it knows`.
#[then("ctxloom refuses and names the engines it knows")]
fn refuses_naming_engines(w: &mut World) {
    let ran = w.ran();
    ran.fails();
    let said = ran.said();
    assert!(
        said.contains("mock") || said.contains("claude-code"),
        "ctxloom refused without naming any engine it knows: {said}"
    );
}

// ------------------------------------------------------------ the features

common_steps!();

/// Run one feature file with this World; whether it passed.
pub(super) fn run(file: &Path) -> bool {
    super::gherkin_common::run_feature::<World>(file)
}
