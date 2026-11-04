//! Profiles: what one is, how a document decodes into one, and how a profile
//! resolves with its parents (Go's `internal/core/profiles`).
//!
//! A profile owns no content: it names bundles, parents and a mask, and the
//! result is the context one agent starts with. Resolution is depth first
//! (parents, then the profile's own settings), a diamond's shared ancestor is
//! resolved once, a cycle and a chain deeper than [`MAX_DEPTH`] are refused,
//! and a parent that cannot be loaded is skipped with a finding.

use std::collections::{BTreeMap, BTreeSet};

use angzarr_yaml::Value;

use crate::error::ProfileError;
use crate::findings::Finding;
use crate::refs;
use crate::yamlx::{self, key_line};

/// The deepest a chain of parents may go.
pub const MAX_DEPTH: usize = 64;

/// A direct fragment reference: a name (which may carry an `@<commit>` pin)
/// and the priority that places it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FragmentRef {
    pub name: String,
    pub priority: i64,
}

/// Lifecycle hooks a profile declares: neutral events, and an engine's own.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct HooksConfig {
    /// Neutral events (`pre_tool`, ...) to their hooks as written.
    pub unified: BTreeMap<String, Vec<Value>>,
    /// Engine name to that engine's native events to their hooks.
    pub ext: BTreeMap<String, BTreeMap<String, Vec<Value>>>,
}

impl HooksConfig {
    /// Whether it declares any hook.
    pub fn has_any(&self) -> bool {
        self.unified.values().any(|h| !h.is_empty())
            || self
                .ext
                .values()
                .flat_map(BTreeMap::values)
                .any(|h| !h.is_empty())
    }

    /// `other`'s hooks after this one's; a hook identical to one already
    /// there is not added twice.
    pub fn merge(&mut self, other: &HooksConfig) {
        for (event, hooks) in &other.unified {
            append_hooks(self.unified.entry(event.clone()).or_default(), hooks);
        }
        for (engine, events) in &other.ext {
            let mine = self.ext.entry(engine.clone()).or_default();
            for (event, hooks) in events {
                append_hooks(mine.entry(event.clone()).or_default(), hooks);
            }
        }
    }
}

/// `hooks` after `into`'s, those identical to one already there left out.
fn append_hooks(into: &mut Vec<Value>, hooks: &[Value]) {
    for hook in hooks {
        if !into.contains(hook) {
            into.push(hook.clone());
        }
    }
}

/// A profile as its document declares it.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Profile {
    /// The name it is known by (the file's name); never in the document.
    pub name: String,
    pub description: String,
    pub parents: Vec<String>,
    /// Descriptive; selects nothing.
    pub tags: Vec<String>,
    /// Fragment tags that select content.
    pub select_tags: Vec<String>,
    /// The engine label this profile prefers.
    pub llm: String,
    pub bundles: Vec<String>,
    pub commands: Vec<String>,
    pub skills: Vec<String>,
    pub fragments: Vec<FragmentRef>,
    pub bundle_items: Vec<String>,
    pub hooks: HooksConfig,
    pub variables: BTreeMap<String, String>,
    pub exclude_fragments: Vec<String>,
    pub exclude_mcp: Vec<String>,
    pub deny_tools: Vec<String>,
}

/// The keys a profile document may declare.
pub const PROFILE_KEYS: [&str; 15] = [
    "bundle_items",
    "bundles",
    "commands",
    "deny_tools",
    "description",
    "exclude_fragments",
    "exclude_mcp",
    "fragments",
    "hooks",
    "llm",
    "parents",
    "select_tags",
    "skills",
    "tags",
    "variables",
];

/// Where a profile's value sits in the text it was read from, so a refusal
/// can say which line.
#[derive(Clone, Copy, Debug)]
pub struct Located<'a> {
    pub text: &'a str,
    /// The keys leading to the profile's own mapping (empty for a profile
    /// file).
    pub prefix: &'a [&'a str],
}

impl Located<'_> {
    fn line(&self, below: &[&str]) -> u64 {
        let path: Vec<&str> = self.prefix.iter().chain(below).copied().collect();
        key_line(self.text, &path)
    }
}

