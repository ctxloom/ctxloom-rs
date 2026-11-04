//! Ported from Go's `internal/adapters/content/tree_test.go`,
//! `hook_order_test.go`, `treewalk_characterization_test.go` and
//! `internal/core/bundles` (`schema_version_test.go`, `profile_refs_step_test.go`,
//! `tree_read.go`'s three refusals) at 860336c4e, on Go's own fixture tree
//! `code-quality` (a copy in `testdata/tree`).

use super::*;
use crate::testdata::{code_quality, files};

angzarr_presentation::english_for!(&[crate::EN_US.1]);

fn read(files: &Files) -> Bundle {
    read_tree("code-quality", files).unwrap_or_else(|e| panic!("{}", e.english()))
}

// tree_test.go: TestBundle_RefsEnumeratesEveryKind: the exact set of items the
// fixture holds, every kind, hooks by NAME; no sidecar is an item of its own
// and a distilled form is not a second item.
#[test]
fn test_a_tree_holds_exactly_the_items_the_fixture_has() {
    let bundle = read(&code_quality());
    assert_eq!(
        bundle.fragments.keys().collect::<Vec<_>>(),
        ["solid", "tricky"]
    );
    assert_eq!(bundle.commands.keys().collect::<Vec<_>>(), ["review"]);
    assert_eq!(bundle.mcp.keys().collect::<Vec<_>>(), ["postgres"]);
    assert_eq!(bundle.skills.keys().collect::<Vec<_>>(), ["code-reviewer"]);
    assert_eq!(bundle.profiles.keys().collect::<Vec<_>>(), ["strict"]);
    let hooks: Vec<String> = bundle
        .hooks
        .iter()
        .map(|h| format!("{}/{}", h.event, h.name))
        .collect();
    assert_eq!(
        hooks,
        ["pre_tool/audit", "pre_tool/guard", "session_start/greet"]
    );
    assert_eq!(bundle.version, "1.0.0");
    assert_eq!(bundle.description, "golden corpus code-quality");
}

// tree_test.go: TestSurfaces_DecodeAuthoredFields (fragments)
#[test]
fn test_a_fragment_decodes_its_authored_fields_and_its_distilled_form() {
    let bundle = read(&code_quality());
    let solid = &bundle.fragments["solid"];
    assert_eq!(solid.body.tags, ["go", "design"]);
    assert!(
        solid.body.content.contains("Single responsibility"),
        "{:?}",
        solid.body.content
    );
    assert!(
        !solid.body.content.contains("tags:"),
        "front matter is not body"
    );
    assert_eq!(
        solid.body.distilled.trim(),
        "SOLID: SRP, OCP, LSP, ISP, DIP."
    );
    assert_eq!(solid.body.distilled_by, "test-model-1");
    assert!(
        solid.body.content_hash.starts_with("sha256:"),
        "content_hash is carried verbatim"
    );
    assert_eq!(
        solid.body.notes,
        "Human-facing note, never sent to a model."
    );

    let tricky = &bundle.fragments["tricky"];
    assert!(tricky.body.no_distill, "no_distill is decoded");
    assert!(tricky.body.content.contains("{{ not_a_template }}"));
    assert_eq!(
        tricky.body.content.matches("---").count(),
        2,
        "a body with its own rules is not corrupted: {:?}",
        tricky.body.content
    );
}

// Fragment file `description:` is the fragment's premise (surface_markdown.go
// itemBody / bundles tree_read.go addFragment).
#[test]
fn test_a_fragments_description_is_its_premise() {
    let tree = files(&[
        ("bundle.yaml", "version: 1.0.0\n"),
        (
            "fragments/golang.md",
            "---\ndescription: when editing Go code\ntags: [go]\n---\nGO-RULES-BODY\n",
        ),
        ("fragments/plain.md", "PLAIN\n"),
    ]);
    let bundle = read_tree("x", &tree).unwrap();
    assert_eq!(bundle.fragments["golang"].premise, "when editing Go code");
    assert_eq!(bundle.fragments["plain"].premise, "");
}

