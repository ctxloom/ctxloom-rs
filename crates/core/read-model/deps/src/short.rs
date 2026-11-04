//! A per-remote short reference spelled out (Go's
//! `remote.CanonicalizeShortRef`): `origin/demo#profiles/dev` is the profile
//! `dev` of the bundle `demo` of the repository the remote `origin` is
//! registered for.

use library::refs::{has_scheme, is_fetch_address, Parts};

use crate::address::canonical_ref;
use crate::reference::RemoteRef;

/// `reference` with a registered remote's name as its first segment expanded
/// to the canonical reference it stands for (the item selector and a `@`
/// version kept).
///
/// - A reference that is already self-contained (canonical, a fetch address,
///   a `ctxloom+local:` one) needs no registry and is returned as it is.
/// - A bare name, or a `<segment>/<path>` whose first segment names no
///   remote, is the project's own: returned as it is.
/// - `local_exists(base)` is true for a project bundle of that very name: the
///   local one wins over a remote of the same spelling.
pub fn expand(
    reference: &str,
    url_of: &dyn Fn(&str) -> Option<String>,
    local_exists: &dyn Fn(&str) -> bool,
) -> String {
    let reference = reference.trim();
    if let Some(remote) = RemoteRef::parse(reference) {
        let bundle = remote.bundle_ref();
        return if remote.item.is_empty() {
            bundle
        } else {
            format!("{bundle}#{}", remote.item)
        };
    }
    if has_scheme(reference) || reference.starts_with("ctxloom:") || is_fetch_address(reference) {
        return reference.to_string();
    }
    let (base, selector) = match reference.split_once('#') {
        Some((base, selector)) => (base, Some(selector)),
        None => (reference, None),
    };
    let (path_part, version) = match base.split_once('@') {
        Some((path, version)) => (path, version),
        None => (base, ""),
    };
    let Some((alias, bundle)) = path_part.split_once('/') else {
        return reference.to_string();
    };
    if alias.is_empty() || bundle.is_empty() || local_exists(path_part) {
        return reference.to_string();
    }
    let Some(identity) = url_of(alias).and_then(|url| canonical_ref(&url, bundle)) else {
        return reference.to_string();
    };
    let versioned = match Parts::parse(&identity) {
        Ok(parts) => Parts {
            version: version.to_string(),
            ..parts
        }
        .render(true),
        Err(_) => identity,
    };
    match selector {
        Some(selector) => format!("{versioned}#{selector}"),
        None => versioned,
    }
}

/// `reference` read out of the repository at `url`, at the commit `sha`: a
/// short same-repository reference (`demo`, `lang/go#fragments/x`) is that
/// bundle of the same repository, pinned to the commit the profile was read at
/// unless it pins its own version. A reference that names its own source
/// passes through unchanged.
pub fn within_repository(reference: &str, url: &str, sha: &str) -> String {
    let reference = reference.trim();
    let self_contained = RemoteRef::parse(reference).is_some()
        || has_scheme(reference)
        || reference.starts_with("ctxloom:")
        || is_fetch_address(reference);
    if self_contained || url.is_empty() {
        return reference.to_string();
    }
    let (base, item) = match reference.find('#') {
        Some(at) => reference.split_at(at),
        None => (reference, ""),
    };
    let (bundle, version) = base.split_once('@').unwrap_or((base, ""));
    let version = if version.is_empty() { sha } else { version };
    let Some(identity) = canonical_ref(url, bundle) else {
        return reference.to_string();
    };
    match Parts::parse(&identity) {
        Ok(parts) => format!(
            "{}{item}",
            Parts {
                version: version.to_string(),
                ..parts
            }
            .render(true)
        ),
        Err(_) => reference.to_string(),
    }
}

#[cfg(test)]
#[path = "short.test.rs"]
mod tests;
