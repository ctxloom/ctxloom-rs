//! Ported from Go's `internal/core/profiles` tests (`profiles_test.go`,
//! `schema_validation_test.go`, `fragment_ref_empty_test.go`,
//! `own_repo_test.go`, `grammar_test.go`) at 860336c4e: profile resolution
//! (parents first, depth 64, cycles refused, diamonds memoized, exclusions
//! accumulate, variables, `llm` inheritance, cross-repo refused).

use std::collections::BTreeMap;

use super::*;

angzarr_presentation::english_for!(&[crate::EN_US.1]);

/// Profiles by canonical key, as the project bundle holds them.
struct Source(BTreeMap<String, Profile>);

fn key(name: &str) -> String {
    format!("ctxloom+local:project#profiles/{name}")
}

fn source(profiles: &[(&str, &str)]) -> Source {
    Source(
        profiles
            .iter()
            .map(|(name, text)| {
                let mut profile = Profile::decode(text).unwrap_or_else(|e| panic!("{name}: {e}"));
                profile.name = key(name);
                (key(name), profile)
            })
            .collect(),
    )
}

impl ProfileSource for Source {
    fn load(&self, key: &str) -> Result<Profile, LoadError> {
        self.0.get(key).cloned().ok_or(LoadError::LocalNotFound {
            name: key.to_string(),
        })
    }
}

fn resolved(src: &Source, name: &str) -> ResolvedProfile {
    let mut findings = Vec::new();
    resolve(src, name, &mut findings).unwrap_or_else(|e| panic!("{name}: {}", e.english()))
}

fn strings(of: &[&str]) -> Vec<String> {
    of.iter().map(ToString::to_string).collect()
}

// profiles_test.go: TestLoader_ResolveProfile
#[test]
fn test_resolution_takes_parents_first_and_the_child_overrides_variables() {
    let src = source(&[
        (
            "parent",
            "description: Parent profile\nbundles:\n  - parent-bundle\ntags:\n  - parent-tag\nvariables:\n  inherited: parent-value\n  override_me: parent-value\n",
        ),
        (
            "child",
            "description: Child profile\nparents:\n  - parent\nbundles:\n  - child-bundle\ntags:\n  - child-tag\nvariables:\n  override_me: child-value\n  new_var: child-only\n",
        ),
    ]);
    let r = resolved(&src, "child");
    assert_eq!(r.bundles, strings(&["parent-bundle", "child-bundle"]));
    assert_eq!(r.tags, strings(&["parent-tag", "child-tag"]));
    assert_eq!(r.variables["override_me"], "child-value");
    assert_eq!(r.variables["inherited"], "parent-value");
    assert_eq!(r.variables["new_var"], "child-only");
}

// profiles_test.go: TestLoader_ResolveProfile_Commands
#[test]
fn test_curated_commands_union_with_the_parents_in_declaration_order() {
    let src = source(&[
        ("parent", "description: Parent profile\ncommands:\n  - \"tools#commands/review\"\n"),
        (
            "child",
            "description: Child profile\nparents:\n  - parent\ncommands:\n  - \"tools#commands/explain\"\n  - \"tools#commands/review\"\n",
        ),
    ]);
    assert_eq!(
        resolved(&src, "child").commands,
        strings(&["tools#commands/review", "tools#commands/explain"])
    );
}

// profiles_test.go: TestLoader_ResolveProfile_LLM
#[test]
fn test_a_childs_llm_wins_and_a_child_without_one_inherits() {
    let src = source(&[
        ("parent", "llm: agy-code\nbundles:\n  - parent-bundle\n"),
        ("overrider", "parents:\n  - parent\nllm: claude-fast\n"),
        (
            "inheritor",
            "parents:\n  - parent\nbundles:\n  - child-bundle\n",
        ),
    ]);
    assert_eq!(
        resolved(&src, "overrider").llm,
        "claude-fast",
        "child llm should win over parent"
    );
    assert_eq!(
        resolved(&src, "inheritor").llm,
        "agy-code",
        "child should inherit parent llm"
    );
}

