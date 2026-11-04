//! A bundle read from its tree of files (Go's `internal/adapters/content`
//! tree reader and `bundles.ReadTree`): an envelope `bundle.yaml` and, per
//! kind, `fragments/<n>.md` (+ `<n>.distilled.md`), `prompts/<n>.md`,
//! `mcp/<n>.yaml` (+ `.<n>.meta.yaml`), `skills/<n>/...` (+ the sidecar
//! `skills/.<n>.meta.yaml` beside the package), `hooks/<event>/<n>.yaml`
//! (+ a sidecar with `order` and `tags`) and `profiles/<n>.yaml`.

use std::collections::BTreeMap;

use angzarr_yaml::Value;

use crate::document::{exports, hook, mcp_server, parse_with_generation, PROFILE_REFS};
use crate::error::BundleError;
use crate::frontmatter::{is_meta_path, split_front_matter, stem_of};
use crate::model::{Bundle, Command, Fragment, ItemBody, Skill, SkillFile, HOOK_EVENTS};
use crate::profile::Profile;
use crate::yamlx;

/// The files of a bundle's tree by path relative to the bundle, slash
/// separated: `bundle.yaml`, `fragments/solid.md`,
/// `skills/.code-reviewer.meta.yaml`.
pub type Files = BTreeMap<String, Vec<u8>>;

/// Read the tree of the bundle `name`. The envelope's `schema_version` (0 to
/// 4) is the tree's: items carry no format key. A tree that also declares
/// items inline in its envelope, or holds a file no kind of item claims, or a
/// sidecar on a kind that keeps none, is refused.
pub fn read_tree(name: &str, files: &Files) -> Result<Bundle, BundleError> {
    let envelope = files.get(MANIFEST).ok_or_else(|| BundleError::NoManifest {
        bundle: name.to_string(),
    })?;
    let (mut bundle, generation) = parse_with_generation(&String::from_utf8_lossy(envelope))?;
    let inline = inline_keys(&bundle);
    if !inline.is_empty() {
        return Err(BundleError::InlineAndItems {
            bundle: name.to_string(),
            keys: inline.join(", "),
        });
    }
    let reader = Reader {
        bundle: name,
        files,
        generation,
    };
    reader.fragments(&mut bundle)?;
    reader.commands(&mut bundle)?;
    reader.mcp(&mut bundle)?;
    reader.profiles(&mut bundle)?;
    reader.skills(&mut bundle)?;
    reader.hooks(&mut bundle)?;
    Ok(bundle)
}

/// The file every tree holds at its root.
pub const MANIFEST: &str = "bundle.yaml";

/// The item-bearing envelope keys that are populated.
fn inline_keys(bundle: &Bundle) -> Vec<&'static str> {
    let mut out = Vec::new();
    if !bundle.fragments.is_empty() {
        out.push("fragments");
    }
    if !bundle.commands.is_empty() {
        out.push("commands");
    }
    if !bundle.mcp.is_empty() {
        out.push("mcp");
    }
    if !bundle.skills.is_empty() {
        out.push("skills");
    }
    if !bundle.profiles.is_empty() {
        out.push("profiles");
    }
    if !bundle.hooks.is_empty() {
        out.push("hooks");
    }
    out
}

/// A tree being read.
struct Reader<'a> {
    bundle: &'a str,
    files: &'a Files,
    generation: u32,
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

impl<'a> Reader<'a> {
    /// The files under the kind directory `dir`, as `(rest of path, bytes)`.
    fn under(&self, dir: &str) -> Vec<(&'a str, &'a [u8])> {
        let prefix = format!("{dir}/");
        self.files
            .iter()
            .filter_map(|(path, bytes)| {
                path.strip_prefix(&prefix)
                    .map(|rest| (rest, bytes.as_slice()))
            })
            .collect()
    }

