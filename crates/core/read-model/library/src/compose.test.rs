//! Ported from Go's `internal/core/composite` tests (`select_test.go`,
//! `order_test.go`, `ingest_test.go`, `assemble_test.go`) at 860336c4e, on
//! the same corpus (`testdata::corpus`: "alpha" and "beta").

use super::*;
use crate::catalog::item_ref;
use crate::refs::ItemKind;
use crate::testdata::corpus;

const ALPHA_RULES: &str = "ctxloom+local:alpha#fragments/rules";
const ALPHA_STYLE: &str = "ctxloom+local:alpha#fragments/style";
const ALPHA_MAYBE: &str = "ctxloom+local:alpha#fragments/maybe";
const BETA_TAGGED: &str = "ctxloom+local:beta#fragments/tagged";

fn ask(name: &str, priority: i64) -> FragmentAsk {
    FragmentAsk {
        name: name.to_string(),
        version: String::new(),
        priority,
    }
}

fn names(asks: &[FragmentAsk]) -> Vec<&str> {
    asks.iter().map(|a| a.name.as_str()).collect()
}

fn strings(of: &[&str]) -> Vec<String> {
    of.iter().map(ToString::to_string).collect()
}

fn profile(f: impl FnOnce(&mut ResolvedProfile)) -> ResolvedProfile {
    let mut p = ResolvedProfile::default();
    f(&mut p);
    p
}

fn frag(name: &str, priority: i64) -> crate::profile::FragmentRef {
    crate::profile::FragmentRef {
        name: name.to_string(),
        priority,
    }
}

fn select_all(profiles: &[ResolvedProfile], request: &SelectRequest) -> Selection {
    select(profiles, &corpus(), request, &mut Vec::new())
}

// order_test.go: TestBookend_HighestFirstSecondHighestLast
#[test]
fn test_bookend_places_the_highest_first_and_the_second_highest_last() {
    let cases: Vec<(&str, Vec<FragmentAsk>, Vec<&str>)> = vec![
        ("empty", vec![], vec![]),
        ("single fragment", vec![ask("a", 5)], vec!["a"]),
        // Sorted by priority, descending.
        (
            "two fragments",
            vec![ask("low", 1), ask("high", 10)],
            vec!["high", "low"],
        ),
        // high(10) at the start, med(5) at the end, low(1) in the middle.
        (
            "three fragments - bookend",
            vec![ask("low", 1), ask("med", 5), ask("high", 10)],
            vec!["high", "low", "med"],
        ),
        // a(5) first, b(4) last, c, d, e between.
        (
            "five fragments - full bookend",
            vec![
                ask("e", 1),
                ask("d", 2),
                ask("c", 3),
                ask("b", 4),
                ask("a", 5),
            ],
            vec!["a", "c", "d", "e", "b"],
        ),
        (
            "same priority - stable order",
            vec![ask("first", 0), ask("second", 0), ask("third", 0)],
            vec!["first", "third", "second"],
        ),
    ];
    for (what, input, want) in cases {
        assert_eq!(names(&bookend(input)), want, "{what}");
    }
}

// order_test.go: TestDedupe_KeepsTheHighestPriorityPerName
#[test]
fn test_dedupe_keeps_the_highest_priority_per_name() {
    let result = dedupe(vec![
        ask("a", 5),
        ask("b", 3),
        ask("a", 10),
        ask("c", 1),
        ask("b", 2),
    ]);
    assert_eq!(result.len(), 3);
    let priority = |name: &str| result.iter().find(|a| a.name == name).unwrap().priority;
    assert_eq!(priority("a"), 10, "should keep higher priority for 'a'");
    assert_eq!(
        priority("b"),
        3,
        "should keep first (higher) priority for 'b'"
    );
    assert_eq!(priority("c"), 1);
    assert_eq!(names(&result), ["a", "b", "c"], "first-occurrence order");
}

// order: an explicit version beats the default on a repeat
#[test]
fn test_dedupe_prefers_an_explicit_version() {
    let pinned = FragmentAsk {
        version: "abc123".to_string(),
        ..ask("x", 0)
    };
    let result = dedupe(vec![ask("x", 0), pinned]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].version, "abc123");
}

