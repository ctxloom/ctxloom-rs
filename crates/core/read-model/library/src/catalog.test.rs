//! Ported from Go's `internal/core/bundles` tests (`item_tags_test.go`,
//! `item_ask_test.go`, `listing_order_test.go`, `catalog_ask_test.go`) at
//! 860336c4e, on the corpus of `internal/core/composite/corpus_test.go`.

use super::*;

angzarr_presentation::english_for!(&[crate::EN_US.1]);

use crate::testdata::{catalog_of, corpus, ALPHA, BETA};

fn names(infos: &[ContentInfo]) -> Vec<&str> {
    infos.iter().map(|i| i.name.as_str()).collect()
}

fn strings(of: &[&str]) -> Vec<String> {
    of.iter().map(ToString::to_string).collect()
}

// item_tags_test.go: TestListAllFragments_TagsAreEachListedOnce
#[test]
fn test_an_items_tags_are_the_bundles_then_its_own_each_once() {
    let cat = catalog_of(&[(
        "test",
        "version: \"1.0\"\ntags: [testing, tdd]\nfragments:\n  frag:\n    tags: [testing, mutation]\n    content: c\n",
    )]);
    let infos = cat.list_fragments();
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].tags, strings(&["testing", "tdd", "mutation"]));
    assert_eq!(
        item_tags(&strings(&["a", "b"]), &strings(&["b", "c", "a"])),
        strings(&["a", "b", "c"])
    );
}

// listing_order_test.go: TestCatalog_ListAll_ItemsListByNameWithinABundle
#[test]
fn test_items_list_by_name_within_a_bundle() {
    let items = "  zulu: {content: z}\n  alpha: {content: a}\n  mike: {content: m}\n  echo: {content: e}\n  kilo: {content: k}\n  bravo: {content: b}\n  yankee: {content: y}\n  charlie: {content: c}\n";
    let cat = catalog_of(&[(
        "order",
        &format!("version: \"1.0\"\nfragments:\n{items}commands:\n{items}"),
    )]);
    let want = [
        "alpha", "bravo", "charlie", "echo", "kilo", "mike", "yankee", "zulu",
    ];
    assert_eq!(names(&cat.list_fragments()), want);
    assert_eq!(names(&cat.list_commands()), want);
}

// catalog.go sortReads: bundles list by name, so a listing is a function of
// the corpus whatever order the entries came in.
#[test]
fn test_bundles_list_by_name_whatever_order_they_arrive_in() {
    let cat = catalog_of(&[("beta", BETA), ("alpha", ALPHA)]);
    let bundles: Vec<&str> = cat.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(bundles, ["alpha", "beta"]);
    let listed: Vec<(String, String)> = cat
        .list_fragments()
        .into_iter()
        .map(|f| (f.bundle, f.name))
        .collect();
    assert_eq!(listed[0], ("alpha".to_string(), "maybe".to_string()));
    assert_eq!(
        listed.last().unwrap(),
        &("beta".to_string(), "tagged".to_string())
    );
}

// catalog.go Lookup: a bundle is its name, ctxloom+local:<name> or the older
// ctxloom:local@bundles/<name>; a miss is "bundle not found: <ask>".
#[test]
fn test_a_bundle_is_found_by_every_spelling_of_its_name() {
    let cat = corpus();
    for ask in [
        "alpha",
        "ctxloom+local:alpha",
        "ctxloom:local@bundles/alpha",
    ] {
        assert_eq!(cat.lookup(ask).unwrap().name, "alpha", "{ask}");
    }
    let err = cat.lookup("nosuch").expect_err("not there");
    assert_eq!(err.english(), "bundle not found: nosuch");
    assert!(matches!(
        cat.lookup("ctxloom+local:nosuch"),
        Err(CatalogError::BundleNotFound { .. })
    ));
}

// item_ask_test.go: TestReadCommand_PromptsAliasReachesTheSameItem
#[test]
fn test_the_prompts_alias_reaches_the_same_command() {
    let cat = catalog_of(&[(
        "kit",
        "version: \"1.0\"\ncommands:\n  deploy:\n    content: run the deploy script\n",
    )]);
    let via_commands = cat.read_command("kit#commands/deploy").unwrap();
    let via_prompts = cat.read_command("kit#prompts/deploy").unwrap();
    assert_eq!(via_commands.len(), 1);
    assert_eq!(via_prompts.len(), 1);
    assert_eq!(via_commands[0].item_ref, via_prompts[0].item_ref);
    assert_eq!(via_commands[0].item_ref, "ctxloom+local:kit#prompts/deploy");
    assert_eq!(via_prompts[0].item, "deploy");
}