fn bad(key: &str, detail: impl Into<String>) -> ProfileError {
    ProfileError::BadValue {
        key: key.to_string(),
        detail: detail.into(),
    }
}

/// A list of strings under `key`; in strict reading a null entry is refused.
fn list(map: &Value, key: &str, strict: bool) -> Result<Vec<String>, ProfileError> {
    let Some(value) = map.get(key) else {
        return Ok(Vec::new());
    };
    let items = yamlx::string_list(value).map_err(|d| bad(key, d))?;
    if strict {
        if let Some(i) = items.iter().position(String::is_empty) {
            return Err(bad(
                key,
                format!("item {} must be a non-empty string", i + 1),
            ));
        }
    }
    Ok(items)
}

fn fragment_refs(
    value: &Value,
    strict: bool,
    at: &Located<'_>,
) -> Result<Vec<FragmentRef>, ProfileError> {
    let items = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Seq(items) => items,
        other => {
            return Err(bad(
                "fragments",
                format!("expected a list, found {}", yamlx::kind(other)),
            ))
        }
    };
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let empty = ProfileError::EmptyFragment {
            line: at.line(&["fragments"]) + 1 + i as u64,
        };
        match item {
            Value::Null => return Err(empty),
            Value::Scalar(name) if name.trim().is_empty() => return Err(empty),
            Value::Scalar(name) => out.push(FragmentRef {
                name: name.clone(),
                priority: 0,
            }),
            Value::Map(entries) => {
                if strict {
                    for (key, _) in entries {
                        if key != "name" && key != "priority" {
                            return Err(ProfileError::UnknownKey {
                                key: key.clone(),
                                line: at.line(&["fragments", "[]", key]),
                                suggestion: yamlx::nearest(key, &["name", "priority"]),
                                valid: "name, priority".to_string(),
                            });
                        }
                    }
                }
                let name = item.get("name").map(yamlx::text).unwrap_or_default();
                if name.trim().is_empty() {
                    return Err(empty);
                }
                let priority = match item.get("priority") {
                    Some(p) => yamlx::integer(p).map_err(|d| bad("fragments", d))?,
                    None => 0,
                };
                out.push(FragmentRef { name, priority });
            }
            Value::Seq(_) => {
                return Err(bad("fragments", "an entry is a name or {name, priority}"));
            }
        }
    }
    Ok(out)
}

fn hooks_config(value: &Value, at: &Located<'_>) -> Result<HooksConfig, ProfileError> {
    let mut out = HooksConfig::default();
    let Value::Map(entries) = value else {
        return if matches!(value, Value::Null) {
            Ok(out)
        } else {
            Err(bad("hooks", "expected a mapping"))
        };
    };
    for (key, inner) in entries {
        match key.as_str() {
            "unified" => out.unified = hook_events(inner, "hooks.unified")?,
            "ext" => {
                if let Value::Map(engines) = inner {
                    for (engine, events) in engines {
                        out.ext.insert(
                            engine.clone(),
                            hook_events(events, &format!("hooks.ext.{engine}"))?,
                        );
                    }
                } else if !matches!(inner, Value::Null) {
                    return Err(bad("hooks.ext", "expected a mapping"));
                }
            }
            other => {
                return Err(ProfileError::UnknownKey {
                    key: other.to_string(),
                    line: at.line(&["hooks", other]),
                    suggestion: yamlx::nearest(other, &["ext", "unified"]),
                    valid: "ext, unified".to_string(),
                })
            }
        }
    }
    Ok(out)
}

fn hook_events(value: &Value, at: &str) -> Result<BTreeMap<String, Vec<Value>>, ProfileError> {
    match value {
        Value::Null => Ok(BTreeMap::new()),
        Value::Map(events) => events
            .iter()
            .map(|(event, hooks)| match hooks {
                Value::Null => Ok((event.clone(), Vec::new())),
                Value::Seq(items) => Ok((event.clone(), items.clone())),
                _ => Err(bad(&format!("{at}.{event}"), "expected a list of hooks")),
            })
            .collect(),
        _ => Err(bad(at, "expected a mapping")),
    }
}

