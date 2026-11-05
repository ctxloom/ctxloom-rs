//! The diagnostics: each says its code and args, and its message uses exactly them.

use super::{diagnostic_samples, EN_US};

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

// Every finding is fatal: exit 3, whatever the mode.
#[test]
fn every_finding_exits_three() {
    for sample in diagnostic_samples() {
        assert!(sample.code().starts_with("isolation-"), "{}", sample.code());
    }
    for finding in [
        super::Finding::NoRuntime {
            demanded: super::Ownership::Rootless,
        },
        super::Finding::ImageUnavailable {
            demanded: super::Ownership::Rootful,
        },
    ] {
        assert_eq!(finding.exit_code(), 3);
    }
}

// The args are the catalog's inputs: the runtime in its own words.
#[test]
fn the_args_name_the_runtimes_in_their_own_words() {
    use super::{Finding, Ownership};
    use angzarr_diagnostic::Diagnostic;
    let arg = |finding: &Finding| -> Vec<(String, String)> {
        finding
            .args()
            .into_iter()
            .map(|(n, a)| (n.to_string(), format!("{a:?}")))
            .collect()
    };
    let none = arg(&Finding::NoRuntime {
        demanded: Ownership::Rootless,
    });
    assert!(none[0].1.contains("container-rootless"), "{none:?}");
    let image = arg(&Finding::ImageUnavailable {
        demanded: Ownership::Rootful,
    });
    assert!(image[0].1.contains("container-rootful"), "{image:?}");
    let mismatch = arg(&Finding::OwnershipMismatch {
        demanded: Ownership::Rootless,
        offered: vec![Ownership::Rootful, Ownership::Rootless],
    });
    assert!(
        mismatch[1]
            .1
            .contains("container-rootful, container-rootless"),
        "{mismatch:?}"
    );
}
