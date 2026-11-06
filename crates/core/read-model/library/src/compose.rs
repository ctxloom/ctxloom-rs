//! Composite selection and assembly (Go's `internal/core/composite`): what a
//! set of resolved profiles, and what the caller asked for by name and by
//! tag, make of a catalog: the fragments in their order, and the context they
//! join into.
//!
//! - [`select`]: per profile, in order, the tag matches, the direct fragment
//!   asks, then each whole-bundle expansion (a bundle's fragments by name), each
//!   filtered by that profile's exclusions; then the caller's named asks and
//!   tags. One entry per item survives (the highest priority any ask gave it;
//!   an explicit `@<commit>` over the default), in first-occurrence order,
//!   then [`bookend`]ed: highest priority first, second-highest last.
//! - [`assemble`]: every selected fragment loaded, the profile variables
//!   substituted, a premised fragment held back (and indexed) unless the caller
//!   named it or the assembly is static, the same item reaching the context
//!   twice assembled once.

use std::collections::{BTreeMap, BTreeSet};

use crate::catalog::{Catalog, ItemRead};
use crate::findings::Finding;
use crate::mustache;
use crate::profile::ResolvedProfile;
use crate::refs::{self, ItemKind, Parts};
use crate::sha256;

/// The text between the sections of an assembled context.
pub const SECTION_SEPARATOR: &str = "\n\n---\n\n";

/// One fragment in the assembly order: its qualified name, an optional pinned
/// content version, and the priority that placed it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FragmentAsk {
    pub name: String,
    pub version: String,
    pub priority: i64,
}

/// The caller's own asks beside the profiles'.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct SelectRequest {
    /// Fragments named by the caller: a bare name or a qualified reference.
    pub fragments: Vec<String>,
    pub tags: Vec<String>,
}

/// What a profile set asks for, resolved and canonicalised.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Selection {
    /// The profile names, in the order asked.
    pub profiles: Vec<String>,
    /// The assembly order.
    pub fragments: Vec<FragmentAsk>,
    /// What the caller named (resolved to the qualified reference); an
    /// explicit ask loads even when premised.
    pub explicit: Vec<String>,
    pub tags: Vec<String>,
    /// All the requested tags when none of them matched a fragment.
    pub missing_tags: Vec<String>,
    /// Curated command and skill asks.
    pub commands: Vec<String>,
    pub skills: Vec<String>,
    /// The profiles' bundle references, first occurrence kept.
    pub bundles: Vec<String>,
    /// The MCP server names the profiles veto.
    pub exclusions: BTreeSet<String>,
    /// Later profiles win.
    pub variables: BTreeMap<String, String>,
    pub deny_tools: Vec<String>,
    /// The first non-empty engine label.
    pub llm: String,
    /// Each profile to the fragment names it pushed.
    pub declared: BTreeMap<String, Vec<String>>,
}

/// Select what `resolved` and `request` ask of `catalog`. A whole-bundle ask
/// that names a bundle the catalog does not hold is skipped and said in
/// `findings`.
pub fn select(
    resolved: &[ResolvedProfile],
    catalog: &Catalog,
    request: &SelectRequest,
    findings: &mut Vec<Finding>,
) -> Selection {
    let mut selector = Selector {
        catalog,
        findings,
        selection: Selection::default(),
        asks: Vec::new(),
        seen: BTreeSet::new(),
    };
    for profile in resolved {
        selector.settings(profile);
        selector.item_asks(profile);
        selector.declare_fragments(profile);
    }
    selector.caller_asks(request);
    let Selector {
        mut selection,
        asks,
        ..
    } = selector;
    selection.fragments = bookend(dedupe(asks));
    selection
}

struct Selector<'a> {
    catalog: &'a Catalog,
    findings: &'a mut Vec<Finding>,
    selection: Selection,
    asks: Vec<FragmentAsk>,
    /// Each deduplicated declaration by kind and value, first wins.
    seen: BTreeSet<String>,
}

