//! What the application answers in the corners the scenario tests leave:
//! each test names the Go behaviour (a Gherkin scenario, a Go test, or a
//! golden of `crates/root/bin/ctxloom/fixtures/golden`) it holds.

use crate::bundles::library_preview;
use crate::fakes::{plain, Setup, CONFIG, ENVELOPE};
use crate::{AppError, ItemKind, Notice, SearchRequest};

fn ok<T>(outcome: crate::Outcome<T>) -> T {
    match outcome.result {
        Ok(value) => value,
        Err(e) => panic!("{e:?}"),
    }
}

fn with<'a>(
    bundles: &'a [(&'a str, &'a [(&'a str, &'a str)])],
    layers: &'a [&'a str],
) -> Setup<'a> {
    Setup {
        bundles,
        layers,
        ..Setup::default()
    }
}

// environment.md: CTXLOOM_DEGRADED=1 (exactly) degrades; nothing else does.
#[test]
fn test_degraded_by_environment_means_ctxloom_degraded_is_one() {
    let on = Setup {
        environ: &[("CTXLOOM_DEGRADED", "1")],
        ..Setup::default()
    }
    .library();
    assert!(on.degraded_by_environment());
    for environ in [
        &[("CTXLOOM_DEGRADED", "0")][..],
        &[("CTXLOOM_DEGRADED", "")][..],
        &[("OTHER", "1")][..],
        &[][..],
    ] {
        let off = Setup {
            environ,
            ..Setup::default()
        }
        .library();
        assert!(!off.degraded_by_environment(), "{environ:?}");
    }
}

// fault_tolerance.feature: CTXLOOM_DEGRADED=1 is --degraded, so an unknown
// config key warns instead of refusing.
#[test]
fn test_the_environment_degrades_a_run_like_the_flag() {
    let layers = ["schema_version: 8\nzzz: 1\n"];
    let strict = with(&[], &layers).library();
    assert!(strict.bundle_list(&plain()).result.is_err());
    let degraded = Setup {
        environ: &[("CTXLOOM_DEGRADED", "1")],
        ..with(&[], &layers)
    }
    .library();
    assert!(degraded.bundle_list(&plain()).result.is_ok());
}

// non_tty_format (plan D28): the configured format off a terminal, if set.
#[test]
fn test_the_preferred_format_is_the_configured_one_or_none() {
    let layers = ["schema_version: 8\nnon_tty_format: text\n"];
    assert_eq!(
        with(&[], &layers).library().preferred_format(&plain()),
        Some("text".to_string())
    );
    assert_eq!(
        with(&[], &[CONFIG]).library().preferred_format(&plain()),
        None
    );
    // A config the strict reader would refuse still yields its format: the
    // choice of a format must not fail the run that reports the failure.
    let bad = ["schema_version: 8\nnon_tty_format: json\nzzz: 1\n"];
    assert_eq!(
        with(&[], &bad).library().preferred_format(&plain()),
        Some("json".to_string())
    );
}

// environment.md: with no project the home's configuration is read and its
// content is not; a project with no bundles directory says so.
#[test]
fn test_listings_say_why_they_are_empty() {
    let demo: &[(&str, &str)] = &[ENVELOPE, ("fragments/a.md", "A\n")];
    let home = Setup {
        home_only: true,
        ..with(&[("demo", demo)], &[CONFIG])
    }
    .library();
    let list = ok(home.bundle_list(&plain()));
    assert!(
        list.rows.is_empty(),
        "the home's content is not a project's"
    );
    assert!(list.no_root);
    assert!(list.hint.starts_with("No project here: run 'ctxloom init'"));
    let bare = Setup {
        no_root: true,
        ..with(&[], &[CONFIG])
    }
    .library();
    let list = ok(bare.bundle_list(&plain()));
    assert!(list.no_root);
    assert!(list.hint.starts_with("Add remote bundles to a profile"));
    let rooted = ok(with(&[("demo", demo)], &[CONFIG])
        .library()
        .bundle_list(&plain()));
    assert!(!rooted.no_root);
    assert_eq!(rooted.rows.len(), 1);
    let profiles = ok(home.profile_list(&plain()));
    assert!(profiles.hint.starts_with("No project here"));
    let profiles = ok(with(&[], &[CONFIG]).library().profile_list(&plain()));
    assert!(profiles.hint.starts_with("Use 'ctxloom profile create"));
    let items = ok(home.item_list(&plain(), ItemKind::Fragment, ""));
    assert!(items.hint.starts_with("No project here"));
    assert_eq!(items.kind, "fragment");
    let none = ok(bare.item_list(&plain(), ItemKind::Command, ""));
    assert_eq!(none.kind, "command");
    assert!(none.hint.starts_with("Add remote bundles"));
}

// bundle.feature: showing a bundle with no bundles directory is refused.
#[test]
fn test_showing_a_bundle_with_no_bundles_directory_is_refused() {
    let bare = Setup {
        no_root: true,
        ..with(&[], &[CONFIG])
    }
    .library();
    assert!(matches!(
        bare.bundle_show(&plain(), "demo").result,
        Err(AppError::NoBundlesDir)
    ));
}