// profiles_test.go: TestLoader_ResolveProfile_CircularReference
#[test]
fn test_a_cycle_is_refused() {
    let src = source(&[("a", "parents:\n  - b\n"), ("b", "parents:\n  - a\n")]);
    let err = resolve(&src, "a", &mut Vec::new()).expect_err("A to B to A");
    assert!(err.english().contains("circular"), "{}", err.english());
    // A profile that is its own parent is the same cycle.
    let selfish = source(&[("selfish", "parents:\n  - selfish\nbundles:\n  - x\n")]);
    let err = resolve(&selfish, "selfish", &mut Vec::new()).expect_err("its own parent");
    assert!(err
        .english()
        .contains("circular profile inheritance detected"));
}

// profiles_test.go: TestLoader_ResolveProfile_NotFound
#[test]
fn test_a_profile_that_is_not_there_is_refused() {
    let src = source(&[]);
    let err = resolve(&src, "nonexistent", &mut Vec::new()).expect_err("not there");
    assert!(
        err.english().contains("profile not found"),
        "{}",
        err.english()
    );
}

// profiles_test.go: TestLoader_ResolveProfile_ParentNotFound: an
// unresolvable parent is skipped, not fatal to the resolution, and said.
#[test]
fn test_a_parent_that_is_not_there_is_skipped_and_said() {
    let src = source(&[(
        "child",
        "parents:\n  - nonexistent-parent\nbundles:\n  - own-bundle\n",
    )]);
    let mut findings = Vec::new();
    let r = resolve(&src, "child", &mut findings).expect("the child still resolves");
    assert_eq!(r.bundles, strings(&["own-bundle"]));
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].kind, "ref");
    assert_eq!(
        findings[0].class,
        crate::findings::Class::Fail,
        "fatal unless --degraded"
    );
    assert_eq!(
        findings[0].text,
        "profile \"ctxloom+local:project#profiles/child\": parent nonexistent-parent does not exist; skipping"
    );
    assert!(findings[0]
        .remedy
        .contains("`ctxloom profile list` names every profile you can use"));
}

// profiles_test.go: TestLoader_ResolveProfile_DiamondInheritance
#[test]
fn test_a_diamond_is_not_a_cycle() {
    let src = source(&[
        ("d", "description: Base profile D\nbundles:\n  - d-bundle\ntags:\n  - d-tag\nvariables:\n  from_d: d-value\n"),
        ("b", "description: Profile B\nparents:\n  - d\nbundles:\n  - b-bundle\ntags:\n  - b-tag\nvariables:\n  from_b: b-value\n"),
        ("c", "description: Profile C\nparents:\n  - d\nbundles:\n  - c-bundle\ntags:\n  - c-tag\nvariables:\n  from_c: c-value\n"),
        ("a", "description: Profile A\nparents:\n  - b\n  - c\nbundles:\n  - a-bundle\ntags:\n  - a-tag\nvariables:\n  from_a: a-value\n"),
    ]);
    let r = resolved(&src, "a");
    for bundle in ["d-bundle", "b-bundle", "c-bundle", "a-bundle"] {
        assert!(
            r.bundles.contains(&bundle.to_string()),
            "{bundle} in {:?}",
            r.bundles
        );
    }
    assert_eq!(
        r.bundles,
        strings(&["d-bundle", "b-bundle", "c-bundle", "a-bundle"])
    );
    for tag in ["d-tag", "b-tag", "c-tag", "a-tag"] {
        assert!(r.tags.contains(&tag.to_string()), "{tag}");
    }
    for (name, value) in [
        ("from_d", "d-value"),
        ("from_b", "b-value"),
        ("from_c", "c-value"),
        ("from_a", "a-value"),
    ] {
        assert_eq!(r.variables[name], value);
    }
}

// profiles_test.go: TestLoader_ResolveProfile_DuplicateParentChainIsMemoized:
// pN declares p(N+1) as its parent TWICE, to depth 24. Unmemoized this is
// 2^24 resolutions; each profile is resolved once.
#[test]
fn test_a_duplicate_parent_chain_is_resolved_once_per_profile() {
    let depth = 24;
    let mut profiles = Vec::new();
    for n in 0..=depth {
        let text = if n == depth {
            "description: leaf\nbundles: [leaf-bundle]\n".to_string()
        } else {
            let next = format!("p{:03}", n + 1);
            format!("parents: [{next}, {next}]\nbundles: [b{n}]\n")
        };
        profiles.push((format!("p{n:03}"), text));
    }
    let refs: Vec<(&str, &str)> = profiles
        .iter()
        .map(|(n, t)| (n.as_str(), t.as_str()))
        .collect();
    let src = source(&refs);
    let r = resolved(&src, "p000");
    assert!(r.bundles.contains(&"leaf-bundle".to_string()));
    assert!(r.bundles.contains(&"b0".to_string()));
    assert_eq!(
        r.bundles.len(),
        depth + 1,
        "every distinct bundle once: {:?}",
        r.bundles
    );
}

