//! A markdown item's front matter, and the names of the files an item's parts
//! are in (Go's `internal/adapters/content/frontmatter.go` and `paths.go`).

/// An item file split into its front matter and its body.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Split<'a> {
    /// The YAML between the fences.
    pub meta: &'a str,
    /// Everything after the closing fence, verbatim.
    pub body: &'a str,
}

/// The front matter of `raw`, if it has any. It is a block only when the very
/// first line is `---` and a later line is `---` too; the first such closing
/// fence ends it and nothing after is scanned (a body may hold its own rule).
/// An unterminated block is not front matter.
pub fn split_front_matter(raw: &str) -> Option<Split<'_>> {
    let rest = cut_fence(raw)?;
    let mut offset = 0;
    while offset <= rest.len() {
        let end = rest[offset..].find('\n');
        let (line, next) = match end {
            Some(i) => (&rest[offset..offset + i], offset + i + 1),
            None => (&rest[offset..], rest.len()),
        };
        if line.trim_end_matches([' ', '\t', '\r']) == FENCE {
            return Some(Split {
                meta: &rest[..offset],
                body: &rest[next..],
            });
        }
        end?;
        offset = next;
    }
    None
}

const FENCE: &str = "---";

/// `text` past its first line, when that line is a fence.
fn cut_fence(text: &str) -> Option<&str> {
    let end = text.find('\n')?;
    (text[..end].trim_end_matches([' ', '\t', '\r']) == FENCE).then(|| &text[end + 1..])
}

/// Whether `path` is a metadata sidecar: a dot-prefixed `.<name>.meta.yaml`.
pub fn is_meta_path(path: &str) -> bool {
    let base = base_name(path);
    base.starts_with('.') && base.ends_with(META_SUFFIX) && base != "..meta.yaml"
}

const META_SUFFIX: &str = ".meta.yaml";

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The sidecar of the content file at `path`: `mcp/postgres.yaml` has
/// `mcp/.postgres.meta.yaml`.
pub fn meta_path(path: &str) -> String {
    let (dir, base) = path.rsplit_once('/').map_or(("", path), |(d, b)| (d, b));
    let stem = base.rsplit_once('.').map_or(base, |(s, _)| s);
    if dir.is_empty() {
        format!(".{stem}{META_SUFFIX}")
    } else {
        format!("{dir}/.{stem}{META_SUFFIX}")
    }
}

/// The sidecar of an item named `name` in the kind directory `dir` that has
/// no single content file (a skill package): `skills`, `code-reviewer` is
/// `skills/.code-reviewer.meta.yaml`. The dot goes on the last segment of the
/// name.
pub fn meta_path_for_name(dir: &str, name: &str) -> String {
    let (parent, base) = name.rsplit_once('/').map_or(("", name), |(p, b)| (p, b));
    if parent.is_empty() {
        format!("{dir}/.{base}{META_SUFFIX}")
    } else {
        format!("{dir}/{parent}/.{base}{META_SUFFIX}")
    }
}

/// The name an item's parts group under: the file's base name, with its
/// extension, a sidecar's dot and `.meta.yaml`, and a `.distilled` form
/// suffix taken off.
pub fn stem_of(path: &str) -> String {
    let base = base_name(path);
    let logical = if is_meta_path(path) {
        base.strip_prefix('.')
            .and_then(|b| b.strip_suffix(META_SUFFIX))
            .unwrap_or(base)
    } else {
        base.rsplit_once('.').map_or(base, |(stem, _)| stem)
    };
    logical
        .strip_suffix(".distilled")
        .unwrap_or(logical)
        .to_string()
}

#[cfg(test)]
#[path = "frontmatter.test.rs"]
mod tests;