// select_test.go: TestSelect_FragmentsAreOneEntryPerItemBookendedByPriority
#[test]
fn test_select_is_one_entry_per_item_bookended_by_priority() {
    let p = profile(|p| {
        p.bundles = strings(&["alpha"]);
        p.fragments = vec![
            frag("alpha#fragments/style", 5),
            frag("beta#fragments/tagged", 3),
            frag("alpha#fragments/rules", -1),
        ];
    });
    let sel = select_all(&[p], &SelectRequest::default());
    // style(5) leads, tagged(3) is second-highest so it closes, and the
    // whole-bundle expansion's rules(0 > -1) and maybe(0) sit between.
    assert_eq!(
        names(&sel.fragments),
        [ALPHA_STYLE, ALPHA_RULES, ALPHA_MAYBE, BETA_TAGGED]
    );
    assert_eq!(sel.fragments[0].priority, 5);
    assert_eq!(
        sel.fragments[1].priority, 0,
        "the higher of the two asks for rules wins"
    );
}

// select_test.go: TestSelect_ExclusionsApplyAndVersionPinsSplit
#[test]
fn test_select_applies_exclusions_and_splits_version_pins() {
    let p = profile(|p| {
        p.bundles = strings(&["alpha"]);
        p.fragments = vec![frag("ctxloom+local:beta@abc123#fragments/tagged", 0)];
        p.exclude_fragments = strings(&["alpha#fragments/maybe"]);
    });
    let sel = select_all(&[p], &SelectRequest::default());
    assert!(!names(&sel.fragments).contains(&ALPHA_MAYBE));
    let tagged = sel
        .fragments
        .iter()
        .find(|a| a.name == BETA_TAGGED)
        .expect("tagged");
    assert_eq!(
        tagged.version, "abc123",
        "the pin rides in version; the name stays version-agnostic"
    );
}

// select_test.go: TestSelect_ExplicitAndTagAsksFoldIn
#[test]
fn test_select_folds_in_the_callers_named_and_tag_asks() {
    let request = SelectRequest {
        fragments: strings(&["maybe"]),
        tags: strings(&["house"]),
    };
    let sel = select_all(&[], &request);
    assert_eq!(sel.explicit, [ALPHA_MAYBE]);
    let mut got = names(&sel.fragments);
    got.sort_unstable();
    assert_eq!(got, [ALPHA_MAYBE, ALPHA_RULES, BETA_TAGGED]);
    assert_eq!(sel.tags, ["house"]);
    assert!(sel.missing_tags.is_empty());

    let none = select_all(
        &[],
        &SelectRequest {
            fragments: vec![],
            tags: strings(&["no-such-tag"]),
        },
    );
    assert_eq!(
        none.missing_tags,
        ["no-such-tag"],
        "all the requested tags when none matched"
    );
}

// select_test.go: TestSelect_CarriesTheProfileSetsDeclarations
#[test]
fn test_select_carries_the_profile_sets_declarations() {
    let first = profile(|p| {
        p.bundles = strings(&["alpha", "beta"]);
        p.commands = strings(&["alpha#commands/release"]);
        p.skills = strings(&["alpha#skills/reviewer"]);
        p.variables = BTreeMap::from([
            ("project".into(), "one".into()),
            ("team".into(), "core".into()),
        ]);
        p.deny_tools = strings(&["Task"]);
    });
    let second = profile(|p| {
        p.bundles = strings(&["beta"]);
        p.variables = BTreeMap::from([("project".into(), "two".into())]);
        p.deny_tools = strings(&["WebFetch", "Task"]);
        p.exclude_mcp = strings(&["db"]);
        p.llm = "fast".to_string();
    });
    let sel = select_all(&[first, second], &SelectRequest::default());
    assert_eq!(sel.commands, ["alpha#commands/release"]);
    assert_eq!(sel.skills, ["alpha#skills/reviewer"]);
    assert_eq!(sel.bundles, ["alpha", "beta"], "first occurrence kept");
    assert_eq!(
        sel.variables,
        BTreeMap::from([
            ("project".into(), "two".into()),
            ("team".into(), "core".into())
        ])
    );
    assert_eq!(sel.deny_tools, ["Task", "WebFetch"]);
    assert_eq!(sel.llm, "fast");
    assert!(sel.exclusions.contains("db"));
}