// profiles_test.go: TestLoader_ResolveProfile_DepthLimit: 70 profiles chained
// p0 <- p1 <- ... <- p69 exceed the limit of 64.
#[test]
fn test_a_chain_past_sixty_four_is_refused() {
    let mut profiles = Vec::new();
    for i in 0..70 {
        let text = if i == 0 {
            "description: Base profile".to_string()
        } else {
            format!("parents:\n  - p{:02}", i - 1)
        };
        profiles.push((format!("p{i:02}"), text));
    }
    let refs: Vec<(&str, &str)> = profiles
        .iter()
        .map(|(n, t)| (n.as_str(), t.as_str()))
        .collect();
    let src = source(&refs);
    let err = resolve(&src, "p69", &mut Vec::new()).expect_err("70 levels");
    assert!(
        err.english().contains("depth exceeds maximum"),
        "{}",
        err.english()
    );
    // The limit is exact: 65 profiles are 64 parents deep and resolve.
    let ok = resolve(&src, "p64", &mut Vec::new());
    assert!(
        ok.is_ok(),
        "a chain of 64 parents resolves: {:?}",
        ok.err().map(|e| e.english())
    );
    let over = resolve(&src, "p65", &mut Vec::new());
    assert!(over.is_err(), "a chain of 65 parents does not");
}

// profiles_test.go: TestResolvedProfile_Merge
#[test]
fn test_merge_unions_lists_and_the_first_variable_wins() {
    let mut r1 = ResolvedProfile {
        bundles: strings(&["b1"]),
        tags: strings(&["t1"]),
        select_tags: strings(&["s1"]),
        commands: strings(&["p1"]),
        variables: BTreeMap::from([
            ("v1".into(), "value1".into()),
            ("shared".into(), "r1".into()),
        ]),
        ..ResolvedProfile::default()
    };
    let r2 = ResolvedProfile {
        bundles: strings(&["b2", "b1"]),
        tags: strings(&["t2"]),
        select_tags: strings(&["s2"]),
        commands: strings(&["p2", "p1"]),
        variables: BTreeMap::from([
            ("v2".into(), "value2".into()),
            ("shared".into(), "r2".into()),
        ]),
        ..ResolvedProfile::default()
    };
    r1.merge(&r2);
    assert_eq!(r1.bundles, strings(&["b1", "b2"]));
    assert_eq!(r1.tags, strings(&["t1", "t2"]));
    assert_eq!(r1.select_tags, strings(&["s1", "s2"]));
    assert_eq!(r1.commands, strings(&["p1", "p2"]));
    assert_eq!(
        r1.variables["shared"], "r1",
        "r1 keeps its value for a shared variable"
    );
    assert_eq!(r1.variables["v1"], "value1");
    assert_eq!(r1.variables["v2"], "value2");
}

// profiles_test.go: TestAppendUnique
#[test]
fn test_append_unique() {
    let cases: [(&[&str], &[&str], &[&str]); 4] = [
        (&[], &["a", "b"], &["a", "b"]),
        (&["a"], &["b", "c"], &["a", "b", "c"]),
        (&["a", "b"], &["b", "c", "a"], &["a", "b", "c"]),
        (&["a", "b"], &["a", "b"], &["a", "b"]),
    ];
    for (slice, items, want) in cases {
        let mut got = strings(slice);
        append_unique(&mut got, &strings(items));
        assert_eq!(got, strings(want), "{slice:?} + {items:?}");
    }
}

