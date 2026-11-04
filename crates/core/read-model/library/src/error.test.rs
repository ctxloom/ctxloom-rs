//! The catalog test for this crate (CLAUDE.md, "Errors are localizable"):
//! every code is prefixed with the crate's name, every sample has a message,
//! and a message uses exactly its error's args. (The binary's `catalog.test.rs`
//! holds the same facts for the whole graph; a crate is judged by its own
//! tests.)

use std::collections::BTreeSet;

use super::*;
use crate::{BundleError, CatalogError, ProfileError};

angzarr_presentation::english_for!(&[crate::EN_US.1]);

/// The `$variables` of the message `id` in the catalog: its own lines, up to
/// the next message. None when the catalog has none.
fn message_vars(id: &str) -> Option<BTreeSet<String>> {
    let mut lines = EN_US
        .1
        .lines()
        .skip_while(|l| !l.starts_with(&format!("{id} =")));
    let first = lines.next()?;
    let rest = lines.take_while(|l| l.starts_with(' ') || l.is_empty());
    let block: Vec<&str> = std::iter::once(first).chain(rest).collect();
    let text = block.join("\n");
    let mut found = BTreeSet::new();
    let mut tail = text.as_str();
    while let Some(at) = tail.find('$') {
        let name: String = tail[at + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if !name.is_empty() {
            found.insert(name);
        }
        tail = &tail[at + 1..];
    }
    Some(found)
}

// CLAUDE.md "Errors are localizable": a code is the crate's name then the error
#[test]
fn test_every_code_is_prefixed_and_its_message_uses_exactly_its_args() {
    let samples = diagnostic_samples();
    for sample in &samples {
        let code = sample.code();
        assert!(code.starts_with("library-"), "{code}");
        let args: BTreeSet<String> = sample.args().iter().map(|(k, _)| k.to_string()).collect();
        assert_eq!(message_vars(code).as_ref(), Some(&args), "{code}");
        let english = sample.english();
        assert!(
            !english.is_empty() && english != code,
            "{code}: {english:?}"
        );
        assert!(
            !english.contains('$') && !english.contains('{'),
            "{code}: {english:?}"
        );
    }
}

// the catalog holds no message no error raises
#[test]
fn test_every_message_of_the_catalog_has_a_sample() {
    let sampled: BTreeSet<String> = diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();
    let messages: BTreeSet<String> = EN_US
        .1
        .lines()
        .filter(|l| l.starts_with("library-"))
        .filter_map(|l| l.split_once(" =").map(|(id, _)| id.to_string()))
        .filter(|id| !id.ends_with("-fix"))
        .collect();
    assert_eq!(sampled, messages);
}

// Display is the diagnostic's code and its args, whatever the error
#[test]
fn test_display_is_the_code_and_the_args() {
    let r = RefError::Syntax {
        detail: "empty".into(),
    };
    assert_eq!(r.to_string(), "library-ref-syntax detail=empty");
    let p = ProfileError::NotFound { name: "x".into() };
    assert_eq!(p.to_string(), "library-profile-not-found name=x");
    let b = BundleError::NoManifest {
        bundle: "demo".into(),
    };
    assert_eq!(b.to_string(), "library-bundle-no-manifest bundle=demo");
    let c = CatalogError::BundleNotFound { ask: "z".into() };
    assert_eq!(c.to_string(), "library-catalog-bundle-not-found ask=z");
    assert_eq!(
        ConfigError::NotAMapping.to_string(),
        "library-config-not-a-mapping"
    );
}

// remote: the plain sentence of a reference error, as a finding's text
#[test]
fn test_a_reference_error_has_a_plain_sentence() {
    assert_eq!(
        RefError::Syntax {
            detail: "empty".into()
        }
        .plain(),
        "malformed bundle reference: empty"
    );
    assert_eq!(
        RefError::Selector {
            selector: "fragments".into()
        }
        .plain(),
        "selector \"fragments\" must be <kind>/<name>"
    );
    assert_eq!(
        RefError::UnknownKind {
            kind: "widgets".into()
        }
        .plain(),
        "unknown item kind \"widgets\" (want fragments|commands|mcp|hooks|skills)"
    );
}