// bundle.feature: show tells which fragments and commands have a distilled
// form, and the first line of each, cut to 70 bytes.
#[test]
fn test_bundle_show_reports_distilled_forms_and_previews() {
    let long = "x".repeat(80);
    let long_fragment = format!("{long}\n");
    let files: &[(&str, &str)] = &[
        ENVELOPE,
        ("fragments/plain.md", "\n\n  first line  \nsecond\n"),
        ("fragments/long.md", &long_fragment),
        ("fragments/both.md", "full\n"),
        ("fragments/both.distilled.md", "short\n"),
        ("prompts/p.md", "---\ndescription: d\n---\nPROMPT\n"),
        ("prompts/q.md", "Q\n"),
        ("prompts/q.distilled.md", "q\n"),
    ];
    let shown = ok(with(&[("b", files)], &[CONFIG])
        .library()
        .bundle_show(&plain(), "b"));
    assert_eq!(shown.fragments["plain"].preview, "first line");
    assert_eq!(
        shown.fragments["long"].preview,
        format!("{}\u{2026}", "x".repeat(67))
    );
    assert!(!shown.fragments["plain"].distilled);
    assert!(shown.fragments["both"].distilled);
    assert!(!shown.commands["p"].distilled);
    assert!(shown.commands["q"].distilled);
    assert_eq!(shown.commands["p"].preview, "PROMPT");
    assert_eq!(shown.commands["p"].description, "d");
}

// bundle show's preview (Go's libraryPreview): the first non-blank line,
// trimmed, cut at 70 bytes on a character boundary with an ellipsis.
#[test]
fn test_the_library_preview_cuts_at_seventy_bytes_on_a_character_boundary() {
    assert_eq!(library_preview(""), "");
    assert_eq!(library_preview("  \n  hello  \nworld"), "hello");
    let exact = "a".repeat(70);
    assert_eq!(library_preview(&exact), exact);
    let over = "a".repeat(71);
    assert_eq!(
        library_preview(&over),
        format!("{}\u{2026}", "a".repeat(67))
    );
    // 40 two-byte characters are 80 bytes; byte 67 is inside a character, so
    // the cut falls back to 66 (33 characters).
    let wide = "\u{e9}".repeat(40);
    assert_eq!(
        library_preview(&wide),
        format!("{}\u{2026}", "\u{e9}".repeat(33))
    );
    // 69 bytes of ASCII and a three-byte character end at 72: cut at 67.
    let edge = format!("{}\u{20ac}", "b".repeat(69));
    assert_eq!(
        library_preview(&edge),
        format!("{}\u{2026}", "b".repeat(67))
    );
}

// bundle view: an unknown mcp server name falls back to the bundle's only
// server; with several it is missing; a profile, skill and command show.
#[test]
fn test_viewing_one_item_of_each_kind() {
    let one: &[(&str, &str)] = &[ENVELOPE, ("mcp/only.yaml", "command: c\n")];
    let two: &[(&str, &str)] = &[
        ENVELOPE,
        ("mcp/a.yaml", "command: c\n"),
        ("mcp/b.yaml", "command: d\n"),
        ("prompts/p.md", "PROMPT-NO-NEWLINE"),
        ("prompts/p.distilled.md", "short\n"),
        ("fragments/f.md", "plain\n"),
        ("profiles/pr.yaml", "description: d\nbundles: [two]\n"),
        ("skills/s/SKILL.md", "---\nname: s\n---\nbody\n"),
        ("skills/.s.meta.yaml", "tags: [t]\nnotes: nn\n"),
    ];
    let library = with(&[("one", one), ("two", two)], &[CONFIG]).library();
    let only = ok(library.bundle_view(&plain(), "one#mcp/anything", false));
    assert_eq!(only.content, "# MCP Server: only\ncommand: c\n");
    assert!(matches!(
        library
            .bundle_view(&plain(), "two#mcp/missing", false)
            .result,
        Err(AppError::ViewMissing { .. })
    ));
    let named = ok(library.bundle_view(&plain(), "two#mcp/b", false));
    assert_eq!(named.content, "# MCP Server: b\ncommand: d\n");
    // A command without a trailing newline gets one; the distilled form is
    // only taken when asked for and there is one.
    let raw = ok(library.bundle_view(&plain(), "two#commands/p", false));
    assert_eq!(raw.content, "PROMPT-NO-NEWLINE\n");
    let short = ok(library.bundle_view(&plain(), "two#commands/p", true));
    assert_eq!(short.content, "# (distilled version)\nshort\n");
    let nodistilled = ok(library.bundle_view(&plain(), "two#fragments/f", true));
    assert_eq!(nodistilled.content, "plain\n");
    let profile = ok(library.bundle_view(&plain(), "two#profiles/pr", false));
    assert_eq!(
        profile.content,
        "# Profile: pr\ndescription: d\nbundles:\n  - two\n"
    );
    assert!(matches!(
        library
            .bundle_view(&plain(), "two#profiles/none", false)
            .result,
        Err(AppError::ViewMissing { .. })
    ));
    let skill = ok(library.bundle_view(&plain(), "two#skills/s", false));
    assert_eq!(skill.content, "# Skill: s\ntags:\n  - t\nnotes: nn\n");
    assert!(matches!(
        library
            .bundle_view(&plain(), "two#skills/none", false)
            .result,
        Err(AppError::ViewMissing { .. })
    ));
    assert!(matches!(
        library
            .bundle_view(&plain(), "two#commands/none", false)
            .result,
        Err(AppError::ViewMissing { .. })
    ));
    assert!(matches!(
        library
            .bundle_view(&plain(), "two#fragments/none", false)
            .result,
        Err(AppError::ViewMissing { .. })
    ));
}

// fragment.feature / command.feature: a --bundle with nothing of the kind is
// an empty listing; one nobody has is refused naming the bundles; a project
// with no bundles at all lists nothing.
#[test]
fn test_listing_an_empty_kind_is_not_a_missing_bundle() {
    let bare: &[(&str, &str)] = &[ENVELOPE];
    let has: &[(&str, &str)] = &[ENVELOPE, ("prompts/p.md", "P\n")];
    let library = with(&[("bare", bare), ("has", has)], &[CONFIG]).library();
    assert!(
        ok(library.item_list(&plain(), ItemKind::Command, ""))
            .rows
            .len()
            == 1
    );
    assert!(ok(library.item_list(&plain(), ItemKind::Fragment, ""))
        .rows
        .is_empty());
    assert!(ok(library.item_list(&plain(), ItemKind::Command, "bare"))
        .rows
        .is_empty());
    assert!(library
        .item_list(&plain(), ItemKind::Command, "nobody")
        .result
        .is_err());
    let nothing = with(&[], &[CONFIG]).library();
    let list = ok(nothing.item_list(&plain(), ItemKind::Command, "anybody"));
    assert!(list.rows.is_empty());
}