// select: a whole-bundle ask for a bundle the catalog lacks is skipped and said
#[test]
fn test_select_skips_a_bundle_that_is_not_there_and_says_so() {
    let p = profile(|p| p.bundles = strings(&["alpha", "does-not-exist"]));
    let mut findings = Vec::new();
    let sel = select(&[p], &corpus(), &SelectRequest::default(), &mut findings);
    assert!(
        !sel.fragments.is_empty(),
        "alpha's fragments are still selected"
    );
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].class, crate::findings::Class::Warn);
    assert_eq!(
        findings[0].text,
        "skipping unresolved bundle \"does-not-exist\": bundle not found: does-not-exist"
    );
}

fn package(profiles: &[ResolvedProfile], request: &SelectRequest, options: Options) -> Package {
    let cat = corpus();
    let selection = select(profiles, &cat, request, &mut Vec::new());
    assemble(&cat, &selection, &options)
}

fn alpha_with_project(project: &str) -> ResolvedProfile {
    profile(|p| {
        p.bundles = strings(&["alpha"]);
        p.variables = BTreeMap::from([("project".into(), project.into())]);
    })
}

fn refs(items: &[Delivered]) -> Vec<&str> {
    items.iter().map(|i| i.reference.as_str()).collect()
}

// assemble_test.go: TestAssemble_ContextIsTheSelectionInOrderSubstituted
#[test]
fn test_the_context_is_the_selection_in_order_substituted() {
    let pkg = package(
        &[alpha_with_project("golden")],
        &SelectRequest::default(),
        Options::default(),
    );
    assert_eq!(
        pkg.context,
        "Prefer small functions.\n\n---\n\nRules for golden."
    );
    assert!(!pkg.hash.is_empty());
    assert_eq!(refs(&pkg.fragments), [ALPHA_STYLE, ALPHA_RULES]);
    assert_eq!(refs(&pkg.premised), [ALPHA_MAYBE]);
    assert_eq!(
        pkg.premised[0].premise,
        "the agent is about to touch a signed bundle"
    );
    assert!(!pkg.fragments[0].distilled);
    assert_eq!(
        pkg.loaded,
        [ALPHA_STYLE, ALPHA_RULES],
        "a held-back fragment is not reported as loaded"
    );
}

// assemble_test.go: TestAssemble_PremisedFragmentsLoadWhenExplicitOrStatic
#[test]
fn test_a_premised_fragment_loads_when_named_or_when_the_assembly_is_static() {
    let named = package(
        &[],
        &SelectRequest {
            fragments: strings(&["maybe"]),
            tags: vec![],
        },
        Options::default(),
    );
    assert_eq!(refs(&named.fragments), [ALPHA_MAYBE]);
    assert!(named.premised.is_empty());

    let static_pkg = package(
        &[alpha_with_project("golden")],
        &SelectRequest::default(),
        Options {
            static_assembly: true,
            ..Options::default()
        },
    );
    assert_eq!(
        refs(&static_pkg.fragments),
        [ALPHA_MAYBE, ALPHA_STYLE, ALPHA_RULES]
    );
    assert!(static_pkg.premised.is_empty());
    assert!(static_pkg
        .context
        .contains("Do not edit a signed bundle in place."));
}

// assemble_test.go: TestAssemble_AnAskThatDoesNotLoadIsAFinding
#[test]
fn test_an_ask_that_does_not_load_is_a_finding_not_a_refusal() {
    let p = profile(|p| {
        p.fragments = vec![
            frag("alpha#fragments/style", 0),
            frag("alpha#fragments/nope", 0),
        ]
    });
    let pkg = package(&[p], &SelectRequest::default(), Options::default());
    assert_eq!(refs(&pkg.fragments), [ALPHA_STYLE]);
    let fails: Vec<_> = pkg
        .findings
        .iter()
        .filter(|f| f.class == crate::findings::Class::Fail)
        .collect();
    assert_eq!(fails.len(), 1, "{:?}", pkg.findings);
    assert_eq!(fails[0].kind, "ref");
    assert_eq!(
        fails[0].text,
        "fragment ctxloom+local:alpha#fragments/nope failed to load (fragment not found: \"nope\" in bundle \"ctxloom+local:alpha\"); skipping"
    );
    assert_eq!(
        fails[0].remedy,
        "fix the fragment ref in the referencing profile, or install its bundle (ctxloom deps pull)"
    );
}