impl Selector<'_> {
    fn once(&mut self, key: String) -> bool {
        self.seen.insert(key)
    }

    fn settings(&mut self, profile: &ResolvedProfile) {
        self.selection.profiles.push(profile.name.clone());
        for (name, value) in &profile.variables {
            self.selection.variables.insert(name.clone(), value.clone());
        }
        if self.selection.llm.is_empty() {
            self.selection.llm.clone_from(&profile.llm);
        }
        for tool in &profile.deny_tools {
            if self.once(format!("deny:{tool}")) {
                self.selection.deny_tools.push(tool.clone());
            }
        }
        for server in &profile.exclude_mcp {
            self.selection.exclusions.insert(server.clone());
        }
    }

    fn item_asks(&mut self, profile: &ResolvedProfile) {
        for reference in &profile.commands {
            if self.once(format!("command:{reference}")) {
                self.selection.commands.push(reference.clone());
            }
        }
        for reference in &profile.skills {
            if self.once(format!("skill:{reference}")) {
                self.selection.skills.push(reference.clone());
            }
        }
        for reference in &profile.bundles {
            if self.once(format!("bundle:{reference}")) {
                self.selection.bundles.push(reference.clone());
            }
        }
    }

    /// The profile's tag matches, its direct fragment asks, then its
    /// whole-bundle expansions, each filtered by its own exclusions.
    fn declare_fragments(&mut self, profile: &ResolvedProfile) {
        let excluded = Exclusions::new(&profile.exclude_fragments);
        let mut asks = fragments_by_tags(self.catalog, &profile.select_tags);
        for fragment in &profile.fragments {
            let (name, version) = refs::split_fragment_version(&fragment.name)
                .unwrap_or_else(|_| (fragment.name.clone(), String::new()));
            asks.push(FragmentAsk {
                name,
                version,
                priority: fragment.priority,
            });
        }
        let refs: Vec<String> = profile
            .bundles
            .iter()
            .chain(&profile.bundle_items)
            .cloned()
            .collect();
        for expanded in expand_bundle_refs(self.catalog, &refs, self.findings) {
            asks.push(FragmentAsk {
                name: expanded.name,
                version: expanded.version,
                priority: 0,
            });
        }
        for ask in asks {
            if excluded.excludes(&ask.name) {
                continue;
            }
            self.selection
                .declared
                .entry(profile.name.clone())
                .or_default()
                .push(ask.name.clone());
            self.asks.push(ask);
        }
    }

    fn caller_asks(&mut self, request: &SelectRequest) {
        for name in &request.fragments {
            let resolved = self.catalog.resolve_fragment_ask(name);
            self.selection.explicit.push(resolved.clone());
            self.asks.push(FragmentAsk {
                name: resolved,
                version: String::new(),
                priority: 0,
            });
        }
        let tagged = fragments_by_tags(self.catalog, &request.tags);
        let any = !tagged.is_empty();
        self.asks.extend(tagged);
        self.selection.tags.clone_from(&request.tags);
        if !request.tags.is_empty() && !any {
            self.selection.missing_tags.clone_from(&request.tags);
        }
    }
}

/// Every fragment of the catalog carrying any of `tags`, as asks at priority
/// 0, in listing order.
fn fragments_by_tags(catalog: &Catalog, tags: &[String]) -> Vec<FragmentAsk> {
    if tags.is_empty() {
        return Vec::new();
    }
    catalog
        .by_tags(tags)
        .into_iter()
        .filter_map(|info| {
            let qualified = format!("{}{}{}", info.bundle, refs::FRAGMENT_SELECTOR, info.name);
            refs::split_fragment_version(&qualified)
                .ok()
                .map(|(name, _)| FragmentAsk {
                    name,
                    version: String::new(),
                    priority: 0,
                })
        })
        .collect()
}

/// A fragment a bundle reference expands to.
struct Expanded {
    name: String,
    version: String,
}