// fault_tolerance.feature: naming an item of the wrong kind says which
// directory the reference named and which was wanted.
#[test]
fn test_a_reference_of_the_wrong_kind_names_both_directories() {
    let files: &[(&str, &str)] = &[
        ENVELOPE,
        ("fragments/f.md", "F\n"),
        ("prompts/p.md", "P\n"),
        ("mcp/m.yaml", "command: c\n"),
    ];
    let library = with(&[("b", files)], &[CONFIG]).library();
    let kinds = |kind: ItemKind, reference: &str| match library
        .item_show(&plain(), kind, reference, false)
        .result
    {
        Err(AppError::WrongKind { got, want, .. }) => (got, want),
        other => panic!("{:?}", other.err()),
    };
    assert_eq!(
        kinds(ItemKind::Fragment, "b#commands/p"),
        ("commands".to_string(), "fragments".to_string())
    );
    assert_eq!(
        kinds(ItemKind::Command, "b#fragments/f"),
        ("fragments".to_string(), "commands".to_string())
    );
    assert_eq!(
        kinds(ItemKind::Fragment, "b#mcp/m"),
        ("mcp".to_string(), "fragments".to_string())
    );
}

// skill.feature: a skill list narrows to a bundle; a skill's tags are its
// bundle's then its own, each once.
#[test]
fn test_skills_list_by_bundle_and_merge_tags_without_repeats() {
    let skill = |name: &str| -> Vec<(String, String)> {
        vec![
            (
                format!("skills/{name}/SKILL.md"),
                format!("---\nname: {name}\ndescription: Does {name}\n---\nbody\n"),
            ),
            (
                format!("skills/.{name}.meta.yaml"),
                "tags: [shared, own]\n".to_string(),
            ),
        ]
    };
    let mut alpha: Vec<(String, String)> = skill("one");
    alpha.push((
        "bundle.yaml".to_string(),
        "schema_version: 4\nversion: 1.0.0\ntags: [shared, top]\n".to_string(),
    ));
    let alpha_refs: Vec<(&str, &str)> = alpha
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let beta_files = skill("two");
    let mut beta_refs: Vec<(&str, &str)> = beta_files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    beta_refs.push(ENVELOPE);
    let library = with(&[("alpha", &alpha_refs), ("beta", &beta_refs)], &[CONFIG]).library();
    let all = ok(library.skill_list(&plain(), ""));
    assert_eq!(all.total, 2);
    assert_eq!(all.rows.len(), 2);
    let only = ok(library.skill_list(&plain(), "beta"));
    assert_eq!(only.total, 2);
    assert_eq!(only.rows.len(), 1);
    assert_eq!(only.rows[0].source, "beta");
    assert_eq!(only.rows[0].name, "beta/two");
    let first = all.rows.iter().find(|r| r.source == "alpha").unwrap();
    assert_eq!(first.tags, ["shared", "top", "own"]);
    assert_eq!(first.description, "Does one");
}

const AGENT_DEV: &str = "schema_version: 8\nagents:\n  dev:\n    llm: claude-code\n    profiles: [dev]\ndefault_agent: dev\n";

// profile.feature: a profile of the project bundle is named bare, any other
// bundle's by its full reference, and its display name is the bare one.
#[test]
fn test_profiles_of_other_bundles_are_named_by_their_full_reference() {
    let project: &[(&str, &str)] = &[ENVELOPE, ("profiles/mine.yaml", "description: m\n")];
    let other: &[(&str, &str)] = &[ENVELOPE, ("profiles/theirs.yaml", "description: t\n")];
    let library = with(&[("project", project), ("other", other)], &[CONFIG]).library();
    let rows = ok(library.profile_list(&plain())).rows;
    let named: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|r| (r.name.as_str(), r.display_name.as_str(), r.bundle.as_str()))
        .collect();
    assert_eq!(
        named,
        [
            (
                "ctxloom+local:other#profiles/theirs",
                "theirs",
                "ctxloom+local:other"
            ),
            ("mine", "mine", "ctxloom+local:project"),
        ]
    );
    let shown = ok(library.profile_show(&plain(), "ctxloom+local:other#profiles/theirs"));
    assert_eq!(shown.name, "ctxloom+local:other#profiles/theirs");
    assert_eq!(shown.bundle, "ctxloom+local:other");
}

// profile.go: a parent that does not exist is skipped with a finding that
// says whether it is missing here or not installed, and suggests the one
// other bundle that has a profile of that name.
#[test]
fn test_a_missing_parent_is_said_with_the_right_remedy() {
    let project: &[(&str, &str)] = &[
        ENVELOPE,
        ("profiles/local.yaml", "parents: [ghost]\n"),
        (
            "profiles/remote.yaml",
            "parents: [\"https://example.com/r.git#profiles/x\"]\n",
        ),
    ];
    let other: &[(&str, &str)] = &[ENVELOPE, ("profiles/ghost.yaml", "description: g\n")];
    let library = with(&[("project", project), ("other", other)], &[CONFIG]).library();
    let run = |profile: &str| {
        let request = crate::RunRequest {
            profile: profile.to_string(),
            ..crate::RunRequest::default()
        };
        ok(library.dry_run(&plain(), &request)).plan.findings
    };
    let local = run("local");
    assert_eq!(local.len(), 1);
    assert!(
        local[0].text.contains("parent ghost does not exist"),
        "{}",
        local[0].text
    );
    assert!(
        local[0]
            .remedy
            .starts_with("did you mean ctxloom+local:other#profiles/ghost?"),
        "{}",
        local[0].remedy
    );
    let remote = run("remote");
    assert_eq!(remote.len(), 1);
    assert!(
        remote[0].text.contains("not installed"),
        "{}",
        remote[0].text
    );
    assert_eq!(remote[0].remedy, "ctxloom deps pull");
}

