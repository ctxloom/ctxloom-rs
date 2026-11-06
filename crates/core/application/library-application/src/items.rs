//! `fragment` and `command` (list, show), `fragment premises`, and `skill`
//! (list, show).

use std::collections::BTreeSet;

use library::catalog::ContentInfo;
use library::frontmatter::split_front_matter;
use library::model::Skill;
use library::refs::{self, ItemKind};
use library::sha256;
use library::{premise, CatalogError};

use crate::error::{AppError, Notice};
use crate::project::{Library, Outcome, Project};
use crate::views::{
    ItemList, ItemRow, ItemShow, PremiseRow, Premises, SkillFileRow, SkillList, SkillRow, SkillShow,
};
use crate::Flags;

/// A kind of item the listing and show commands serve.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Fragment,
    Command,
}

impl Kind {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Kind::Fragment => "fragment",
            Kind::Command => "command",
        }
    }

    pub(crate) fn dir(self) -> &'static str {
        match self {
            Kind::Fragment => "fragments",
            Kind::Command => "commands",
        }
    }

    /// The directory of the kind's files in a bundle's tree (a command's are
    /// in `prompts`).
    pub(crate) fn tree_dir(self) -> &'static str {
        match self {
            Kind::Fragment => library::write::item::FRAGMENTS,
            Kind::Command => library::write::item::PROMPTS,
        }
    }

    fn item(self) -> ItemKind {
        match self {
            Kind::Fragment => ItemKind::Fragment,
            Kind::Command => ItemKind::Prompt,
        }
    }
}

impl Library {
    /// `fragment list [--bundle B]` and `command list [--bundle B]`.
    pub fn item_list(&self, flags: &Flags, kind: Kind, bundle: &str) -> Outcome<ItemList> {
        self.run(flags, |p| p.item_list(kind, bundle))
    }

    /// `fragment show <ref> [--distilled]` and `command show`.
    pub fn item_show(
        &self,
        flags: &Flags,
        kind: Kind,
        reference: &str,
        distilled: bool,
    ) -> Outcome<ItemShow> {
        self.run(flags, |p| p.item_show(kind, reference, distilled))
    }

    /// `fragment premises`.
    pub fn premises(&self, flags: &Flags) -> Outcome<Premises> {
        self.run(flags, |p| Ok(p.premises()))
    }

    /// `skill list [--bundle B]`.
    pub fn skill_list(&self, flags: &Flags, bundle: &str) -> Outcome<SkillList> {
        self.run(flags, |p| p.skill_list(bundle))
    }

    /// `skill show <ref>`.
    pub fn skill_show(&self, flags: &Flags, reference: &str) -> Outcome<SkillShow> {
        self.run(flags, |p| p.skill_show(reference))
    }
}

impl Project {
    fn item_list(&self, kind: Kind, bundle: &str) -> Result<ItemList, AppError> {
        let infos: Vec<ContentInfo> = match kind {
            Kind::Fragment => self.catalog.list_fragments(),
            Kind::Command => self.catalog.list_commands(),
        };
        let all: Vec<ItemRow> = infos
            .into_iter()
            .map(|info| ItemRow {
                reference: format!("{}#{}/{}", info.bundle, kind.dir(), info.name),
                bundle_label: info.bundle.clone(),
                bundle: info.bundle,
                name: info.name,
                tags: info.tags,
                remote: String::new(),
            })
            .collect();
        let total = all.len();
        let rows: Vec<ItemRow> = if bundle.is_empty() {
            all
        } else {
            all.into_iter().filter(|r| r.bundle == bundle).collect()
        };
        if rows.is_empty() && !bundle.is_empty() {
            let known: Vec<&str> = self
                .catalog
                .entries()
                .iter()
                .map(|e| e.name.as_str())
                .collect();
            if !known.is_empty() && !known.contains(&bundle) {
                return Err(AppError::NoBundleNamed {
                    name: bundle.to_string(),
                    known: known.join(", "),
                });
            }
        }
        Ok(ItemList {
            rows,
            kind: kind.word().to_string(),
            bundle_filter: bundle.to_string(),
            total,
            hint: self.listing_hint(),
        })
    }

