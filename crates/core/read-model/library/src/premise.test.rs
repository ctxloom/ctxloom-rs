//! Ported from Go's `internal/adapters/operations/premise_test.go`
//! (860336c4e), and the D24 pin: the index's text is the owner's, byte for
//! byte from Go ctxloom's `PremiseSelectionInstruction()` at release/0.7.

use super::*;
use crate::compose::{assemble, select, Options, SelectRequest};
use crate::profile::ResolvedProfile;
use crate::testdata::catalog_of;

/// `PremiseSelectionInstruction()` at release/0.7 (860336c4e), written out in
/// full: a test that rebuilt it from the code's own pieces would keep agreeing
/// after someone edited the owner's words.
const GO_INSTRUCTION: &str = "# Conditional guidance\n\nSome of this project's guidance applies CONDITIONALLY. Each entry gives a\nfragment's NAME and the PREMISE under which it applies. Load any your\ncontext does not already carry.\n\nMatch against WHAT YOU ARE ABOUT TO DO -- the next action, not the whole\nconversation behind you. Take each premise ON ITS OWN and ask whether it\ndescribes that action; you are not picking the single best match, and\nseveral premises often apply at once.\n\nWhen it is a BORDERLINE call, include it. An unnecessary fragment costs a\nlittle context; one you withhold is never learned to exist and cannot be\nasked for.\n\nSelect on the premise, not on the name. Then call the ctxloom\n`assemble_context` tool with the names you chose (the `bundles` argument)\nand follow what it returns.\n";

/// A bundle shipping three fragments and a profile composing it; `premised`
/// decides whether the third carries a premise. Everything else is identical,
/// which lets a test attribute a byte difference to the premise alone.
fn selection_fixture(premised: bool) -> (crate::catalog::Catalog, ResolvedProfile) {
    let premise = if premised {
        "    premise: \"You are about to remove a worktree.\"\n"
    } else {
        ""
    };
    let doc = format!(
        "version: \"1.0.0\"\nfragments:\n  alpha:\n    content: \"ALPHA-BODY\"\n  beta:\n    content: \"BETA-BODY\"\n  gamma:\n{premise}    content: \"GAMMA-BODY\"\n"
    );
    let profile = ResolvedProfile {
        bundles: vec!["ctxloom:local@bundles/sel".to_string()],
        ..ResolvedProfile::default()
    };
    (catalog_of(&[("sel", &doc)]), profile)
}

fn assembled(premised: bool, fragments: &[&str]) -> (crate::compose::Package, Vec<Entry>) {
    let (cat, profile) = selection_fixture(premised);
    let request = SelectRequest {
        fragments: fragments.iter().map(ToString::to_string).collect(),
        tags: vec![],
    };
    let sel = select(&[profile], &cat, &request, &mut Vec::new());
    let pkg = assemble(&cat, &sel, &Options::default());
    let withheld = pkg
        .premised
        .iter()
        .map(|p| Entry {
            name: p.reference.clone(),
            premise: p.premise.clone(),
        })
        .collect();
    (pkg, withheld)
}

// premise_test.go: TestAssembleContext_PremiselessCorpusIsByteIdentical
#[test]
fn test_a_premiseless_corpus_assembles_byte_identically() {
    let (pkg, withheld) = assembled(false, &[]);
    assert_eq!(
        pkg.context,
        "ALPHA-BODY\n\n---\n\nGAMMA-BODY\n\n---\n\nBETA-BODY"
    );
    assert!(
        withheld.is_empty(),
        "nothing carries a premise, so the index is empty"
    );
    for name in ["alpha", "beta", "gamma"] {
        assert!(
            pkg.loaded
                .contains(&format!("ctxloom+local:sel#fragments/{name}")),
            "{name}"
        );
    }
}

// premise_test.go: TestAssembleContext_PremisedFragmentIsWithheldAndIndexed
#[test]
fn test_a_premised_fragment_is_withheld_and_indexed() {
    let (pkg, withheld) = assembled(true, &[]);
    assert_eq!(
        pkg.context, "ALPHA-BODY\n\n---\n\nBETA-BODY",
        "the withheld one left no gap"
    );
    assert!(
        !pkg.context.contains("GAMMA-BODY"),
        "withheld means the body is not paid for"
    );
    assert!(!pkg
        .loaded
        .contains(&"ctxloom+local:sel#fragments/gamma".to_string()));
    assert_eq!(withheld.len(), 1);
    assert_eq!(withheld[0].premise, "You are about to remove a worktree.");
    assert_eq!(withheld[0].name, "ctxloom+local:sel#fragments/gamma");
}