impl Profile {
    /// Decode a profile document: its keys are the ones a profile may
    /// declare (an unknown one is refused, naming the key and the near one),
    /// `fragments` entries are a name or `{name, priority}` (an empty one is
    /// refused), and an empty document is the empty profile.
    pub fn decode(text: &str) -> Result<Profile, ProfileError> {
        let value = crate::yamlx::parse(text).map_err(|cause| ProfileError::Syntax { cause })?;
        Profile::from_value(&value, true, &Located { text, prefix: &[] })
    }

    /// A profile read from its parsed value. `strict` is the file form's
    /// reading (every key checked, a null entry refused); the form a bundle
    /// document carries inline reads leniently, as Go's does.
    pub fn from_value(
        value: &Value,
        strict: bool,
        at: &Located<'_>,
    ) -> Result<Profile, ProfileError> {
        let entries = match value {
            Value::Null => return Ok(Profile::default()),
            Value::Map(entries) => entries,
            other => {
                return Err(bad(
                    "",
                    format!("expected a mapping, found {}", yamlx::kind(other)),
                ))
            }
        };
        if strict {
            for (key, _) in entries {
                if !PROFILE_KEYS.contains(&key.as_str()) {
                    return Err(ProfileError::UnknownKey {
                        key: key.clone(),
                        line: at.line(&[key]),
                        suggestion: yamlx::nearest(key, &PROFILE_KEYS),
                        valid: PROFILE_KEYS.join(", "),
                    });
                }
            }
        }
        let scalar = |key: &str| -> Result<String, ProfileError> {
            value.get(key).map_or(Ok(String::new()), |v| {
                yamlx::string(v).map_err(|d| bad(key, d))
            })
        };
        Ok(Profile {
            name: String::new(),
            description: scalar("description")?,
            parents: list(value, "parents", strict)?,
            tags: list(value, "tags", strict)?,
            select_tags: list(value, "select_tags", strict)?,
            llm: scalar("llm")?,
            bundles: list(value, "bundles", strict)?,
            commands: list(value, "commands", strict)?,
            skills: list(value, "skills", strict)?,
            fragments: match value.get("fragments") {
                Some(v) => fragment_refs(v, strict, at)?,
                None => Vec::new(),
            },
            bundle_items: list(value, "bundle_items", strict)?,
            hooks: match value.get("hooks") {
                Some(v) => hooks_config(v, at)?,
                None => HooksConfig::default(),
            },
            variables: match value.get("variables") {
                Some(v) => yamlx::string_map(v).map_err(|d| bad("variables", d))?,
                None => BTreeMap::new(),
            },
            exclude_fragments: list(value, "exclude_fragments", strict)?,
            exclude_mcp: list(value, "exclude_mcp", strict)?,
            deny_tools: list(value, "deny_tools", strict)?,
        })
    }

    /// Whether the profile selects anything at all.
    pub fn has_content(&self) -> bool {
        !self.parents.is_empty()
            || !self.select_tags.is_empty()
            || !self.bundles.is_empty()
            || !self.commands.is_empty()
            || !self.skills.is_empty()
            || !self.fragments.is_empty()
            || !self.bundle_items.is_empty()
            || !self.exclude_fragments.is_empty()
            || !self.exclude_mcp.is_empty()
            || !self.deny_tools.is_empty()
            || !self.variables.is_empty()
            || self.hooks.has_any()
            || !self.llm.is_empty()
    }

    /// A remote profile (shipped in the repository `source_url`) may name only
    /// content of that repository; a local profile may name any.
    pub fn check_own_repo(&self, source_url: &str) -> Result<(), ProfileError> {
        if source_url.is_empty() {
            return Ok(());
        }
        let named = self
            .bundles
            .iter()
            .chain(&self.parents)
            .chain(&self.commands)
            .chain(&self.skills)
            .chain(&self.bundle_items)
            .chain(self.fragments.iter().map(|f| &f.name));
        for reference in named {
            let base = reference.split('#').next().unwrap_or(reference);
            if !refs::same_repository(base, source_url) {
                return Err(ProfileError::CrossRepo {
                    profile: self.name.clone(),
                    reference: reference.clone(),
                });
            }
        }
        Ok(())
    }