// substitute_test.go via assembly: an undefined variable is a warning naming
// the fragment, and the tag stays in the text.
#[test]
fn test_an_undefined_variable_warns_naming_the_fragment_and_stays_in_the_text() {
    let p = profile(|p| p.bundles = strings(&["alpha"]));
    let pkg = package(&[p], &SelectRequest::default(), Options::default());
    assert!(
        pkg.context.contains("Rules for {{project}}."),
        "{}",
        pkg.context
    );
    let warn = pkg
        .findings
        .iter()
        .find(|f| f.text.contains("undefined variable"))
        .expect("a warning");
    assert_eq!(warn.class, crate::findings::Class::Warn);
    assert_eq!(
        warn.text,
        "undefined variable: {{project}} (fragment \"ctxloom+local:alpha#fragments/rules\")"
    );
}

// ingest_test.go: TestIngest_DropReportsTheKeptRef
#[test]
fn test_ingest_drops_a_re_arrival_of_the_same_item_and_reports_the_kept_reference() {
    let mut ingest = Ingest::new();
    let rules = "ctxloom+local:dev#fragments/rules";
    let (kept, dup) = ingest.add(&identity_key(rules), "RULES", rules);
    assert!(!dup);
    assert_eq!(kept, rules);

    let (kept, dup) = ingest.add(&identity_key(rules), "RULES", rules);
    assert!(dup, "a re-ingest of the same ref is a duplicate");
    assert_eq!(kept, rules);

    let pinned = "ctxloom+local:dev@abc123#fragments/rules";
    let (kept, dup) = ingest.add(&identity_key(pinned), "RULES", pinned);
    assert!(
        dup,
        "the same item under another spelling is the same content"
    );
    assert_eq!(kept, rules, "the first occurrence is the one kept");

    let companion = "ctxloom+companion:dev#fragments/rules";
    let (kept, dup) = ingest.add(&identity_key(companion), "RULES", companion);
    assert!(!dup, "the same bytes from another source are another item");
    assert_eq!(kept, companion);
    assert_eq!(ingest.join(), format!("RULES{SECTION_SEPARATOR}RULES"));
}

// ingest_test.go: TestIdentityKey_IsSourceQualifiedAndVersionless
#[test]
fn test_identity_key_is_source_qualified_and_versionless() {
    let companion = identity_key("ctxloom+companion:isolation#fragments/isolation-axes");
    let local = identity_key("ctxloom+local:isolation#fragments/isolation-axes");
    assert_ne!(
        companion, local,
        "the companion and local items of one name are two items"
    );
    assert_eq!(
        identity_key("ctxloom+local:beta#fragments/tagged"),
        identity_key("ctxloom+local:beta@abc123#fragments/tagged"),
        "a version does not make another item"
    );
    assert_ne!(
        companion,
        identity_key("ctxloom+companion:isolation#fragments/other-axes")
    );
    assert_ne!(
        companion,
        identity_key("ctxloom+companion:other-bundle#fragments/isolation-axes")
    );
    assert_ne!(
        companion,
        identity_key("ctxloom+companion:isolation#prompts/isolation-axes")
    );
    assert_eq!(identity_key("not a ref"), "not a ref");
    assert_eq!(
        item_ref("isolation", ItemKind::Fragment, "x"),
        "ctxloom+local:isolation#fragments/x"
    );
}

