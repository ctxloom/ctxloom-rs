//! `init`. Go: `cli/init.feature` (release/0.7, f36e8dfb21).

use crate::{personal_name, prompt, SETUP_INTERVIEW};

// init.feature: "Init prompt emits the whole setup interview body": the
// headings from both ends of it.
#[test]
fn the_prompt_carries_the_phase_headings_from_both_ends() {
    let text = prompt();
    for heading in [
        "Phase 1 \u{2014} Open, orient, scan",
        "Phase 2 \u{2014} Companions",
        "Phase 4 \u{2014} Agents",
        "Phase 5 \u{2014} Close",
    ] {
        assert!(text.contains(heading), "{heading}");
    }
    assert!(text.ends_with(&format!("{SETUP_INTERVIEW}\n")));
    assert!(!text.starts_with("---"), "the front matter is dropped");
}

// init.feature: the personal remote is "personal"; the next are numbered.
#[test]
fn the_remotes_are_named_personal_then_numbered() {
    assert_eq!(personal_name(0), "personal");
    assert_eq!(personal_name(1), "personal-2");
    assert_eq!(personal_name(2), "personal-3");
}

#[test]
fn every_sample_is_in_the_catalog() {
    let catalog = crate::EN_US.1;
    for sample in crate::diagnostic_samples() {
        assert!(catalog.contains(&format!("{} =", sample.code())));
    }
}