// profiles_test.go: TestAppendUniqueRefs_CollapsesSpellingsKeepsPinsAndItems
#[test]
fn test_append_unique_refs_collapses_spellings_and_keeps_pins_and_items() {
    let canonical = "ctxloom+git://example.test/repo//bundles/x";
    let legacy = "https://example.test/repo@bundles/x";
    let with = |slice: &[&str], items: &[&str]| {
        let mut got = strings(slice);
        append_unique_refs(&mut got, &strings(items));
        got
    };
    assert_eq!(with(&[canonical], &[legacy]), strings(&[canonical]));
    assert_eq!(with(&[], &[legacy, canonical]), strings(&[legacy]));
    assert_eq!(
        with(&[canonical], &[&format!("{legacy}@v2")]).len(),
        2,
        "a different version pin is not the same ref"
    );
    let a = format!("{canonical}#fragments/a");
    let legacy_b = format!("{legacy}#fragments/b");
    let legacy_a = format!("{legacy}#fragments/a");
    assert_eq!(
        with(&[&a], &[&legacy_b, &legacy_a]),
        strings(&[&a, &legacy_b])
    );
    assert_eq!(
        with(&["a"], &["b", "a"]),
        strings(&["a", "b"]),
        "local names dedupe as written"
    );
}

// profiles_test.go: TestLoader_ResolveProfile_Exclusions
#[test]
fn test_exclusions_accumulate_through_the_parent_chain() {
    let src = source(&[
        ("parent", "bundles:\n  - shared-bundle\nexclude_fragments:\n  - verbose-logging\nexclude_mcp:\n  - slow-server\n"),
        ("child", "parents:\n  - parent\nexclude_fragments:\n  - deprecated-style\n"),
    ]);
    let r = resolved(&src, "child");
    let mut excluded = r.exclude_fragments.clone();
    excluded.sort();
    assert_eq!(excluded, strings(&["deprecated-style", "verbose-logging"]));
    assert_eq!(r.exclude_mcp, strings(&["slow-server"]));
}

// profiles_test.go: TestLoader_ResolveProfile_DenyTools
#[test]
fn test_deny_tools_accumulate_and_dedupe() {
    let src = source(&[
        ("parent", "deny_tools:\n  - Task\n"),
        (
            "child",
            "parents:\n  - parent\ndeny_tools:\n  - Task\n  - WebFetch\n",
        ),
    ]);
    let mut denied = resolved(&src, "child").deny_tools;
    denied.sort();
    assert_eq!(denied, strings(&["Task", "WebFetch"]));
}

// profiles_test.go: TestLoader_ResolveProfile_InlineFields
#[test]
fn test_inline_fields_union_with_the_parents_and_pins_stay_in_the_name() {
    let src = source(&[
        (
            "parent",
            "description: Parent\nfragments:\n  - base-frag\nbundle_items:\n  - remote/b:fragments/x\nhooks:\n  unified:\n    pre_tool:\n      - command: parent-hook\n        type: command\n",
        ),
        (
            "child",
            "description: Child\nparents:\n  - parent\nfragments:\n  - \"remote/b@c1#fragments/pinned\"\n  - name: pri-frag\n    priority: 7\nbundle_items:\n  - remote/b:fragments/y\nhooks:\n  unified:\n    pre_tool:\n      - command: child-hook\n        type: command\n",
        ),
    ]);
    let r = resolved(&src, "child");
    let names: Vec<&str> = r.fragments.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["base-frag", "remote/b@c1#fragments/pinned", "pri-frag"]
    );
    assert_eq!(
        r.fragments[2].priority, 7,
        "child fragment priority is preserved"
    );
    assert_eq!(
        r.bundle_items,
        strings(&["remote/b:fragments/x", "remote/b:fragments/y"])
    );
    let commands: Vec<&str> = r.hooks.unified["pre_tool"]
        .iter()
        .map(|h| h.get("command").and_then(|c| c.as_str()).unwrap_or(""))
        .collect();
    assert_eq!(commands, ["parent-hook", "child-hook"]);
}

// profiles_test.go: TestResolvedProfile_Merge_InlineFields: a fragment
// already there only has its priority raised, never lowered.
#[test]
fn test_merging_fragments_dedupes_by_name_and_only_raises_priority() {
    let mut r1 = ResolvedProfile {
        fragments: vec![
            FragmentRef {
                name: "shared".into(),
                priority: 2,
            },
            FragmentRef {
                name: "only-1".into(),
                priority: 0,
            },
        ],
        ..ResolvedProfile::default()
    };
    let r2 = ResolvedProfile {
        fragments: vec![
            FragmentRef {
                name: "shared".into(),
                priority: 9,
            },
            FragmentRef {
                name: "only-2".into(),
                priority: 1,
            },
        ],
        ..ResolvedProfile::default()
    };
    r1.merge(&r2);
    let got: Vec<(&str, i64)> = r1
        .fragments
        .iter()
        .map(|f| (f.name.as_str(), f.priority))
        .collect();
    assert_eq!(got, [("shared", 9), ("only-1", 0), ("only-2", 1)]);
}