// ingest_test.go: TestIngest_JoinOmitsBlankSections
#[test]
fn test_join_omits_blank_sections() {
    let mut ingest = Ingest::new();
    assert!(
        !ingest
            .add("b#fragment/blank", "   \n ", "b#fragments/blank")
            .1
    );
    assert!(!ingest.add("b#fragment/real", "REAL", "b#fragments/real").1);
    assert_eq!(
        ingest.join(),
        "REAL",
        "a blank section must not be framed by a separator"
    );
}

// assemble_golden_test.go (the corpus golden): the context the Go golden
// profile "golden-auto" assembled for the live consumer, byte for byte,
// without the companion section. Fragments in `fragments_loaded` order;
// `Project golden: always on.` has its variable substituted; the tricky
// fragment's literal mustaches pass through. (The whole-corpus parity is the
// binary's: fixtures/golden/gold-run-dry-auto-json.)
#[test]
fn test_literal_mustaches_in_a_fragment_pass_through_unchanged() {
    use crate::testdata::catalog_of;
    let cat = catalog_of(&[(
        "q",
        "version: 1.0.0\nfragments:\n  tricky:\n    content: \"Body with a horizontal rule below.\\n\\n---\\n\\nAnd a literal mustache pair: {{ not_a_template }} plus {{{ triple }}}.\\n\\n---\\n\"\n",
    )]);
    let sel = select(
        &[profile(|p| p.bundles = strings(&["q"]))],
        &cat,
        &SelectRequest::default(),
        &mut Vec::new(),
    );
    let pkg = assemble(&cat, &sel, &Options::default());
    assert_eq!(
        pkg.context,
        "Body with a horizontal rule below.\n\n---\n\nAnd a literal mustache pair: {{not_a_template}} plus {{triple}}.\n\n---"
    );
    let named: Vec<&str> = pkg.findings.iter().map(|f| f.text.as_str()).collect();
    assert_eq!(
        named,
        [
            "undefined variable: {{not_a_template}} (fragment \"ctxloom+local:q#fragments/tricky\")",
            "undefined variable: {{triple}} (fragment \"ctxloom+local:q#fragments/tricky\")",
        ]
    );
}

// select: a profile's bundle entry may select one fragment, by `#` or by the
// older `:fragments/` alias; any other kind of selector asks for nothing.
#[test]
fn test_a_bundle_entry_selects_one_fragment_by_hash_or_alias_and_only_a_fragment() {
    for entry in [
        "alpha#fragments/style",
        "alpha:fragments/style",
        "ctxloom+local:alpha#fragments/style",
    ] {
        let p = profile(|p| p.bundles = strings(&[entry]));
        let mut findings = Vec::new();
        let sel = select(&[p], &corpus(), &SelectRequest::default(), &mut findings);
        assert_eq!(names(&sel.fragments), [ALPHA_STYLE], "{entry}");
        assert!(findings.is_empty(), "{entry}: {findings:?}");
    }
    for entry in [
        "alpha#commands/review",
        "alpha:commands/review",
        "alpha:mcp",
    ] {
        let p = profile(|p| p.bundles = strings(&[entry]));
        let mut findings = Vec::new();
        let sel = select(&[p], &corpus(), &SelectRequest::default(), &mut findings);
        assert!(sel.fragments.is_empty(), "{entry}: {:?}", sel.fragments);
        assert!(findings.is_empty(), "{entry}: {findings:?}");
    }
}

// order: a version already explicit is kept over a later one
#[test]
fn test_dedupe_keeps_the_first_explicit_version() {
    let pin = |v: &str| FragmentAsk {
        version: v.to_string(),
        ..ask("x", 0)
    };
    let result = dedupe(vec![pin("one"), pin("two"), ask("x", 0)]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].version, "one");
}

// assemble: the package carries the selection it assembled
#[test]
fn test_the_package_carries_its_selection() {
    let cat = corpus();
    let selection = Selection {
        fragments: vec![ask(ALPHA_STYLE, 3)],
        tags: strings(&["house"]),
        ..Selection::default()
    };
    let package = assemble(&cat, &selection, &Options::default());
    assert_eq!(package.selection, selection);
}

