//! The bundles a project holds, and the lookups over them (Go's
//! `bundles.Catalog` and the loader's `ReadFragment`, `ListAllFragments`,
//! `ByTags` and `ResolveFragmentAsk`).
//!
//! A bundle is addressed by its name (its path below the bundles root),
//! `ctxloom+local:<name>`, or the older `ctxloom:local@bundles/<name>`; an
//! item by `<bundle>#<kind>/<name>`. Listings are a function of the corpus:
//! bundles by name, items by name.

use crate::error::CatalogError;
use crate::model::{Bundle, ItemBody};
use crate::refs::{self, has_scheme, ItemKind, Parts, SourceClass};

/// One bundle of the catalog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    /// Its name: the path below the bundles root (`demo`, `team/rules`).
    pub name: String,
    /// Where its `bundle.yaml` is, as the caller shows it.
    pub manifest_path: String,
    pub bundle: Bundle,
}

/// Where an installed remote bundle came from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RemoteOrigin {
    /// Its version-less canonical reference.
    pub reference: String,
    /// The repository's address.
    pub url: String,
    /// The commit it is pinned at.
    pub sha: String,
    /// Its files, by path relative to the bundle.
    pub files: std::collections::BTreeMap<String, Vec<u8>>,
    /// The ones committed executable.
    pub executable: std::collections::BTreeSet<String>,
}

/// The bundles of a project.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Catalog {
    entries: Vec<Entry>,
    /// The entries installed from remotes, by name.
    origins: std::collections::BTreeMap<String, RemoteOrigin>,
}

/// A fragment or command as a listing shows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContentInfo {
    pub name: String,
    pub bundle: String,
    /// The bundle's tags then the item's own, each once, in first-seen order.
    pub tags: Vec<String>,
    pub premise: String,
    pub description: String,
}

/// An item read from the catalog, with what a caller needs to deliver it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ItemRead {
    /// `<bundle>/<item>`.
    pub name: String,
    pub bundle: String,
    pub item: String,
    /// The canonical reference, `ctxloom+local:<bundle>#<dir>/<item>`.
    pub item_ref: String,
    pub tags: Vec<String>,
    pub premise: String,
    pub description: String,
    pub installation: String,
    pub content: String,
    pub distilled: String,
    pub distilled_by: String,
    pub no_distill: bool,
}

impl ItemRead {
    /// The body to deliver, and whether it is the distilled form: the
    /// distilled form when it is preferred and there is one.
    pub fn resolve(&self, prefer_distilled: bool) -> (&str, bool) {
        if prefer_distilled && !self.distilled.is_empty() && !self.no_distill {
            (&self.distilled, true)
        } else {
            (&self.content, false)
        }
    }
}

impl Catalog {
    /// A catalog of `entries`, listed by name.
    pub fn new(mut entries: Vec<Entry>) -> Catalog {
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Catalog {
            entries,
            origins: std::collections::BTreeMap::new(),
        }
    }

    /// Say which of the entries are bundles installed from remotes, by name:
    /// each is also found by its canonical reference.
    pub fn with_origins(
        mut self,
        origins: std::collections::BTreeMap<String, RemoteOrigin>,
    ) -> Catalog {
        self.origins = origins;
        self
    }

    /// Where the entry `name` was installed from; none for a bundle of this
    /// project.
    pub fn origin(&self, name: &str) -> Option<&RemoteOrigin> {
        self.origins.get(name)
    }

    /// Whether `name` is a bundle of this project (not one installed from a
    /// remote).
    pub fn is_local(&self, name: &str) -> bool {
        !self.origins.contains_key(name) && self.entries.iter().any(|e| e.name == name)
    }