// profiles_test.go: TestResolvedProfile_Merge_IdenticalHookCollapses
#[test]
fn test_an_identical_hook_collapses_when_hooks_merge() {
    let hook = Profile::decode(
        "hooks:\n  unified:\n    pre_tool:\n      - command: same\n        type: command\n",
    )
    .unwrap()
    .hooks;
    let mut merged = hook.clone();
    merged.merge(&hook);
    assert_eq!(merged.unified["pre_tool"].len(), 1);
}

// grammar_test.go: TestLoad_SelectorlessNameIsTheProjectBundleProfile and
// docs/reference-grammar.md "Profile references": a bare name is the project
// bundle's profile; `bundle#profiles/<name>` is a local bundle's.
#[test]
fn test_a_bare_profile_name_is_the_project_bundles_profile() {
    assert_eq!(
        canonical_key("developer").unwrap(),
        "ctxloom+local:project#profiles/developer"
    );
    assert_eq!(
        canonical_key("tools#profiles/probe").unwrap(),
        "ctxloom+local:tools#profiles/probe"
    );
    assert_eq!(
        canonical_key("ctxloom+local:tools#profiles/probe").unwrap(),
        "ctxloom+local:tools#profiles/probe"
    );
}

// grammar_test.go: TestValidateProfileName_HashReserved
#[test]
fn test_a_hash_is_reserved_in_a_profile_name() {
    assert!(canonical_key("developer").is_ok());
    // `x#profiles/y` is a selector, not a name; a name holding `#` that is no
    // selector is the refusal.
    let err = canonical_key("weird#name").expect_err("weird#name");
    assert!(err.english().contains("reserved"), "{}", err.english());
}

// grammar_test.go: TestLoad_ExplicitLocalMiss_HasNoPullHint and the local
// source's contract: a miss in the project is a plain not-found, naming it.
#[test]
fn test_a_local_miss_names_the_profile_asked_for() {
    let err = resolve(&source(&[]), "nope", &mut Vec::new()).expect_err("nope");
    assert_eq!(
        err.english(),
        "profile not found: ctxloom+local:project#profiles/nope"
    );
}

// schema_validation_test.go: TestDecode_WellFormedProfile
#[test]
fn test_a_well_formed_profile_decodes() {
    let p = Profile::decode(
        "description: fine\nllm: big\nparents: [base]\ntags: [t]\nselect_tags: [go]\nbundles: [b]\nbundle_items: [\"b#fragments/x\"]\nfragments:\n  - plain\n  - name: pri\n    priority: 3\ncommands: [\"b#commands/c\"]\nskills: [\"b#skills/s\"]\nvariables:\n  K: v\nexclude_fragments: [ef]\nexclude_mcp: [em]\ndeny_tools: [Bash]\nhooks:\n  unified:\n    pre_tool:\n      - command: x\n        type: command\n",
    )
    .expect("every key here is real; a gate that refuses a correct profile is worse than none");
    assert_eq!(p.llm, "big");
    assert_eq!(
        p.fragments,
        [
            FragmentRef {
                name: "plain".into(),
                priority: 0
            },
            FragmentRef {
                name: "pri".into(),
                priority: 3
            },
        ]
    );
    assert_eq!(p.variables["K"], "v");
    assert_eq!(p.deny_tools, strings(&["Bash"]));
}

// schema_validation_test.go: TestDecode_SchemaViolationIsRefused (the
// violations a text reader can see: an unknown key, an empty entry).
#[test]
fn test_a_profile_that_breaks_the_schema_is_refused_naming_the_violation() {
    let err = Profile::decode("select_tagz: [go]\nbundles: [real]\n").expect_err("unknown key");
    assert!(err.english().contains("select_tagz"), "{}", err.english());
    let err = Profile::decode("bundles:\n  - \n  - real\n").expect_err("empty list entry");
    assert!(err.english().contains("bundles"), "{}", err.english());
    let err = Profile::decode("fragments:\n  - \n  - real\n").expect_err("empty fragments entry");
    assert!(
        err.english().contains("must name a fragment"),
        "{}",
        err.english()
    );
}

