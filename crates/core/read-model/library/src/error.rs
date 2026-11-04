//! Why the library read model refused something: references, bundles,
//! profiles, the catalog and the config. Every error is a code, args and an
//! en-US message (`locales/en-US/messages.ftl`), and a third party's text (the
//! YAML reader's) is the arg `ext_err_txt`, verbatim.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// A reference that does not parse.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RefError {
    /// Not a canonical reference: why.
    Syntax { detail: String },
    /// A selector that is not `<kind>/<name>`.
    Selector { selector: String },
    /// A selector naming a kind there is not.
    UnknownKind { kind: String },
}

impl RefError {
    /// The sentence for a finding's text, where no catalog is at hand.
    pub fn plain(&self) -> String {
        match self {
            RefError::Syntax { detail } => format!("malformed bundle reference: {detail}"),
            RefError::Selector { selector } => {
                format!("selector \"{selector}\" must be <kind>/<name>")
            }
            RefError::UnknownKind { kind } => {
                format!("unknown item kind \"{kind}\" (want fragments|commands|mcp|hooks|skills)")
            }
        }
    }
}

impl fmt::Display for RefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for RefError {}

impl Diagnostic for RefError {
    fn code(&self) -> &str {
        match self {
            RefError::Syntax { .. } => code!("ref-syntax"),
            RefError::Selector { .. } => code!("ref-selector"),
            RefError::UnknownKind { .. } => code!("ref-unknown-kind"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            RefError::Syntax { detail } => vec![("detail", detail.as_str().into())],
            RefError::Selector { selector } => vec![("selector", selector.as_str().into())],
            RefError::UnknownKind { kind } => vec![("kind", kind.as_str().into())],
        }
    }
}

/// A profile that does not decode, resolve or stay in its repository.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProfileError {
    /// The document is not YAML the reader takes.
    Syntax { cause: angzarr_yaml::Error },
    /// A key a profile may not declare.
    UnknownKey {
        key: String,
        line: u64,
        suggestion: String,
        valid: String,
    },
    /// A key holding the wrong shape of value.
    BadValue { key: String, detail: String },
    /// A `fragments` entry that names no fragment.
    EmptyFragment { line: u64 },
    /// A remote profile naming content outside its own repository.
    CrossRepo { profile: String, reference: String },
    /// A profile name that cannot be one (`#` is reserved).
    BadName { name: String },
    /// No such profile.
    NotFound { name: String },
    /// A profile that is its own ancestor.
    Circular { name: String },
    /// A chain of parents deeper than the limit.
    DepthExceeded { max: u64 },
    /// A parent that could not be resolved, and why.
    Parent {
        parent: String,
        cause: Box<ProfileError>,
    },
    /// A profile that is there and cannot be read.
    Unreadable { name: String, detail: String },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for ProfileError {}

impl Diagnostic for ProfileError {
    fn code(&self) -> &str {
        match self {
            ProfileError::Syntax { .. } => code!("profile-syntax"),
            ProfileError::UnknownKey { .. } => code!("profile-unknown-key"),
            ProfileError::BadValue { .. } => code!("profile-bad-value"),
            ProfileError::EmptyFragment { .. } => code!("profile-empty-fragment"),
            ProfileError::CrossRepo { .. } => code!("profile-cross-repo"),
            ProfileError::BadName { .. } => code!("profile-bad-name"),
            ProfileError::NotFound { .. } => code!("profile-not-found"),
            ProfileError::Circular { .. } => code!("profile-circular"),
            ProfileError::DepthExceeded { .. } => code!("profile-depth-exceeded"),
            ProfileError::Parent { .. } => code!("profile-parent"),
            ProfileError::Unreadable { .. } => code!("profile-unreadable"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            ProfileError::Syntax { cause } => vec![("cause", Arg::Nested(Box::new(cause.clone())))],
            ProfileError::UnknownKey {
                key,
                line,
                suggestion,
                valid,
            } => vec![
                ("key", key.as_str().into()),
                ("line", (*line).into()),
                ("suggestion", suggestion.as_str().into()),
                ("near", Arg::Uint(u64::from(!suggestion.is_empty()))),
                ("valid", valid.as_str().into()),
            ],
            ProfileError::BadValue { key, detail } => vec![
                ("key", key.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            ProfileError::EmptyFragment { line } => vec![("line", (*line).into())],
            ProfileError::CrossRepo { profile, reference } => vec![
                ("profile", profile.as_str().into()),
                ("reference", reference.as_str().into()),
            ],
            ProfileError::BadName { name }
            | ProfileError::NotFound { name }
            | ProfileError::Circular { name } => vec![("name", name.as_str().into())],
            ProfileError::DepthExceeded { max } => vec![("max", (*max).into())],
            ProfileError::Parent { parent, cause } => vec![
                ("parent", parent.as_str().into()),
                ("cause", Arg::Nested(cause.clone())),
            ],
            ProfileError::Unreadable { name, detail } => vec![
                ("name", name.as_str().into()),
                ("ext_err_txt", detail.as_str().into()),
            ],
        }
    }
}

/// A bundle that cannot be read: its envelope, or one of its items.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BundleError {
    /// A document that declares no version and no items.
    Empty { bytes: u64 },
    /// An envelope written by a newer format than this reads.
    Newer { generation: u64, current: u64 },
    /// An envelope's `schema_version` that is not an integer.
    NotInteger,
    /// A document the YAML reader does not take.
    Syntax { cause: angzarr_yaml::Error },
    /// A key the bundle schema does not model.
    UnknownKey {
        key: String,
        line: u64,
        site: String,
        suggestion: String,
        valid: String,
    },
    /// A value of the wrong shape: the key's path and what is wrong.
    BadValue { path: String, detail: String },
    /// A tree with no `bundle.yaml`.
    NoManifest { bundle: String },
    /// An envelope that declares items inline beside item files.
    InlineAndItems { bundle: String, keys: String },
    /// An item that cannot be read: which, and why.
    Item {
        bundle: String,
        item: String,
        detail: String,
    },
    /// A file in the tree no kind of item claims.
    Unclaimed { bundle: String, path: String },
    /// A metadata sidecar on a kind that keeps none.
    UnexplainedMeta {
        kind: String,
        item: String,
        path: String,
    },
    /// A hook narrowed by an engine's own matcher.
    HookMatcher { hook: String, matcher: String },
    /// A skill package with no files.
    SkillEmpty { bundle: String, skill: String },
    /// A profile of the document that does not decode.
    Profile { name: String, cause: ProfileError },
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for BundleError {}

impl Diagnostic for BundleError {
    fn code(&self) -> &str {
        match self {
            BundleError::Empty { .. } => code!("bundle-empty"),
            BundleError::Newer { .. } => code!("bundle-newer"),
            BundleError::NotInteger => code!("bundle-not-integer"),
            BundleError::Syntax { .. } => code!("bundle-syntax"),
            BundleError::UnknownKey { .. } => code!("bundle-unknown-key"),
            BundleError::BadValue { .. } => code!("bundle-bad-value"),
            BundleError::NoManifest { .. } => code!("bundle-no-manifest"),
            BundleError::InlineAndItems { .. } => code!("bundle-inline-and-items"),
            BundleError::Item { .. } => code!("bundle-item"),
            BundleError::Unclaimed { .. } => code!("bundle-unclaimed"),
            BundleError::UnexplainedMeta { .. } => code!("bundle-unexplained-meta"),
            BundleError::HookMatcher { .. } => code!("bundle-hook-matcher"),
            BundleError::SkillEmpty { .. } => code!("bundle-skill-empty"),
            BundleError::Profile { .. } => code!("bundle-profile"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            BundleError::Empty { bytes } => vec![("bytes", (*bytes).into())],
            BundleError::Newer {
                generation,
                current,
            } => vec![
                ("generation", (*generation).into()),
                ("current", (*current).into()),
            ],
            BundleError::NotInteger => Vec::new(),
            BundleError::Syntax { cause } => vec![("cause", Arg::Nested(Box::new(cause.clone())))],
            BundleError::UnknownKey {
                key,
                line,
                site,
                suggestion,
                valid,
            } => vec![
                ("key", key.as_str().into()),
                ("line", (*line).into()),
                ("site", site.as_str().into()),
                ("suggestion", suggestion.as_str().into()),
                ("near", Arg::Uint(u64::from(!suggestion.is_empty()))),
                ("valid", valid.as_str().into()),
            ],
            BundleError::BadValue { path, detail } => vec![
                ("path", path.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            BundleError::NoManifest { bundle } => vec![("bundle", bundle.as_str().into())],
            BundleError::InlineAndItems { bundle, keys } => vec![
                ("bundle", bundle.as_str().into()),
                ("keys", keys.as_str().into()),
            ],
            BundleError::Item {
                bundle,
                item,
                detail,
            } => vec![
                ("bundle", bundle.as_str().into()),
                ("item", item.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            BundleError::Unclaimed { bundle, path } => vec![
                ("bundle", bundle.as_str().into()),
                ("path", path.as_str().into()),
            ],
            BundleError::UnexplainedMeta { kind, item, path } => vec![
                ("kind", kind.as_str().into()),
                ("item", item.as_str().into()),
                ("path", path.as_str().into()),
            ],
            BundleError::HookMatcher { hook, matcher } => vec![
                ("hook", hook.as_str().into()),
                ("matcher", matcher.as_str().into()),
            ],
            BundleError::SkillEmpty { bundle, skill } => vec![
                ("bundle", bundle.as_str().into()),
                ("skill", skill.as_str().into()),
            ],
            BundleError::Profile { name, cause } => vec![
                ("name", name.as_str().into()),
                ("cause", Arg::Nested(Box::new(cause.clone()))),
            ],
        }
    }
}

/// A bundle or an item that the catalog cannot give.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CatalogError {
    /// No such bundle.
    BundleNotFound { ask: String },
    /// A name that more than one bundle answers to.
    Ambiguous { ask: String, candidates: String },
    /// No such fragment in the bundle.
    FragmentNotFound { item: String, bundle: String },
    /// No fragment of that name in any bundle.
    FragmentNameNotFound { name: String },
    /// No such command in the bundle.
    CommandNotFound { item: String, bundle: String },
    /// No command of that name in any bundle.
    CommandNameNotFound { name: String },
    /// A selector that parsed and asks for the wrong kind of item.
    BadItemRef {
        ask: String,
        kind: String,
        wanted: String,
    },
    /// A `#` selector that does not parse.
    ItemSyntax { ask: String, cause: RefError },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for CatalogError {}

impl Diagnostic for CatalogError {
    fn code(&self) -> &str {
        match self {
            CatalogError::BundleNotFound { .. } => code!("catalog-bundle-not-found"),
            CatalogError::Ambiguous { .. } => code!("catalog-ambiguous"),
            CatalogError::FragmentNotFound { .. } => code!("catalog-fragment-not-found"),
            CatalogError::FragmentNameNotFound { .. } => code!("catalog-fragment-name-not-found"),
            CatalogError::CommandNotFound { .. } => code!("catalog-command-not-found"),
            CatalogError::CommandNameNotFound { .. } => code!("catalog-command-name-not-found"),
            CatalogError::BadItemRef { .. } => code!("catalog-bad-item-ref"),
            CatalogError::ItemSyntax { .. } => code!("catalog-item-syntax"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            CatalogError::BundleNotFound { ask } => vec![("ask", ask.as_str().into())],
            CatalogError::Ambiguous { ask, candidates } => vec![
                ("ask", ask.as_str().into()),
                ("candidates", candidates.as_str().into()),
            ],
            CatalogError::FragmentNotFound { item, bundle }
            | CatalogError::CommandNotFound { item, bundle } => vec![
                ("item", item.as_str().into()),
                ("bundle", bundle.as_str().into()),
            ],
            CatalogError::FragmentNameNotFound { name }
            | CatalogError::CommandNameNotFound { name } => vec![("name", name.as_str().into())],
            CatalogError::BadItemRef { ask, kind, wanted } => vec![
                ("ask", ask.as_str().into()),
                ("kind", kind.as_str().into()),
                ("wanted", wanted.as_str().into()),
            ],
            CatalogError::ItemSyntax { ask, cause } => vec![
                ("ask", ask.as_str().into()),
                ("cause", Arg::Nested(Box::new(cause.clone()))),
            ],
        }
    }
}

/// A config that cannot be read at all.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConfigError {
    /// The text is not YAML the reader takes.
    Syntax { cause: angzarr_yaml::Error },
    /// The root is not a mapping.
    NotAMapping,
    /// An edit in place that the editor could not make.
    Unwritable { detail: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for ConfigError {}

impl Diagnostic for ConfigError {
    fn code(&self) -> &str {
        match self {
            ConfigError::Syntax { .. } => code!("config-syntax"),
            ConfigError::NotAMapping => code!("config-not-a-mapping"),
            ConfigError::Unwritable { .. } => code!("config-unwritable"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            ConfigError::Syntax { cause } => vec![("cause", Arg::Nested(Box::new(cause.clone())))],
            ConfigError::NotAMapping => Vec::new(),
            ConfigError::Unwritable { detail } => vec![("detail", detail.as_str().into())],
        }
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let mut all = Vec::new();
    all.extend(ref_samples());
    all.extend(profile_samples());
    all.extend(bundle_samples());
    all.extend(catalog_samples());
    all.extend(config_samples());
    all.extend(crate::write::archive::diagnostic_samples());
    all
}

#[cfg(any(test, feature = "test-support"))]
fn sample_yaml_error() -> angzarr_yaml::Error {
    angzarr_yaml::parse("a: [b\n").expect_err("an unterminated flow sequence")
}

#[cfg(any(test, feature = "test-support"))]
fn ref_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(RefError::Syntax {
            detail: "empty".to_string(),
        }),
        Box::new(RefError::Selector {
            selector: "fragments".to_string(),
        }),
        Box::new(RefError::UnknownKind {
            kind: "widgets".to_string(),
        }),
    ]
}

#[cfg(any(test, feature = "test-support"))]
fn profile_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(ProfileError::Syntax {
            cause: sample_yaml_error(),
        }),
        Box::new(ProfileError::UnknownKey {
            key: "select_tagz".to_string(),
            line: 1,
            suggestion: "select_tags".to_string(),
            valid: "bundles, select_tags".to_string(),
        }),
        Box::new(ProfileError::BadValue {
            key: "llm".to_string(),
            detail: "expected a string".to_string(),
        }),
        Box::new(ProfileError::EmptyFragment { line: 2 }),
        Box::new(ProfileError::CrossRepo {
            profile: "dev".to_string(),
            reference: "https://example.com/r@bundles/x".to_string(),
        }),
        Box::new(ProfileError::BadName {
            name: "a#b".to_string(),
        }),
        Box::new(ProfileError::NotFound {
            name: "dev".to_string(),
        }),
        Box::new(ProfileError::Circular {
            name: "a".to_string(),
        }),
        Box::new(ProfileError::DepthExceeded { max: 64 }),
        Box::new(ProfileError::Parent {
            parent: "base".to_string(),
            cause: Box::new(ProfileError::Circular {
                name: "base".to_string(),
            }),
        }),
        Box::new(ProfileError::Unreadable {
            name: "dev".to_string(),
            detail: "permission denied".to_string(),
        }),
    ]
}

#[cfg(any(test, feature = "test-support"))]
fn bundle_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(BundleError::Empty { bytes: 0 }),
        Box::new(BundleError::Newer {
            generation: 5,
            current: 4,
        }),
        Box::new(BundleError::NotInteger),
        Box::new(BundleError::Syntax {
            cause: sample_yaml_error(),
        }),
        Box::new(BundleError::UnknownKey {
            key: "hoooks".to_string(),
            line: 2,
            site: "the top level of the bundle".to_string(),
            suggestion: "hooks".to_string(),
            valid: "hooks, mcp".to_string(),
        }),
        Box::new(BundleError::BadValue {
            path: "mcp.m.args".to_string(),
            detail: "expected a list".to_string(),
        }),
        Box::new(BundleError::NoManifest {
            bundle: "demo".to_string(),
        }),
        Box::new(BundleError::InlineAndItems {
            bundle: "demo".to_string(),
            keys: "fragments".to_string(),
        }),
        Box::new(BundleError::Item {
            bundle: "demo".to_string(),
            item: "fragments/a".to_string(),
            detail: "unreadable".to_string(),
        }),
        Box::new(BundleError::Unclaimed {
            bundle: "demo".to_string(),
            path: "fragments/README.txt".to_string(),
        }),
        Box::new(BundleError::UnexplainedMeta {
            kind: "fragments".to_string(),
            item: "solid".to_string(),
            path: "fragments/.solid.meta.yaml".to_string(),
        }),
        Box::new(BundleError::HookMatcher {
            hook: "pre_tool/h".to_string(),
            matcher: "Bash".to_string(),
        }),
        Box::new(BundleError::SkillEmpty {
            bundle: "demo".to_string(),
            skill: "s".to_string(),
        }),
        Box::new(BundleError::Profile {
            name: "dev".to_string(),
            cause: ProfileError::EmptyFragment { line: 3 },
        }),
    ]
}

#[cfg(any(test, feature = "test-support"))]
fn catalog_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(CatalogError::BundleNotFound {
            ask: "nosuch".to_string(),
        }),
        Box::new(CatalogError::Ambiguous {
            ask: "x".to_string(),
            candidates: "ctxloom+local:a, ctxloom+local:b".to_string(),
        }),
        Box::new(CatalogError::FragmentNotFound {
            item: "nope".to_string(),
            bundle: "demo".to_string(),
        }),
        Box::new(CatalogError::FragmentNameNotFound {
            name: "nope".to_string(),
        }),
        Box::new(CatalogError::CommandNotFound {
            item: "nope".to_string(),
            bundle: "demo".to_string(),
        }),
        Box::new(CatalogError::CommandNameNotFound {
            name: "nope".to_string(),
        }),
        Box::new(CatalogError::BadItemRef {
            ask: "kit#commands/deploy".to_string(),
            kind: "prompt".to_string(),
            wanted: "fragment".to_string(),
        }),
        Box::new(CatalogError::ItemSyntax {
            ask: "kit#widgets/x".to_string(),
            cause: RefError::UnknownKind {
                kind: "widgets".to_string(),
            },
        }),
    ]
}

#[cfg(any(test, feature = "test-support"))]
fn config_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(ConfigError::Syntax {
            cause: sample_yaml_error(),
        }),
        Box::new(ConfigError::NotAMapping),
        Box::new(ConfigError::Unwritable {
            detail: "NotFound".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