    /// The bundle and item a reference names, for a command serving `kind`.
    pub(crate) fn item_target(
        &self,
        kind: Kind,
        reference: &str,
    ) -> Result<(String, String), AppError> {
        let expected = format!("bundle#{}/name", kind.dir());
        let Some((bundle, selector)) = reference.split_once('#') else {
            return Err(AppError::BadReference {
                expected,
                got: reference.to_string(),
            });
        };
        let (got, item) = refs::parse_selector(selector).map_err(|cause| {
            AppError::cause(CatalogError::ItemSyntax {
                ask: reference.to_string(),
                cause,
            })
        })?;
        if got != kind.item() {
            return Err(AppError::WrongKind {
                ask: reference.to_string(),
                got: selector_dir(got).to_string(),
                want: kind.dir().to_string(),
                expected,
            });
        }
        if bundle.is_empty() {
            return Err(AppError::BadReference {
                expected: format!("a bundle name in {reference:?}"),
                got: reference.to_string(),
            });
        }
        Ok((bundle.to_string(), item))
    }

    fn item_show(
        &self,
        kind: Kind,
        reference: &str,
        distilled: bool,
    ) -> Result<ItemShow, AppError> {
        let (bundle, name) = self.item_target(kind, reference)?;
        let entry = self.bundle(&bundle)?;
        let body = match kind {
            Kind::Fragment => entry.bundle.fragments.get(&name).map(|f| &f.body),
            Kind::Command => entry.bundle.commands.get(&name).map(|c| &c.body),
        };
        let Some(body) = body else {
            let available: Vec<&str> = match kind {
                Kind::Fragment => entry.bundle.fragments.keys().map(String::as_str).collect(),
                Kind::Command => entry.bundle.commands.keys().map(String::as_str).collect(),
            };
            return Err(AppError::ItemMissing {
                kind: kind.word().to_string(),
                name,
                available: available.join(", "),
            });
        };
        let use_distilled = distilled && !body.distilled.is_empty();
        Ok(ItemShow {
            name,
            reference: reference.to_string(),
            content: if use_distilled {
                body.distilled.clone()
            } else {
                body.content.clone()
            },
            distilled: use_distilled,
        })
    }

    fn premises(&mut self) -> Premises {
        let entries = premise::index(&self.catalog_with_companions());
        if entries.is_empty() {
            return Premises {
                instruction: premise::NONE.to_string(),
                fragments: Vec::new(),
            };
        }
        Premises {
            instruction: premise::selection_instruction(),
            fragments: entries
                .into_iter()
                .map(|e| PremiseRow {
                    name: e.name,
                    premise: e.premise,
                })
                .collect(),
        }
    }

    /// Every skill of every bundle that loads, by bundle then name.
    fn skills(&mut self) -> Result<Vec<LoadedSkill>, AppError> {
        let mut out = Vec::new();
        let mut withheld = Vec::new();
        for entry in self.catalog.entries() {
            let executable = match self.catalog.origin(&entry.name) {
                Some(origin) => origin.executable.clone(),
                None => self
                    .content
                    .executable_files(&entry.name)
                    .map_err(AppError::Cause)?,
            };
            for (name, skill) in &entry.bundle.skills {
                match load_skill(&entry.name, &entry.bundle.tags, name, skill, &executable) {
                    Ok(loaded) => out.push(loaded),
                    Err(why) => {
                        withheld.push(Notice::text(format!("skill \"{name}\" withheld: {why}")))
                    }
                }
            }
        }
        self.notices.extend(withheld);
        Ok(out)
    }