// tree_test.go: TestSurfaces_DecodeAuthoredFields (command and its engine blocks)
#[test]
fn test_a_command_carries_every_engines_block_as_authored() {
    let bundle = read(&code_quality());
    let review = &bundle.commands["review"];
    assert_eq!(
        review.description,
        "Review a diff against the project's standards"
    );
    assert_eq!(review.body.installation, "Run after staging your changes.");
    assert_eq!(review.body.tags, ["review"]);
    assert_eq!(review.body.content.trim(), "Review the staged diff.");
    let claude = review
        .exports
        .get("claude-code")
        .expect("claude-code exports");
    assert_eq!(claude.get("enabled").and_then(|v| v.as_str()), Some("true"));
    assert_eq!(
        claude.get("description").and_then(|v| v.as_str()),
        Some("Review the staged diff")
    );
    assert_eq!(
        claude.get("argument_hint").and_then(|v| v.as_str()),
        Some("[path]")
    );
    assert_eq!(claude.get("model").and_then(|v| v.as_str()), Some("sonnet"));
    let tools: Vec<&str> = claude
        .get("allowed_tools")
        .and_then(|v| v.as_seq())
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(tools, ["Read", "Grep"]);
    let disabled = review
        .exports
        .get("disabled-engine")
        .expect("a disabled block is carried");
    assert_eq!(
        disabled.get("enabled").and_then(|v| v.as_str()),
        Some("false")
    );
    assert!(
        review.exports.contains_key("some-future-engine"),
        "an engine this build never heard of is carried, not dropped"
    );
}

// tree_test.go: TestMCP_SidecarIsAComponentAndContentFileStaysPure and
// TestSurfaces_DecodeAuthoredFields (mcp)
#[test]
fn test_an_mcp_server_reads_its_file_and_its_sidecar() {
    let bundle = read(&code_quality());
    let postgres = &bundle.mcp["postgres"];
    assert_eq!(postgres.command, "mcp-postgres");
    assert_eq!(postgres.args, ["--readonly"]);
    assert_eq!(postgres.env.len(), 3);
    assert_eq!(postgres.env["PGPORT"], "5432");
    assert_eq!(postgres.notes, "Read-only connection to the app database.");
    assert_eq!(postgres.installation, "brew install mcp-postgres");
}

// tree_test.go: TestSkill_DotPrefixedSidecarIsAComponentAndDeclaresExecutability
// and TestSurfaces_DecodeAuthoredFields (skill)
#[test]
fn test_a_skill_package_lists_its_files_and_the_sidecar_declares_the_executable() {
    let mut tree = code_quality();
    let bundle = read(&tree);
    let skill = &bundle.skills["code-reviewer"];
    let paths: Vec<&str> = skill.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        ["SKILL.md", "references/checklist.md", "scripts/run.sh"]
    );
    assert_eq!(skill.notes, "Wraps the house review checklist.");
    assert_eq!(skill.tags, ["review"]);
    assert!(
        skill.exports.contains_key("claude-code") && skill.exports.contains_key("other-engine")
    );
    let executable: Vec<&str> = skill
        .files
        .iter()
        .filter(|f| f.executable)
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(
        executable,
        ["scripts/run.sh"],
        "the declaration is the source of truth"
    );

    // Editing ONLY the sidecar, dropping the declaration, drops the mode.
    tree.insert(
        "skills/.code-reviewer.meta.yaml".to_string(),
        b"tags:\n  - review\nnotes: Wraps the house review checklist.\n".to_vec(),
    );
    let bundle = read(&tree);
    assert!(bundle.skills["code-reviewer"]
        .files
        .iter()
        .all(|f| !f.executable));
}

// hook_order_test.go + tree_test.go: TestHook_TwoHooksInOneEventHaveNameIdentity
// and the decoded fields of the fixture's hooks.
#[test]
fn test_hooks_decode_their_fields_and_have_name_identity() {
    let bundle = read(&code_quality());
    let by_name = |event: &str, name: &str| {
        bundle
            .hooks
            .iter()
            .find(|h| h.event == event && h.name == name)
            .unwrap_or_else(|| panic!("{event}/{name}"))
    };
    let audit = by_name("pre_tool", "audit");
    assert_eq!(
        (audit.tool.as_str(), audit.kind.as_str()),
        ("file_edit", "command")
    );
    assert_eq!(audit.command, "audit-log record");
    assert!(audit.is_async);
    let guard = by_name("pre_tool", "guard");
    assert_eq!(guard.tool, "shell");
    assert_eq!(guard.timeout, 30);
    let greet = by_name("session_start", "greet");
    assert_eq!(greet.kind, "prompt");
    assert_eq!(greet.prompt, "Load the project context.");
    assert!(greet.pre_tool_fallback);
    assert_eq!(
        bundle.hook_ids(),
        ["pre_tool/0", "pre_tool/1", "session_start/0"]
    );
}

