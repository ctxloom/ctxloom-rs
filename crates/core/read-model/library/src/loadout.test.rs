//! Go's `bundles.ParseLoadout` (`loadout_test.go`): one document, two
//! sections, strict at every level.

use super::*;

angzarr_presentation::english_for!(&[crate::EN_US.1]);

fn parse(text: &str) -> Loadout {
    parse_loadout(text).unwrap_or_else(|e| panic!("{}", e.english()))
}

fn refused(text: &str) -> BundleError {
    parse_loadout(text).expect_err("should be refused")
}

#[test]
fn a_run_section_is_a_bundle() {
    let lo = parse(
        "run:\n  version: 1.0.0\n  fragments:\n    guide:\n      content: |\n        # Guide\n\n        Line two.\n      premise: you are about to guide\n",
    );
    let guide = &lo.run.fragments["guide"];
    assert_eq!(guide.body.content, "# Guide\n\nLine two.\n");
    assert_eq!(guide.premise, "you are about to guide");
    assert_eq!(lo.run.version, "1.0.0");
    assert!(lo.init.is_zero());
}

#[test]
fn an_init_section_alone_is_a_loadout_with_an_empty_run() {
    let lo = parse("init:\n  tooling: |\n    RUN apk add jq\n");
    assert_eq!(lo.init.tooling, "RUN apk add jq\n");
    assert_eq!(lo.run, Bundle::default());
}

#[test]
fn every_init_field_is_read_by_name() {
    let lo = parse(
        "init:\n  setup_guidance: ask first\n  tooling: jq\n  legacy_context:\n    - docs/a.md\n    - docs/b.md\n  questions:\n    - id: q1\n      prompt: Which one?\n",
    );
    assert_eq!(lo.init.setup_guidance, "ask first");
    assert_eq!(lo.init.tooling, "jq");
    assert_eq!(lo.init.legacy_context, ["docs/a.md", "docs/b.md"]);
    assert_eq!(
        lo.init.questions,
        [InitQuestion {
            id: "q1".to_string(),
            prompt: "Which one?".to_string()
        }]
    );
    assert!(!lo.init.is_zero());
}

#[test]
fn both_sections_are_one_document() {
    let lo = parse("run:\n  version: 1.0.0\ninit:\n  tooling: jq\n");
    assert_eq!(lo.run.version, "1.0.0");
    assert_eq!(lo.init.tooling, "jq");
}

#[test]
fn a_document_with_neither_section_is_refused() {
    for text in [
        "",
        "# nothing\n",
        "null\n",
        "{}\n",
        "init: {}\n",
        "run:\ninit:\n",
    ] {
        assert!(
            matches!(refused(text), BundleError::LoadoutEmpty { .. }),
            "{text:?}"
        );
    }
    let BundleError::LoadoutEmpty { bytes } = refused("# c\n") else {
        panic!("empty");
    };
    assert_eq!(bytes, 4);
}

#[test]
fn a_bare_bundle_at_the_top_is_refused_as_an_unknown_key() {
    let BundleError::UnknownKey {
        key,
        line,
        site,
        valid,
        ..
    } = refused("version: 1.0.0\nfragments:\n  a:\n    content: x\n")
    else {
        panic!("unknown key");
    };
    assert_eq!((key.as_str(), line), ("version", 1));
    assert_eq!(site, "the top level of the loadout");
    assert_eq!(valid, "init, run");
}

#[test]
fn a_misspelled_top_key_says_what_was_meant() {
    let BundleError::UnknownKey { suggestion, .. } = refused("rum:\n  version: 1\n") else {
        panic!("unknown key");
    };
    assert_eq!(suggestion, "run");
}

#[test]
fn a_misspelled_init_key_is_refused() {
    let BundleError::UnknownKey {
        key,
        line,
        site,
        suggestion,
        valid,
    } = refused("run:\n  version: 1\ninit:\n  toling: jq\n")
    else {
        panic!("unknown key");
    };
    assert_eq!((key.as_str(), line), ("toling", 4));
    assert_eq!(site, "`init`");
    assert_eq!(suggestion, "tooling");
    assert_eq!(valid, "legacy_context, questions, setup_guidance, tooling");
}

#[test]
fn a_misspelled_question_key_is_refused() {
    let BundleError::UnknownKey { key, site, .. } =
        refused("init:\n  questions:\n    - id: a\n      promt: x\n")
    else {
        panic!("unknown key");
    };
    assert_eq!(key, "promt");
    assert_eq!(site, "`init.questions[]`");
}

#[test]
fn a_run_section_is_judged_as_any_bundle_is() {
    // An unknown key inside run: is refused by the bundle's own strictness.
    let BundleError::UnknownKey { key, .. } = refused("run:\n  version: 1\n  frags: {}\n") else {
        panic!("unknown key");
    };
    assert_eq!(key, "frags");
    // And a run section that declares nothing is an empty bundle: refused,
    // unless init carries the document.
    assert!(matches!(
        refused("run:\n  description: only\n"),
        BundleError::Empty { .. }
    ));
}

#[test]
fn values_of_the_wrong_shape_are_refused() {
    for text in [
        "init: just text\n",
        "init:\n  legacy_context: a\n",
        "init:\n  questions: nope\n",
        "init:\n  questions:\n    - plain\n",
        "init:\n  tooling:\n    - a\n",
        "- a\n",
    ] {
        assert!(
            matches!(refused(text), BundleError::BadValue { .. }),
            "{text:?}"
        );
    }
}

#[test]
fn text_the_yaml_reader_refuses_is_a_syntax_error() {
    assert!(matches!(refused("run: [\n"), BundleError::Syntax { .. }));
}