    /// The profile's references moved onto the canonical grammar (the
    /// format step of an envelope below generation 2): a bundle or parent
    /// written as a fetch address is re-spelled, the rest left as written.
    pub fn canonicalize_refs(&mut self) {
        for bundle in &mut self.bundles {
            *bundle = canonical_stored_ref(bundle);
        }
        for parent in &mut self.parents {
            *parent = canonical_stored_ref(parent);
        }
    }
}

/// A reference written as a fetch address, in the canonical grammar (any
/// version pin and `#` selector kept); anything else as written.
pub fn canonical_stored_ref(reference: &str) -> String {
    if !refs::is_fetch_address(reference) {
        return reference.to_string();
    }
    let (base, selector) = match reference.find('#') {
        Some(i) => (&reference[..i], &reference[i..]),
        None => (reference, ""),
    };
    let canonical = refs::canonical_spelling(base);
    if canonical == base {
        reference.to_string()
    } else {
        canonical + selector
    }
}

/// A profile after its parents are folded in.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct ResolvedProfile {
    /// The name it was asked for under; never inherited.
    pub name: String,
    pub bundles: Vec<String>,
    pub tags: Vec<String>,
    pub select_tags: Vec<String>,
    pub commands: Vec<String>,
    pub skills: Vec<String>,
    pub fragments: Vec<FragmentRef>,
    pub bundle_items: Vec<String>,
    pub hooks: HooksConfig,
    pub variables: BTreeMap<String, String>,
    pub llm: String,
    pub exclude_fragments: Vec<String>,
    pub exclude_mcp: Vec<String>,
    pub deny_tools: Vec<String>,
}

impl ResolvedProfile {
    /// Fold `other` in after what is here: lists union (first spelling
    /// kept), a variable already here is kept, the first non-empty `llm`
    /// stays, exclusions and denials accumulate.
    pub fn merge(&mut self, other: &ResolvedProfile) {
        append_unique_refs(&mut self.bundles, &other.bundles);
        append_unique(&mut self.tags, &other.tags);
        append_unique(&mut self.select_tags, &other.select_tags);
        append_unique_refs(&mut self.commands, &other.commands);
        append_unique_refs(&mut self.skills, &other.skills);
        append_unique_fragments(&mut self.fragments, &other.fragments);
        append_unique_refs(&mut self.bundle_items, &other.bundle_items);
        self.hooks.merge(&other.hooks);
        for (name, value) in &other.variables {
            self.variables
                .entry(name.clone())
                .or_insert_with(|| value.clone());
        }
        if self.llm.is_empty() {
            self.llm.clone_from(&other.llm);
        }
        append_unique(&mut self.exclude_fragments, &other.exclude_fragments);
        append_unique(&mut self.exclude_mcp, &other.exclude_mcp);
        append_unique(&mut self.deny_tools, &other.deny_tools);
    }
}

/// `items` after `slice`, those already there left out.
pub fn append_unique(slice: &mut Vec<String>, items: &[String]) {
    let mut seen: BTreeSet<String> = slice.iter().cloned().collect();
    for item in items {
        if seen.insert(item.clone()) {
            slice.push(item.clone());
        }
    }
}

/// As [`append_unique`], for bundle references: two spellings of one bundle
/// (`ctxloom+git://h/r//bundles/x` and `https://h/r@bundles/x`) are one entry,
/// the first kept; a different version pin or item selector stays distinct.
pub fn append_unique_refs(slice: &mut Vec<String>, items: &[String]) {
    let mut seen: BTreeSet<String> = slice.iter().map(|s| refs::canonical_spelling(s)).collect();
    for item in items {
        if seen.insert(refs::canonical_spelling(item)) {
            slice.push(item.clone());
        }
    }
}