// premise_test.go: TestAssembleContext_ExplicitAskLoadsPremisedFragment
#[test]
fn test_naming_a_premised_fragment_loads_it() {
    let (pkg, withheld) = assembled(true, &["ctxloom:local@bundles/sel#fragments/gamma"]);
    assert!(
        pkg.context.contains("GAMMA-BODY"),
        "the ask IS the selection"
    );
    assert!(
        withheld.is_empty(),
        "nothing was withheld, so nothing is offered back"
    );
}

// premise_test.go: TestAssembleContext_UnknownAskIsReportedMissing
#[test]
fn test_an_unknown_ask_is_reported_not_silently_empty() {
    let (cat, profile) = selection_fixture(true);
    let request = SelectRequest {
        fragments: vec!["ctxloom:local@bundles/sel#fragments/nosuch".to_string()],
        tags: vec![],
    };
    let sel = select(&[profile], &cat, &request, &mut Vec::new());
    assert!(
        sel.explicit
            .contains(&"ctxloom+local:sel#fragments/nosuch".to_string()),
        "an ask is resolved to its qualified name at intake: {:?}",
        sel.explicit
    );
    let pkg = assemble(&cat, &sel, &Options::default());
    assert!(!pkg
        .loaded
        .contains(&"ctxloom+local:sel#fragments/nosuch".to_string()));
}

// premise_test.go: TestPremiseIndex_ListsOnlyPremisedFragments
#[test]
fn test_the_index_lists_only_premised_fragments_by_qualified_name() {
    let (cat, _) = selection_fixture(true);
    let entries = index(&cat);
    assert_eq!(
        entries.len(),
        1,
        "unconditional fragments are not decisions"
    );
    assert_eq!(entries[0].name, "sel#fragments/gamma");
    assert_eq!(entries[0].premise, "You are about to remove a worktree.");
    let (plain, _) = selection_fixture(false);
    assert!(index(&plain).is_empty());
}

// premise_test.go: sorted by name so the same corpus always renders the same
// index (a row order that moved would defeat prompt caching).
#[test]
fn test_the_index_is_sorted_by_name() {
    let cat = catalog_of(&[
        ("zeta", "version: 1.0.0\nfragments:\n  b:\n    premise: PB\n    content: x\n"),
        ("alpha", "version: 1.0.0\nfragments:\n  z:\n    premise: PZ\n    content: x\n  a:\n    premise: PA\n    content: x\n"),
    ]);
    let names: Vec<String> = index(&cat).into_iter().map(|e| e.name).collect();
    assert_eq!(
        names,
        ["alpha#fragments/a", "alpha#fragments/z", "zeta#fragments/b"]
    );
}

// premise_test.go: TestRenderPremiseIndex_EmptyRendersNothing
#[test]
fn test_an_empty_index_renders_nothing() {
    assert_eq!(render(&[]), "");
}

// premise_test.go: TestRenderPremiseIndex_CarriesNameAndPremiseOnly
#[test]
fn test_a_row_carries_the_name_and_the_premise_only() {
    let out = render(&[Entry {
        name: "worktree-lifecycle".into(),
        premise: "You are about to remove a worktree.".into(),
    }]);
    assert_eq!(
        out,
        format!("{GO_INSTRUCTION}\n- worktree-lifecycle: You are about to remove a worktree.")
    );
}

// D24: the instruction is the owner's, byte for byte.
#[test]
fn test_the_instruction_is_byte_identical_to_gos() {
    assert_eq!(selection_instruction(), GO_INSTRUCTION);
}

// premise_test.go: TestRenderPremiseIndex_KeepsTheThreeMeasuredProperties
#[test]
fn test_the_index_keeps_the_three_measured_properties() {
    let got = render(&[Entry {
        name: "alpha".into(),
        premise: "You are about to do a thing.".into(),
    }]);
    for phrase in [
        "ON ITS OWN",
        "not picking the single best match",
        "BORDERLINE",
        "never learned to exist",
        "ABOUT TO DO",
        "not the whole",
        "assemble_context",
        "`bundles`",
        "alpha: You are about to do a thing.",
    ] {
        assert!(got.contains(phrase), "{phrase}");
    }
}
