//! Ported from Go's `internal/core/bundles` tests (`upgrade_test.go`,
//! `strict_test.go`, `profile_refs_step_test.go`) at 860336c4e: the inline
//! document form and the migrations of its envelope.

use super::*;

angzarr_presentation::english_for!(&[crate::EN_US.1]);

fn parse(text: &str) -> Bundle {
    parse_document(text).unwrap_or_else(|e| panic!("{}", e.english()))
}

// upgrade_test.go: TestParseBundle_MigratesPromptsKeyToCommands
#[test]
fn test_a_legacy_prompts_key_migrates_to_commands() {
    let b = parse("version: \"1.0\"\nprompts:\n  review:\n    description: d\n    content: c\n");
    assert_eq!(
        b.commands["review"].body.content, "c",
        "legacy prompts: must migrate into commands"
    );
    assert!(b.fragments.is_empty(), "no fragments declared");
}

// upgrade_test.go: TestParseBundle_CommandsKeyUnchanged
#[test]
fn test_a_current_commands_key_loads_unchanged() {
    let b = parse("version: \"1.0\"\ncommands:\n  review:\n    content: c\n");
    assert_eq!(b.commands["review"].body.content, "c");
}

// upgrade_test.go: TestParseBundle_CommandsWinsOverLegacyPrompts
#[test]
fn test_commands_win_over_a_legacy_prompts_key() {
    let b = parse(
        "version: \"1.0\"\ncommands:\n  new:\n    content: n\nprompts:\n  old:\n    content: o\n",
    );
    assert!(b.commands.contains_key("new"));
    assert!(
        !b.commands.contains_key("old"),
        "legacy prompts: must not override commands:"
    );
}

// strict_test.go: TestParseBundle_LegacyPromptsKeyIsMigratedBeforeStrictDecodeRefusesIt
// and the item `llm:` to `exports:` rename of generation 0 to 1.
#[test]
fn test_the_migration_runs_before_the_strict_decode_and_renames_llm_to_exports() {
    let b = parse(
        "version: \"1.0.0\"\nprompts:\n  review:\n    content: review this\n    description: a legacy prompt\n    llm:\n      claude-code:\n        enabled: false\n",
    );
    let review = &b.commands["review"];
    assert_eq!(review.body.content, "review this");
    assert_eq!(review.description, "a legacy prompt");
    assert!(
        review.exports.contains_key("claude-code"),
        "llm: becomes exports:"
    );
    assert!(
        b.fragments.is_empty(),
        "nothing else may be invented by the migration"
    );
}

// upgrade_test.go: TestParseBundle_ContentShapedSkillIsRefused
#[test]
fn test_a_content_shaped_skill_is_refused() {
    let err = parse_document(
        "version: \"1.0\"\nskills:\n  humanize:\n    path: skills/humanize\n  review:\n    content: c\n",
    )
    .expect_err("content under skills:");
    assert!(err.english().contains("content"), "{}", err.english());
}

// upgrade_test.go: TestParseBundle_NewShapeSkillsKeyParsesAsSkill and
// _NewShapeSkillsKeyDefaultsPath
#[test]
fn test_a_skill_package_reference_parses_and_its_path_defaults_empty() {
    let b = parse(
        "version: \"1.0\"\nskills:\n  humanize:\n    path: skills/humanize\n    tags: [writing]\n",
    );
    assert_eq!(b.skills["humanize"].path, "skills/humanize");
    assert_eq!(b.skills["humanize"].tags, ["writing"]);
    let minimal = parse("version: \"1.0\"\nskills:\n  humanize:\n    notes: a note\n");
    assert_eq!(minimal.skills["humanize"].path, "");
    assert_eq!(minimal.skills["humanize"].notes, "a note");
}

// upgrade_test.go: TestParseBundle_MalformedIsAParseFailureNotAVersionFault
#[test]
fn test_a_malformed_document_is_a_parse_failure_not_a_version_fault() {
    let err = parse_document("fragments: [unterminated\n").expect_err("not YAML");
    assert!(
        matches!(err, BundleError::Syntax { .. }),
        "{}",
        err.english()
    );
}