/// The fragments the bundle references name: a whole bundle's by name order,
/// a targeted `bundle#fragments/x` one; one per item, an explicit version over
/// the default. A bundle the catalog does not hold is skipped and said.
fn expand_bundle_refs(
    catalog: &Catalog,
    references: &[String],
    findings: &mut Vec<Finding>,
) -> Vec<Expanded> {
    let mut out: Vec<Expanded> = Vec::new();
    for reference in references {
        for expanded in expand_one(catalog, reference, findings) {
            match out.iter_mut().find(|e| e.name == expanded.name) {
                Some(existing) => {
                    if existing.version.is_empty() {
                        existing.version = expanded.version;
                    }
                }
                None => out.push(expanded),
            }
        }
    }
    out
}

fn unresolved(findings: &mut Vec<Finding>, reference: &str, why: &str) {
    let finding = Finding::warn(
        "bundle",
        format!("skipping unresolved bundle \"{reference}\": {why}"),
    );
    if !findings.contains(&finding) {
        findings.push(finding);
    }
}

fn expand_one(catalog: &Catalog, reference: &str, findings: &mut Vec<Finding>) -> Vec<Expanded> {
    if reference.is_empty() {
        return Vec::new();
    }
    // The item selector: `#` is canonical; `:fragments/`, `:commands/` and
    // `:mcp` are the older aliases.
    let separator = reference.find('#').or_else(|| {
        [":fragments/", ":commands/", ":mcp"]
            .iter()
            .find_map(|marker| reference.find(marker))
    });
    if let Some(at) = separator {
        let (bundle, rest) = (&reference[..at], &reference[at + 1..]);
        if !rest.starts_with("fragments/") {
            return Vec::new();
        }
        let (canonical, version) = match refs::bundle_ref_and_version(bundle) {
            Ok(pair) => pair,
            Err(e) => {
                unresolved(findings, bundle, &e.plain());
                return Vec::new();
            }
        };
        return match refs::parse_selector(rest) {
            Ok((kind, name)) => vec![Expanded {
                name: format!("{canonical}#{}/{name}", kind.dir()),
                version,
            }],
            Err(e) => {
                unresolved(findings, reference, &e.plain());
                Vec::new()
            }
        };
    }
    let (canonical, version) = match refs::bundle_ref_and_version(reference) {
        Ok(pair) => pair,
        Err(e) => {
            unresolved(findings, reference, &e.plain());
            return Vec::new();
        }
    };
    let Ok(entry) = catalog.lookup(&canonical) else {
        unresolved(
            findings,
            reference,
            &format!("bundle not found: {reference}"),
        );
        return Vec::new();
    };
    let mut out: Vec<Expanded> = entry
        .bundle
        .fragments
        .keys()
        .map(|name| Expanded {
            name: format!("{canonical}#fragments/{name}"),
            version: version.clone(),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// A profile's fragment exclusions: by canonical reference, as authored, or
/// by bare fragment name.
struct Exclusions(BTreeSet<String>);

impl Exclusions {
    fn new(references: &[String]) -> Exclusions {
        Exclusions(
            references
                .iter()
                .map(|e| {
                    refs::split_fragment_version(e)
                        .map_or_else(|_| e.clone(), |(canonical, _)| canonical)
                })
                .collect(),
        )
    }

    fn excludes(&self, name: &str) -> bool {
        let canonical = refs::split_fragment_version(name)
            .map_or_else(|_| name.to_string(), |(canonical, _)| canonical);
        if self.0.contains(&canonical) {
            return true;
        }
        canonical
            .split_once(refs::FRAGMENT_SELECTOR)
            .is_some_and(|(_, bare)| self.0.contains(bare))
    }
}

/// One ask per name: the highest priority any ask gave it, an explicit
/// version over the default, in first-occurrence order.
pub fn dedupe(asks: Vec<FragmentAsk>) -> Vec<FragmentAsk> {
    let mut out: Vec<FragmentAsk> = Vec::new();
    for ask in asks {
        match out.iter_mut().find(|existing| existing.name == ask.name) {
            Some(existing) => {
                existing.priority = existing.priority.max(ask.priority);
                if existing.version.is_empty() && !ask.version.is_empty() {
                    existing.version = ask.version;
                }
            }
            None => out.push(ask),
        }
    }
    out
}

/// The highest priority first, the second-highest last, the rest between in
/// descending priority (stable): the "lost in the middle" placement. One or
/// two asks are simply in priority order.
pub fn bookend(asks: Vec<FragmentAsk>) -> Vec<FragmentAsk> {
    let mut sorted = asks;
    sorted.sort_by_key(|ask| std::cmp::Reverse(ask.priority));
    if sorted.len() <= 2 {
        return sorted;
    }
    let second = sorted.remove(1);
    sorted.push(second);
    sorted
}

/// `selection` with every fragment of the catalog's companion loadouts asked
/// for after the profile-selected ones (Go's `companionAsks`): companions by
/// reference, fragments by name, each at priority 0 and after the bookending.
/// "Unconditional" means not profile-selected: each ask still goes through
/// [`assemble`], so a premise holds it back and a second copy of an item is
/// assembled once.
pub fn with_companion_asks(mut selection: Selection, catalog: &Catalog) -> Selection {
    selection.fragments.extend(
        catalog
            .companion_fragments()
            .into_iter()
            .map(|name| FragmentAsk {
                name,
                version: String::new(),
                priority: 0,
            }),
    );
    selection
}

/// What an assembly takes besides the selection.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Options {
    /// Deliver a fragment's distilled form where it has one (config
    /// `use_distilled`, on unless turned off).
    pub prefer_distilled: bool,
    /// A static assembly (nothing behind the surface can pull a withheld
    /// fragment later) includes the premised fragments.
    pub static_assembly: bool,
}

/// A fragment as delivered or held back.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Delivered {
    /// The reference the ask named, as qualified.
    pub reference: String,
    pub premise: String,
    /// The body with the profile variables substituted, trimmed.
    pub body: String,
    pub distilled: bool,
}

/// The assembled context and what became of each ask.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Package {
    pub context: String,
    /// The context's SHA-256, hex.
    pub hash: String,
    pub fragments: Vec<Delivered>,
    /// The premised fragments held back for the premise index.
    pub premised: Vec<Delivered>,
    /// Every fragment that loaded, in load order, a collapsed duplicate
    /// included.
    pub loaded: Vec<String>,
    /// Asks that did not load (fatal unless degraded), undefined variables
    /// and duplicates (warnings), in the order they happened.
    pub findings: Vec<Finding>,
    pub selection: Selection,
}

