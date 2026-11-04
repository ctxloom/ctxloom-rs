//! The reference grammar (`docs/reference-grammar.md`; Go's
//! `internal/shared/refuri` and `internal/core/ident`): how a bundle, an item
//! of a bundle and a version are spelled, parsed and rendered.
//!
//! `ctxloom+<class>:` carries the source class: `git`
//! (`ctxloom+git://host/owner/repo//bundles/<b>`), `file`
//! (`ctxloom+file:///abs/repo//bundles/<b>`), `local` (`ctxloom+local:<b>`,
//! a bundle of this project) and `companion` (`ctxloom+companion:<bin>`). `//`
//! separates the repository's path from the bundle's, `@<version>` follows the
//! bundle, and `#<kind>/<name>` selects one item.

use crate::error::RefError;

/// The scheme prefix every canonical reference begins with.
pub const SCHEME_PREFIX: &str = "ctxloom+";

/// Where a bundle comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceClass {
    Git,
    File,
    Local,
    Companion,
}

impl SourceClass {
    /// The classes there are.
    pub const ALL: [SourceClass; 4] = [
        SourceClass::Git,
        SourceClass::File,
        SourceClass::Local,
        SourceClass::Companion,
    ];

    /// The class's name in a scheme (`git`, `file`, `local`, `companion`).
    pub fn name(self) -> &'static str {
        match self {
            SourceClass::Git => "git",
            SourceClass::File => "file",
            SourceClass::Local => "local",
            SourceClass::Companion => "companion",
        }
    }
}

/// A canonical reference, taken apart.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Parts {
    pub class: SourceClass,
    /// The host (git only), as canonicalized.
    pub host: String,
    /// The repository's path (git and file), with its leading slash.
    pub repo_path: String,
    /// The bundle's name: a path of one or more segments.
    pub bundle: String,
    /// The version expression after `@`, decoded; empty for none.
    pub version: String,
    /// The `#` fragment (an item selector), decoded; empty for none.
    pub fragment: String,
}

/// Whether `raw` claims the canonical family (begins `ctxloom+`), so that a
/// class this build does not know is refused rather than read as a bare name.
pub fn has_scheme(raw: &str) -> bool {
    raw.trim().starts_with(SCHEME_PREFIX)
}

fn syntax(detail: impl Into<String>) -> RefError {
    RefError::Syntax {
        detail: detail.into(),
    }
}