// strict_test.go: TestParseBundle_RejectsUnknownTopLevelKey
#[test]
fn test_an_unknown_top_level_key_is_refused_naming_it_with_a_suggestion_and_a_line() {
    let err = parse_document("version: \"1.0.0\"\nhoooks:\n  post_tool:\n    - command: echo hi\n")
        .expect_err("hoooks");
    let said = err.english();
    assert!(
        said.contains("hoooks"),
        "the error must NAME the offending key: {said}"
    );
    assert!(
        said.contains("did you mean `hooks`?"),
        "a near-miss must be suggested: {said}"
    );
    assert!(
        said.contains("line 2"),
        "the error must locate the key: {said}"
    );
}

// strict_test.go: TestParseBundle_RejectsUnknownKeyInsideAnItem
#[test]
fn test_an_unknown_key_inside_an_item_says_where_it_would_have_been_legal() {
    let err = parse_document(
        "version: \"1.0.0\"\nmcp:\n  thinking:\n    command: npx\n    note: dropped in silence\n",
    )
    .expect_err("note");
    let said = err.english();
    assert!(said.contains("note"));
    assert!(said.contains("did you mean `notes`?"), "{said}");
    assert!(said.contains("mcp.<name>"), "{said}");
    assert!(
        said.contains("valid keys there: args, command, env, headers, installation, notes, served_by, tags, url"),
        "{said}"
    );
}

// strict_test.go: TestParseBundle_StrictDecodeLeavesTheEmptyDocumentRuleIntact
#[test]
fn test_an_empty_document_is_not_a_bundle() {
    for doc in ["", "   \n", "# just a comment\n", "---\n", "null\n", "{}\n"] {
        let err = parse_document(doc).expect_err(doc);
        assert!(
            err.english().contains("bundle is empty"),
            "{doc:?}: {}",
            err.english()
        );
    }
}

// strict_test.go: TestParseBundle_AcceptsEveryKeyTheSchemaModels
#[test]
fn test_every_key_the_schema_models_is_accepted() {
    let b = parse(
        r#"version: "1.0.0"
tags: [a]
author: someone
description: d
notes: n
installation: i
fragments:
  f:
    content: c
    tags: [t]
    notes: n
    installation: i
    content_hash: h
    distilled: d
    distilled_by: model
    no_distill: true
commands:
  c:
    description: d
    content: body
mcp:
  m:
    command: npx
    args: [-y, thing]
    env: {K: V}
    notes: n
    installation: i
skills:
  s:
    path: skills/s
    tags: [t]
    notes: n
profiles:
  p:
    description: d
    parents: [base]
    bundles: [other]
hooks:
  post_file_edit:
    - command: echo hi
      tool: file_edit
"#,
    );
    assert_eq!(b.fragments.len(), 1);
    assert_eq!(b.commands.len(), 1);
    assert_eq!(b.mcp.len(), 1);
    assert_eq!(b.skills.len(), 1);
    assert_eq!(b.profiles.len(), 1);
    assert_eq!(b.hooks.len(), 1);
    let f = &b.fragments["f"];
    assert_eq!(
        (
            f.body.distilled.as_str(),
            f.body.distilled_by.as_str(),
            f.body.no_distill
        ),
        ("d", "model", true)
    );
    assert_eq!(b.mcp["m"].env["K"], "V");
    assert_eq!(b.mcp["m"].args, ["-y", "thing"]);
    assert_eq!(b.requires_commands, Vec::<String>::new());
}

// content/document_test.go + assemble tests' corpus: a fragment's `premise:`
// in the document form is the premise.
#[test]
fn test_a_fragments_premise_in_a_document() {
    let b = parse("version: 1.0.0\nfragments:\n  maybe:\n    premise: \"the agent is about to touch a signed bundle\"\n    content: \"Do not edit a signed bundle in place.\"\n");
    assert_eq!(
        b.fragments["maybe"].premise,
        "the agent is about to touch a signed bundle"
    );
}

const FETCH_BUNDLE: &str = "https://github.com/o/r@bundles/core";
const CANONICAL_BUNDLE: &str = "ctxloom+git://github.com/o/r//bundles/core";
const FETCH_PARENT: &str = "https://github.com/o/r@bundles/kit#profiles/base";
const CANONICAL_PARENT: &str = "ctxloom+git://github.com/o/r//bundles/kit#profiles/base";