// ingest_test.go via assemble: the same item named two ways is assembled once
// and the second naming is reported against the first; the same name twice
// is not worth a finding.
#[test]
fn test_a_duplicate_under_another_name_is_reported_and_under_the_same_name_is_not() {
    let cat = corpus();
    let two_names = Selection {
        fragments: vec![ask(ALPHA_STYLE, 0), ask("style", 0)],
        ..Selection::default()
    };
    let package = assemble(&cat, &two_names, &Options::default());
    assert_eq!(package.findings.len(), 1, "{:?}", package.findings);
    assert!(package.findings[0].text.contains("both name the item"));
    let one_name = Selection {
        fragments: vec![ask(ALPHA_STYLE, 0), ask(ALPHA_STYLE, 0)],
        ..Selection::default()
    };
    let package = assemble(&cat, &one_name, &Options::default());
    assert!(package.findings.is_empty(), "{:?}", package.findings);
}

// ---------------------------------------------------------------- companions

const LOADOUT: &str = "version: 1.0.0\nfragments:\n  guide:\n    content: \"Companion guide.\"\n  later:\n    premise: \"about to do the later thing\"\n    content: \"Later.\"\n";

fn corpus_with_companion() -> Catalog {
    corpus().with_companions(vec![(
        "reprise".to_string(),
        crate::document::parse_document(LOADOUT).unwrap(),
    )])
}

fn companion_selection(catalog: &Catalog, profile_fragments: &[&str]) -> Selection {
    let profile = profile(|p| {
        p.fragments = profile_fragments.iter().map(|f| frag(f, 0)).collect();
    });
    let selection = select(
        &[profile],
        &corpus(),
        &SelectRequest::default(),
        &mut Vec::new(),
    );
    with_companion_asks(selection, catalog)
}

#[test]
fn test_companion_fragments_follow_the_profile_selected_ones() {
    let catalog = corpus_with_companion();
    let selection = companion_selection(&catalog, &["style", "rules"]);
    assert_eq!(
        names(&selection.fragments),
        [
            "style",
            "rules",
            "ctxloom+companion:reprise#fragments/guide",
            "ctxloom+companion:reprise#fragments/later",
        ]
    );
    assert!(selection.fragments[2..]
        .iter()
        .all(|a| a.priority == 0 && a.version.is_empty()));
}

#[test]
fn test_without_companions_the_selection_is_as_it_was() {
    let catalog = corpus();
    let plain = select(
        &[profile(|p| p.fragments = vec![frag("style", 0)])],
        &catalog,
        &SelectRequest::default(),
        &mut Vec::new(),
    );
    assert_eq!(with_companion_asks(plain.clone(), &catalog), plain);
}

#[test]
fn test_a_companion_fragment_is_delivered_unless_it_has_a_premise() {
    let catalog = corpus_with_companion();
    let selection = companion_selection(&catalog, &["style"]);
    let package = assemble(&catalog, &selection, &Options::default());
    assert_eq!(
        package.context,
        format!("Prefer small functions.{SECTION_SEPARATOR}Companion guide.")
    );
    assert_eq!(package.premised.len(), 1);
    assert_eq!(
        package.premised[0].reference,
        "ctxloom+companion:reprise#fragments/later"
    );
    assert_eq!(package.premised[0].premise, "about to do the later thing");
    assert_eq!(package.premised[0].body, "Later.");
    assert!(package.findings.is_empty(), "{:?}", package.findings);
}

#[test]
fn test_a_premised_companion_fragment_loads_when_named() {
    let catalog = corpus_with_companion();
    let mut selection = companion_selection(&catalog, &[]);
    selection
        .explicit
        .push("ctxloom+companion:reprise#fragments/later".to_string());
    let package = assemble(&catalog, &selection, &Options::default());
    assert!(package.context.contains("Later."));
    assert!(package.premised.is_empty());
}

#[test]
fn test_a_companion_fragment_the_profile_names_too_is_assembled_once() {
    let catalog = corpus_with_companion();
    let selection = companion_selection(&catalog, &["ctxloom+companion:reprise#fragments/guide"]);
    let package = assemble(&catalog, &selection, &Options::default());
    assert_eq!(package.context, "Companion guide.");
    assert_eq!(package.fragments.len(), 1);
}