// schema_validation_test.go: TestDecode_UnknownKey_SuggestsTheNearKey
#[test]
fn test_an_unknown_key_suggests_the_near_key() {
    let err = Profile::decode("select_tagz: [go]\nbundles: [real]\n").expect_err("top level");
    assert!(err.english().contains("select_tagz"));
    assert!(err.english().contains("`select_tags`"), "{}", err.english());
    let err = Profile::decode("fragments:\n  - name: pri\n    priorty: 3\n")
        .expect_err("fragments entry");
    assert!(err.english().contains("priorty"));
    assert!(err.english().contains("`priority`"), "{}", err.english());
}

// schema_validation_test.go: TestDecode_DoesNotRespellRefs
#[test]
fn test_decode_does_not_respell_refs() {
    let reference = "https://github.com/o/r@bundles/core";
    let p = Profile::decode(&format!(
        "bundles:\n  - {reference}\nparents:\n  - {reference}#profiles/base\n"
    ))
    .unwrap();
    assert_eq!(p.bundles, strings(&[reference]));
    assert_eq!(p.parents, strings(&[&format!("{reference}#profiles/base")]));
}

// fragment_ref_empty_test.go: TestProfile_EmptyFragmentEntryIsRefused and
// TestProfile_NonEmptyFragmentEntriesStillDecode
#[test]
fn test_an_empty_fragment_entry_is_refused_and_a_named_one_decodes() {
    for (what, text) in [
        ("empty scalar", "fragments:\n  - \"\"\n"),
        ("whitespace scalar", "fragments:\n  - \"   \"\n"),
        ("bare null item", "fragments:\n  - \n"),
        ("struct empty name", "fragments:\n  - name: \"\"\n"),
    ] {
        let err = Profile::decode(text).expect_err(what);
        assert!(
            err.english().contains("must name a fragment"),
            "{what}: {}",
            err.english()
        );
    }
    let p = Profile::decode("fragments:\n  - plain\n  - name: with-priority\n    priority: 5\n")
        .unwrap();
    assert_eq!(p.fragments.len(), 2);
    assert_eq!(
        (p.fragments[0].name.as_str(), p.fragments[0].priority),
        ("plain", 0)
    );
    assert_eq!(
        (p.fragments[1].name.as_str(), p.fragments[1].priority),
        ("with-priority", 5)
    );
}

// profiles_test.go: TestPriorityOrdering (content/tree_test.go
// TestProfile_PriorityOrderingRoundTrips): duplicates and a negative
// priority survive decoding as written.
#[test]
fn test_priorities_survive_decoding_in_order_with_duplicates() {
    let p = Profile::decode(
        "description: Strict review profile\nparents:\n  - base\ntags:\n  - review\nfragments:\n  - solid\n  - name: tricky\n    priority: 10\n  - name: solid\n    priority: -5\nbundles:\n  - code-quality#fragments/solid\n",
    )
    .unwrap();
    let got: Vec<(&str, i64)> = p
        .fragments
        .iter()
        .map(|f| (f.name.as_str(), f.priority))
        .collect();
    assert_eq!(got, [("solid", 0), ("tricky", 10), ("solid", -5)]);
}

