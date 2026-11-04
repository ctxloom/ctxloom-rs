//! The library commands as the program runs them: the command line parsed by
//! this crate's own nouns, the application over in-memory ports, the answer
//! as bytes. Each test names the Go command (`internal/adapters/cli`) or the
//! Gherkin feature it holds the adapter to.

use angzarr_presentation::{Catalog, Format};
use clap::Command;
use library_application::fakes::{Setup, ENVELOPE};

use super::*;

const DEMO: &[(&str, &str)] = &[
    ENVELOPE,
    (
        "fragments/golang.md",
        "---\ndescription: when editing Go code\ntags: [go]\n---\nGO-RULES\n",
    ),
    ("fragments/testing.md", "# testing\n\nTEST-BODY\n"),
    ("fragments/ctl.md", "a\u{7}b\n"),
    (
        "fragments/dist.md",
        "---\ntags: [distilled]\n---\nlong form\n",
    ),
    ("fragments/dist.distilled.md", "short form\n"),
    (
        "prompts/review.md",
        "---\ndescription: Review code\n---\nREVIEW-BODY\n",
    ),
    (
        "skills/reviewer/SKILL.md",
        "---\nname: reviewer\ndescription: Reviews code\n---\nSKILL-BODY\n",
    ),
    (
        "mcp/tree-sitter.yaml",
        "command: tree-sitter\nargs: [--stdio]\n",
    ),
];

const PROJECT: &[(&str, &str)] = &[
    ENVELOPE,
    ("profiles/broken.yaml", "bundles: [does-not-exist]\n"),
    (
        "profiles/dev.yaml",
        "description: dev profile\nbundles: [demo]\ntags: [team]\n",
    ),
];

const AGENTS: &str = "schema_version: 8\ncompanions: [acme]\nagents:\n  dev:\n    llm: claude-code\n    profiles: [dev]\n    runtime: host\ndefault_agent: dev\n";

/// What one invocation left behind.
struct Ran {
    exit: Result<u8, String>,
    out: String,
    err: String,
}

impl Ran {
    fn code(&self) -> u8 {
        match &self.exit {
            Ok(code) => *code,
            Err(failure) => panic!("failed: {failure}"),
        }
    }

    fn refused(&self) -> &str {
        match &self.exit {
            Ok(code) => panic!("exit {code}, not a failure: {}", self.out),
            Err(failure) => failure,
        }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.out).unwrap_or_else(|e| panic!("{e}: {}", self.out))
    }
}

/// How an invocation is made.
#[derive(Clone, Copy)]
struct How {
    explicit: Option<Format>,
    terminal: bool,
    degraded: bool,
}

const JSON: How = How {
    explicit: Some(Format::Json),
    terminal: false,
    degraded: false,
};

const TEXT: How = How {
    explicit: Some(Format::Text),
    terminal: false,
    degraded: false,
};