// hook_order_test.go: TestHookOrder_SortsByDeclaredOrderNotByName,
// _AbsentSortsAfterEveryDeclaredOrder, _TiesBreakByName
#[test]
fn test_hooks_sort_by_declared_order_then_name_and_undeclared_last() {
    let tree = files(&[
        ("bundle.yaml", "version: 1.0.0\n"),
        ("hooks/pre_tool/a.yaml", "command: a\n"),
        ("hooks/pre_tool/.a.meta.yaml", "order: 300\n"),
        ("hooks/pre_tool/b.yaml", "command: b\n"),
        ("hooks/pre_tool/.b.meta.yaml", "order: 100\n"),
        ("hooks/pre_tool/c.yaml", "command: c\n"),
        ("hooks/pre_tool/d.yaml", "command: d\n"),
        ("hooks/pre_tool/e.yaml", "command: e\n"),
        ("hooks/pre_tool/.e.meta.yaml", "order: 100\n"),
    ]);
    let bundle = read_tree("x", &tree).unwrap();
    let names: Vec<&str> = bundle.hooks.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(
        names,
        ["b", "e", "a", "c", "d"],
        "declared order first (ties by name), undeclared last in name order"
    );
    assert_eq!(bundle.hooks[0].order, Some(100));
    assert_eq!(bundle.hooks[3].order, None);
}

// hook_tool_test.go: a hook narrows by tool CLASS only: an engine-native
// matcher is refused, naming it.
#[test]
fn test_a_hook_with_an_engine_matcher_is_refused() {
    let tree = files(&[
        ("bundle.yaml", "schema_version: 4\nversion: 1.0.0\n"),
        ("hooks/pre_tool/h.yaml", "matcher: Bash\ncommand: x\n"),
    ]);
    let err = read_tree("badhook", &tree).expect_err("matcher");
    let said = err.english();
    assert!(said.contains("pre_tool/h"), "{said}");
    assert!(
        said.contains("matcher \"Bash\" names an engine's own tools"),
        "{said}"
    );
    assert!(
        said.contains("narrow the hook with `tool:` (one of shell, file_edit, skill)"),
        "{said}"
    );
}

// tree_test.go: TestProfile_PriorityOrderingRoundTrips (the decoded profile)
#[test]
fn test_a_profile_item_decodes_with_its_priorities() {
    let bundle = read(&code_quality());
    let strict = &bundle.profiles["strict"];
    assert_eq!(strict.description, "Strict review profile");
    assert_eq!(strict.parents, ["base"]);
    let got: Vec<(&str, i64)> = strict
        .fragments
        .iter()
        .map(|f| (f.name.as_str(), f.priority))
        .collect();
    assert_eq!(got, [("solid", 0), ("tricky", 10), ("solid", -5)]);
    assert_eq!(strict.bundles, ["code-quality#fragments/solid"]);
}

// tree_test.go: TestWalk_MalformedDirectoriesFailLoud: none of these may be
// misread as an item, and none may be silently dropped.
#[test]
fn test_a_file_no_kind_claims_fails_loud_naming_it() {
    for (what, path, body) in [
        (
            "skill directory with no descriptor",
            "skills/broken/notes.md",
            "no descriptor here\n",
        ),
        (
            "skill nested a level too deep",
            "skills/nested/inner/SKILL.md",
            "---\nname: inner\n---\nbody\n",
        ),
        (
            "foreign extension in a kind dir",
            "fragments/README.txt",
            "not a fragment\n",
        ),
        (
            "mcp server a level too deep",
            "mcp/subdir/thing.yaml",
            "command: x\n",
        ),
    ] {
        let mut tree = code_quality();
        tree.insert(path.to_string(), body.as_bytes().to_vec());
        let err = read_tree("code-quality", &tree).expect_err(what);
        assert!(
            err.english().contains(path),
            "{what}: the error names the file: {}",
            err.english()
        );
    }
}