    fn unclaimed(&self, dir: &str, rests: &[&str]) -> BundleError {
        BundleError::Unclaimed {
            bundle: self.bundle.to_string(),
            path: rests
                .iter()
                .map(|rest| format!("{dir}/{rest}"))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    fn item_error(&self, item: &str, detail: String) -> BundleError {
        BundleError::Item {
            bundle: self.bundle.to_string(),
            item: item.to_string(),
            detail,
        }
    }

    /// A flat kind directory: `<stem><ext>` items, `.<stem>.meta.yaml`
    /// sidecars, and (for markdown kinds) `<stem>.distilled.md` forms. Anything
    /// else is unclaimed; a sidecar on a kind that keeps none is refused.
    fn flat(
        &self,
        dir: &str,
        ext: &str,
        keeps_meta: bool,
        distilled_form: bool,
    ) -> Result<BTreeMap<String, FlatItem<'a>>, BundleError> {
        let mut items: BTreeMap<String, FlatItem<'a>> = BTreeMap::new();
        let mut stray = Vec::new();
        for (rest, bytes) in self.under(dir) {
            if rest.contains('/') {
                stray.push(rest);
                continue;
            }
            if is_meta_path(rest) {
                let stem = stem_of(rest);
                if !keeps_meta {
                    return Err(BundleError::UnexplainedMeta {
                        kind: dir.to_string(),
                        item: stem,
                        path: format!("{dir}/{rest}"),
                    });
                }
                items.entry(stem).or_default().meta = Some(bytes);
                continue;
            }
            let Some(base) = rest.strip_suffix(ext) else {
                stray.push(rest);
                continue;
            };
            if base.is_empty() || base.starts_with('.') {
                stray.push(rest);
                continue;
            }
            match base.strip_suffix(".distilled").filter(|_| distilled_form) {
                Some(stem) => items.entry(stem.to_string()).or_default().distilled = Some(bytes),
                None => items.entry(base.to_string()).or_default().raw = Some(bytes),
            }
        }
        if stray.is_empty() {
            Ok(items)
        } else {
            Err(self.unclaimed(dir, &stray))
        }
    }

    fn fragments(&self, bundle: &mut Bundle) -> Result<(), BundleError> {
        for (name, item) in self.flat("fragments", ".md", false, true)? {
            let doc = MdItem::read(self, &format!("fragments/{name}"), &item)?;
            bundle.fragments.insert(
                name,
                Fragment {
                    body: doc.body,
                    premise: doc.description,
                },
            );
        }
        Ok(())
    }

    fn commands(&self, bundle: &mut Bundle) -> Result<(), BundleError> {
        for (name, item) in self.flat("prompts", ".md", false, true)? {
            let doc = MdItem::read(self, &format!("prompts/{name}"), &item)?;
            bundle.commands.insert(
                name,
                Command {
                    body: doc.body,
                    description: doc.description,
                    exports: doc.exports,
                },
            );
        }
        Ok(())
    }

    fn mcp(&self, bundle: &mut Bundle) -> Result<(), BundleError> {
        for (name, item) in self.flat("mcp", ".yaml", true, false)? {
            let at = format!("mcp/{name}");
            let content = self.yaml(item.raw)?;
            let meta = self.yaml(item.meta)?;
            let mut server = mcp_server(&content, &at)?;
            server.tags = strings(&meta, "tags", &at)?;
            server.notes = string(&meta, "notes", &at)?;
            server.installation = string(&meta, "installation", &at)?;
            bundle.mcp.insert(name, server);
        }
        Ok(())
    }

    fn profiles(&self, bundle: &mut Bundle) -> Result<(), BundleError> {
        for (name, item) in self.flat("profiles", ".yaml", false, false)? {
            let source = text(item.raw.unwrap_or_default());
            let mut profile = Profile::decode(&source).map_err(|cause| BundleError::Profile {
                name: name.clone(),
                cause,
            })?;
            profile.name = name.clone();
            if self.generation < PROFILE_REFS {
                profile.canonicalize_refs();
            }
            bundle.profiles.insert(name, profile);
        }
        Ok(())
    }

    /// Parse a YAML file's text; absent or blank is the empty mapping.
    fn yaml(&self, bytes: Option<&[u8]>) -> Result<Value, BundleError> {
        match bytes {
            None => Ok(Value::Null),
            Some(b) if b.iter().all(u8::is_ascii_whitespace) => Ok(Value::Null),
            Some(b) => crate::yamlx::parse(&text(b)).map_err(|cause| BundleError::Syntax { cause }),
        }
    }

    fn skills(&self, bundle: &mut Bundle) -> Result<(), BundleError> {
        // Packages: `skills/<name>/<file...>`; sidecars: `skills/.<name>.meta.yaml`.
        let mut packages: BTreeMap<String, Vec<(&str, &[u8])>> = BTreeMap::new();
        let mut sidecars: BTreeMap<String, &[u8]> = BTreeMap::new();
        let mut unclaimed: Vec<String> = Vec::new();
        for (rest, bytes) in self.under("skills") {
            if let Some((name, inner)) = rest.split_once('/') {
                if name.is_empty() || name.starts_with('.') || inner.is_empty() {
                    unclaimed.push(rest.to_string());
                } else {
                    packages
                        .entry(name.to_string())
                        .or_default()
                        .push((inner, bytes));
                }
            } else if is_meta_path(rest) {
                sidecars.insert(stem_of(rest), bytes);
            } else {
                unclaimed.push(rest.to_string());
            }
        }
        // A package with no descriptor claims nothing: its files are unclaimed.
        let mut kept = BTreeMap::new();
        for (name, files) in packages {
            if files.iter().any(|(path, _)| *path == SKILL_DESCRIPTOR) {
                kept.insert(name, files);
            } else {
                unclaimed.extend(files.iter().map(|(path, _)| format!("{name}/{path}")));
            }
        }
        unclaimed.extend(
            sidecars
                .keys()
                .filter(|name| !kept.contains_key(*name))
                .map(|name| format!(".{name}.meta.yaml")),
        );
        if !unclaimed.is_empty() {
            let rests: Vec<&str> = unclaimed.iter().map(String::as_str).collect();
            return Err(self.unclaimed("skills", &rests));
        }
        for (name, files) in kept {
            let meta = self.yaml(sidecars.get(&name).copied())?;
            let at = format!("skills/{name}");
            let executable = strings(&meta, "executable", &at)?;
            let mut package: Vec<SkillFile> = files
                .iter()
                .map(|(path, bytes)| SkillFile {
                    path: (*path).to_string(),
                    bytes: bytes.to_vec(),
                    executable: executable.iter().any(|e| e == path),
                })
                .collect();
            package.sort_by(|a, b| a.path.cmp(&b.path));
            bundle.skills.insert(
                name,
                Skill {
                    path: String::new(),
                    tags: strings(&meta, "tags", &at)?,
                    notes: string(&meta, "notes", &at)?,
                    exports: exports(&meta, &at)?,
                    files: package,
                },
            );
        }
        Ok(())
    }

    fn hooks(&self, bundle: &mut Bundle) -> Result<(), BundleError> {
        let mut by_event: BTreeMap<String, BTreeMap<String, FlatItem<'a>>> = BTreeMap::new();
        let mut stray = Vec::new();
        for (rest, bytes) in self.under("hooks") {
            let Some((event, file)) = rest.split_once('/') else {
                stray.push(rest);
                continue;
            };
            if file.contains('/') {
                stray.push(rest);
                continue;
            }
            if is_meta_path(file) {
                by_event
                    .entry(event.to_string())
                    .or_default()
                    .entry(stem_of(file))
                    .or_default()
                    .meta = Some(bytes);
                continue;
            }
            match file.strip_suffix(".yaml") {
                Some(name) if !name.is_empty() && !name.starts_with('.') => {
                    by_event
                        .entry(event.to_string())
                        .or_default()
                        .entry(name.to_string())
                        .or_default()
                        .raw = Some(bytes);
                }
                _ => stray.push(rest),
            }
        }
        if !stray.is_empty() {
            return Err(self.unclaimed("hooks", &stray));
        }
        for event in HOOK_EVENTS {
            let Some(items) = by_event.get(event) else {
                continue;
            };
            let mut hooks = Vec::new();
            for (name, item) in items {
                let at = format!("{event}/{name}");
                let content = self.yaml(item.raw)?;
                let meta = self.yaml(item.meta)?;
                let matcher = string(&content, "matcher", &at)?;
                if !matcher.is_empty() {
                    return Err(BundleError::HookMatcher { hook: at, matcher });
                }
                let mut hook = hook(&content, event, name, &at)?;
                hook.order = match meta.get("order") {
                    Some(Value::Null) | None => None,
                    Some(v) => {
                        Some(yamlx::integer(v).map_err(|detail| self.item_error(&at, detail))?)
                    }
                };
                hook.tags = strings(&meta, "tags", &at)?;
                hooks.push(hook);
            }
            hooks.sort_by(|a, b| match (a.order, b.order) {
                (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.name.cmp(&b.name)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            });
            bundle.hooks.extend(hooks);
        }
        Ok(())
    }
}

/// The skill descriptor every package holds.
const SKILL_DESCRIPTOR: &str = "SKILL.md";

/// One item's parts in a flat kind directory.
#[derive(Default)]
struct FlatItem<'a> {
    raw: Option<&'a [u8]>,
    distilled: Option<&'a [u8]>,
    meta: Option<&'a [u8]>,
}

/// A markdown item decoded: its front matter and body.
struct MdItem {
    body: ItemBody,
    description: String,
    exports: BTreeMap<String, Value>,
}

impl MdItem {
    fn read(reader: &Reader<'_>, at: &str, item: &FlatItem<'_>) -> Result<MdItem, BundleError> {
        let raw = text(item.raw.unwrap_or_default());
        let (meta, body) = split(&raw);
        let meta = parse_meta(&meta)?;
        let mut out = MdItem {
            body: ItemBody {
                tags: strings(&meta, "tags", at)?,
                notes: string(&meta, "notes", at)?,
                installation: string(&meta, "installation", at)?,
                content: body,
                content_hash: string(&meta, "content_hash", at)?,
                no_distill: match meta.get("no_distill") {
                    Some(v) => yamlx::boolean(v).map_err(|d| reader.item_error(at, d))?,
                    None => false,
                },
                ..ItemBody::default()
            },
            description: string(&meta, "description", at)?,
            exports: exports(&meta, at)?,
        };
        if let Some(distilled) = item.distilled {
            let (dmeta, dbody) = split(&text(distilled));
            let dmeta = parse_meta(&dmeta)?;
            out.body.distilled = dbody;
            out.body.distilled_by = string(&dmeta, "distilled_by", at)?;
        }
        Ok(out)
    }
}

/// An item file's front matter and body.
fn split(raw: &str) -> (String, String) {
    match split_front_matter(raw) {
        Some(parts) => (parts.meta.to_string(), parts.body.to_string()),
        None => (String::new(), raw.to_string()),
    }
}

fn parse_meta(meta: &str) -> Result<Value, BundleError> {
    if meta.trim().is_empty() {
        return Ok(Value::Null);
    }
    crate::yamlx::parse(meta).map_err(|cause| BundleError::Syntax { cause })
}

fn strings(value: &Value, key: &str, at: &str) -> Result<Vec<String>, BundleError> {
    match value.get(key) {
        Some(v) => yamlx::string_list(v).map_err(|detail| BundleError::BadValue {
            path: format!("{at}.{key}"),
            detail,
        }),
        None => Ok(Vec::new()),
    }
}

fn string(value: &Value, key: &str, at: &str) -> Result<String, BundleError> {
    match value.get(key) {
        Some(v) => yamlx::string(v).map_err(|detail| BundleError::BadValue {
            path: format!("{at}.{key}"),
            detail,
        }),
        None => Ok(String::new()),
    }
}

/// Every file of the tree as `bundle view` prints it: the envelope first,
/// then the rest by path, each under a `==> path <==` line, a blank line
/// between, and a final newline added where a file lacks one.
pub fn dump(files: &Files) -> String {
    let mut ordered: Vec<(&String, &Vec<u8>)> = Vec::new();
    if let Some(envelope) = files.get_key_value(MANIFEST) {
        ordered.push(envelope);
    }
    ordered.extend(files.iter().filter(|(path, _)| path.as_str() != MANIFEST));
    let blocks: Vec<String> = ordered
        .into_iter()
        .map(|(path, bytes)| {
            let mut body = text(bytes);
            if !body.ends_with('\n') {
                body.push('\n');
            }
            format!("==> {path} <==\n{body}")
        })
        .collect();
    blocks.join("\n")
}

#[cfg(test)]
#[path = "tree.test.rs"]
mod tests;