    /// Every skill as a listing row, by bundle then name.
    pub(crate) fn skill_rows(&mut self) -> Result<Vec<SkillRow>, AppError> {
        let mut rows: Vec<SkillRow> = self
            .skills()?
            .into_iter()
            .map(|s| SkillRow {
                name: format!("{}/{}", s.bundle, s.item),
                description: s.description,
                tags: s.tags,
                source: s.bundle,
                file_count: s.files.len() as u64,
            })
            .collect();
        rows.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.name.cmp(&b.name)));
        Ok(rows)
    }

    fn skill_list(&mut self, bundle: &str) -> Result<SkillList, AppError> {
        let mut rows = self.skill_rows()?;
        let total = rows.len();
        if !bundle.is_empty() {
            rows.retain(|r| r.source == bundle);
        }
        Ok(SkillList {
            rows,
            bundle_filter: bundle.to_string(),
            total,
        })
    }

    fn skill_show(&mut self, reference: &str) -> Result<SkillShow, AppError> {
        let (bundle, item) = match reference.split_once('#') {
            None => (None, reference.to_string()),
            Some((bundle, selector)) => {
                let (kind, item) = refs::parse_selector(selector).map_err(|cause| {
                    AppError::cause(CatalogError::ItemSyntax {
                        ask: reference.to_string(),
                        cause,
                    })
                })?;
                if kind != ItemKind::Skill {
                    return Err(AppError::cause(CatalogError::BadItemRef {
                        ask: reference.to_string(),
                        kind: kind.label().to_string(),
                        wanted: ItemKind::Skill.label().to_string(),
                    }));
                }
                (Some(self.bundle(bundle)?.name.clone()), item)
            }
        };
        let all = self.skills()?;
        let found = all
            .into_iter()
            .find(|s| s.item == item && bundle.as_ref().is_none_or(|b| *b == s.bundle));
        match (found, bundle) {
            (Some(skill), _) => Ok(show_of(skill)),
            (None, Some(bundle)) => Err(AppError::SkillNotFound { name: item, bundle }),
            (None, None) => Err(AppError::SkillNameNotFound { name: item }),
        }
    }
}

fn selector_dir(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Prompt => "commands",
        other => other.dir(),
    }
}

/// A skill package, loaded.
struct LoadedSkill {
    bundle: String,
    item: String,
    name: String,
    description: String,
    license: String,
    compatibility: String,
    allowed_tools: Vec<String>,
    body: String,
    tags: Vec<String>,
    files: Vec<(String, Vec<u8>, bool)>,
}

fn load_skill(
    bundle: &str,
    bundle_tags: &[String],
    name: &str,
    skill: &Skill,
    executable: &BTreeSet<String>,
) -> Result<LoadedSkill, String> {
    let descriptor = skill
        .files
        .iter()
        .find(|f| f.path == "SKILL.md")
        .ok_or_else(|| "SKILL.md is required".to_string())?;
    let text = String::from_utf8_lossy(&descriptor.bytes).into_owned();
    let parts =
        split_front_matter(&text).ok_or_else(|| "SKILL.md has no frontmatter".to_string())?;
    let meta = library_yaml(parts.meta)?;
    let field = |key: &str| -> String {
        meta.get(key)
            .and_then(angzarr_yaml::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let allowed_tools = meta
        .get("allowed-tools")
        .and_then(angzarr_yaml::Value::as_seq)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    let mut tags: Vec<String> = Vec::new();
    for tag in bundle_tags.iter().chain(&skill.tags) {
        if !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    Ok(LoadedSkill {
        bundle: bundle.to_string(),
        item: name.to_string(),
        name: name.to_string(),
        description: field("description"),
        license: field("license"),
        compatibility: field("compatibility"),
        allowed_tools,
        body: parts.body.to_string(),
        tags,
        files: skill
            .files
            .iter()
            .map(|f| {
                let key = format!("skills/{name}/{}", f.path);
                (f.path.clone(), f.bytes.clone(), executable.contains(&key))
            })
            .collect(),
    })
}

fn library_yaml(text: &str) -> Result<angzarr_yaml::Value, String> {
    if text.trim().is_empty() {
        return Ok(angzarr_yaml::Value::Null);
    }
    angzarr_yaml::parse(text).map_err(|e| format!("invalid SKILL.md frontmatter: {e}"))
}

fn show_of(skill: LoadedSkill) -> SkillShow {
    SkillShow {
        name: skill.name,
        bundle: skill.bundle,
        description: skill.description,
        license: skill.license,
        compatibility: skill.compatibility,
        allowed_tools: skill.allowed_tools,
        body: skill.body,
        files: skill
            .files
            .into_iter()
            .map(|(path, bytes, executable)| SkillFileRow {
                path,
                sha256: format!("sha256:{}", sha256::hex(&bytes)),
                mode: if executable { "0755" } else { "0644" }.to_string(),
            })
            .collect(),
    }
}