    /// Every bundle, by name.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The bundle asked for as `ask`: a canonical or older local reference
    /// exactly, else a name (the name a listing shows, then the name the
    /// bundle declares for itself).
    pub fn lookup(&self, ask: &str) -> Result<&Entry, CatalogError> {
        let not_found = || CatalogError::BundleNotFound {
            ask: ask.to_string(),
        };
        if let Some(name) = local_name(ask) {
            return self
                .entries
                .iter()
                .find(|e| e.name == name)
                .ok_or_else(not_found);
        }
        if has_scheme(ask) && Parts::parse(ask).is_ok() {
            return self.installed(ask).ok_or_else(not_found);
        }
        let mut by_listing = Vec::new();
        let mut by_declared = Vec::new();
        for entry in &self.entries {
            if entry.name == ask {
                by_listing.push(entry);
            } else if !entry.bundle.name.is_empty() && entry.bundle.name == ask {
                by_declared.push(entry);
            }
        }
        let matches = if by_listing.is_empty() {
            by_declared
        } else {
            by_listing
        };
        match matches.as_slice() {
            [one] => Ok(one),
            [] => Err(not_found()),
            many => Err(CatalogError::Ambiguous {
                ask: ask.to_string(),
                candidates: many
                    .iter()
                    .map(|e| format!("ctxloom+local:{}", e.name))
                    .collect::<Vec<_>>()
                    .join(", "),
            }),
        }
    }

    /// The entry installed from the remote bundle `ask` is a reference to.
    fn installed(&self, ask: &str) -> Option<&Entry> {
        let (canonical, _) = refs::bundle_ref_and_version(ask).ok()?;
        let name = self
            .origins
            .iter()
            .find(|(_, origin)| origin.reference == canonical)
            .map(|(name, _)| name)?;
        self.entries.iter().find(|e| e.name == *name)
    }

    /// Every fragment of every bundle, bundles by name, fragments by name.
    pub fn list_fragments(&self) -> Vec<ContentInfo> {
        self.entries
            .iter()
            .flat_map(|entry| {
                entry
                    .bundle
                    .fragments
                    .iter()
                    .map(|(name, fragment)| ContentInfo {
                        name: name.clone(),
                        bundle: entry.name.clone(),
                        tags: item_tags(&entry.bundle.tags, &fragment.body.tags),
                        premise: fragment.premise.clone(),
                        description: String::new(),
                    })
            })
            .collect()
    }

    /// Every command of every bundle, in the same order.
    pub fn list_commands(&self) -> Vec<ContentInfo> {
        self.entries
            .iter()
            .flat_map(|entry| {
                entry
                    .bundle
                    .commands
                    .iter()
                    .map(|(name, command)| ContentInfo {
                        name: name.clone(),
                        bundle: entry.name.clone(),
                        tags: item_tags(&entry.bundle.tags, &command.body.tags),
                        premise: String::new(),
                        description: command.description.clone(),
                    })
            })
            .collect()
    }

    /// The fragments carrying any of `tags`, in listing order.
    pub fn by_tags(&self, tags: &[String]) -> Vec<ContentInfo> {
        self.list_fragments()
            .into_iter()
            .filter(|info| info.tags.iter().any(|t| tags.contains(t)))
            .collect()
    }

    /// A fragment named in a request, as the qualified reference a selection
    /// quotes back: `bundle#fragments/name` is canonicalized; a bare name
    /// becomes the first bundle's that has it (by name order); a name no
    /// bundle has stays as written.
    pub fn resolve_fragment_ask(&self, name: &str) -> String {
        if let Some((base, selector)) = name.split_once('#') {
            return match refs::canonical_bundle_ref(base) {
                Ok(canonical) => format!("{canonical}#{selector}"),
                Err(_) => name.to_string(),
            };
        }
        self.entries
            .iter()
            .find(|e| e.bundle.fragments.contains_key(name))
            .map_or_else(
                || name.to_string(),
                |e| item_ref(&e.name, ItemKind::Fragment, name),
            )
    }

    /// The fragments an `ask` names: `bundle#fragments/name` one; a bare name
    /// every bundle's of that name.
    pub fn read_fragment(&self, ask: &str) -> Result<Vec<ItemRead>, CatalogError> {
        match self.scoped(ask, ItemKind::Fragment)? {
            Some((entry, item)) => {
                let fragment = entry.bundle.fragments.get(&item).ok_or_else(|| {
                    CatalogError::FragmentNotFound {
                        item: item.clone(),
                        bundle: entry.name.clone(),
                    }
                })?;
                Ok(vec![read_item(
                    entry,
                    ItemKind::Fragment,
                    &item,
                    &fragment.body,
                    &fragment.premise,
                    "",
                )])
            }
            None => {
                let found: Vec<ItemRead> = self
                    .entries
                    .iter()
                    .filter_map(|entry| {
                        entry.bundle.fragments.get(ask).map(|f| {
                            read_item(entry, ItemKind::Fragment, ask, &f.body, &f.premise, "")
                        })
                    })
                    .collect();
                if found.is_empty() {
                    Err(CatalogError::FragmentNameNotFound {
                        name: ask.to_string(),
                    })
                } else {
                    Ok(found)
                }
            }
        }
    }