fn go(line: &[&str], how: How, layers: &[&str]) -> Ran {
    let setup = Setup {
        bundles: &[("demo", DEMO), ("project", PROJECT)],
        layers,
        path: &[("acme", "/usr/bin/acme")],
        ..Setup::default()
    };
    let program = Command::new("ctxloom")
        .subcommand_required(true)
        .subcommands(nouns());
    let mut argv = vec!["ctxloom"];
    argv.extend_from_slice(line);
    let matches = program.try_get_matches_from(argv).unwrap_or_else(|e| {
        panic!("{line:?} does not parse: {e}");
    });
    let noun = matches.subcommand_name().unwrap().to_string();
    let call = Call {
        matches: &matches,
        explicit_format: how.explicit,
        default_format: Format::Json,
        stdout_is_terminal: how.terminal,
        degraded: how.degraded,
        no_companions: false,
        config_set: Vec::new(),
    };
    let catalog = Catalog::en_us(&[library_application::EN_US]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = run(&setup.library(), &noun, &call, &catalog, &mut out, &mut err)
        .map_err(|f| format!("{}:{}", f.cause.code(), f.exit));
    Ran {
        exit,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

const BARE: &[&str] = &[AGENTS];

fn json_of(line: &[&str]) -> serde_json::Value {
    let ran = go(line, JSON, BARE);
    assert_eq!(ran.code(), 0, "{line:?}");
    ran.json()
}

fn text_of(line: &[&str]) -> String {
    let ran = go(line, TEXT, BARE);
    assert_eq!(ran.code(), 0, "{line:?}");
    ran.out
}

// config.feature: `config show` prints the effective configuration, one
// section, or only what the files set (--raw); a bare `config` is `show`.
#[test]
fn test_config_show_prints_the_effective_configuration_a_section_or_the_raw_layers() {
    let all = json_of(&["config", "show"]);
    assert_eq!(all["default_agent"], "dev");
    assert_eq!(all["llm"]["defaults"]["primary"], "claude-code");
    assert_eq!(json_of(&["config"]), all);
    assert_eq!(
        json_of(&["config", "show", "companions"]),
        serde_json::json!(["acme"])
    );
    let raw = json_of(&["config", "show", "--raw"]);
    assert!(raw.get("llm").is_none());
    assert_eq!(raw["companions"], serde_json::json!(["acme"]));
    assert!(text_of(&["config", "show", "companions"]).starts_with("- acme\n"));
    assert!(text_of(&["config"]).contains("default_agent: dev\n"));
    assert_eq!(
        go(&["config", "show", "nosuch"], JSON, BARE).refused(),
        "library-application-unknown-section:1"
    );
}

// bundle.feature: list, show and view; the bare noun lists.
#[test]
fn test_bundle_list_show_and_view() {
    let list = json_of(&["bundle", "list"]);
    assert_eq!(list[0]["name"], "demo");
    assert_eq!(list[0]["fragment_count"], 4);
    assert_eq!(json_of(&["bundle"]), list);
    assert_eq!(json_of(&["bundle", "ls"]), list);
    assert!(text_of(&["bundle"]).starts_with("Installed bundles (2):\n"));
    let show = json_of(&["bundle", "show", "demo"]);
    assert_eq!(show["name"], "demo");
    assert_eq!(
        show["fragments"]["golang"]["premise"],
        "when editing Go code"
    );
    let text = text_of(&["bundle", "show", "demo"]);
    assert!(text.starts_with("Bundle: demo\nVersion: 1.0.0\n"));
    let view = json_of(&["bundle", "view", "demo"]);
    assert!(view["content"]
        .as_str()
        .unwrap()
        .contains("==> fragments/golang.md <=="));
    assert!(text_of(&["bundle", "view", "demo"]).starts_with("==> bundle.yaml <==\n"));
    let long = json_of(&["bundle", "view", "demo#fragments/dist"]);
    assert_eq!(long["content"], "long form\n");
    let short = json_of(&["bundle", "view", "demo#fragments/dist", "-d"]);
    assert_eq!(short["distilled"], true);
    assert_eq!(short["content"], "# (distilled version)\nshort form\n");
    assert_eq!(
        go(&["bundle", "show", "nosuch"], JSON, BARE).refused(),
        "library-catalog-bundle-not-found:1"
    );
}

// fragment.feature, command.feature: list (filtered by --bundle), show
// (text only), premises; a bare noun lists.
#[test]
fn test_fragment_and_command_list_show_and_premises() {
    let fragments = json_of(&["fragment", "list"]);
    assert_eq!(fragments.as_array().unwrap().len(), 4);
    assert_eq!(json_of(&["fragment"]), fragments);
    assert_eq!(
        json_of(&["fragment", "list", "--bundle", "project"]),
        serde_json::json!([])
    );
    assert_eq!(
        go(&["fragment", "list", "--bundle", "nosuch"], JSON, BARE).refused(),
        "library-application-no-bundle-named:1"
    );
    assert!(text_of(&["fragment", "list"]).starts_with("Fragments (4):\n"));
    let commands = json_of(&["command", "list"]);
    assert_eq!(commands[0]["ref"], "demo#commands/review");
    assert_eq!(json_of(&["command"]), commands);
    assert_eq!(
        text_of(&["fragment", "show", "demo#fragments/golang"]),
        "golang\n\nGO-RULES\n"
    );
    assert_eq!(
        text_of(&["command", "show", "demo#commands/review"]),
        "review\n\nREVIEW-BODY\n"
    );
    assert_eq!(
        text_of(&["fragment", "show", "demo#fragments/dist", "-d"]),
        "# (distilled version)\ndist\n\nshort form\n"
    );
    assert!(
        !go(&["fragment", "show", "demo#commands/review"], TEXT, BARE)
            .refused()
            .is_empty()
    );
    let premises = json_of(&["fragment", "premises"]);
    assert_eq!(premises["fragments"][0]["name"], "demo#fragments/golang");
    assert!(text_of(&["fragment", "premises"])
        .ends_with("- demo#fragments/golang: when editing Go code\n"));
}

// fragment_show.go: a structured --format is refused for `show` when it was
// asked for, not when it was the default.
#[test]
fn test_an_item_show_refuses_an_asked_for_json_but_not_a_default_one() {
    let asked = go(&["fragment", "show", "demo#fragments/golang"], JSON, BARE);
    assert_eq!(asked.refused(), "library-application-unsupported-format:1");
    let by_default = How {
        explicit: None,
        terminal: false,
        degraded: false,
    };
    let ran = go(
        &["fragment", "show", "demo#fragments/golang"],
        by_default,
        BARE,
    );
    assert_eq!(ran.code(), 0);
    assert_eq!(ran.out, "golang\n\nGO-RULES\n");
    assert_eq!(
        go(&["command", "show", "demo#commands/review"], JSON, BARE).refused(),
        "library-application-unsupported-format:1"
    );
}

// skill.feature
#[test]
fn test_skill_list_and_show() {
    let list = json_of(&["skill", "list"]);
    assert_eq!(list[0]["name"], "demo/reviewer");
    assert_eq!(json_of(&["skill"]), list);
    assert_eq!(
        json_of(&["skill", "list", "--bundle", "project"]),
        serde_json::json!([])
    );
    let show = json_of(&["skill", "show", "demo#skills/reviewer"]);
    assert_eq!(show["body"], "SKILL-BODY\n");
    assert!(text_of(&["skill", "show", "demo#skills/reviewer"])
        .starts_with("demo#skills/reviewer\n\ndescription: Reviews code\n"));
    assert!(text_of(&["skill"]).starts_with("Skills (1):\n"));
}

// profile.feature, agent.feature, llm.feature, companion.feature, mcp.feature
#[test]
fn test_profile_agent_llm_companion_and_mcp_commands() {
    let profiles = json_of(&["profile", "list"]);
    assert_eq!(profiles[1]["name"], "dev");
    assert_eq!(json_of(&["profile"]), profiles);
    assert_eq!(
        json_of(&["profile", "show", "dev"])["description"],
        "dev profile"
    );
    assert!(text_of(&["profile", "show", "dev"]).starts_with("Profile: dev\n"));
    assert!(text_of(&["profile"]).starts_with("Profiles (2):\n"));
    let agents = json_of(&["agent", "list"]);
    assert_eq!(agents[0]["name"], "dev");
    assert_eq!(json_of(&["agent"]), agents);
    let show = json_of(&["agent", "show", "dev"]);
    assert_eq!(show["resolved"]["model"], "opus");
    assert!(text_of(&["agent", "show", "dev"])
        .starts_with("Agent: dev\nEngine (declared): claude-code\n"));
    assert!(text_of(&["agent"]).starts_with("Agents (1):\n"));
    let llms = json_of(&["llm", "list"]);
    assert_eq!(llms[0]["label"], "claude-code");
    assert_eq!(json_of(&["llm"]), llms);
    assert!(text_of(&["llm"]).starts_with("Available LLMs:\n"));
    let companions = json_of(&["companion", "list"]);
    assert_eq!(companions[0]["bin"], "ctxloom-companion-acme");
    assert_eq!(json_of(&["companion"]), companions);
    assert!(text_of(&["companion"]).contains("NOT ON PATH"));
    let servers = json_of(&["mcp", "server", "list"]);
    assert_eq!(servers["servers"][0]["name"], "tree-sitter");
    assert_eq!(json_of(&["mcp"]), servers);
    assert_eq!(json_of(&["mcp", "server"]), servers);
    assert_eq!(
        json_of(&["mcp", "server", "show", "tree-sitter"])["found"],
        true
    );
    assert!(
        text_of(&["mcp", "server", "show", "tree-sitter"]).starts_with("MCP Server: tree-sitter\n")
    );
    assert!(text_of(&["mcp"]).starts_with("MCP Servers:\n"));
}

// search.feature: query, --tag, --type, --local; --remote is refused here.
#[test]
fn test_search_filters_by_tag_and_type_and_refuses_remote() {
    let hits = json_of(&["search", "golang"]);
    assert_eq!(hits["count"], 1);
    assert_eq!(hits["local"][0]["name"], "golang");
    assert_eq!(json_of(&["search", "-t", "go", "--local"])["count"], 1);
    assert_eq!(
        json_of(&["search", "--tag", "nosuch", "golang"])["count"],
        0
    );
    assert_eq!(
        json_of(&["search", "--type", "skill", "rev"])["local"][0]["type"],
        "skill"
    );
    assert_eq!(json_of(&["search", "--type", "command", "rev"])["count"], 1);
    assert_eq!(
        json_of(&["search", "--type", "fragment", "rev"])["count"],
        0
    );
    assert_eq!(
        go(&["search", "--remote", "x"], JSON, BARE).refused(),
        "library-application-search-remote:2"
    );
    assert!(text_of(&["search", "golang"]).starts_with("Results (1):\n"));
}

// context.feature
#[test]
fn test_context_assemble_takes_profiles_fragments_and_tags() {
    let by_profile = json_of(&["context", "assemble", "-p", "dev"]);
    assert_eq!(by_profile["profiles"], serde_json::json!(["dev"]));
    assert_eq!(
        json_of(&["context", "assemble", "--profile", "dev"]),
        by_profile
    );
    let by_fragment = json_of(&["context", "assemble", "-f", "demo#fragments/golang"]);
    assert_eq!(by_fragment["context"], "GO-RULES");
    let several = json_of(&[
        "context",
        "assemble",
        "-f",
        "demo#fragments/golang,demo#fragments/testing",
    ]);
    assert_eq!(several["fragments_loaded"].as_array().unwrap().len(), 2);
    let by_tag = go(
        &[
            "context",
            "assemble",
            "-f",
            "demo#fragments/golang",
            "-t",
            "nosuch",
        ],
        JSON,
        BARE,
    );
    assert_eq!(by_tag.refused(), "library-application-no-fragments:1");
    assert_eq!(json_of(&["context"]), by_profile);
    assert!(text_of(&["context", "assemble", "-p", "dev"]).ends_with("short form\n"));
}

// run.feature: `run --dry-run` previews; without it this build refuses.
#[test]
fn test_a_dry_run_carries_its_arguments_into_the_plan() {
    let plan = json_of(&["run", "--dry-run", "hello", "big", "world"]);
    assert_eq!(plan["prompt"], "hello big world");
    assert_eq!(plan["profiles"], serde_json::json!(["dev"]));
    assert_eq!(
        json_of(&["run", "--dry-run", "--prompt", "pp", "hello"])["prompt"],
        "pp"
    );
    assert_eq!(
        json_of(&["run", "--dry-run", "-l", "claude-fast", "-p", "dev"])["llm"],
        "claude-fast"
    );
    assert_eq!(
        json_of(&["run", "--dry-run", "-r", "review"])["prompt"],
        "REVIEW-BODY\n"
    );
    let agent = json_of(&[
        "run",
        "--dry-run",
        "--workspace",
        "worktree",
        "--agent",
        "dev",
    ]);
    assert_eq!(
        (agent["agent"].as_str(), agent["workspace"].as_str()),
        (Some("dev"), Some("worktree"))
    );
    let only = json_of(&[
        "run",
        "--dry-run",
        "-f",
        "demo#fragments/golang",
        "-t",
        "go",
    ]);
    assert_eq!(
        only["fragments"],
        serde_json::json!(["ctxloom+local:demo#fragments/golang"])
    );
    assert!(text_of(&["run", "--dry-run", "hello"])
        .starts_with("=== LLM ===\nclaude-code (claude-code)\n"));
    assert!(text_of(&["run", "--dry-run", "--agent", "dev"]).starts_with("=== Agent ===\ndev ("));
    assert_eq!(
        go(&["run"], JSON, BARE).refused(),
        "library-application-run-needs-dry-run:2"
    );
}

// fault_tolerance.feature: fatal findings abort a preview (exit 3) after the
// plan is printed; --degraded previews past them.
#[test]
fn test_fatal_findings_print_the_plan_and_abort() {
    let ran = go(&["run", "--dry-run", "-p", "broken"], JSON, BARE);
    assert_eq!(ran.refused(), "library-application-startup-aborted:3");
    assert_eq!(ran.json()["findings"][0]["fatal"], true);
    assert!(ran
        .err
        .starts_with("{\"prog\":\"ctxloom\",\"warning\":\"failed to load bundle"));
    let degraded = How {
        degraded: true,
        ..JSON
    };
    let past = go(&["run", "--dry-run", "-p", "broken"], degraded, BARE);
    assert_eq!(past.code(), 0);
    assert_eq!(past.json()["findings"][0]["fatal"], false);
}

// clidiag.Warn: a notice is one json line under json, words and a fix line
// under text.
#[test]
fn test_notices_are_said_as_a_json_line_or_as_words_with_their_fix() {
    let ghost = ["schema_version: 8\nagents:\n  ghost: {}\n  real:\n    llm: claude-code\n"];
    let json = go(&["agent", "list"], JSON, &ghost);
    assert_eq!(json.code(), 0);
    let line: serde_json::Value = serde_json::from_str(json.err.trim_end()).unwrap();
    assert_eq!(line["prog"], "ctxloom");
    assert!(line["warning"]
        .as_str()
        .unwrap()
        .starts_with("agents.ghost in /p/.ctxloom/layer-0.yaml declares neither"));
    assert!(line.get("remedy").is_none());
    let text = go(&["agent", "list"], TEXT, &ghost);
    assert!(text.err.starts_with("ctxloom: warning: agents.ghost in "));
    assert!(!text.err.contains("fix:"));
    let broken = go(&["run", "--dry-run", "-p", "broken"], TEXT, BARE);
    assert!(broken
        .err
        .starts_with("ctxloom: warning: failed to load bundle \"does-not-exist\""));
    assert!(broken
        .err
        .contains("\n  fix: run `ctxloom deps pull` or fix the bundle ref, or pass --degraded\n"));
    let by_json = go(
        &["run", "--dry-run", "-p", "broken"],
        How {
            degraded: true,
            ..JSON
        },
        BARE,
    );
    assert!(by_json.err.contains("\"remedy\":\"run `ctxloom deps pull`"));
}

// inert_render.go: a body altered for the terminal is said so on the error
// stream, in text; the json form is the raw bytes.
#[test]
fn test_a_body_rendered_inert_is_noted_and_json_is_raw() {
    let text = go(&["bundle", "view", "demo#fragments/ctl"], TEXT, BARE);
    assert_eq!(text.out, "a\u{27e8}U+0007\u{27e9}b\n");
    assert_eq!(
        text.err,
        "ctxloom: warning: demo#fragments/ctl is publisher-authored and was rendered inert for this terminal (1 control character(s) escaped); use --format json for the raw bytes\n"
    );
    let json = go(&["bundle", "view", "demo#fragments/ctl"], JSON, BARE);
    assert_eq!(json.json()["content"], "a\u{7}b\n");
    assert!(json.err.is_empty());
}

// The format: asked for, a terminal's text, the configuration's
// `non_tty_format`, else the program's default.
#[test]
fn test_the_format_is_asked_for_a_terminals_or_the_configurations() {
    let line = ["companion", "list"];
    let asked_text = go(&line, TEXT, BARE);
    assert!(asked_text.out.starts_with("acme"));
    let default = How {
        explicit: None,
        terminal: false,
        degraded: false,
    };
    assert!(go(&line, default, BARE).out.starts_with('['));
    let terminal = How {
        terminal: true,
        ..default
    };
    assert!(go(&line, terminal, BARE).out.starts_with("acme"));
    let text_config = ["schema_version: 8\ncompanions: [acme]\nnon_tty_format: text\n"];
    assert!(go(&line, default, &text_config).out.starts_with("acme"));
    let json_config = ["schema_version: 8\ncompanions: [acme]\nnon_tty_format: json\n"];
    assert!(go(&line, default, &json_config).out.starts_with('['));
    assert!(go(&line, JSON, &text_config).out.starts_with('['));
}

// A noun the matches do not hold is not run.
#[test]
fn test_running_a_noun_the_line_does_not_name_is_refused() {
    let program = Command::new("ctxloom").subcommands(nouns());
    let matches = program.get_matches_from(["ctxloom", "llm", "list"]);
    let call = Call {
        matches: &matches,
        explicit_format: None,
        default_format: Format::Json,
        stdout_is_terminal: false,
        degraded: false,
        no_companions: false,
        config_set: Vec::new(),
    };
    let library = Setup::default().library();
    let catalog = Catalog::en_us(&[library_application::EN_US]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let wrong = run(&library, "bundle", &call, &catalog, &mut out, &mut err).unwrap_err();
    assert_eq!(wrong.cause.code(), "library-application-search-nothing");
    assert!(out.is_empty());
}

/// A stream that cannot be written.
struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("pipe closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// An answer that cannot be written is a failure with its own code and exit 1.
#[test]
fn test_an_answer_that_cannot_be_written_fails_with_its_cause() {
    let program = Command::new("ctxloom").subcommands(nouns());
    let matches = program.get_matches_from(["ctxloom", "llm", "list"]);
    let call = Call {
        matches: &matches,
        explicit_format: Some(Format::Json),
        default_format: Format::Json,
        stdout_is_terminal: false,
        degraded: false,
        no_companions: false,
        config_set: Vec::new(),
    };
    let library = Setup::default().library();
    let catalog = Catalog::en_us(&[library_application::EN_US]).unwrap();
    let failure = run(
        &library,
        "llm",
        &call,
        &catalog,
        &mut Broken,
        &mut Vec::new(),
    )
    .unwrap_err();
    assert_eq!(failure.exit, 1);
    assert_eq!(failure.cause.code(), "ctxloom-cli-output");
    let args = failure.cause.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].0, "ext_err_txt");
    assert_eq!(
        format!("{:?}", args[0].1),
        format!("{:?}", angzarr_diagnostic::Arg::from("pipe closed"))
    );
    assert!(failure.cause.to_string().contains("ctxloom-cli-output"));
}

// clidiag.Warn: the remedy of a notice is the notice's own args under the
// code `<code>-fix`; it shows as that code.
#[test]
fn test_a_remedy_shows_as_its_code_and_carries_the_notices_args() {
    let notice = Notice::text("a thing");
    let fix = notices::FixOf(&notice, "library-application-notice-text-fix".to_string());
    assert_eq!(
        format!("{fix:?}"),
        "FixOf(library-application-notice-text-fix)"
    );
    assert!(fix
        .to_string()
        .contains("library-application-notice-text-fix"));
    assert_eq!(fix.code(), "library-application-notice-text-fix");
    assert_eq!(fix.args().len(), notice.args().len());
}