// own_repo_test.go: TestCheckOwnRepo
#[test]
fn test_a_remote_profile_may_name_only_its_own_repository() {
    let own_repo = "https://github.com/acme/tools";
    let other_repo = "https://github.com/evil/elsewhere";
    let own = format!("{own_repo}@bundles/kit");
    let other = format!("{other_repo}@bundles/payload");
    let build = |f: &dyn Fn(&mut Profile)| {
        let mut p = Profile {
            name: format!("{own}#profiles/dev"),
            ..Profile::default()
        };
        f(&mut p);
        p
    };
    // Own bundles, own parent, own items.
    let fine = build(&|p| {
        p.bundles = vec![
            own.clone(),
            "git@github.com:acme/tools@bundles/other".into(),
        ];
        p.parents = vec![format!("{own}#profiles/base")];
        p.bundle_items = vec![format!("{own}#fragments/f")];
        p.commands = vec![format!("{own}#commands/c")];
        p.skills = vec![format!("{own}#skills/s")];
        p.fragments = vec![FragmentRef {
            name: format!("{own}#fragments/g"),
            priority: 0,
        }];
    });
    fine.check_own_repo(own_repo).expect("own content");

    let breaking: Vec<(&str, Profile, String)> = vec![
        (
            "bundle from another repo",
            build(&|p| p.bundles = vec![own.clone(), other.clone()]),
            other.clone(),
        ),
        (
            "parent from another repo",
            build(&|p| p.parents = vec![format!("{other}#profiles/base")]),
            format!("{other}#profiles/base"),
        ),
        (
            "bundle item from another repo",
            build(&|p| p.bundle_items = vec![format!("{other}#fragments/f")]),
            format!("{other}#fragments/f"),
        ),
        (
            "command from another repo",
            build(&|p| p.commands = vec![format!("{other}#commands/c")]),
            format!("{other}#commands/c"),
        ),
        (
            "skill from another repo",
            build(&|p| p.skills = vec![format!("{other}#skills/s")]),
            format!("{other}#skills/s"),
        ),
        (
            "fragment from another repo",
            build(&|p| {
                p.fragments = vec![FragmentRef {
                    name: format!("{other}#fragments/g"),
                    priority: 0,
                }]
            }),
            format!("{other}#fragments/g"),
        ),
        (
            "the consumer's local content",
            build(&|p| p.bundles = vec!["ctxloom:local@bundles/kit".into()]),
            "ctxloom:local@bundles/kit".to_string(),
        ),
    ];
    for (what, profile, bad) in breaking {
        let err = profile.check_own_repo(own_repo).expect_err(what);
        let said = err.english();
        assert!(
            said.contains("a remote profile may refer only to bundles in its own repository"),
            "{what}: {said}"
        );
        assert!(
            said.contains(&profile.name),
            "{what}: the refusal names the profile: {said}"
        );
        assert!(
            said.contains(&bad),
            "{what}: the refusal names the reference: {said}"
        );
        profile
            .check_own_repo("")
            .expect("a local profile may name any repository");
    }
}

fn hook(text: &str) -> Value {
    Value::Scalar(text.to_string())
}

// profiles.go HooksConfig.HasAny: a hook anywhere, neutral or an engine's own
#[test]
fn test_hooks_config_knows_whether_it_declares_a_hook() {
    let mut hooks = HooksConfig::default();
    assert!(!hooks.has_any());
    hooks.unified.insert("pre_tool".into(), Vec::new());
    assert!(!hooks.has_any(), "an event with no hooks declares none");
    hooks
        .ext
        .entry("claude-code".into())
        .or_default()
        .insert("x".into(), Vec::new());
    assert!(!hooks.has_any());
    let mut unified = hooks.clone();
    unified.unified.insert("pre_tool".into(), vec![hook("a")]);
    assert!(unified.has_any());
    let mut ext = hooks.clone();
    ext.ext
        .entry("claude-code".into())
        .or_default()
        .insert("x".into(), vec![hook("a")]);
    assert!(ext.has_any());
}

// profiles.go Profile.HasContent: any selecting key makes content; tags,
// description and the like alone do not
#[test]
fn test_a_profile_has_content_when_any_selecting_key_is_set() {
    assert!(!Profile::decode("").unwrap().has_content());
    assert!(!Profile::decode("description: d\ntags: [t]\n")
        .unwrap()
        .has_content());
    for text in [
        "parents: [p]\n",
        "select_tags: [t]\n",
        "bundles: [b]\n",
        "commands: [c]\n",
        "skills: [s]\n",
        "fragments: [f]\n",
        "bundle_items: [\"b#fragments/x\"]\n",
        "exclude_fragments: [f]\n",
        "exclude_mcp: [m]\n",
        "deny_tools: [Bash]\n",
        "variables:\n  K: v\n",
        "hooks:\n  unified:\n    pre_tool:\n      - command: x\n",
        "llm: big\n",
    ] {
        assert!(Profile::decode(text).unwrap().has_content(), "{text:?}");
    }
}

// fragment_ref_empty_test.go: the refusal names the line of the empty entry
#[test]
fn test_an_empty_fragment_entry_names_its_line() {
    let text = "description: x\nfragments:\n  - a\n  - b\n  - \n";
    match Profile::decode(text) {
        Err(ProfileError::EmptyFragment { line }) => assert_eq!(line, 5),
        other => panic!("expected the empty entry, got {:?}", other.map(|_| ())),
    }
}