/// Assemble `selection` over `catalog`.
pub fn assemble(catalog: &Catalog, selection: &Selection, options: &Options) -> Package {
    let explicit: BTreeSet<&str> = selection.explicit.iter().map(String::as_str).collect();
    let mut package = Package {
        selection: selection.clone(),
        ..Package::default()
    };
    let mut ingest = Ingest::new();
    for ask in &selection.fragments {
        let loaded = match load(catalog, ask) {
            Ok(loaded) => loaded,
            Err(message) => {
                package.findings.push(load_failure(ask, &message));
                continue;
            }
        };
        let (content, distilled) = loaded.resolve(options.prefer_distilled);
        let mut findings = Vec::new();
        let body = mustache::substitute(content.trim(), &selection.variables, &mut |msg| {
            findings.push(Finding::warn(
                "fragment",
                format!("{msg} (fragment \"{}\")", ask.name),
            ));
        });
        package.findings.extend(findings);
        let item = Delivered {
            reference: ask.name.clone(),
            premise: loaded.premise.clone(),
            body,
            distilled,
        };
        let held_back = !loaded.premise.is_empty()
            && !options.static_assembly
            && !explicit.contains(ask.name.as_str());
        if held_back {
            package.premised.push(item);
            continue;
        }
        package.loaded.push(ask.name.clone());
        let identity = identity_key(&loaded.item_ref);
        let (kept, duplicate) = ingest.add(&identity, &item.body, &ask.name);
        if duplicate {
            if kept != ask.name {
                package.findings.push(Finding::warn(
                    "fragment",
                    format!(
                        "\"{kept}\" and \"{}\" both name the item {identity} and carry identical content, so it was assembled once, from the first: both references are valid, and nothing was lost",
                        ask.name
                    ),
                ));
            }
            continue;
        }
        package.fragments.push(item);
    }
    package.context = ingest.join();
    package.hash = sha256::hex(package.context.as_bytes());
    package
}

