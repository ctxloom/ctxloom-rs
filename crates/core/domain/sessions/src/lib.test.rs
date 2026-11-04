//! The rules that need no stream: the payloads on the wire and the catalog.
//! The rules a stream decides are `aggregate.test.rs`'s, through
//! angzarr-embedded's kernel.

use crate::{diagnostic_samples, EN_US, NAMES_DOMAIN, SESSION_DOMAIN};

// The domains are spelled as the markup takes them, literals in the
// `#[aggregate]` attributes; the constants are what the rest of ctxloom reads.
#[test]
fn the_domain_names_are_the_ones_the_plan_gives() {
    assert_eq!(SESSION_DOMAIN, "sessions");
    assert_eq!(NAMES_DOMAIN, "session-names");
}

// Every code is prefixed with the crate's name and has a message that uses
// exactly its args (the binary's catalog test holds the same for the whole
// graph; this one fails in this crate first).
#[test]
fn every_diagnostic_has_a_message_that_uses_exactly_its_args() {
    let (name, catalog) = EN_US;
    assert_eq!(name, "sessions");
    for sample in diagnostic_samples() {
        let code = sample.code();
        assert!(code.starts_with("sessions-"), "{code} is prefixed");
        let line = catalog
            .lines()
            .find(|l| l.starts_with(&format!("{code} =")))
            .unwrap_or_else(|| panic!("{code} has no message"));
        let mut args: Vec<&str> = sample.args().into_iter().map(|(n, _)| n).collect();
        args.sort_unstable();
        let mut used: Vec<&str> = line
            .split('$')
            .skip(1)
            .map(|rest| {
                rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                    .next()
                    .unwrap()
            })
            .collect();
        used.sort_unstable();
        used.dedup();
        assert_eq!(args, used, "{code}");
    }
}

// Every sample says its code (Display is the code then the args, never
// English), its catalog message uses exactly its args, and the samples are
// not empty.
#[test]
fn every_sample_displays_its_code_and_its_message_uses_exactly_its_args() {
    let catalogs: Vec<&str> = vec![EN_US.1];
    let samples = diagnostic_samples();
    assert!(!samples.is_empty());
    for sample in samples {
        let shown = format!("{sample}");
        assert!(shown.starts_with(sample.code()), "{shown}");
        let names: Vec<&str> = sample.args().into_iter().map(|(n, _)| n).collect();
        assert_eq!(shown.contains('='), !names.is_empty(), "{shown}");
        let line = catalogs
            .iter()
            .flat_map(|c| c.lines())
            .find(|l| l.starts_with(&format!("{} =", sample.code())))
            .unwrap_or_else(|| panic!("{} has no message", sample.code()));
        let mut used: Vec<&str> = line
            .split('$')
            .skip(1)
            .map(|r| {
                r.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                    .next()
                    .unwrap()
            })
            .collect();
        used.sort_unstable();
        used.dedup();
        let mut names = names;
        names.sort_unstable();
        assert_eq!(names, used, "{}", sample.code());
    }
}