// profile_refs_step_test.go: TestParseBundle_ProfileRefsStep: an envelope
// older than the step has its profiles' stored refs moved onto the canonical
// grammar; a current one is read as written.
#[test]
fn test_profile_refs_move_onto_the_canonical_grammar_below_generation_2() {
    let doc = |generation: u32| {
        format!(
            "schema_version: {generation}\nversion: 1.0.0\nprofiles:\n  dev:\n    bundles:\n      - {FETCH_BUNDLE}\n    parents:\n      - {FETCH_PARENT}\n"
        )
    };
    let old = parse(&doc(PROFILE_REFS - 1));
    assert_eq!(old.profiles["dev"].bundles, [CANONICAL_BUNDLE]);
    assert_eq!(old.profiles["dev"].parents, [CANONICAL_PARENT]);
    let current = parse(&doc(PROFILE_REFS));
    assert_eq!(
        current.profiles["dev"].bundles,
        [FETCH_BUNDLE],
        "a current envelope is not migrated"
    );
}

// schema_version_test.go + upgrade.go: a stamp past the current generation is
// refused, not read as far as it can be.
#[test]
fn test_a_newer_generation_is_refused_and_every_older_one_reads() {
    let err =
        parse_document("schema_version: 5\nversion: 1.0.0\nfragments:\n  f:\n    content: c\n")
            .expect_err("newer");
    assert!(
        matches!(
            err,
            BundleError::Newer {
                generation: 5,
                current: 4
            }
        ),
        "{}",
        err.english()
    );
    for generation in 0..=CURRENT {
        let doc = format!(
            "schema_version: {generation}\nversion: 1.0.0\nfragments:\n  f:\n    content: c\n"
        );
        parse_document(&doc).unwrap_or_else(|e| panic!("generation {generation}: {}", e.english()));
    }
}

// refs: the document parser takes the corpus Go's composite tests are built on.
#[test]
fn test_the_composite_corpus_alpha_parses() {
    let alpha = "version: 1.0.0\ndescription: alpha corpus\nfragments:\n  rules:\n    tags: [house]\n    content: \"Rules for {{project}}.\"\n  style:\n    content: \"Prefer small functions.\"\n  maybe:\n    premise: \"the agent is about to touch a signed bundle\"\n    content: \"Do not edit a signed bundle in place.\"\ncommands:\n  review:\n    description: Review the diff\n    content: \"Review the staged diff.\"\n    llm:\n      claude-code:\n        description: Review (claude)\n        argument_hint: \"[path]\"\n        allowed_tools: [Read]\n        model: sonnet\n  release:\n    description: Cut a release\n    content: \"Prepare the release notes.\"\n    llm:\n      claude-code:\n        enabled: false\nmcp:\n  db:\n    command: mcp-db\n";
    let b = parse(alpha);
    assert_eq!(
        b.fragments.keys().collect::<Vec<_>>(),
        ["maybe", "rules", "style"]
    );
    assert_eq!(b.fragments["rules"].body.tags, ["house"]);
    assert_eq!(b.commands["review"].description, "Review the diff");
    assert_eq!(b.mcp["db"].command, "mcp-db");
}

fn refused(text: &str) -> String {
    parse_document(text).expect_err(text).english()
}

// upgrade_test.go: the generation-0 migration renames `llm:` to `exports:` on
// commands and skills, and on nothing else
#[test]
fn test_the_llm_to_exports_migration_covers_skills_and_no_other_kind() {
    let b = parse(
        "version: \"1.0.0\"\nskills:\n  reviewer:\n    path: skills/reviewer\n    llm:\n      claude-code:\n        enabled: false\n",
    );
    assert!(b.skills["reviewer"].exports.contains_key("claude-code"));
    let said = refused("version: \"1.0.0\"\nfragments:\n  f:\n    content: c\n    llm:\n      claude-code:\n        enabled: false\n");
    assert!(said.contains("llm"), "{said}");
}

// strict_test.go: an unknown key is refused wherever the schema is closed
#[test]
fn test_an_unknown_key_is_refused_in_every_closed_place() {
    for (text, site) in [
        (
            "fragments:\n  f:\n    content: c\n    bogus: 1\n",
            "fragments.<name>",
        ),
        (
            "commands:\n  c:\n    content: c\n    bogus: 1\n",
            "commands.<name>",
        ),
        (
            "skills:\n  s:\n    path: p\n    bogus: 1\n",
            "skills.<name>",
        ),
        ("requires:\n  commands: [git]\n  bogus: 1\n", "requires"),
        ("hooks:\n  bogus: []\n", "hooks"),
        (
            "hooks:\n  pre_tool:\n    - command: x\n      bogus: 1\n",
            "hooks.pre_tool[]",
        ),
    ] {
        let said = refused(&format!("version: \"1.0.0\"\n{text}"));
        assert!(said.contains("bogus"), "{text:?}: {said}");
        assert!(said.contains(site), "{text:?}: {said}");
    }
}