/// Fragment references by name; a later one only raises the priority.
fn append_unique_fragments(slice: &mut Vec<FragmentRef>, items: &[FragmentRef]) {
    for item in items {
        match slice.iter_mut().find(|f| f.name == item.name) {
            Some(existing) => existing.priority = existing.priority.max(item.priority),
            None => slice.push(item.clone()),
        }
    }
}

/// Why a profile could not be loaded.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LoadError {
    /// No such profile, and only this project could hold it (a bare name, or
    /// a `ctxloom+local:` reference).
    LocalNotFound { name: String },
    /// No such profile, and it may be in a bundle not pulled yet.
    NotFound { name: String },
    /// The profile is there and cannot be read.
    Unreadable { name: String, detail: String },
}

/// Where profiles come from: the project bundle's, and the other bundles'.
pub trait ProfileSource {
    /// The profile at the canonical `key` (`ctxloom+local:<bundle>#profiles/<name>`).
    fn load(&self, key: &str) -> Result<Profile, LoadError>;

    /// The canonical keys of every profile there is, for a "did you mean".
    fn keys(&self) -> Vec<String> {
        Vec::new()
    }
}

/// The selector that places a profile in a bundle.
const PROFILE_SELECTOR: &str = "#profiles/";

/// The canonical key of a profile asked for as `ask`: a bare name is the
/// project bundle's profile of that name (`ctxloom+local:project#profiles/<name>`),
/// `bundle#profiles/<name>` is a local bundle's, and a canonical key stays.
pub fn canonical_key(ask: &str) -> Result<String, ProfileError> {
    let bad_name = || ProfileError::BadName {
        name: ask.to_string(),
    };
    if let Some(i) = ask.find('#') {
        let (bundle, selector) = (&ask[..i], &ask[i..]);
        if !selector.starts_with(PROFILE_SELECTOR) {
            return Err(bad_name());
        }
        return match refs::canonical_bundle_ref(bundle) {
            Ok(canonical) => Ok(format!("{canonical}{selector}")),
            Err(_) => Ok(ask.to_string()),
        };
    }
    if refs::is_fetch_address(ask) {
        return Ok(ask.to_string());
    }
    if ask.is_empty() || ask.contains(['/', '\\']) || ask == "." || ask == ".." {
        return Err(bad_name());
    }
    Ok(format!("ctxloom+local:project{PROFILE_SELECTOR}{ask}"))
}

/// The remedy for a parent only this project can hold and does not.
fn missing_local_parent_fix(
    source: &dyn ProfileSource,
    name: &str,
    parent: &str,
    key: &str,
) -> String {
    let leaf = key.rsplit(PROFILE_SELECTOR).next().unwrap_or(key);
    let suffix = format!("{PROFILE_SELECTOR}{leaf}");
    let matches: Vec<String> = source
        .keys()
        .into_iter()
        .filter(|k| k.ends_with(&suffix))
        .collect();
    let listing = match matches.as_slice() {
        [one] => format!("did you mean {one}? Another bundle's profile is named by its full ref"),
        _ => "`ctxloom profile list` names every profile you can use".to_string(),
    };
    format!(
        "{listing}; otherwise create the profile {parent}, or correct or remove that parent in profile {name}"
    )
}

/// Resolve the profile asked for as `ask`, with its parents. A parent that
/// cannot be loaded is skipped and said in `findings`; a cycle, and a chain
/// past [`MAX_DEPTH`], are refused.
pub fn resolve(
    source: &dyn ProfileSource,
    ask: &str,
    findings: &mut Vec<Finding>,
) -> Result<ResolvedProfile, ProfileError> {
    let key = canonical_key(ask)?;
    let mut run = Resolution {
        source,
        findings,
        memo: BTreeMap::new(),
    };
    run.resolve(&key, &BTreeSet::new(), 0)
        .map_err(|fault| match fault {
            Fault::Structural(e) => e,
            Fault::Load(LoadError::LocalNotFound { name } | LoadError::NotFound { name }) => {
                ProfileError::NotFound { name }
            }
            Fault::Load(LoadError::Unreadable { name, detail }) => {
                ProfileError::Unreadable { name, detail }
            }
        })
}