    /// The commands an `ask` names, as [`read_fragment`](Catalog::read_fragment).
    pub fn read_command(&self, ask: &str) -> Result<Vec<ItemRead>, CatalogError> {
        match self.scoped(ask, ItemKind::Prompt)? {
            Some((entry, item)) => {
                let command = entry.bundle.commands.get(&item).ok_or_else(|| {
                    CatalogError::CommandNotFound {
                        item: item.clone(),
                        bundle: entry.name.clone(),
                    }
                })?;
                Ok(vec![read_item(
                    entry,
                    ItemKind::Prompt,
                    &item,
                    &command.body,
                    "",
                    &command.description,
                )])
            }
            None => {
                let found: Vec<ItemRead> = self
                    .entries
                    .iter()
                    .filter_map(|entry| {
                        entry.bundle.commands.get(ask).map(|c| {
                            read_item(entry, ItemKind::Prompt, ask, &c.body, "", &c.description)
                        })
                    })
                    .collect();
                if found.is_empty() {
                    Err(CatalogError::CommandNameNotFound {
                        name: ask.to_string(),
                    })
                } else {
                    Ok(found)
                }
            }
        }
    }

    /// The bundle and item an `ask` of the form `<bundle>#<kind>/<item>` names,
    /// when the kind is the one wanted; `None` for a plain name.
    fn scoped(
        &self,
        ask: &str,
        wanted: ItemKind,
    ) -> Result<Option<(&Entry, String)>, CatalogError> {
        let Some((base, selector)) = ask.split_once('#') else {
            return Ok(None);
        };
        let (kind, item) =
            refs::parse_selector(selector).map_err(|cause| CatalogError::ItemSyntax {
                ask: ask.to_string(),
                cause,
            })?;
        if kind != wanted {
            return Err(CatalogError::BadItemRef {
                ask: ask.to_string(),
                kind: kind.label().to_string(),
                wanted: wanted.label().to_string(),
            });
        }
        Ok(Some((self.lookup(base)?, item)))
    }
}

/// The name of the local bundle a canonical or older local reference names.
fn local_name(ask: &str) -> Option<String> {
    if let Some(rest) = ask.strip_prefix("ctxloom:local@bundles/") {
        return Some(rest.split('@').next().unwrap_or(rest).to_string());
    }
    let parts = Parts::parse(ask).ok()?;
    (parts.class == SourceClass::Local).then_some(parts.bundle)
}

fn read_item(
    entry: &Entry,
    kind: ItemKind,
    item: &str,
    body: &ItemBody,
    premise: &str,
    description: &str,
) -> ItemRead {
    ItemRead {
        name: format!("{}/{item}", entry.name),
        bundle: entry.name.clone(),
        item: item.to_string(),
        item_ref: item_ref(&entry.name, kind, item),
        tags: item_tags(&entry.bundle.tags, &body.tags),
        premise: premise.to_string(),
        description: description.to_string(),
        installation: body.installation.clone(),
        content: body.content.clone(),
        distilled: body.distilled.clone(),
        distilled_by: body.distilled_by.clone(),
        no_distill: body.no_distill,
    }
}

/// `tags` after the bundle's: each once, in first-seen order.
pub fn item_tags(bundle_tags: &[String], own: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in bundle_tags.iter().chain(own) {
        if !out.contains(tag) {
            out.push(tag.clone());
        }
    }
    out
}

/// The canonical reference of an item of a local bundle.
pub fn item_ref(bundle: &str, kind: ItemKind, name: &str) -> String {
    format!("ctxloom+local:{bundle}#{}/{name}", kind.dir())
}

#[cfg(test)]
#[path = "catalog.test.rs"]
mod tests;
