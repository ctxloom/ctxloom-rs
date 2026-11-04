//! A unified diff of two texts (Go's `difflib.GetUnifiedDiffString`, context
//! 3).

/// The unified diff from `before` to `after` of the file `path`: `--- a/<path>`
/// and `+++ b/<path>`, then hunks of three lines of context, each prefixed
/// ` `, `-` or `+`. Empty when the two are the same.
pub fn unified_diff(path: &str, before: &str, after: &str) -> String {
    let (a, b) = (lines(before), lines(after));
    let ops = edit_script(&a, &b);
    if ops.iter().all(|op| matches!(op, Op::Equal(_))) {
        return String::new();
    }
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    for hunk in hunks(&ops) {
        // The lines of each text before the hunk.
        let (old, new) = ops[..hunk.start]
            .iter()
            .fold((0, 0), |(old, new), op| match op {
                Op::Equal(_) => (old + 1, new + 1),
                Op::Del(_) => (old + 1, new),
                Op::Add(_) => (old, new + 1),
            });
        out.push_str(&hunk_text(&ops[hunk], old, new));
    }
    out
}

/// What a line of the edit script is.
enum Op<'a> {
    /// In both.
    Equal(&'a str),
    /// Only in the old text.
    Del(&'a str),
    /// Only in the new text.
    Add(&'a str),
}

/// Lines of context around a change.
const CONTEXT: usize = 3;

/// `text` as lines, without the final line break.
fn lines(text: &str) -> Vec<&str> {
    let mut all: Vec<&str> = text.split('\n').collect();
    if all.last() == Some(&"") {
        all.pop();
    }
    all
}

/// The lines of `a` and `b` as a sequence of equal, deleted and added lines:
/// their common start and end first, then a longest common subsequence of
/// what is between (replaced whole when that would be too large to work out).
fn edit_script<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<Op<'a>> {
    let start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let end = a[start..]
        .iter()
        .rev()
        .zip(b[start..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (mid_a, mid_b) = (&a[start..a.len() - end], &b[start..b.len() - end]);
    let mut ops: Vec<Op<'a>> = a[..start].iter().map(|l| Op::Equal(l)).collect();
    ops.extend(middle(mid_a, mid_b));
    ops.extend(a[a.len() - end..].iter().map(|l| Op::Equal(l)));
    ops
}

/// The edit script of the lines between the common start and end.
fn middle<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<Op<'a>> {
    const LIMIT: usize = 4_000_000;
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > LIMIT {
        let mut ops: Vec<Op<'a>> = a.iter().map(|l| Op::Del(l)).collect();
        ops.extend(b.iter().map(|l| Op::Add(l)));
        return ops;
    }
    // lcs[i][j]: the length of a longest common subsequence of a[i..], b[j..].
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut ops = Vec::new();
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            ops.push(Op::Equal(a[i]));
            i += 1;
            j += 1;
        } else if i < n && lcs[i + 1][j] == lcs[i][j] {
            ops.push(Op::Del(a[i]));
            i += 1;
        } else {
            ops.push(Op::Add(b[j]));
            j += 1;
        }
    }
    ops
}

/// The ranges of the script that are hunks: each change with its context,
/// changes no more than twice the context apart sharing one.
fn hunks(ops: &[Op<'_>]) -> Vec<std::ops::Range<usize>> {
    let changes: Vec<usize> = (0..ops.len())
        .filter(|&i| !matches!(ops[i], Op::Equal(_)))
        .collect();
    let mut hunks: Vec<std::ops::Range<usize>> = Vec::new();
    for &at in &changes {
        let from = at.saturating_sub(CONTEXT);
        let to = (at + CONTEXT + 1).min(ops.len());
        match hunks.last_mut() {
            Some(last) if from <= last.end => last.end = to,
            _ => hunks.push(from..to),
        }
    }
    hunks
}

/// One hunk as text: its header (given the lines of each text before it),
/// then its lines.
fn hunk_text(ops: &[Op<'_>], old_before: usize, new_before: usize) -> String {
    let (mut old_len, mut new_len) = (0, 0);
    let mut body = String::new();
    for op in ops {
        let (mark, line) = match op {
            Op::Equal(line) => {
                old_len += 1;
                new_len += 1;
                (' ', line)
            }
            Op::Del(line) => {
                old_len += 1;
                ('-', line)
            }
            Op::Add(line) => {
                new_len += 1;
                ('+', line)
            }
        };
        body.push(mark);
        body.push_str(line);
        body.push('\n');
    }
    format!(
        "@@ -{} +{} @@\n{body}",
        range(old_before, old_len),
        range(new_before, new_len)
    )
}

/// A unified range: `start,length`; a one-line range is `start`, an empty one
/// is `before,0` (the line it comes after).
fn range(begin: usize, length: usize) -> String {
    match length {
        0 => format!("{begin},0"),
        1 => format!("{}", begin + 1),
        _ => format!("{},{length}", begin + 1),
    }
}

#[cfg(test)]
#[path = "diff.test.rs"]
mod tests;