fn search_bundle() -> Vec<(String, String)> {
    let mut files = vec![
        (
            "bundle.yaml".to_string(),
            "schema_version: 4\nversion: 1.0.0\n".to_string(),
        ),
        (
            "fragments/alpha.md".to_string(),
            "---\ntags: [zebra]\n---\nA\n".to_string(),
        ),
        ("fragments/zebra-fragment.md".to_string(), "Z\n".to_string()),
        (
            "fragments/plain.md".to_string(),
            "---\ntags: [other]\n---\nP\n".to_string(),
        ),
        ("prompts/zebra-prompt.md".to_string(), "C\n".to_string()),
        ("prompts/unrelated.md".to_string(), "C\n".to_string()),
        (
            "profiles/zebra-profile.yaml".to_string(),
            "description: d\n".to_string(),
        ),
        (
            "profiles/by-description.yaml".to_string(),
            "description: a zebra herd\n".to_string(),
        ),
        (
            "profiles/by-tag.yaml".to_string(),
            "tags: [zebra]\n".to_string(),
        ),
        (
            "profiles/nothing.yaml".to_string(),
            "description: d\ntags: [x]\n".to_string(),
        ),
        (
            "skills/zebra-skill/SKILL.md".to_string(),
            "---\nname: zebra-skill\ndescription: d\n---\nb\n".to_string(),
        ),
        (
            "skills/by-desc/SKILL.md".to_string(),
            "---\nname: by-desc\ndescription: about a zebra\n---\nb\n".to_string(),
        ),
        (
            "skills/by-tag/SKILL.md".to_string(),
            "---\nname: by-tag\ndescription: d\n---\nb\n".to_string(),
        ),
        (
            "skills/.by-tag.meta.yaml".to_string(),
            "tags: [zebra]\n".to_string(),
        ),
        (
            "skills/other/SKILL.md".to_string(),
            "---\nname: other\ndescription: d\n---\nb\n".to_string(),
        ),
        (
            "mcp/zebra-server.yaml".to_string(),
            "command: run-zebra\n".to_string(),
        ),
        (
            "mcp/by-command.yaml".to_string(),
            "command: zebra-bin --x\n".to_string(),
        ),
        (
            "mcp/by-url.yaml".to_string(),
            "url: https://zebra.example/mcp\n".to_string(),
        ),
        (
            "mcp/by-endpoint.yaml".to_string(),
            "served_by: session-endpoint\n".to_string(),
        ),
        ("mcp/quiet.yaml".to_string(), "command: quiet\n".to_string()),
    ];
    files.sort();
    files
}

fn search_library() -> crate::Library {
    let files = search_bundle();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let layers = ["schema_version: 8\nagents:\n  d:\n    llm: claude-code\n    profiles: [all]\ndefault_agent: d\n"];
    let mut with_all = refs.clone();
    with_all.push(("profiles/all.yaml", "bundles: [project]\n"));
    Setup {
        bundles: &[("project", &with_all)],
        layers: &layers,
        ..Setup::default()
    }
    .library()
}

fn ask(query: &str, kind: &str) -> SearchRequest {
    SearchRequest {
        query: query.to_string(),
        kind: kind.to_string(),
        local: true,
        ..SearchRequest::default()
    }
}

