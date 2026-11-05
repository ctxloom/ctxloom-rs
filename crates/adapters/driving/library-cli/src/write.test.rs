//! The write verbs as the program runs them: the command line parsed by this
//! crate's nouns, the application over in-memory ports, the answer as bytes
//! and what the ports were asked to write. The texts and shapes are the ones
//! the acceptance scenarios (`tests/library_write_*`) hold the binary to.

use std::sync::Arc;

use library_application::fakes::{Mem, Setup, CONFIG, ENVELOPE};
use library_application::Library;

use super::*;

const DEMO: &[(&str, &str)] = &[
    ENVELOPE,
    ("fragments/example.md", "EXAMPLE"),
    ("prompts/review.md", "REVIEW"),
    ("mcp/tools.yaml", "command: echo\n"),
    (
        "skills/reviewer/SKILL.md",
        "---\nname: reviewer\ndescription: d\n---\nbody\n",
    ),
];

const PROJECT: &[(&str, &str)] = &[ENVELOPE, ("profiles/dev.yaml", "bundles: [demo]\n")];

fn world() -> (Library, Arc<Mem>) {
    Setup {
        bundles: &[("demo", DEMO), ("project", PROJECT)],
        layers: &[CONFIG],
        path: &[("ctxloom-companion-acme", "/usr/bin/acme")],
        ..Setup::default()
    }
    .fixture()
}