// mcp validation (wire/mcp.go Validate): one target, and a usable one
#[test]
fn test_an_mcp_server_has_exactly_one_usable_target() {
    let server = |f: &dyn Fn(&mut McpServer)| {
        let mut s = McpServer::default();
        f(&mut s);
        mcp_target_fault(&s)
    };
    let none = "mcp server: none of command, url or served_by is set";
    assert_eq!(server(&|_| {}).as_deref(), Some(none));
    assert_eq!(server(&|s| s.command = "x".into()), None);
    assert_eq!(
        server(&|s| {
            s.command = "x".into();
            s.args = vec!["a".into()];
        }),
        None
    );
    assert_eq!(
        server(&|s| s.args = vec!["a".into()]).as_deref(),
        Some(none)
    );
    assert_eq!(server(&|s| s.url = "https://h/x".into()), None);
    assert_eq!(server(&|s| s.url = "http://h/x".into()), None);
    assert_eq!(
        server(&|s| s.url = "ftp://h/x".into()).as_deref(),
        Some("mcp server: url scheme must be http or https: \"ftp://h/x\"")
    );
    assert_eq!(server(&|s| s.served_by = "session-endpoint".into()), None);
    assert_eq!(
        server(&|s| s.served_by = "elsewhere".into()).as_deref(),
        Some("mcp server: served_by must be session-endpoint, got \"elsewhere\"")
    );
    for two in [
        |s: &mut McpServer| {
            s.command = "x".into();
            s.url = "https://h".into();
        },
        |s: &mut McpServer| {
            s.command = "x".into();
            s.served_by = "session-endpoint".into();
        },
        |s: &mut McpServer| {
            s.url = "https://h".into();
            s.served_by = "session-endpoint".into();
        },
        |s: &mut McpServer| {
            s.args = vec!["a".into()];
            s.served_by = "session-endpoint".into();
        },
    ] {
        let fault = server(&two).expect("two targets");
        assert!(
            fault.starts_with("mcp server: more than one of command"),
            "{fault}"
        );
    }
}

// the envelope's own fields reach the bundle
#[test]
fn test_the_envelope_fields_reach_the_bundle() {
    let b = parse(
        "name: mine\nversion: \"2.0.0\"\nauthor: me\nnotes: some notes\ninstallation: install it\ndescription: d\ntags: [a, b]\n",
    );
    assert_eq!(
        (
            b.name.as_str(),
            b.author.as_str(),
            b.notes.as_str(),
            b.installation.as_str()
        ),
        ("mine", "me", "some notes", "install it")
    );
    assert_eq!(b.version, "2.0.0");
    assert_eq!(b.tags, ["a", "b"]);
    assert!(b.hooks.is_empty());
}

// bundles: requires.commands names are command names; hooks decode in order
#[test]
fn test_required_commands_are_command_names_and_hooks_decode_in_event_order() {
    let b = parse("version: \"1.0.0\"\nrequires:\n  commands: [git, ripgrep-2.x, a_b+c]\n");
    assert_eq!(b.requires_commands, ["git", "ripgrep-2.x", "a_b+c"]);
    let said = refused("version: \"1.0.0\"\nrequires:\n  commands: [git, a/b]\n");
    assert!(said.contains("a/b"), "{said}");
    for good in ["a", "A0", "a.b_c+d-e"] {
        assert!(is_command_name(good), "{good}");
    }
    for bad in ["", ".a", "-a", "a b", "a/b", "\u{e9}"] {
        assert!(!is_command_name(bad), "{bad:?}");
    }
    let b = parse(
        "version: \"1.0.0\"\nhooks:\n  post_tool:\n    - command: second\n  pre_tool:\n    - command: first\n    - command: next\n",
    );
    let order: Vec<String> = b
        .hooks
        .iter()
        .map(|h| format!("{}/{}", h.event, h.name))
        .collect();
    assert_eq!(order, ["pre_tool/0", "pre_tool/1", "post_tool/0"]);
}