// schema_validation_test.go: an unknown key names its line, at the top and
// under hooks
#[test]
fn test_an_unknown_key_names_its_line() {
    match Profile::decode("description: a\nbogus: 1\n") {
        Err(ProfileError::UnknownKey { line, key, .. }) => {
            assert_eq!((line, key.as_str()), (2, "bogus"))
        }
        other => panic!("expected an unknown key, got {:?}", other.map(|_| ())),
    }
    match Profile::decode("hooks:\n  unified: {}\n  bogus: 1\n") {
        Err(ProfileError::UnknownKey {
            line, key, valid, ..
        }) => {
            assert_eq!(
                (line, key.as_str(), valid.as_str()),
                (3, "bogus", "ext, unified")
            );
        }
        other => panic!("expected an unknown key, got {:?}", other.map(|_| ())),
    }
}

// profiles.go: hooks may be written with nothing under an event, an engine or
// `ext`; a non-mapping `ext` is refused
#[test]
fn test_hooks_with_nothing_under_them_decode_and_a_scalar_ext_is_refused() {
    let p = Profile::decode(
        "hooks:\n  unified:\n    pre_tool:\n  ext:\n    claude-code:\n    codex:\n      post_tool:\n",
    )
    .expect("empty hook sections");
    assert_eq!(p.hooks.unified["pre_tool"], Vec::<Value>::new());
    assert!(p.hooks.ext["claude-code"].is_empty());
    assert!(p.hooks.ext["codex"]["post_tool"].is_empty());
    assert!(Profile::decode("hooks:\n  ext:\n").is_ok());
    let err = Profile::decode("hooks:\n  ext: nope\n").expect_err("a scalar ext");
    assert!(err.english().contains("hooks.ext"), "{}", err.english());
    let err =
        Profile::decode("hooks:\n  unified:\n    pre_tool: nope\n").expect_err("a scalar list");
    assert!(
        err.english().contains("hooks.unified.pre_tool"),
        "{}",
        err.english()
    );
}

// profiles.go: a name that cannot be a profile's is refused
#[test]
fn test_a_name_that_cannot_be_a_profiles_is_refused() {
    for bad in ["", "a/b", "a\\b", ".", "..", "x#notprofiles/y"] {
        assert!(
            matches!(canonical_key(bad), Err(ProfileError::BadName { .. })),
            "{bad:?}"
        );
    }
    assert_eq!(
        canonical_key("dev"),
        Ok("ctxloom+local:project#profiles/dev".to_string())
    );
}

// profiles_test.go: the profile that resolves carries the name it was asked for
#[test]
fn test_a_resolved_profile_carries_the_name_asked() {
    let src = source(&[("base", "bundles: [b]\n"), ("dev", "parents: [base]\n")]);
    assert_eq!(resolved(&src, "dev").name, key("dev"));
}

/// A source that lists its profiles, for the "did you mean".
struct Listing(Source, Vec<String>);

impl ProfileSource for Listing {
    fn load(&self, key: &str) -> Result<Profile, LoadError> {
        self.0.load(key)
    }
    fn keys(&self) -> Vec<String> {
        self.1.clone()
    }
}

// profiles_test.go: a missing local parent that exactly one other bundle's
// profile answers to is offered; several, or none, are not
#[test]
fn test_a_missing_parent_offers_the_one_profile_that_answers_to_its_name() {
    let child = source(&[("child", "parents: [base]\n")]);
    let ask = |keys: &[&str]| {
        let listing = Listing(
            Source(child.0.clone()),
            keys.iter().map(ToString::to_string).collect(),
        );
        let mut findings = Vec::new();
        resolve(&listing, "child", &mut findings).expect("resolves");
        findings.remove(0).remedy
    };
    let one = ask(&[
        "ctxloom+local:other#profiles/base",
        "ctxloom+local:other#profiles/else",
    ]);
    assert!(
        one.starts_with("did you mean ctxloom+local:other#profiles/base? Another bundle's profile is named by its full ref;"),
        "{one}"
    );
    let two = ask(&[
        "ctxloom+local:a#profiles/base",
        "ctxloom+local:b#profiles/base",
    ]);
    assert!(
        two.starts_with("`ctxloom profile list` names every profile you can use;"),
        "{two}"
    );
    let none = ask(&[]);
    assert!(none.starts_with("`ctxloom profile list`"), "{none}");
    assert!(Source(BTreeMap::new()).keys().is_empty());
}