/// An ask that did not load, as the finding that voices it.
fn load_failure(ask: &FragmentAsk, message: &str) -> Finding {
    if ask.version.is_empty() {
        Finding::fail(
            "ref",
            format!("fragment {} failed to load ({message}); skipping", ask.name),
            "fix the fragment ref in the referencing profile, or install its bundle (ctxloom deps pull)",
        )
    } else {
        Finding::fail(
            "ref",
            format!("withholding {}@{}: {message}", ask.name, ask.version),
            "fix the pinned version in the referencing profile, or ctxloom deps pull",
        )
    }
}

/// One ask loaded from the catalog.
fn load(catalog: &Catalog, ask: &FragmentAsk) -> Result<ItemRead, String> {
    if !ask.version.is_empty() {
        return Err(format!(
            "version {} of {} cannot be read: pinned content versions are resolved from the bundle's repository, which this build does not read",
            ask.version, ask.name
        ));
    }
    let Some((base, selector)) = ask.name.split_once('#') else {
        return catalog
            .read_fragment(&ask.name)
            .ok()
            .and_then(|found| found.into_iter().next())
            .ok_or_else(|| format!("fragment not found: {}", ask.name));
    };
    let (kind, item) = refs::parse_selector(selector).map_err(|e| e.plain())?;
    if kind != ItemKind::Fragment {
        return Err(format!(
            "bad item reference: \"{}\" selects a {}, not a fragment",
            ask.name,
            kind.label()
        ));
    }
    let entry = catalog
        .lookup(base)
        .map_err(|_| format!("bundle not found: {base}"))?;
    let canonical = refs::canonical_bundle_ref(base).unwrap_or_else(|_| base.to_string());
    catalog
        .read_fragment(&format!("{}#fragments/{item}", entry.name))
        .ok()
        .and_then(|found| found.into_iter().next())
        .ok_or_else(|| format!("fragment not found: \"{item}\" in bundle \"{canonical}\""))
}

/// The identity the context dedupes on: the source-qualified, version-less
/// item reference. Two sources' items of one name are two items; two spellings
/// of one item (with and without a version) are one. A reference outside the
/// canonical grammar is its own identity.
pub fn identity_key(reference: &str) -> String {
    Parts::parse(reference).map_or_else(|_| reference.to_string(), |parts| parts.render(false))
}

/// The ingest layer of the context: every fragment passes through [`add`](Ingest::add),
/// which decides whether it is new content or a second copy of content
/// already ingested (the same item AND the same bytes ignoring surrounding
/// whitespace). The first occurrence is kept; nothing reorders.
#[derive(Clone, Default, Debug)]
pub struct Ingest {
    parts: Vec<String>,
    seen: BTreeMap<(String, String), String>,
}

impl Ingest {
    pub fn new() -> Ingest {
        Ingest::default()
    }

    /// Ingest one fragment: the reference kept and whether this one was
    /// dropped as a duplicate of it.
    pub fn add(&mut self, item: &str, content: &str, reference: &str) -> (String, bool) {
        let id = (item.to_string(), content.trim().to_string());
        if let Some(kept) = self.seen.get(&id) {
            return (kept.clone(), true);
        }
        self.seen.insert(id, reference.to_string());
        self.parts.push(content.to_string());
        (reference.to_string(), false)
    }

    /// The ingested fragments as the context: blank sections left out.
    pub fn join(&self) -> String {
        self.parts
            .iter()
            .filter(|part| !part.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(SECTION_SEPARATOR)
    }
}

#[cfg(test)]
#[path = "compose.test.rs"]
mod tests;