// tree_test.go: TestDecode_RefusesUnexplainedSidecars
#[test]
fn test_a_sidecar_on_a_kind_that_keeps_none_is_refused() {
    for (what, path, body) in [
        (
            "fragment",
            "fragments/.solid.meta.yaml",
            "tags:\n  - smuggled\n",
        ),
        ("profile", "profiles/.strict.meta.yaml", "owner: nobody\n"),
    ] {
        let mut tree = code_quality();
        tree.insert(path.to_string(), body.as_bytes().to_vec());
        let err = read_tree("code-quality", &tree).expect_err(what);
        let said = err.english();
        assert!(said.contains(path), "{what}: names the sidecar: {said}");
        assert!(
            said.contains("the .<name>.meta.yaml name is reserved for ctxloom metadata"),
            "{what}: states why the file is refused: {said}"
        );
        assert!(!said.to_lowercase().contains("signature"), "{what}: {said}");
    }
}

// tree_read.go ReadTree: the three refusals. No envelope: nothing can supply
// a version. An envelope that ALSO declares items inline: two answers for one
// item. (A tree with no items at all is accepted: the project bundle holds
// only profiles, and Go lists an envelope-only tree.)
#[test]
fn test_a_tree_without_an_envelope_or_with_inline_items_is_refused() {
    let no_envelope = files(&[("fragments/f.md", "ITEM\n")]);
    let err = read_tree("demo", &no_envelope).expect_err("no envelope");
    assert!(err.english().contains("demo"), "{}", err.english());
    assert!(err.english().contains("bundle.yaml"), "{}", err.english());

    let inline = files(&[
        (
            "bundle.yaml",
            "schema_version: 4\nversion: 1.0.0\nfragments:\n  x:\n    content: INLINE\n",
        ),
        ("fragments/f.md", "ITEM\n"),
    ]);
    let err = read_tree("inline", &inline).expect_err("two answers");
    let said = err.english();
    assert!(said.contains("still declares fragments inline"), "{said}");
    assert!(said.contains("half-migrated"), "{said}");

    let only_envelope = files(&[("bundle.yaml", "schema_version: 4\nversion: 1.0.0\n")]);
    let bundle = read_tree("noitems", &only_envelope).expect("an envelope alone is a bundle");
    assert!(bundle.fragments.is_empty() && bundle.profiles.is_empty());
}

// upgrade.go envelopeSteps: a tree reads at every generation 0 to 4 to the
// same model; the envelope's stamp is the tree's.
#[test]
fn test_a_tree_reads_the_same_at_every_envelope_generation() {
    let mut models = Vec::new();
    for generation in 0..=4 {
        let envelope = if generation == 0 {
            "version: 1.0.0\ndescription: d\n".to_string()
        } else {
            format!("schema_version: {generation}\nversion: 1.0.0\ndescription: d\n")
        };
        let tree = files(&[
            ("bundle.yaml", &envelope),
            ("fragments/f.md", "---\ntags: [t]\n---\nBODY\n"),
            ("prompts/p.md", "---\ndescription: pd\n---\nPROMPT\n"),
            ("mcp/m.yaml", "command: server\n"),
        ]);
        models.push(
            read_tree("g", &tree)
                .unwrap_or_else(|e| panic!("generation {generation}: {}", e.english())),
        );
    }
    for model in &models[1..] {
        assert_eq!(model, &models[0]);
    }
    assert_eq!(models[0].fragments["f"].body.content.trim(), "BODY");
}

// schema_version_test.go + the bundle warning texts at 860336c4e: an envelope
// newer than this build reads, one whose stamp is not an integer, an unknown
// key, an empty file and a file that is not YAML are each refused by name.
#[test]
fn test_a_malformed_envelope_is_refused_by_name() {
    let tree = |envelope: &str| files(&[("bundle.yaml", envelope), ("fragments/f.md", "ITEM\n")]);
    let said = |envelope: &str| {
        read_tree("b", &tree(envelope))
            .expect_err(envelope)
            .english()
    };

    let newer = said("schema_version: 5\nversion: 1.0.0\n");
    assert!(newer.contains("schema_version 5"), "{newer}");
    assert!(
        newer.contains("written by a newer format than this binary reads"),
        "{newer}"
    );
    assert!(newer.contains("this binary reads up to 4"), "{newer}");

    let not_int = said("schema_version: abc\nversion: 1.0.0\n");
    assert!(
        not_int.contains("format version cannot be read: schema_version is not an integer"),
        "{not_int}"
    );

    let unknown = said("schema_version: 4\nversion: 1.0.0\nbogus: 1\n");
    assert!(
        unknown.contains("unknown key `bogus` on line 3"),
        "{unknown}"
    );
    assert!(
        unknown.contains("(at the top level of the bundle)"),
        "{unknown}"
    );
    assert!(
        unknown.contains("valid keys there: author, commands, description, fragments, hooks"),
        "{unknown}"
    );

    let empty = said("");
    assert!(empty.contains("bundle is empty"), "{empty}");
    assert!(
        empty.contains("0 bytes parsed to a document declaring no version and no items"),
        "{empty}"
    );

    let not_yaml = said("::bad [\n");
    assert!(not_yaml.contains("invalid bundle YAML"), "{not_yaml}");
}

