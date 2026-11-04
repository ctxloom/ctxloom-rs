//! The catalog test for this crate (CLAUDE.md, "Errors are localizable"):
//! every code is prefixed with the crate's name, every sample has a message,
//! and a message uses exactly its error's args.

use std::collections::BTreeSet;

use super::*;

angzarr_presentation::english_for!(&[crate::EN_US.1]);

/// The `$variables` of the message `id`: its own lines, up to the next
/// message.
fn message_vars(id: &str) -> Option<BTreeSet<String>> {
    let mut lines = EN_US
        .1
        .lines()
        .skip_while(|l| !l.starts_with(&format!("{id} =")));
    let first = lines.next()?;
    let rest = lines.take_while(|l| l.starts_with(' ') || l.is_empty());
    let text: Vec<&str> = std::iter::once(first).chain(rest).collect();
    let text = text.join("\n");
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

#[test]
fn test_every_code_is_prefixed_and_its_message_uses_exactly_its_args() {
    let samples = diagnostic_samples();
    assert!(!samples.is_empty());
    for sample in &samples {
        let code = sample.code();
        assert!(code.starts_with("deps-"), "{code}");
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

#[test]
fn test_every_message_of_the_catalog_has_a_sample() {
    let sampled: BTreeSet<String> = diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();
    let messages: BTreeSet<String> = EN_US
        .1
        .lines()
        .filter(|l| l.starts_with("deps-"))
        .filter_map(|l| l.split_once(" =").map(|(id, _)| id.to_string()))
        .filter(|id| !id.ends_with("-fix"))
        .collect();
    assert_eq!(sampled, messages);
}

#[test]
fn test_display_is_the_code_and_the_args() {
    assert_eq!(DepsError::EmptyName.to_string(), "deps-empty-name");
    assert_eq!(
        DepsError::RemoteExists { name: "o".into() }.to_string(),
        "deps-remote-exists name=o"
    );
}
