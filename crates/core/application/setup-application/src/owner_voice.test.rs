use super::*;

// D24: the owner's text, pinned. The digests are of the text at Go release/0.7
// f36e8dfb2 (cmd/ctxloom/loadout.yaml, run.fragments.isolation-axes).
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[test]
fn test_the_isolation_fragment_is_the_owners_text_unedited() {
    assert_eq!(ISOLATION_AXES.len(), 2084);
    assert_eq!(ISOLATION_AXES_DISTILLED.len(), 1825);
    assert_eq!(fnv(ISOLATION_AXES), PINNED_FULL);
    assert_eq!(fnv(ISOLATION_AXES_DISTILLED), PINNED_DISTILLED);
    for text in [ISOLATION_AXES, ISOLATION_AXES_DISTILLED] {
        assert!(text.starts_with("## Isolation: specify both axes\n"));
        assert!(text.ends_with("requested.\n"));
    }
}

#[test]
fn test_the_fragment_follows_the_context_after_a_rule_and_stands_alone_in_an_empty_one() {
    let joined = with_own_fragment("CONTEXT", true);
    assert!(joined.starts_with("CONTEXT\n\n---\n\n## Isolation"));
    assert!(joined.ends_with(ISOLATION_AXES_DISTILLED));
    assert_eq!(with_own_fragment("  \n", false), ISOLATION_AXES);
    assert!(with_own_fragment("c", false).ends_with(ISOLATION_AXES));
}

const PINNED_FULL: u64 = 8215271392870792838;
const PINNED_DISTILLED: u64 = 10487370699630746071;