// item_ask_test.go: TestReadFragment_CommandSelectorIsRefusedByKindNotByHash
#[test]
fn test_a_command_selector_given_to_a_fragment_read_is_refused_by_kind() {
    let cat = catalog_of(&[(
        "kit",
        "version: \"1.0\"\ncommands:\n  deploy:\n    content: run the deploy script\n",
    )]);
    for ask in ["kit#commands/deploy", "kit#prompts/deploy"] {
        let err = cat.read_fragment(ask).expect_err(ask);
        assert!(
            matches!(err, CatalogError::BadItemRef { .. }),
            "{ask}: {}",
            err.english()
        );
        assert!(
            err.english().contains("selects a prompt, not a fragment"),
            "{}",
            err.english()
        );
    }
}

// bundles.go ReadFragment: a missing fragment names it and its bundle; a bare
// name no bundle has says so.
#[test]
fn test_a_fragment_that_is_not_there_is_named() {
    let cat = corpus();
    let err = cat.read_fragment("alpha#fragments/nope").expect_err("nope");
    assert_eq!(
        err.english(),
        "fragment not found: \"nope\" in bundle \"alpha\""
    );
    let err = cat.read_fragment("nope").expect_err("nope");
    assert_eq!(err.english(), "fragment not found: nope");
    let read = cat.read_fragment("alpha#fragments/rules").unwrap();
    assert_eq!(read[0].name, "alpha/rules");
    assert_eq!(read[0].item_ref, "ctxloom+local:alpha#fragments/rules");
    assert_eq!(read[0].tags, strings(&["house"]));
}

// bundles.go ByTags: the fragments carrying any of the tags, in listing order
#[test]
fn test_by_tags_selects_any_tag_in_listing_order() {
    let cat = corpus();
    assert_eq!(
        names(&cat.by_tags(&strings(&["house"]))),
        ["rules", "tagged"]
    );
    assert!(cat.by_tags(&strings(&["nothing"])).is_empty());
}

// bundles.go ResolveFragmentAsk (select_test.go: TestSelect_ExplicitAndTagAsksFoldIn):
// a bare name resolves to the qualified reference of the first bundle holding
// it; one no bundle holds stays as written.
#[test]
fn test_a_bare_fragment_ask_resolves_to_its_qualified_reference() {
    let cat = corpus();
    assert_eq!(
        cat.resolve_fragment_ask("maybe"),
        "ctxloom+local:alpha#fragments/maybe"
    );
    assert_eq!(cat.resolve_fragment_ask("nowhere"), "nowhere");
    assert_eq!(
        cat.resolve_fragment_ask("beta#fragments/tagged"),
        "ctxloom+local:beta#fragments/tagged"
    );
}

// pipeline: the distilled form is delivered when preferred and present.
#[test]
fn test_the_distilled_form_is_delivered_when_preferred_and_present() {
    let cat = catalog_of(&[(
        "kit",
        "version: \"1.0\"\nfragments:\n  both:\n    content: long\n    distilled: short\n  raw:\n    content: only\n",
    )]);
    let both = &cat.read_fragment("kit#fragments/both").unwrap()[0];
    assert_eq!(both.resolve(true), ("short", true));
    assert_eq!(both.resolve(false), ("long", false));
    let raw = &cat.read_fragment("kit#fragments/raw").unwrap()[0];
    assert_eq!(raw.resolve(true), ("only", false));
}

// catalog.go Lookup: a name the listing shows wins, then the name a bundle
// declares for itself; two bundles declaring one name are ambiguous.
#[test]
fn test_a_bundle_is_found_by_its_declared_name_and_two_of_them_are_ambiguous() {
    let cat = catalog_of(&[
        ("one", "version: \"1.0\"\nname: shared\n"),
        ("two", "version: \"1.0\"\nname: shared\n"),
        ("three", "version: \"1.0\"\nname: solo\n"),
        ("four", "version: \"1.0\"\n"),
    ]);
    assert_eq!(cat.lookup("solo").unwrap().name, "three");
    assert_eq!(cat.lookup("four").unwrap().name, "four");
    match cat.lookup("shared") {
        Err(CatalogError::Ambiguous { ask, candidates }) => {
            assert_eq!(ask, "shared");
            assert_eq!(candidates, "ctxloom+local:one, ctxloom+local:two");
        }
        other => panic!(
            "expected ambiguity, got {:?}",
            other.map(|e| e.name.clone())
        ),
    }
    for ask in ["", "missing"] {
        assert!(
            matches!(cat.lookup(ask), Err(CatalogError::BundleNotFound { .. })),
            "{ask:?}"
        );
    }
}

// catalog.go Lookup: an ask that only looks like a scheme but is not a
// reference is still a name.
#[test]
fn test_a_listing_name_that_looks_like_a_scheme_is_still_found() {
    let cat = catalog_of(&[("ctxloom+odd", "version: \"1.0\"\n")]);
    assert_eq!(cat.lookup("ctxloom+odd").unwrap().name, "ctxloom+odd");
}