/// Why a resolution step failed: the profile could not be loaded (a parent's
/// is skipped and said), or the structure is wrong (always fatal).
enum Fault {
    Load(LoadError),
    Structural(ProfileError),
}

struct Resolution<'a> {
    source: &'a dyn ProfileSource,
    findings: &'a mut Vec<Finding>,
    memo: BTreeMap<String, ResolvedProfile>,
}

impl Resolution<'_> {
    fn resolve(
        &mut self,
        name: &str,
        visited: &BTreeSet<String>,
        depth: usize,
    ) -> Result<ResolvedProfile, Fault> {
        if depth > MAX_DEPTH {
            return Err(Fault::Structural(ProfileError::DepthExceeded {
                max: MAX_DEPTH as u64,
            }));
        }
        if visited.contains(name) {
            return Err(Fault::Structural(ProfileError::Circular {
                name: name.to_string(),
            }));
        }
        let mut visited = visited.clone();
        visited.insert(name.to_string());
        if let Some(done) = self.memo.get(name) {
            return Ok(done.clone());
        }
        let profile = self.source.load(name).map_err(Fault::Load)?;
        let mut resolved = ResolvedProfile {
            name: name.to_string(),
            ..ResolvedProfile::default()
        };
        for parent in &profile.parents {
            let parent_key = canonical_key(parent).unwrap_or_else(|_| parent.clone());
            match self.resolve(&parent_key, &visited, depth + 1) {
                Ok(done) => resolved.merge(&done),
                Err(Fault::Structural(cause)) => {
                    return Err(Fault::Structural(ProfileError::Parent {
                        parent: parent.clone(),
                        cause: Box::new(cause),
                    }))
                }
                Err(Fault::Load(failure)) => self.report(name, parent, &parent_key, &failure),
            }
        }
        append_unique_refs(&mut resolved.bundles, &profile.bundles);
        append_unique(&mut resolved.tags, &profile.tags);
        append_unique(&mut resolved.select_tags, &profile.select_tags);
        append_unique_refs(&mut resolved.commands, &profile.commands);
        append_unique_refs(&mut resolved.skills, &profile.skills);
        append_unique_fragments(&mut resolved.fragments, &profile.fragments);
        append_unique_refs(&mut resolved.bundle_items, &profile.bundle_items);
        resolved.hooks.merge(&profile.hooks);
        for (k, v) in &profile.variables {
            resolved.variables.insert(k.clone(), v.clone());
        }
        if !profile.llm.is_empty() {
            resolved.llm.clone_from(&profile.llm);
        }
        append_unique(&mut resolved.exclude_fragments, &profile.exclude_fragments);
        append_unique(&mut resolved.exclude_mcp, &profile.exclude_mcp);
        append_unique(&mut resolved.deny_tools, &profile.deny_tools);
        self.memo.insert(name.to_string(), resolved.clone());
        Ok(resolved)
    }

    /// Say that `parent` of `name` was skipped, once.
    fn report(&mut self, name: &str, parent: &str, key: &str, failure: &LoadError) {
        let finding = match failure {
            LoadError::LocalNotFound { .. } => Finding::fail(
                "ref",
                format!("profile \"{name}\": parent {parent} does not exist; skipping"),
                missing_local_parent_fix(self.source, name, parent, key),
            ),
            LoadError::NotFound { .. } => Finding::fail(
                "ref",
                format!(
                    "profile \"{name}\": parent {parent} not installed; skipping (run `ctxloom deps pull` to install)"
                ),
                "ctxloom deps pull",
            ),
            LoadError::Unreadable { detail, .. } => Finding::fail(
                "ref",
                format!("profile \"{name}\": parent {parent} failed to load ({detail}); skipping"),
                "fix or remove the parent profile file",
            ),
        };
        if !self.findings.contains(&finding) {
            self.findings.push(finding);
        }
    }
}

#[cfg(test)]
#[path = "profile.test.rs"]
mod tests;