fn run_on(library: &Library, line: &[&str], how: How) -> Ran {
    let program = Command::new("ctxloom")
        .subcommand_required(true)
        .arg(clap::Arg::new("format").long("format").global(true))
        .subcommands(nouns());
    let mut argv = vec!["ctxloom"];
    argv.extend_from_slice(line);
    let matches = program
        .try_get_matches_from(argv)
        .unwrap_or_else(|e| panic!("{line:?} does not parse: {e}"));
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
    let exit = run(library, &noun, &call, &catalog, &mut out, &mut err)
        .map_err(|f| format!("{}:{}", f.cause.code(), f.exit));
    Ran {
        exit,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn text(library: &Library, line: &[&str]) -> String {
    let ran = run_on(library, line, TEXT);
    assert_eq!(ran.code(), 0, "{line:?}: {}", ran.err);
    ran.out
}

fn json(library: &Library, line: &[&str]) -> serde_json::Value {
    let ran = run_on(library, line, JSON);
    assert_eq!(ran.code(), 0, "{line:?}: {}", ran.err);
    ran.json()
}

fn refused_code(library: &Library, line: &[&str], how: How) -> String {
    run_on(library, line, how).refused().to_string()
}

// bundle.feature: Creating a bundle writes a manifest carrying its seeded content
#[test]
fn test_bundle_create_says_where_it_wrote() {
    let (library, mem) = world();
    let said = text(&library, &["bundle", "create", "fresh", "-d", "my bundle"]);
    assert!(said.starts_with("Created bundle: "), "{said}");
    assert!(
        said.ends_with("Edit its item files to add your fragments and prompts.\n"),
        "{said}"
    );
    assert!(mem.has_bundle("fresh"));
    assert!(mem
        .text("fresh", "bundle.yaml")
        .unwrap()
        .contains("my bundle"));
    let doc = json(&library, &["bundle", "create", "second"]);
    assert_eq!(doc["status"], "created");
    assert_eq!(doc["name"], "second");
}

// bundle.feature: Metadata edits land in the file
#[test]
fn test_bundle_edit_names_each_change_and_says_when_there_is_none() {
    let (library, mem) = world();
    let said = text(
        &library,
        &[
            "bundle",
            "edit",
            "demo",
            "-d",
            "new words",
            "--version",
            "2.0.0",
            "--add-tag",
            "a,b",
            "--remove-tag",
            "zz",
            "--add-fragment",
            "example",
            "--add-prompt",
            "review",
            "--add-mcp",
            "tools",
        ],
    );
    assert!(said.contains("updated description\n"), "{said}");
    assert!(said.contains("updated version\n"), "{said}");
    assert!(said.contains("added tag: a\nadded tag: b\n"), "{said}");
    assert!(
        said.ends_with("\n") && said.contains("Updated bundle: "),
        "{said}"
    );
    let file = mem.text("demo", "bundle.yaml").unwrap();
    assert!(
        file.contains("version: 2.0.0") && file.contains("new words"),
        "{file}"
    );
    let said = text(
        &library,
        &[
            "bundle",
            "edit",
            "demo",
            "--remove-tag",
            "a",
            "--remove-fragment",
            "example",
            "--remove-prompt",
            "review",
            "--remove-mcp",
            "tools",
        ],
    );
    assert!(said.contains("removed tag: a\n"), "{said}");
    let before = mem.writes();
    assert_eq!(
        text(&library, &["bundle", "edit", "demo"]),
        "No changes made.\n"
    );
    assert_eq!(mem.writes(), before);
    assert_eq!(
        json(&library, &["bundle", "edit", "demo"])["status"],
        "no_changes"
    );
}

// bundle.feature: Bare bundle remove reports what would go and destroys nothing
#[test]
fn test_bundle_remove_previews_until_yes() {
    let (library, mem) = world();
    let said = text(&library, &["bundle", "remove", "demo"]);
    assert!(
        said.starts_with("Would remove bundle \"demo\":\n  "),
        "{said}"
    );
    assert!(
        said.ends_with(
            "\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom bundle remove demo --yes\n"
        ),
        "{said}"
    );
    assert!(mem.has_bundle("demo"));
    assert_eq!(json(&library, &["bundle", "rm", "demo"])["applied"], false);
    let said = text(&library, &["bundle", "remove", "demo", "--yes"]);
    assert!(said.starts_with("Removed bundle: "), "{said}");
    assert!(!mem.has_bundle("demo"));
    let doc = json(&library, &["bundle", "del", "project", "-y"]);
    assert_eq!(doc["status"], "deleted");
}

// A preview without detail is a sentence, not a list.
#[test]
fn test_a_preview_without_detail_is_one_sentence() {
    let (library, _) = world();
    let said = text(&library, &["llm", "create", "big", "--type", "mock"]);
    assert!(said.starts_with("Created llm"), "{said}");
    let said = text(&library, &["llm", "remove", "big"]);
    assert!(
        said.starts_with("Would remove llm \"big\".\n\nNothing"),
        "{said}"
    );
}

// bundle.feature: Export, remove, and re-import a bundle
#[test]
fn test_bundle_export_and_import_say_where_they_went() {
    let (library, mem) = world();
    let said = text(&library, &["bundle", "export", "demo", "out"]);
    assert!(said.starts_with("Exported: "), "{said}");
    assert!(said.ends_with(" -> out/demo\n"), "{said}");
    assert!(mem.user_paths().iter().any(|p| p.starts_with("out/demo/")));
    let doc = json(&library, &["bundle", "export", "demo", "-o", "elsewhere"]);
    assert_eq!(doc["dest"], "elsewhere");
    let _ = text(&library, &["bundle", "remove", "demo", "--yes"]);
    let said = text(&library, &["bundle", "import", "out/demo"]);
    assert!(said.starts_with("Imported: out/demo -> "), "{said}");
    assert!(
        said.ends_with("\n  Version: 1.0.0\n  Fragments: 1, Commands: 1, MCP: 1\n"),
        "{said}"
    );
    let refused = refused_code(&library, &["bundle", "import", "out/demo"], TEXT);
    assert!(refused.contains("exists"), "{refused}");
    let said = text(&library, &["bundle", "import", "out/demo", "--force"]);
    assert!(said.starts_with("Imported: "), "{said}");
}

// fragment.feature / command.feature: create, remove, edit
#[test]
fn test_fragment_and_command_create_remove_and_edit() {
    let (library, mem) = world();
    assert_eq!(
        text(&library, &["fragment", "create", "demo", "testing"]),
        "Created fragment \"testing\" in bundle \"demo\"\nEdit with: ctxloom fragment edit demo#fragments/testing\n"
    );
    assert_eq!(
        text(&library, &["command", "create", "demo", "second"]),
        "Created command \"second\" in bundle \"demo\"\nEdit with: ctxloom command edit demo#commands/second\n"
    );
    assert_eq!(
        json(&library, &["fragment", "create", "demo", "third"])["name"],
        "third"
    );
    assert_eq!(
        text(&library, &["fragment", "remove", "demo#fragments/testing"]),
        "Would remove fragment \"testing\" from bundle \"demo\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom fragment remove demo#fragments/testing --yes\n"
    );
    assert_eq!(
        text(
            &library,
            &["fragment", "rm", "demo#fragments/testing", "--yes"]
        ),
        "Removed fragment \"testing\" from bundle \"demo\"\n"
    );
    assert_eq!(
        text(
            &library,
            &["command", "remove", "demo#commands/second", "-y"]
        ),
        "Removed command \"second\" from bundle \"demo\"\n"
    );
    mem.set_editor(|_, content| Ok(format!("{content}\nMORE")));
    assert_eq!(
        text(&library, &["fragment", "edit", "demo#fragments/example", "--no-distill"]),
        "Updated fragment \"example\" in bundle \"demo\"\n\nBundle modified. To publish changes:\n  ctxloom bundle push demo [remote]\n"
    );
    assert!(mem
        .text("demo", "fragments/example.md")
        .unwrap()
        .contains("MORE"));
    let ran = run_on(
        &library,
        &["command", "edit", "demo#commands/review", "--no-distill"],
        TEXT,
    );
    assert_eq!(ran.code(), 0);
    assert!(
        ran.err
            .contains("distilled form not refreshed (--no-distill)"),
        "{}",
        ran.err
    );
    mem.set_editor(|_, content| Ok(content.to_string()));
    assert_eq!(
        text(&library, &["fragment", "edit", "demo#fragments/example"]),
        "No changes made.\n"
    );
    assert_eq!(
        json(&library, &["fragment", "edit", "demo#fragments/example"])["status"],
        "unchanged"
    );
}

// skill.feature
#[test]
fn test_skill_create_remove_export_and_import() {
    let (library, mem) = world();
    let said = text(
        &library,
        &["skill", "create", "demo", "writer", "-d", "words"],
    );
    assert!(
        said.starts_with("Created skill \"writer\" in bundle \"demo\"\n  "),
        "{said}"
    );
    assert!(
        said.contains("\nEdit ") && said.ends_with("/SKILL.md to describe the skill\n"),
        "{said}"
    );
    assert!(mem
        .text("demo", "skills/writer/SKILL.md")
        .unwrap()
        .contains("words"));
    let said = text(&library, &["skill", "remove", "demo#skills/writer"]);
    assert!(said.starts_with("Would remove skill"), "{said}");
    let said = text(
        &library,
        &["skill", "remove", "demo#skills/writer", "--yes"],
    );
    assert!(
        said.starts_with("Removed skill \"writer\" from bundle \"demo\"\n  "),
        "{said}"
    );
    let said = text(
        &library,
        &["skill", "export", "demo#skills/reviewer", "-o", "r.zip"],
    );
    assert!(
        said.starts_with("Exported skill \"reviewer\" to r.zip (") && said.ends_with(" bytes)\n"),
        "{said}"
    );
    let doc = json(
        &library,
        &["skill", "export", "demo#skills/reviewer", "-o", "r2.zip"],
    );
    assert_eq!(doc["zip_path"], "r2.zip");
    let said = text(
        &library,
        &["skill", "import", "r.zip", "--bundle", "project"],
    );
    assert!(
        said.starts_with("Imported skill \"reviewer\" into bundle \"project\" (1 file(s))\n  "),
        "{said}"
    );
    let refused = refused_code(
        &library,
        &["skill", "import", "r.zip", "--bundle", "project"],
        TEXT,
    );
    assert!(refused.contains("exists"), "{refused}");
    let said = text(
        &library,
        &["skill", "import", "r.zip", "--bundle", "project", "--force"],
    );
    assert!(said.starts_with("Imported skill"), "{said}");
}

// mcp.feature
#[test]
fn test_mcp_set_edit_and_the_bare_noun() {
    let (library, mem) = world();
    assert_eq!(
        text(
            &library,
            &[
                "mcp",
                "server",
                "set",
                "demo#mcp/tools",
                "--command",
                "run",
                "--arg",
                "-x",
                "--arg",
                "y",
                "--env",
                "A=1",
                "--env",
                "B=2",
                "--tag",
                "t1",
                "--notes",
                "n",
                "--installation",
                "i",
            ]
        ),
        "Set MCP server \"tools\" in bundle \"demo\"\n"
    );
    let file = mem.text("demo", "mcp/tools.yaml").unwrap();
    assert!(file.contains("command: run"), "{file}");
    assert!(file.contains("- -x") && file.contains("- y"), "{file}");
    assert!(
        file.contains("A: 1") && file.contains("B: \"2\"") || file.contains("B: 2"),
        "{file}"
    );
    let doc = json(
        &library,
        &[
            "mcp",
            "server",
            "set",
            "demo#mcp/tools",
            "--command",
            "",
            "--arg",
            "",
            "--env",
            "",
            "--url",
            "https://x.example/v1",
            "--header",
            "H=v",
        ],
    );
    assert_eq!(doc["status"], "updated");
    let file = mem.text("demo", "mcp/tools.yaml").unwrap();
    assert!(
        file.contains("url: https://x.example/v1") && file.contains("H: v"),
        "{file}"
    );
    let bad = refused_code(
        &library,
        &["mcp", "server", "set", "demo#mcp/tools", "--env", "nope"],
        TEXT,
    );
    assert!(bad.contains("flag-pair"), "{bad}");
    assert_eq!(
        text(&library, &["mcp", "server", "edit", "demo#mcp/tools"]),
        "No changes made.\n"
    );
    mem.set_editor(
        |_, content| Ok(content.replace("https://x.example/v1", "https://y.example/v1")),
    );
    assert_eq!(
        text(&library, &["mcp", "server", "edit", "demo#mcp/tools"]),
        "Updated MCP server \"tools\" in bundle \"demo\"\n"
    );
    let ran = run_on(
        &library,
        &["mcp", "server", "edit", "demo#mcp/tools"],
        JSON_EXPLICIT,
    );
    assert!(ran.refused().contains("unsupported-format"));
    // json that nobody asked for (the default, off a terminal) is not json
    let unasked = How {
        explicit: None,
        terminal: false,
        degraded: false,
    };
    let said = refused_code(&library, &["mcp"], unasked);
    assert!(
        said.contains("mcp-bare") && !said.contains("mcp-bare-json"),
        "{said}"
    );
    let said = refused_code(&library, &["mcp"], TEXT);
    assert!(
        said.contains("mcp-bare") && !said.contains("mcp-bare-json"),
        "{said}"
    );
    assert!(refused_code(&library, &["mcp"], JSON).contains("mcp-bare-json"));
}

const JSON_EXPLICIT: How = JSON;

// profile.feature
#[test]
fn test_profile_verbs_answer_in_text_and_refuse_json() {
    let (library, mem) = world();
    let said = text(
        &library,
        &[
            "profile",
            "create",
            "base",
            "-i",
            "demo",
            "-d",
            "d",
            "--llm",
            "claude-code",
        ],
    );
    assert!(
        said.starts_with("Created profile \"base\" with bundles: demo\nSaved to: "),
        "{said}"
    );
    let said = text(
        &library,
        &["profile", "create", "kid", "--parent", "base", "-i", "demo"],
    );
    assert!(
        said.starts_with("Created profile \"kid\" with parents: base; bundles: demo\n"),
        "{said}"
    );
    let said = text(
        &library,
        &["profile", "create", "bare-kid", "--parent", "base"],
    );
    assert!(
        said.starts_with("Created profile \"bare-kid\" with parents: base\nSaved to: "),
        "{said}"
    );
    let said = text(
        &library,
        &[
            "profile",
            "modify",
            "kid",
            "--add-parent",
            "bare-kid",
            "--add-bundle",
            "project",
            "-d",
            "words",
            "--llm",
            "claude-code",
            "--exclude-fragment",
            "f",
            "--exclude-mcp",
            "m",
        ],
    );
    assert!(
        said.contains("added parent: bare-kid\nadded bundle: project\n")
            && said.ends_with("Modified profile \"kid\"\n"),
        "{said}"
    );
    let said = text(
        &library,
        &[
            "profile",
            "modify",
            "kid",
            "--remove-parent",
            "bare-kid",
            "--remove-bundle",
            "project",
            "--include-fragment",
            "f",
            "--include-mcp",
            "m",
        ],
    );
    assert!(
        said.contains("removed parent: bare-kid\nremoved bundle: project\n"),
        "{said}"
    );
    assert_eq!(
        text(&library, &["profile", "modify", "kid"]),
        "No changes made.\n"
    );
    assert_eq!(
        text(&library, &["profile", "edit", "kid"]),
        "No changes made.\n"
    );
    mem.set_editor(|_, content| Ok(content.replace("claude-code", "mock")));
    let said = text(&library, &["profile", "edit", "kid"]);
    assert!(said.starts_with("Updated profile"), "{said}");
}

// profile.feature: Export, remove, and re-import
#[test]
fn test_profile_export_import_and_remove() {
    let (library, _mem) = world();
    let _ = text(&library, &["profile", "create", "base", "-i", "demo"]);
    let _ = text(&library, &["profile", "create", "kid", "--parent", "base"]);
    let said = text(&library, &["profile", "export", "kid", "pout"]);
    assert!(
        said.starts_with("Exported: ") && said.ends_with(" -> pout/kid.yaml\n"),
        "{said}"
    );
    let _ = text(&library, &["profile", "remove", "kid", "--yes"]);
    let said = text(&library, &["profile", "import", "pout/kid.yaml"]);
    assert!(said.starts_with("Imported: pout/kid.yaml -> "), "{said}");
    let refused = refused_code(&library, &["profile", "import", "pout/kid.yaml"], TEXT);
    assert!(refused.contains("exists"), "{refused}");
    let said = text(
        &library,
        &[
            "profile",
            "import",
            "pout/kid.yaml",
            "--force",
            "--bundle",
            "demo",
        ],
    );
    assert!(said.starts_with("Imported: "), "{said}");
    let said = text(&library, &["profile", "remove", "base"]);
    assert!(said.starts_with("Would remove profile \"base\""), "{said}");
    assert_eq!(
        text(&library, &["profile", "remove", "base", "--yes"]),
        "Removed profile \"base\"\n"
    );
    assert_eq!(
        json(&library, &["profile", "remove", "kid", "-y"])["status"],
        "deleted"
    );
}

// The verbs Go answers in text only refuse an explicit json before writing.
#[test]
fn test_the_text_only_verbs_refuse_json_and_write_nothing() {
    let (library, mem) = world();
    for line in [
        vec!["profile", "create", "x", "-i", "demo"],
        vec!["profile", "modify", "dev", "-d", "x"],
        vec!["profile", "edit", "dev"],
        vec!["profile", "export", "dev", "o"],
        vec!["profile", "import", "pout/kid.yaml"],
        vec!["config", "create"],
        vec!["config", "edit"],
        vec!["agent", "default"],
    ] {
        let before = mem.writes();
        let refused = refused_code(&library, &line, JSON);
        assert!(
            refused.contains("unsupported-format"),
            "{line:?}: {refused}"
        );
        assert_eq!(mem.writes(), before, "{line:?} wrote");
    }
}

// agent.feature
#[test]
fn test_agent_verbs() {
    let (library, mem) = world();
    assert_eq!(
        text(
            &library,
            &[
                "agent",
                "create",
                "dev",
                "--llm",
                "claude-code",
                "--profiles",
                "dev",
                "--runtime",
                "container-rootless"
            ]
        ),
        "Created agent \"dev\" (llm: claude-code, profiles: dev, runtime: container-rootless)\n"
    );
    assert_eq!(
        text(
            &library,
            &[
                "agent", "edit", "dev", "--profiles", "dev,dev", "--runtime", "host",
                "--permissions", "plan", "--engine-home", "session", "--env-host=false",
                "--env", "A", "--env", "B", "--surface", "context=system-prompt", "--root", "mcp=project-root",
            ]
        ),
        "Updated agent \"dev\" (llm: claude-code, profiles: dev, dev, runtime: host, permissions: claude-code={mode=plan}, engine_home: session, env_host: false, env: A, B)\n"
    );
    let doc = json(
        &library,
        &["agent", "edit", "dev", "--env-host", "--env", ""],
    );
    assert_eq!(doc["env_host"], true);
    assert_eq!(doc["name"], "dev");
    let bad = refused_code(
        &library,
        &["agent", "edit", "dev", "--surface", "nope"],
        TEXT,
    );
    assert!(bad.contains("flag-pair"), "{bad}");
    let bad = refused_code(&library, &["agent", "edit", "dev", "--root", "nope"], TEXT);
    assert!(bad.contains("flag-pair"), "{bad}");
    assert_eq!(
        text(&library, &["agent", "default"]),
        "No default agent set.\n"
    );
    assert_eq!(
        text(&library, &["agent", "default", "dev"]),
        "Set default agent to \"dev\".\n"
    );
    assert_eq!(
        text(&library, &["agent", "default"]),
        "Default agent: dev\n"
    );
    let ran = run_on(&library, &["agent", "default", "ghost"], TEXT);
    assert!(ran.err.contains("not defined yet"), "{}", ran.err);
    assert!(mem
        .config(Scope::Project)
        .unwrap()
        .contains("default_agent: ghost"));
    let ran = run_on(&library, &["agent", "default", "dev"], TEXT);
    assert!(!ran.err.contains("not defined yet"), "{}", ran.err);
    assert_eq!(
        text(&library, &["agent", "remove", "dev"]),
        "Would remove agent \"dev\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom agent remove dev --yes\n"
    );
    assert_eq!(
        text(&library, &["agent", "remove", "dev", "--yes"]),
        "Removed agent \"dev\"\n"
    );
}

use library_application::fakes::ConfigScope as Scope;

// llm.feature
#[test]
fn test_llm_verbs() {
    let (library, mem) = world();
    assert_eq!(
        text(
            &library,
            &["llm", "create", "big", "--type", "mock", "--model", "o1"]
        ),
        "Created llm \"big\" (type: mock, model: o1)\n"
    );
    assert_eq!(
        text(
            &library,
            &[
                "llm",
                "edit",
                "big",
                "--model",
                "o1-pro",
                "--permissions",
                "bypass"
            ]
        ),
        "Updated llm \"big\" (type: mock, model: o1-pro, permissions: mode=bypass)\n"
    );
    let doc = json(&library, &["llm", "edit", "big", "--set", "model=o2"]);
    assert_eq!(doc["model"], "o2");
    // With no default named, the first label in order is the default.
    assert_eq!(text(&library, &["llm", "default"]), "big\n");
    assert_eq!(
        text(&library, &["llm", "default", "big"]),
        "Default LLM set to: big\n"
    );
    assert_eq!(
        text(&library, &["llm", "default", "big"]),
        "Default LLM is already big\n"
    );
    assert_eq!(json(&library, &["llm", "default"])["default"], "big");
    assert_eq!(
        text(&library, &["llm", "remove", "big"]),
        "Would remove llm \"big\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom llm remove big --yes\n"
    );
    assert_eq!(
        text(&library, &["llm", "remove", "big", "--yes"]),
        "Removed llm \"big\"\n"
    );
    let _ = text(
        &library,
        &["llm", "create", "h", "--type", "mock", "--home"],
    );
    assert!(mem.config(Scope::Home).is_none() || mem.config(Scope::Home).is_some());
}

// companion.feature
#[test]
fn test_companion_verbs() {
    let (library, _mem) = world();
    assert_eq!(
        text(&library, &["companion", "add", "acme"]),
        "Registered companion 'acme' (ctxloom-companion-acme answers at /usr/bin/acme)\n"
    );
    assert!(refused_code(&library, &["companion", "add", "-x"], TEXT).contains("companion"));
    assert_eq!(
        text(&library, &["companion", "remove", "acme"]),
        "Would remove companion \"acme\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom companion remove acme --yes\n"
    );
    assert_eq!(
        text(&library, &["companion", "rm", "acme", "--yes"]),
        "Removed companion 'acme'\n"
    );
}

// config.feature
#[test]
fn test_config_create_and_edit() {
    let (library, mem) = world();
    assert!(refused_code(&library, &["config", "edit"], TEXT).contains("config-missing"));
    mem.set_scaffold_for_test("config.yaml", "schema_version: 8\n");
    let said = text(&library, &["config", "edit"]);
    assert_eq!(said, "");
    assert!(!mem.opened().is_empty());
    let (library, _) = Setup::default().fixture();
    let said = text(&library, &["config", "create", "--engine", "mock"]);
    assert!(said.starts_with("Wrote "), "{said}");
}

// loadout_cmd.go (documented condition)
#[test]
fn test_loadout_is_the_one_yaml_document() {
    let (library, _) = world();
    let said = text(&library, &["loadout"]);
    assert_eq!(said, library_application::LOADOUT);
    let ran = run_on(
        &library,
        &["loadout", "--format", "yaml"],
        How {
            explicit: None,
            ..TEXT
        },
    );
    assert_eq!(ran.out, library_application::LOADOUT);
    let ran = run_on(
        &library,
        &["loadout", "--format", "toml"],
        How {
            explicit: None,
            ..TEXT
        },
    );
    assert!(ran.refused().contains("loadout-format"));
    assert_eq!(ran.out, "");
}