fn hits(result: &crate::SearchResults) -> Vec<(String, String, String)> {
    result
        .local
        .iter()
        .map(|h| (h.kind.clone(), h.name.clone(), h.r#match.clone()))
        .collect()
}

fn pick(kind: &str, name: &str, how: &str) -> (String, String, String) {
    (kind.to_string(), name.to_string(), how.to_string())
}

// search.feature: every kind is searched, a match is by name, then (kind
// by kind) description or tag; nothing else is a hit.
#[test]
fn test_a_search_finds_each_kind_by_the_fields_that_kind_has() {
    let library = search_library();
    let all = ok(library.search(&plain(), &ask("zebra", "")));
    assert_eq!(all.count as usize, all.local.len());
    assert_eq!(all.hidden_local, 0);
    assert_eq!(
        hits(&all),
        [
            pick("mcp_server", "by-command", "name"),
            pick("mcp_server", "by-url", "name"),
            pick("skill", "project/zebra-skill", "name"),
            pick("fragment", "zebra-fragment", "name"),
            pick("profile", "zebra-profile", "name"),
            pick("command", "zebra-prompt", "name"),
            pick("mcp_server", "zebra-server", "name"),
            pick("fragment", "alpha", "tag"),
            pick("profile", "by-tag", "tag"),
            pick("skill", "project/by-tag", "tag"),
            pick("profile", "by-description", "description"),
            pick("skill", "project/by-desc", "description"),
        ]
    );
    let none = ok(library.search(&plain(), &ask("qqq", "")));
    assert!(none.local.is_empty());
}

// search.feature: --type narrows to one kind.
#[test]
fn test_a_search_by_type_looks_at_that_kind_only() {
    let library = search_library();
    let names = |kind: &str| -> Vec<String> {
        ok(library.search(&plain(), &ask("zebra", kind)))
            .local
            .iter()
            .map(|h| format!("{}:{}", h.kind, h.name))
            .collect()
    };
    assert_eq!(names("command"), ["command:zebra-prompt"]);
    assert_eq!(
        names("fragment"),
        ["fragment:zebra-fragment", "fragment:alpha"]
    );
    assert_eq!(
        names("skill"),
        [
            "skill:project/zebra-skill",
            "skill:project/by-tag",
            "skill:project/by-desc"
        ]
    );
    assert_eq!(
        names("profile"),
        [
            "profile:zebra-profile",
            "profile:by-tag",
            "profile:by-description"
        ]
    );
    assert_eq!(
        names("mcp_server"),
        [
            "mcp_server:by-command",
            "mcp_server:by-url",
            "mcp_server:zebra-server"
        ]
    );
    assert!(names("bundle").is_empty());
}

// search.feature: the registered servers are matched on name or on what
// they run, reach or are served by.
#[test]
fn test_a_server_is_found_by_its_name_or_its_target() {
    let library = search_library();
    let found = ok(library.search(&plain(), &ask("session-endpoint", "mcp_server")));
    assert_eq!(found.local.len(), 1);
    assert_eq!(found.local[0].name, "by-endpoint");
    assert_eq!(found.local[0].source, "session-endpoint");
    let by_url = ok(library.search(&plain(), &ask("example/mcp", "mcp_server")));
    assert_eq!(by_url.local[0].source, "https://zebra.example/mcp");
    let by_command = ok(library.search(&plain(), &ask("quiet", "mcp_server")));
    assert_eq!(by_command.local[0].source, "quiet");
    let by_name = ok(library.search(&plain(), &ask("by-command", "mcp_server")));
    assert_eq!(by_name.local[0].source, "zebra-bin --x");
}

// search.feature: --tag with no query lists the fragments with that tag and
// nothing of the other kinds.
#[test]
fn test_a_search_by_tag_alone_lists_fragments_only() {
    let library = search_library();
    let request = SearchRequest {
        tags: vec!["zebra".to_string()],
        local: true,
        ..SearchRequest::default()
    };
    let found = ok(library.search(&plain(), &request));
    assert_eq!(hits(&found), [pick("fragment", "alpha", "name")]);
}

// search.feature: --remote alone is refused; with --local it is a local
// search; neither is a local search.
#[test]
fn test_remote_search_is_refused_unless_local_is_asked_too() {
    let library = search_library();
    let both = SearchRequest {
        query: "zebra".to_string(),
        local: true,
        remote: true,
        ..SearchRequest::default()
    };
    assert!(!ok(library.search(&plain(), &both)).local.is_empty());
    let neither = SearchRequest {
        query: "zebra".to_string(),
        ..SearchRequest::default()
    };
    assert!(!ok(library.search(&plain(), &neither)).local.is_empty());
}

// search.feature: at most 100 local results; the rest are counted and a
// warning says to narrow.
#[test]
fn test_a_search_caps_its_local_results_and_says_so() {
    let mut files: Vec<(String, String)> = (0..105)
        .map(|i| (format!("fragments/hit{i:03}.md"), "x\n".to_string()))
        .collect();
    files.push((ENVELOPE.0.to_string(), ENVELOPE.1.to_string()));
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let library = with(&[("b", &refs)], &[CONFIG]).library();
    let out = library.search(&plain(), &ask("hit", "fragment"));
    assert_eq!(out.notices.len(), 1);
    assert!(
        matches!(&out.notices[0], Notice::Finding { text, .. } if text.starts_with("5 more local match(es)"))
    );
    let found = out.result.unwrap();
    assert_eq!(
        (found.count, found.hidden_local, found.local.len()),
        (100, 5, 100)
    );
    // Exactly the cap is not over it.
    let exact = library.search(&plain(), &ask("hit00", "fragment"));
    assert!(exact.notices.is_empty());
    let found = exact.result.unwrap();
    assert_eq!((found.count, found.hidden_local), (10, 0));
    let full: Vec<(String, String)> = (0..100)
        .map(|i| (format!("fragments/hit{i:03}.md"), "x\n".to_string()))
        .chain([(ENVELOPE.0.to_string(), ENVELOPE.1.to_string())])
        .collect();
    let refs: Vec<(&str, &str)> = full.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let out = with(&[("b", &refs)], &[CONFIG])
        .library()
        .search(&plain(), &ask("hit", "fragment"));
    assert!(out.notices.is_empty());
    assert_eq!(out.result.unwrap().hidden_local, 0);
}

fn say(d: &dyn angzarr_diagnostic::Diagnostic) -> String {
    d.code().to_string()
}

type Files = Vec<(&'static str, &'static str)>;

fn two_servers() -> (Files, Files) {
    (
        vec![ENVELOPE, ("mcp/srv.yaml", "command: one\n")],
        vec![ENVELOPE, ("mcp/srv.yaml", "command: two\n")],
    )
}

// mcp.feature: a server name two bundles declare is withheld from the
// second with a fatal finding; one bundle reached twice is not a clash.
#[test]
fn test_a_server_name_claimed_by_two_bundles_is_a_finding_but_one_bundle_twice_is_not() {
    let (x, y) = two_servers();
    let project: &[(&str, &str)] = &[
        ENVELOPE,
        ("profiles/clash.yaml", "bundles: [x, y]\n"),
        ("profiles/twice-a.yaml", "bundles: [x]\n"),
        ("profiles/twice-b.yaml", "bundles: [x]\n"),
    ];
    let layers = ["schema_version: 8\nagents:\n  twice:\n    llm: claude-code\n    profiles: [twice-a, twice-b]\n"];
    let library = with(&[("project", project), ("x", &x), ("y", &y)], &layers).library();
    let clash = crate::RunRequest {
        profile: "clash".to_string(),
        ..crate::RunRequest::default()
    };
    let plan = ok(library.dry_run(&plain(), &clash)).plan;
    assert_eq!(plan.findings.len(), 1);
    assert!(plan.findings[0]
        .text
        .starts_with("MCP server name \"srv\" is claimed by two different sources"));
    assert!(plan.findings[0].fatal);
    let twice = crate::RunRequest {
        agent: "twice".to_string(),
        ..crate::RunRequest::default()
    };
    assert!(ok(library.dry_run(&plain(), &twice))
        .plan
        .findings
        .is_empty());
}

// agent.feature: the engine behind a label is its configured type; an engine
// named bare is itself; any other name is the built-in default; the label a
// profile prefers is used when the binding names none.
#[test]
fn test_an_agent_resolves_its_engine_from_its_label_or_its_profiles() {
    let project: &[(&str, &str)] = &[
        ENVELOPE,
        ("profiles/plain.yaml", "description: p\n"),
        ("profiles/prefers.yaml", "llm: opencode\n"),
    ];
    let layers = ["schema_version: 8\nagents:\n  bare:\n    llm: opencode\n  weird:\n    llm: weird-label\n  inherits:\n    profiles: [plain, prefers]\n  typed:\n    llm: fancy\nllm:\n  configs:\n    fancy:\n      type: mock\n      model: m1\n"];
    let library = with(&[("project", project)], &layers).library();
    let resolved = |name: &str| {
        ok(library.agent_show(&plain(), name, &say))
            .resolved
            .unwrap_or_else(|| panic!("{name} did not resolve"))
    };
    let bare = resolved("bare");
    assert_eq!(
        (bare.label.as_str(), bare.backend.as_str()),
        ("opencode", "opencode")
    );
    let weird = resolved("weird");
    assert_eq!(
        (weird.label.as_str(), weird.backend.as_str()),
        ("weird-label", "claude-code")
    );
    let inherits = resolved("inherits");
    assert_eq!(inherits.label, "opencode");
    let typed = resolved("typed");
    assert_eq!(
        (
            typed.backend.as_str(),
            typed.model.as_str(),
            typed.effective_permissions.as_str()
        ),
        ("mock", "m1", "")
    );
}

// agent.feature: a bundle's server that is reached by URL or served by the
// session needs nothing on PATH; one that is run needs its command, unless
// the container image stages it.
#[test]
fn test_requirements_name_only_the_commands_a_host_must_have() {
    let files: &[(&str, &str)] = &[
        ENVELOPE,
        ("mcp/run.yaml", "command: tool --flag\n"),
        ("mcp/staged.yaml", "command: ltk serve\n"),
        (
            "mcp/remote.yaml",
            "url: https://x.example/mcp\ncommand: ignored\n",
        ),
        (
            "mcp/endpoint.yaml",
            "served_by: session-endpoint\ncommand: ignored2\n",
        ),
        ("mcp/blank.yaml", "args: [a]\n"),
        ("profiles/all.yaml", "bundles: [project]\n"),
    ];
    let layers = ["schema_version: 8\nagents:\n  a:\n    llm: claude-code\n    profiles: [all]\n"];
    let shown = ok(with(&[("project", files)], &layers)
        .library()
        .agent_show(&plain(), "a", &say));
    let commands: Vec<(&str, &str)> = shown
        .requirements
        .iter()
        .map(|r| (r.command.as_str(), r.sources[0].reference.as_str()))
        .collect();
    assert_eq!(commands, [("tool", "run")]);
    assert!(shown.requirements_error.is_empty());
}

// agent.feature: the permission posture a run takes, in the engine's words.
#[test]
fn test_the_effective_posture_follows_the_binding_then_the_label_then_the_default() {
    let project: &[(&str, &str)] = &[ENVELOPE, ("profiles/p.yaml", "description: p\n")];
    let layers = ["schema_version: 8\npermissions:\n  approver: reviewer\nagents:\n  neutral:\n    llm: claude-code\n    permissions:\n      approver: \"\"\n  foreign:\n    llm: claude-code\n    permissions:\n      opencode:\n        mode: whatever\n  own:\n    llm: claude-code\n    permissions:\n      approver: none\n      claude-code:\n        mode: plan\n  modeless:\n    llm: claude-code\n    permissions:\n      claude-code:\n        other: x\n  labelled:\n    llm: strict\n  labelled-approver:\n    llm: strict\n    permissions:\n      approver: human\n  mocked:\n    llm: mock\nllm:\n  configs:\n    strict:\n      type: claude-code\n      permissions:\n        mode: bypassPermissions\n        approver: none\n"];
    let library = with(&[("project", project)], &layers).library();
    // --degraded keeps a value the schema refuses (an empty approver), so the
    // fall-through past an empty declaration can be seen.
    let degraded = crate::Flags {
        degraded: true,
        ..plain()
    };
    let seen = |name: &str| {
        let r = ok(library.agent_show(&degraded, name, &say))
            .resolved
            .unwrap();
        (r.effective_permissions, r.approver)
    };
    let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
    assert_eq!(seen("neutral"), pair("acceptEdits", "reviewer"));
    assert_eq!(seen("foreign"), pair("", "reviewer"));
    assert_eq!(seen("own"), pair("plan", "none"));
    assert_eq!(seen("modeless"), pair("acceptEdits", "reviewer"));
    assert_eq!(seen("labelled"), pair("bypassPermissions", "none"));
    assert_eq!(
        seen("labelled-approver"),
        pair("bypassPermissions", "human")
    );
    assert_eq!(seen("mocked"), pair("", "reviewer"));
    let bare = Setup {
        ..with(
            &[("project", project)],
            &["schema_version: 8\nagents:\n  a:\n    llm: claude-code\n"],
        )
    }
    .library();
    let r = ok(bare.agent_show(&plain(), "a", &say)).resolved.unwrap();
    assert_eq!(r.approver, "human");
}

// run.feature: workspace and runtime are the flag's or the binding's, else
// the project's, else none and the host.
#[test]
fn test_a_dry_run_settles_the_workspace_and_runtime_axes() {
    let agent = "agents:\n  dev:\n    llm: claude-code\n    runtime: container-rootless\n  plain:\n    llm: claude-code\n";
    let run = |layers: &str, request: crate::RunRequest| {
        let layers = [layers];
        ok(with(&[], &layers).library().dry_run(&plain(), &request))
            .plan
            .axes
    };
    let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
    let dev = crate::RunRequest {
        agent: "dev".to_string(),
        ..crate::RunRequest::default()
    };
    let plain_agent = crate::RunRequest {
        agent: "plain".to_string(),
        ..crate::RunRequest::default()
    };
    let bare = format!("schema_version: 8\n{agent}");
    assert_eq!(run(&bare, dev.clone()), pair("none", "container-rootless"));
    assert_eq!(run(&bare, plain_agent.clone()), pair("none", "host"));
    let configured =
        format!("schema_version: 8\nworkspace: worktree\nruntime: container-rootful\n{agent}");
    assert_eq!(
        run(&configured, dev.clone()),
        pair("worktree", "container-rootless")
    );
    assert_eq!(
        run(&configured, plain_agent.clone()),
        pair("worktree", "container-rootful")
    );
    let flagged = crate::RunRequest {
        workspace: "none".to_string(),
        ..plain_agent
    };
    assert_eq!(run(&configured, flagged), pair("none", "container-rootful"));
}

// run.feature: the prompt is the one given, else the saved command's body.
#[test]
fn test_the_prompt_is_given_or_the_saved_commands_body() {
    let files: &[(&str, &str)] = &[ENVELOPE, ("prompts/saved.md", "SAVED BODY\n")];
    let layers = ["schema_version: 8\nagents:\n  d:\n    llm: claude-code\ndefault_agent: d\n"];
    let library = with(&[("b", files)], &layers).library();
    let prompt = |given: &str, saved: &str| {
        library
            .dry_run(
                &plain(),
                &crate::RunRequest {
                    prompt: given.to_string(),
                    saved_command: saved.to_string(),
                    ..crate::RunRequest::default()
                },
            )
            .result
            .map(|p| p.plan.prompt)
    };
    assert_eq!(prompt("", "").unwrap(), "");
    assert_eq!(prompt("given", "").unwrap(), "given");
    assert_eq!(prompt("given", "saved").unwrap(), "given");
    assert_eq!(prompt("", "saved").unwrap(), "SAVED BODY\n");
    assert!(matches!(
        prompt("", "nosuch"),
        Err(AppError::SavedCommand { .. })
    ));
}

// fault_tolerance.feature: with --degraded the default agent may be missing
// and the run goes on with nothing; any other missing agent is still named.
#[test]
fn test_degraded_tolerates_a_missing_default_agent_only() {
    let degraded = crate::Flags {
        degraded: true,
        ..plain()
    };
    let nothing = with(&[], &[CONFIG]).library();
    let plan = ok(nothing.dry_run(&degraded, &crate::RunRequest::default())).plan;
    assert!(plan.profiles.is_empty());
    assert!(matches!(
        nothing
            .dry_run(&plain(), &crate::RunRequest::default())
            .result,
        Err(AppError::LaunchNoAgents)
    ));
    let named = crate::RunRequest {
        agent: "nosuch".to_string(),
        ..crate::RunRequest::default()
    };
    assert!(matches!(
        nothing.dry_run(&degraded, &named).result,
        Err(AppError::LaunchNoAgent { .. })
    ));
    let ghost = with(&[], &["schema_version: 8\ndefault_agent: ghost\n"]).library();
    assert!(ghost
        .dry_run(&degraded, &crate::RunRequest::default())
        .result
        .is_ok());
    assert!(matches!(
        ghost
            .dry_run(&plain(), &crate::RunRequest::default())
            .result,
        Err(AppError::LaunchNoAgent { .. })
    ));
}

// run.feature: --llm must name a configured label or an engine; the refusal
// lists what is usable: the labels and the real engines, each once, sorted.
#[test]
fn test_an_unknown_llm_lists_the_usable_ones() {
    let layers = ["schema_version: 8\nllm:\n  configs:\n    opencode:\n      type: opencode\n      model: m\n    zed:\n      type: mock\n"];
    let request = crate::RunRequest {
        profile: "x".to_string(),
        llm: "nosuch".to_string(),
        ..crate::RunRequest::default()
    };
    match with(&[], &layers)
        .library()
        .dry_run(&plain(), &request)
        .result
    {
        Err(AppError::UnknownLlm { usable, .. }) => {
            assert!(!usable.contains("mock"), "{usable}");
            let words: Vec<&str> = usable.split(", ").collect();
            let mut sorted = words.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(words, sorted, "sorted, each once: {usable}");
            assert!(
                words.contains(&"claude-code")
                    && words.contains(&"opencode")
                    && words.contains(&"zed")
            );
        }
        other => panic!("{:?}", other.err()),
    }
}

// run.feature: warnings (a placeholder left as written) are said but are not
// the startup gate's findings.
#[test]
fn test_only_failing_findings_are_the_plans_findings() {
    let files: &[(&str, &str)] = &[ENVELOPE, ("fragments/f.md", "hello {{unset}}\n")];
    let library = with(&[("b", files)], &[CONFIG]).library();
    let request = crate::RunRequest {
        fragments: vec!["f".to_string()],
        ..crate::RunRequest::default()
    };
    let out = library.dry_run(&plain(), &request);
    assert_eq!(out.notices.len(), 1, "the warning is said");
    let previewed = out.result.unwrap();
    assert!(previewed.plan.findings.is_empty());
    assert!(previewed.fatal.is_empty());
}

// operations.AssembleContext: with no ask the default agent's profiles are
// assembled; with an ask of any kind they are not.
#[test]
fn test_assembling_with_no_ask_takes_the_default_agents_profiles() {
    let project: &[(&str, &str)] = &[
        ENVELOPE,
        ("profiles/dev.yaml", "bundles: [demo]\n"),
        ("fragments/own.md", "---\ntags: [t]\n---\nOWN\n"),
    ];
    let demo: &[(&str, &str)] = &[ENVELOPE, ("fragments/testing.md", "TEST-BODY\n")];
    let library = with(&[("project", project), ("demo", demo)], &[AGENT_DEV]).library();
    let by_default = ok(library.assemble(&plain(), &crate::AssembleRequest::default()));
    assert_eq!(by_default.profiles, ["dev"]);
    assert!(by_default.context.contains("TEST-BODY"));
    for request in [
        crate::AssembleRequest {
            fragments: vec!["own".to_string()],
            ..crate::AssembleRequest::default()
        },
        crate::AssembleRequest {
            tags: vec!["t".to_string()],
            ..crate::AssembleRequest::default()
        },
    ] {
        let asked = ok(library.assemble(&plain(), &request));
        assert!(asked.profiles.is_empty(), "{:?}", asked.profiles);
        assert!(!asked.context.contains("TEST-BODY"));
    }
    let named = ok(library.assemble(
        &plain(),
        &crate::AssembleRequest {
            profiles: vec!["dev".to_string()],
            ..crate::AssembleRequest::default()
        },
    ));
    assert_eq!(named.profiles, ["dev"]);
}

fn degraded() -> crate::Flags {
    crate::Flags {
        degraded: true,
        ..plain()
    }
}

// fault_tolerance.feature: a value of the wrong shape in any layer makes the
// config unreadable (the defaults alone), even when a later layer mends it.
#[test]
fn test_a_layer_of_the_wrong_shape_reduces_the_config_even_if_a_later_layer_mends_it() {
    let layers = [
        "schema_version: 8\ncompanions: acme\n",
        "schema_version: 8\ncompanions: [beta]\neditor:\n  command: vim\n",
    ];
    let library = with(&[], &layers).library();
    let editor = ok(library.config_show(&degraded(), Some("editor"), false));
    assert_eq!(editor.json, serde_json::json!({}));
    let companions = ok(library.config_show(&degraded(), Some("companions"), false));
    assert_eq!(companions.json, serde_json::json!([]));
    // Mended on its own, the later layer reads.
    let mended = with(&[], &layers[1..]).library();
    let editor = ok(mended.config_show(&plain(), Some("editor"), false));
    assert_eq!(editor.json["command"], "vim");
}

// fault_tolerance.feature: each violation is said as a warning with what to
// do about it: remove an unknown key, or give a known one a value it accepts.
#[test]
fn test_a_violation_is_said_with_its_remedy() {
    let layers = ["schema_version: 8\nzzz: 1\ncompanions: acme\n"];
    let out = with(&[], &layers)
        .library()
        .config_show(&degraded(), None, false);
    let remedies: Vec<&str> = out
        .notices
        .iter()
        .filter_map(|n| match n {
            Notice::Finding { remedy, .. } => Some(remedy.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        remedies,
        [
            "remove or rename the key in config.yaml (ctxloom config edit)",
            "give the key a value the schema accepts, or remove it (ctxloom config edit)",
        ]
    );
}

// fault_tolerance.feature: --degraded drops an unknown key, at the top and
// below a section, and reads on.
#[test]
fn test_degraded_drops_unknown_keys_at_any_depth() {
    let layers = ["schema_version: 8\nzzz: 1\neditor:\n  command: vim\n  nonsense: 2\n"];
    let library = with(&[], &layers).library();
    let shown = ok(library.config_show(&degraded(), None, true));
    assert!(shown.json.get("zzz").is_none(), "{}", shown.json);
    assert_eq!(shown.json["editor"], serde_json::json!({"command": "vim"}));
}

// merge.go dropEnginelessAgents: an agent bound by profiles alone is kept,
// one with an empty list of profiles and no llm is dropped.
#[test]
fn test_an_agent_is_kept_when_profiles_alone_bind_it() {
    let layers = ["schema_version: 8\nagents:\n  by-profiles:\n    profiles: [p]\n  empty:\n    profiles: []\n    runtime: host\n"];
    let out = with(&[], &layers).library().agent_list(&plain());
    assert_eq!(out.notices.len(), 1);
    let rows = out.result.unwrap();
    assert_eq!(
        rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["by-profiles"]
    );
}

// config.feature: env_host is read as a boolean, absent when unset.
#[test]
fn test_an_agents_env_host_is_true_false_or_unset() {
    let layers = ["schema_version: 8\nagents:\n  on:\n    llm: claude-code\n    env_host: true\n  off:\n    llm: claude-code\n    env_host: false\n  unset:\n    llm: claude-code\n"];
    let rows = ok(with(&[], &layers).library().agent_list(&plain()));
    let by = |name: &str| rows.iter().find(|r| r.name == name).unwrap().env_host;
    assert_eq!(
        (by("on"), by("off"), by("unset")),
        (Some(true), Some(false), None)
    );
}

// config.feature: the sections `config show` accepts.
#[test]
fn test_the_config_sections_are_the_schemas() {
    let sections = crate::Library::config_sections();
    assert!(sections.contains(&"editor") && sections.contains(&"companions"));
    assert!(sections.len() > 10);
    assert!(sections.iter().all(|s| !s.is_empty() && *s != "xyzzy"));
}