const FETCH_BUNDLE: &str = "https://github.com/o/r@bundles/core";
const CANONICAL_BUNDLE: &str = "ctxloom+git://github.com/o/r//bundles/core";
const FETCH_PARENT: &str = "https://github.com/o/r@bundles/kit#profiles/base";
const CANONICAL_PARENT: &str = "ctxloom+git://github.com/o/r//bundles/kit#profiles/base";

// profile_refs_step_test.go: TestReadTree_ProfileItemsFollowTheEnvelopeGeneration
#[test]
fn test_a_trees_profile_refs_follow_the_envelope_generation() {
    for (generation, bundles, parents) in [
        (1, CANONICAL_BUNDLE, CANONICAL_PARENT),
        (2, FETCH_BUNDLE, FETCH_PARENT),
    ] {
        let profile = format!("bundles:\n  - {FETCH_BUNDLE}\nparents:\n  - {FETCH_PARENT}\n");
        let tree = files(&[
            (
                "bundle.yaml",
                &format!("schema_version: {generation}\nversion: 1.0.0\n"),
            ),
            ("profiles/dev.yaml", &profile),
        ]);
        let bundle = read_tree("kit", &tree).unwrap();
        assert_eq!(
            bundle.profiles["dev"].bundles,
            [bundles],
            "generation {generation}"
        );
        assert_eq!(
            bundle.profiles["dev"].parents,
            [parents],
            "generation {generation}"
        );
    }
}

// treewalk_characterization_test.go + testdata/golden/treewalk.txt: the tree
// holds these 14 files (the golden's `file` lines), and dump lists them.
#[test]
fn test_dump_prints_the_envelope_then_every_file_by_path() {
    let tree = files(&[
        ("bundle.yaml", "version: 1.0.0\n"),
        ("fragments/b.md", "B"),
        ("fragments/a.md", "A\n"),
    ]);
    assert_eq!(
        dump(&tree),
        "==> bundle.yaml <==\nversion: 1.0.0\n\n==> fragments/a.md <==\nA\n\n==> fragments/b.md <==\nB\n"
    );
}

fn unclaimed_path(extra: &str) -> String {
    let mut tree = code_quality();
    tree.insert(extra.to_string(), b"x".to_vec());
    match read_tree("code-quality", &tree) {
        Err(BundleError::Unclaimed { path, .. }) => path,
        other => panic!(
            "{extra}: expected an unclaimed file, got {:?}",
            other.map(|_| ())
        ),
    }
}

// tree_read.go: a file no kind claims is refused by name, whatever its shape:
// a nameless or hidden item, a package with no name or no file, a hook with
// no name.
#[test]
fn test_a_file_no_kind_claims_is_refused_by_its_path() {
    for extra in [
        "fragments/.md",
        "fragments/.hidden.md",
        "skills/.hidden/SKILL.md",
        "skills//x",
        "skills/pkg/",
        "hooks/pre_tool/.yaml",
        "hooks/pre_tool/.hidden.yaml",
    ] {
        assert_eq!(unclaimed_path(extra), extra, "{extra}");
    }
}

// tree_read.go: a blank sidecar is the empty document
#[test]
fn test_a_blank_sidecar_is_the_empty_document() {
    let mut tree = code_quality();
    tree.insert("mcp/.postgres.meta.yaml".to_string(), b" \n\t\n".to_vec());
    let bundle = read(&tree);
    assert_eq!(bundle.mcp.keys().collect::<Vec<_>>(), ["postgres"]);
}
