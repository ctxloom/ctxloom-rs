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
