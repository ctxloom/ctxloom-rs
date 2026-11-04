//! The diagnostics: each says its code and args, and its message uses exactly them.

use super::{diagnostic_samples, CATALOGS};

// Every sample says its code (Display is the code then the args, never
// English), its catalog message uses exactly its args, and the samples are
// not empty.
#[test]
fn every_sample_displays_its_code_and_its_message_uses_exactly_its_args() {
    let catalogs: Vec<&str> = CATALOGS.iter().map(|c| c.1).collect();
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
