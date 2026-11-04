//! Publisher-authored text made safe to show on a terminal (Go's
//! `internal/core/displaysafe` and `adapters/cli/inert_render.go`): every
//! character a bundle's author could use to repaint a terminal or hide part
//! of what is printed (ESC and the sequences it opens, the other controls,
//! bidi overrides, zero-width characters, invalid UTF-8) comes out as a visible
//! marker, never as the character itself.

/// `s` with its dangerous characters shown as markers. `multiline` keeps
/// line breaks (CRLF is one); without it a newline is a marker like any other
/// control. A tab is four spaces.
pub fn display_safe(s: &str, multiline: bool) -> String {
    let text;
    let s = if multiline && s.contains("\r\n") {
        text = s.replace("\r\n", "\n");
        text.as_str()
    } else {
        s
    };
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' if multiline => out.push('\n'),
            '\t' => out.push_str("    "),
            '\u{1b}' => out.push_str("\u{27e8}ESC\u{27e9}"),
            c if !is_display_safe(c) => {
                out.push_str(&format!("\u{27e8}U+{:04X}\u{27e9}", c as u32))
            }
            c => out.push(c),
        }
    }
    out
}

/// Whether a terminal shows `c` as itself: not a control, not a format
/// character, not a line or paragraph separator.
fn is_display_safe(c: char) -> bool {
    !(c.is_control() || is_format(c) || matches!(c, '\u{2028}' | '\u{2029}'))
}

/// Unicode's general category Cf.
fn is_format(c: char) -> bool {
    const RANGES: [(u32, u32); 20] = [
        (0x00AD, 0x00AD),
        (0x0600, 0x0605),
        (0x061C, 0x061C),
        (0x06DD, 0x06DD),
        (0x070F, 0x070F),
        (0x0890, 0x0891),
        (0x08E2, 0x08E2),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x2064),
        (0x2066, 0x206F),
        (0xFEFF, 0xFEFF),
        (0xFFF9, 0xFFFB),
        (0x110BD, 0x110BD),
        (0x110CD, 0x110CD),
        (0x13430, 0x1343F),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0001, 0xE007F),
    ];
    let code = c as u32;
    RANGES.iter().any(|&(lo, hi)| (lo..=hi).contains(&code))
}

/// `s` made safe for one line, cut to `max_bytes` with an ellipsis where it
/// is longer.
pub fn field(s: &str, max_bytes: usize) -> String {
    ellipsize(&display_safe(s, false), max_bytes)
}

/// `s` cut to at most `max_bytes` on a character boundary.
pub fn truncate_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// `s` cut to `max_bytes` with an ellipsis where it is longer.
pub fn ellipsize(s: &str, max_bytes: usize) -> String {
    const ELLIPSIS: &str = "\u{2026}";
    if s.len() <= max_bytes {
        return s.to_string();
    }
    if max_bytes <= ELLIPSIS.len() {
        return truncate_bytes(s, max_bytes).to_string();
    }
    format!(
        "{}{ELLIPSIS}",
        truncate_bytes(s, max_bytes - ELLIPSIS.len())
    )
}

/// The first line of `content`, trimmed, cut to 70 bytes: a preview.
pub fn preview(content: &str) -> String {
    let first = content.trim().split('\n').next().unwrap_or("").trim();
    ellipsize(first, 70)
}

/// The most bytes of publisher-authored body shown.
const BODY_MAX_BYTES: usize = 256 << 10;

/// The longest run of line breaks that survives a collapse.
const MAX_NEWLINE_RUN: usize = 4;

/// A publisher-authored body made safe to show, and the notice that says it
/// was altered.
pub struct Body {
    /// The text, each line behind `indent`, ending in a line break.
    pub text: String,
    /// What was done to it, empty if nothing.
    pub notice: String,
}

/// A body of text from a bundle, rendered inert: unsafe characters shown as
/// markers, runs of blank lines collapsed (when asked), the whole cut to
/// 256 KiB; each line behind `indent`. `empty` stands for no body at all.
pub fn body(indent: &str, empty: &str, collapse: bool, reference: &str, content: &str) -> Body {
    let safe = display_safe(content, true);
    let escaped =
        safe.matches('\u{27e8}').count() as i64 - content.matches('\u{27e8}').count() as i64;
    let (collapsed, removed) = if collapse {
        collapse_blank_runs(&safe)
    } else {
        (safe, 0)
    };
    let mut parts = Vec::new();
    if escaped > 0 {
        parts.push(format!("{escaped} control character(s) escaped"));
    }
    if removed > 0 {
        parts.push(format!("{removed} blank line(s) collapsed"));
    }
    let mut shown = collapsed.as_str();
    if shown.len() > BODY_MAX_BYTES {
        let from = shown.len();
        shown = truncate_bytes(shown, BODY_MAX_BYTES);
        parts.push(format!("truncated to {} of {from} bytes", shown.len()));
    }
    let trimmed = shown.trim_end_matches('\n');
    let mut text = String::new();
    if trimmed.is_empty() {
        if !empty.is_empty() {
            text.push_str(indent);
            text.push_str(empty);
        }
        text.push('\n');
    } else {
        for line in trimmed.split('\n') {
            text.push_str(indent);
            text.push_str(line);
            text.push('\n');
        }
    }
    let notice = if parts.is_empty() {
        String::new()
    } else {
        format!(
            "{} is publisher-authored and was rendered inert for this terminal ({}); use --format json for the raw bytes",
            field(reference, 256),
            parts.join(", ")
        )
    };
    Body { text, notice }
}

/// Runs of line breaks longer than four cut to four; how many were cut.
fn collapse_blank_runs(s: &str) -> (String, usize) {
    let mut out = String::with_capacity(s.len());
    let (mut run, mut removed) = (0, 0);
    for c in s.chars() {
        if c == '\n' {
            run += 1;
            if run > MAX_NEWLINE_RUN {
                removed += 1;
                continue;
            }
        } else {
            run = 0;
        }
        out.push(c);
    }
    (out, removed)
}

#[cfg(test)]
#[path = "inert.test.rs"]
mod tests;
