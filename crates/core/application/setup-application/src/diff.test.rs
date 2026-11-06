//! The unified diff, pinned by whole outputs.

use super::unified;

fn lines(range: std::ops::RangeInclusive<u32>) -> String {
    range.map(|n| format!("line {n}\n")).collect()
}

#[test]
fn test_the_same_text_has_no_diff() {
    assert_eq!(unified("a\nb\n", "a\nb\n", "x", "y"), "");
    assert_eq!(unified("", "", "x", "y"), "");
}

#[test]
fn test_a_line_only_in_the_second_is_added() {
    assert_eq!(
        unified("a\nb\n", "a\nNEW\nb\n", "theirs", "mine"),
        "--- theirs\n+++ mine\n@@ -1,2 +1,3 @@\n a\n+NEW\n b\n"
    );
}

#[test]
fn test_a_line_only_in_the_first_is_removed() {
    assert_eq!(
        unified("a\nGONE\nb\n", "a\nb\n", "theirs", "mine"),
        "--- theirs\n+++ mine\n@@ -1,3 +1,2 @@\n a\n-GONE\n b\n"
    );
}

#[test]
fn test_a_changed_line_is_removed_then_added() {
    assert_eq!(
        unified("a\nold\nb\n", "a\nnew\nb\n", "x", "y"),
        "--- x\n+++ y\n@@ -1,3 +1,3 @@\n a\n-old\n+new\n b\n"
    );
}

#[test]
fn test_an_empty_side_spells_its_range_as_zero_lines() {
    assert_eq!(
        unified("", "a\n", "x", "y"),
        "--- x\n+++ y\n@@ -0,0 +1 @@\n+a\n"
    );
    assert_eq!(
        unified("a\n", "", "x", "y"),
        "--- x\n+++ y\n@@ -1 +0,0 @@\n-a\n"
    );
}

#[test]
fn test_context_stops_at_three_lines() {
    let from = lines(1..=10);
    let to = from.replace("line 5\n", "FIVE\n");
    assert_eq!(
        unified(&from, &to, "x", "y"),
        "--- x\n+++ y\n@@ -2,7 +2,7 @@\n line 2\n line 3\n line 4\n-line 5\n+FIVE\n line 6\n line 7\n line 8\n"
    );
}

#[test]
fn test_distant_changes_make_separate_hunks() {
    let from = lines(1..=20);
    let to = from
        .replace("line 2\n", "TWO\n")
        .replace("line 19\n", "NINETEEN\n");
    let diff = unified(&from, &to, "x", "y");
    assert_eq!(diff.matches("@@ -").count(), 2, "{diff}");
    assert!(diff.contains("@@ -1,5 +1,5 @@\n"), "{diff}");
    assert!(diff.contains("@@ -16,5 +16,5 @@\n"), "{diff}");
}

#[test]
fn test_changes_within_two_contexts_share_a_hunk() {
    let from = lines(1..=12);
    // Six unchanged lines between the two changes: 2 * context.
    let near = from.replace("line 2\n", "A\n").replace("line 9\n", "B\n");
    assert_eq!(unified(&from, &near, "x", "y").matches("@@ -").count(), 1);
    // Seven apart: two hunks.
    let far = from.replace("line 2\n", "A\n").replace("line 10\n", "B\n");
    assert_eq!(unified(&from, &far, "x", "y").matches("@@ -").count(), 2);
}

/// The length of the longest common subsequence, by the plain recursion.
fn lcs(a: &[&str], b: &[&str]) -> usize {
    match (a.split_first(), b.split_first()) {
        (Some((x, a_rest)), Some((y, b_rest))) => {
            if x == y {
                1 + lcs(a_rest, b_rest)
            } else {
                lcs(a_rest, b).max(lcs(a, b_rest))
            }
        }
        _ => 0,
    }
}

#[test]
fn test_the_edit_script_is_minimal_and_loses_no_line() {
    // A fixed generator over a three-letter alphabet: many repeated lines,
    // so a wrong choice of what to keep costs extra changes.
    let mut seed: u32 = 7;
    let mut next = move |bound: u32| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (seed >> 16) % bound
    };
    for _ in 0..300 {
        let make = |len: u32, next: &mut dyn FnMut(u32) -> u32| -> Vec<&'static str> {
            (0..len)
                .map(|_| ["a", "b", "c"][next(3) as usize])
                .collect()
        };
        let (la, lb) = (next(8), next(8));
        let from = make(la, &mut next);
        let to = make(lb, &mut next);
        let text = |lines: &[&str]| lines.iter().map(|l| format!("{l}\n")).collect::<String>();
        let diff = unified(&text(&from), &text(&to), "x", "y");
        let removed = diff
            .lines()
            .filter(|l| l.starts_with('-') && !l.starts_with("---"))
            .count();
        let added = diff
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .count();
        let kept = lcs(&from, &to);
        assert_eq!(removed, from.len() - kept, "{from:?} -> {to:?}\n{diff}");
        assert_eq!(added, to.len() - kept, "{from:?} -> {to:?}\n{diff}");
        // With every line within context of a change, the hunks are the texts.
        if from.len() <= 3 && to.len() <= 3 {
            let side = |marks: [char; 2]| -> Vec<&str> {
                diff.lines()
                    .skip(2)
                    .filter(|l| !l.starts_with("@@"))
                    .filter(|l| marks.contains(&l.chars().next().unwrap_or('?')))
                    .map(|l| &l[1..])
                    .collect()
            };
            if !diff.is_empty() {
                assert_eq!(side([' ', '-']), from, "{diff}");
                assert_eq!(side([' ', '+']), to, "{diff}");
            }
        }
    }
}