impl Parts {
    /// Parse a canonical reference.
    pub fn parse(raw: &str) -> Result<Parts, RefError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(syntax("empty"));
        }
        if let Some(at) = raw.bytes().position(|b| b < 0x20 || b == 0x7f) {
            return Err(syntax(format!("control character at byte {at}")));
        }
        if let Some(at) = encoded_slash(raw) {
            return Err(syntax(format!(
                "encoded slash (%2F) at byte {at} is not addressable"
            )));
        }
        let decoded = decode_unreserved_escapes(raw).map_err(syntax)?;

        let (scheme, rest) = decoded.split_once(':').unwrap_or(("", decoded.as_str()));
        let class = SourceClass::ALL
            .into_iter()
            .find(|c| scheme.strip_prefix(SCHEME_PREFIX) == Some(c.name()))
            .ok_or_else(|| {
                syntax(format!(
                    "unknown scheme {scheme:?} (want {p}git|{p}file|{p}local|{p}companion)",
                    p = SCHEME_PREFIX
                ))
            })?;
        let (rest, fragment) = match rest.split_once('#') {
            Some((before, after)) => (before, path_unescape(after).map_err(syntax)?),
            None => (rest, String::new()),
        };
        if rest.contains('?') {
            return Err(syntax("query string is not part of a bundle reference"));
        }
        let mut parts = Parts {
            class,
            host: String::new(),
            repo_path: String::new(),
            bundle: String::new(),
            version: String::new(),
            fragment,
        };
        match rest.strip_prefix("//") {
            Some(authority_and_path) => {
                let (authority, path) = match authority_and_path.find('/') {
                    Some(i) => authority_and_path.split_at(i),
                    None => (authority_and_path, ""),
                };
                if authority.contains('@') {
                    return Err(syntax("userinfo is not part of a bundle reference"));
                }
                if parts.is_external() {
                    parts.parse_external(authority, path)?;
                } else {
                    return Err(syntax(format!(
                        "{SCHEME_PREFIX}{} takes an opaque name, not a path",
                        class.name()
                    )));
                }
            }
            None if parts.is_external() => parts.parse_external("", rest)?,
            None => parts.parse_internal(rest)?,
        }
        Ok(parts)
    }

    fn parse_external(&mut self, authority: &str, path: &str) -> Result<(), RefError> {
        if self.class == SourceClass::Git {
            if authority.is_empty() {
                return Err(syntax(format!("{SCHEME_PREFIX}git requires a host")));
            }
            self.host = canonical_host(authority)?;
        } else if !authority.is_empty() {
            return Err(syntax(format!(
                "{SCHEME_PREFIX}file takes no host (use {SCHEME_PREFIX}file:///<abs-path>)"
            )));
        }
        let (before, after) = path.split_once("//").ok_or_else(|| {
            syntax("missing \"//\" separator between repository path and bundle path")
        })?;
        let (bundle_esc, version) = split_version(after)?;
        let repo_esc = remove_dot_segments(before);
        let bundle_path = remove_dot_segments(&bundle_esc);
        self.version = version;
        let Some(name) = bundle_path.strip_prefix(BUNDLE_MARKER) else {
            return Err(syntax(format!(
                "bundle path must begin {BUNDLE_MARKER:?} after \"//\""
            )));
        };
        if name.is_empty() {
            return Err(syntax("empty bundle name"));
        }
        let repo_path =
            path_unescape(&repo_esc).map_err(|e| syntax(format!("repository path: {e}")))?;
        if repo_path.is_empty() || repo_path == "/" {
            return Err(syntax("empty repository path"));
        }
        self.bundle = path_unescape(name).map_err(|e| syntax(format!("bundle name: {e}")))?;
        self.repo_path = repo_path;
        Ok(())
    }

    fn parse_internal(&mut self, opaque: &str) -> Result<(), RefError> {
        if opaque.starts_with('/') {
            return Err(syntax(format!(
                "{SCHEME_PREFIX}{} takes an opaque name, not a path",
                self.class.name()
            )));
        }
        let (name_esc, version) = split_version(opaque)?;
        self.version = version;
        let name = path_unescape(&name_esc).map_err(|e| syntax(format!("name: {e}")))?;
        if name.is_empty() {
            return Err(syntax(format!("empty {} name", self.class.name())));
        }
        self.bundle = name;
        Ok(())
    }

    /// Render the reference, with its version or without.
    pub fn render(&self, with_version: bool) -> String {
        let mut out = format!("{SCHEME_PREFIX}{}:", self.class.name());
        let version = if with_version && !self.version.is_empty() {
            format!("@{}", escape_at(&escape_path(&self.version)))
        } else {
            String::new()
        };
        if self.is_external() {
            out.push_str("//");
            out.push_str(&self.host);
            out.push_str(&escape_path(&self.repo_path));
            out.push_str("//");
            out.push_str(BUNDLE_MARKER);
            out.push_str(&escape_at(&escape_path(&self.bundle)));
        } else {
            out.push_str(&escape_at(&escape_path(&self.bundle)));
        }
        out.push_str(&version);
        if !self.fragment.is_empty() {
            out.push('#');
            out.push_str(&escape_fragment(&self.fragment));
        }
        out
    }

    /// Whether the source is a repository (git or file).
    pub fn is_external(&self) -> bool {
        matches!(self.class, SourceClass::Git | SourceClass::File)
    }

    /// A `ctxloom+git` reference to `bundle` in `repo_path` at `host`.
    pub fn git(host: &str, repo_path: &str, bundle: &str) -> Result<Parts, RefError> {
        Parts {
            class: SourceClass::Git,
            host: host.to_string(),
            repo_path: leading_slash(repo_path),
            bundle: bundle.to_string(),
            version: String::new(),
            fragment: String::new(),
        }
        .mint()
    }

    /// A `ctxloom+file` reference to `bundle` in the repository at `repo_path`.
    pub fn file(repo_path: &str, bundle: &str) -> Result<Parts, RefError> {
        Parts {
            class: SourceClass::File,
            host: String::new(),
            repo_path: leading_slash(repo_path),
            bundle: bundle.to_string(),
            version: String::new(),
            fragment: String::new(),
        }
        .mint()
    }

    /// A `ctxloom+local` reference to `bundle`.
    pub fn local(bundle: &str) -> Result<Parts, RefError> {
        Parts::opaque(SourceClass::Local, bundle)
    }

    /// A `ctxloom+companion` reference to the companion `bin`.
    pub fn companion(bin: &str) -> Result<Parts, RefError> {
        Parts::opaque(SourceClass::Companion, bin)
    }

    fn opaque(class: SourceClass, bundle: &str) -> Result<Parts, RefError> {
        Parts {
            class,
            host: String::new(),
            repo_path: String::new(),
            bundle: bundle.to_string(),
            version: String::new(),
            fragment: String::new(),
        }
        .mint()
    }

    /// `self` rendered and parsed again: a reference built here clears the
    /// gate a reference typed as text clears.
    pub fn mint(self) -> Result<Parts, RefError> {
        if self.bundle.is_empty() {
            return Err(syntax("empty bundle name"));
        }
        Parts::parse(&self.render(true))
    }
}

fn leading_slash(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

const BUNDLE_MARKER: &str = "bundles/";

/// The position of a `%2F` (any case) in `raw`.
fn encoded_slash(raw: &str) -> Option<usize> {
    let b = raw.as_bytes();
    (0..b.len().saturating_sub(2))
        .find(|&i| b[i] == b'%' && b[i + 1] == b'2' && matches!(b[i + 2], b'F' | b'f'))
}

fn is_unreserved(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~')
}

/// `s` with every escape of an unreserved character turned into the
/// character, and the rest's hex uppercased.
fn decode_unreserved_escapes(s: &str) -> Result<String, String> {
    if !s.contains('%') {
        return Ok(s.to_string());
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'%' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        if i + 2 >= b.len() {
            return Err(format!("truncated percent-escape at byte {i}"));
        }
        let (hi, lo) = (unhex(b[i + 1]), unhex(b[i + 2]));
        let (Some(hi), Some(lo)) = (hi, lo) else {
            return Err(format!(
                "invalid percent-escape {:?} at byte {i}",
                String::from_utf8_lossy(&b[i..i + 3])
            ));
        };
        let c = hi << 4 | lo;
        if is_unreserved(c) {
            out.push(c);
        } else {
            out.push(b'%');
            out.push(b[i + 1].to_ascii_uppercase());
            out.push(b[i + 2].to_ascii_uppercase());
        }
        i += 3;
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}

fn unhex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Go's `url.PathUnescape`: `%XX` decoded, anything else (a `+` too) as it is.
fn path_unescape(s: &str) -> Result<String, String> {
    if !s.contains('%') {
        return Ok(s.to_string());
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let pair = b
                .get(i + 1..i + 3)
                .and_then(|p| Some((unhex(p[0])?, unhex(p[1])?)));
            let Some((hi, lo)) = pair else {
                let shown: String = String::from_utf8_lossy(&b[i..b.len().min(i + 3)]).into_owned();
                return Err(format!("invalid URL escape {shown:?}"));
            };
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}

/// `@` is the version delimiter, so it is escaped inside a name or a version.
fn escape_at(s: &str) -> String {
    s.replace('@', "%40")
}

/// Go's `url.URL{Path: p}.EscapedPath()`.
pub fn escape_path(path: &str) -> String {
    escape(path, |c| {
        matches!(
            c,
            b'$' | b'&' | b'+' | b',' | b'/' | b':' | b';' | b'=' | b'@'
        )
    })
}

fn escape_fragment(fragment: &str) -> String {
    escape(fragment, |c| {
        matches!(
            c,
            b'$' | b'&'
                | b'+'
                | b','
                | b'/'
                | b':'
                | b';'
                | b'='
                | b'?'
                | b'@'
                | b'!'
                | b'('
                | b')'
                | b'*'
        )
    })
}

fn escape(s: &str, keep: impl Fn(u8) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &c in s.as_bytes() {
        if is_unreserved(c) || keep(c) {
            out.push(char::from(c));
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

/// The version after the last `@`, decoded; the rest, still escaped.
fn split_version(escaped: &str) -> Result<(String, String), RefError> {
    match escaped.rfind('@') {
        None => Ok((escaped.to_string(), String::new())),
        Some(at) => {
            let version =
                path_unescape(&escaped[at + 1..]).map_err(|e| syntax(format!("version: {e}")))?;
            Ok((escaped[..at].to_string(), version))
        }
    }
}

/// RFC 3986 dot-segment removal on an escaped path.
fn remove_dot_segments(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for seg in path.trim_start_matches('/').split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    let joined = out.join("/");
    if absolute {
        format!("/{joined}")
    } else {
        joined
    }
}

/// A host in the form references compare on: lower case, a default port
/// dropped, an IPv6 literal bracketed.
fn canonical_host(authority: &str) -> Result<String, RefError> {
    // As Go's url.Parse reads an authority: a bracketed literal then an
    // optional port, else everything after the last colon is the port; a port
    // is digits (or nothing).
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (host, after) = inner
            .split_once(']')
            .ok_or_else(|| syntax("missing ']' in host"))?;
        let port = if after.is_empty() {
            ""
        } else {
            after
                .strip_prefix(':')
                .ok_or_else(|| syntax(format!("invalid port {after:?} after host")))?
        };
        (host, port)
    } else {
        authority.rsplit_once(':').unwrap_or((authority, ""))
    };
    if !port.bytes().all(|b| b.is_ascii_digit()) {
        return Err(syntax(format!("invalid port \":{port}\" after host")));
    }
    if host.is_empty() {
        return Err(syntax(format!("{SCHEME_PREFIX}git requires a host")));
    }
    let shown = if host.contains(':') {
        format!("[{}]", host.to_lowercase())
    } else {
        host.to_lowercase()
    };
    Ok(if port.is_empty() || port == "443" {
        shown
    } else {
        format!("{shown}:{port}")
    })
}

/// Whether `raw` is spelled as a fetch address: an http(s) or file URL, or an
/// scp-like `user@host:path` (Go's `remote.IsFetchAddressRef`).
pub fn is_fetch_address(raw: &str) -> bool {
    raw.starts_with("https://")
        || raw.starts_with("http://")
        || raw.starts_with("file://")
        || is_scp_form(raw)
}

fn is_scp_form(raw: &str) -> bool {
    if raw.contains("://") {
        return false;
    }
    raw.split_once(':')
        .is_some_and(|(user_host, _)| user_host.contains('@') && !user_host.contains('/'))
}

/// A fetch address taken apart: the repository (host and path as canonical)
/// and what follows its first `@`.
struct Fetch {
    class: SourceClass,
    host: String,
    repo_path: String,
    after: String,
}

fn fetch_parts(base: &str) -> Option<Fetch> {
    let (class, host, path) = if let Some(rest) = base.strip_prefix("file://") {
        (SourceClass::File, String::new(), rest.to_string())
    } else if let Some(rest) = base
        .strip_prefix("https://")
        .or_else(|| base.strip_prefix("http://"))
    {
        let (host, path) = rest.split_once('/')?;
        (SourceClass::Git, host.to_lowercase(), format!("/{path}"))
    } else if is_scp_form(base) {
        let (user_host, path) = base.split_once(':')?;
        let host = user_host.rsplit_once('@')?.1.to_lowercase();
        (SourceClass::Git, host, format!("/{path}"))
    } else {
        return None;
    };
    let (repo, after) = match path.split_once('@') {
        Some((repo, after)) => (repo.to_string(), after.to_string()),
        None => (path, String::new()),
    };
    let repo = repo.trim_end_matches('/');
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    Some(Fetch {
        class,
        host,
        repo_path: repo.to_string(),
        after,
    })
}

/// `raw` re-spelled in the canonical grammar when it is a fetch address
/// naming a bundle (`<url>@bundles/<path>[@version]`, with any `#` item
/// selector kept); anything else as written (Go's `remote.CanonicalSpelling`).
pub fn canonical_spelling(raw: &str) -> String {
    let (base, selector) = match raw.find('#') {
        Some(i) => (&raw[..i], &raw[i..]),
        None => (raw, ""),
    };
    let Some(fetch) = fetch_parts(base) else {
        return raw.to_string();
    };
    let Some(bundle_and_version) = fetch.after.strip_prefix(BUNDLE_MARKER) else {
        return raw.to_string();
    };
    let (bundle, version) = match bundle_and_version.split_once('@') {
        Some((b, v)) => (b, v),
        None => (bundle_and_version, ""),
    };
    let minted = match fetch.class {
        SourceClass::File => Parts::file(&fetch.repo_path, bundle),
        _ => Parts::git(&fetch.host, &fetch.repo_path, bundle),
    };
    match minted {
        Ok(mut parts) => {
            parts.version = version.to_string();
            parts.render(true) + selector
        }
        Err(_) => raw.to_string(),
    }
}

/// The version-less canonical identity of a bundle named as `raw`: a
/// canonical reference, a fetch address, the older `ctxloom:local@bundles/<n>`,
/// or a bare local name (Go's `remote.CanonicalBundleRef`).
pub fn canonical_bundle_ref(raw: &str) -> Result<String, RefError> {
    bundle_ref_and_version(raw).map(|(canonical, _)| canonical)
}

/// As [`canonical_bundle_ref`], with the version the reference pins (empty
/// for none): a bare name pins nothing.
pub fn bundle_ref_and_version(raw: &str) -> Result<(String, String), RefError> {
    let raw = raw.trim();
    let from_parts = |parts: Parts| (parts.render(false), parts.version);
    if has_scheme(raw) {
        return Parts::parse(raw).map(from_parts);
    }
    if let Some(rest) = raw.strip_prefix("ctxloom:local@bundles/") {
        let (name, version) = rest.split_once('@').unwrap_or((rest, ""));
        let parts = Parts::local(name)?;
        return Ok((parts.render(false), version.to_string()));
    }
    if is_fetch_address(raw) {
        let canonical = canonical_spelling(raw);
        if has_scheme(&canonical) {
            return Parts::parse(&canonical).map(from_parts);
        }
        return Err(syntax(format!("{raw} names a source but does not parse")));
    }
    Parts::local(raw).map(|p| (p.render(false), String::new()))
}

/// The selector that addresses a fragment of a bundle.
pub const FRAGMENT_SELECTOR: &str = "#fragments/";

/// A qualified fragment reference's bundle in the canonical grammar and the
/// version it pins: `X@<commit>#fragments/n` is (`<canonical X>#fragments/n`,
/// `<commit>`). A reference with no fragment selector (a bare name) is
/// returned as written, unpinned (Go's `remote.SplitFragmentVersion`).
pub fn split_fragment_version(raw: &str) -> Result<(String, String), RefError> {
    let raw = raw.trim();
    let (base, selector) = match raw.find('#') {
        Some(i) => (&raw[..i], &raw[i..]),
        None => (raw, ""),
    };
    if !selector.starts_with(FRAGMENT_SELECTOR) {
        return Ok((raw.to_string(), String::new()));
    }
    let (canonical, version) = bundle_ref_and_version(base)?;
    Ok((format!("{canonical}{selector}"), version))
}

/// The repository `raw` names, as a key two spellings of it share: a fetch
/// address, a repository URL, or a canonical git or file reference. `None`
/// for a reference with no repository (local, companion, a bare name).
pub fn repository_key(raw: &str) -> Option<String> {
    let base = raw.find('#').map_or(raw, |i| &raw[..i]);
    if has_scheme(base) {
        let parts = Parts::parse(base).ok()?;
        let path = parts.repo_path.trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        return match parts.class {
            SourceClass::Git => Some(format!("{}{path}", parts.host)),
            SourceClass::File => Some(format!("file://{path}")),
            _ => None,
        };
    }
    let fetch = fetch_parts(base)?;
    Some(match fetch.class {
        SourceClass::File => format!("file://{}", fetch.repo_path),
        _ => format!("{}{}", fetch.host, fetch.repo_path),
    })
}

/// Whether `a` and `b` name one repository.
pub fn same_repository(a: &str, b: &str) -> bool {
    match (repository_key(a), repository_key(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// What an item is, as a bundle holds it. `Prompt` is a command: its
/// directory is `prompts`, its selector `commands` (or `prompts`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ItemKind {
    Fragment,
    Prompt,
    Mcp,
    Hook,
    Skill,
}

impl ItemKind {
    /// What the kind is called in a sentence (`fragment`, `prompt`, `mcp`,
    /// `hook`, `skill`).
    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Fragment => "fragment",
            ItemKind::Prompt => "prompt",
            ItemKind::Mcp => "mcp",
            ItemKind::Hook => "hook",
            ItemKind::Skill => "skill",
        }
    }

    /// The directory the kind's items are in, and the segment of a canonical
    /// item reference (`fragments`, `prompts`, `mcp`, `hooks`, `skills`).
    pub fn dir(self) -> &'static str {
        match self {
            ItemKind::Fragment => "fragments",
            ItemKind::Prompt => "prompts",
            ItemKind::Mcp => "mcp",
            ItemKind::Hook => "hooks",
            ItemKind::Skill => "skills",
        }
    }
}

/// A `<kind>/<name>` selector: `fragments`, `commands` (or `prompts`), `mcp`,
/// `hooks` or `skills`, and a name.
pub fn parse_selector(selector: &str) -> Result<(ItemKind, String), RefError> {
    let selector = selector.trim();
    let bad = || RefError::Selector {
        selector: selector.to_string(),
    };
    let (kind_dir, name) = selector.split_once('/').ok_or_else(bad)?;
    if name.is_empty() {
        return Err(bad());
    }
    let kind = match kind_dir {
        "fragments" => ItemKind::Fragment,
        "commands" | "prompts" => ItemKind::Prompt,
        "mcp" => ItemKind::Mcp,
        "hooks" => ItemKind::Hook,
        "skills" => ItemKind::Skill,
        other => {
            return Err(RefError::UnknownKind {
                kind: other.to_string(),
            })
        }
    };
    Ok((kind, name.to_string()))
}

#[cfg(test)]
#[path = "refs.test.rs"]
mod tests;
