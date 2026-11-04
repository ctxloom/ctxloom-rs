//! `bundle list`, `bundle show` and `bundle view`.

use std::collections::BTreeMap;

use library::catalog::Entry;
use library::model::Bundle;
use library::refs::{self, ItemKind};
use library::tree;
use library::CatalogError;

use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::views::{
    BundleList, BundleRow, BundleShow, BundleView, ShownCommand, ShownFragment, ShownMcp,
    ShownSkill,
};
use crate::Flags;

impl Library {
    /// `bundle list`.
    pub fn bundle_list(&self, flags: &Flags) -> Outcome<BundleList> {
        self.run(flags, |p| Ok(p.bundle_list()))
    }

    /// `bundle show <name>`.
    pub fn bundle_show(&self, flags: &Flags, ask: &str) -> Outcome<BundleShow> {
        self.run(flags, |p| p.bundle_show(ask))
    }

    /// `bundle view <name[#path]> [--distilled]`.
    pub fn bundle_view(&self, flags: &Flags, ask: &str, distilled: bool) -> Outcome<BundleView> {
        self.run(flags, |p| p.bundle_view(ask, distilled))
    }
}

fn row(entry: &Entry) -> BundleRow {
    let bundle = &entry.bundle;
    BundleRow {
        name: entry.name.clone(),
        reference: format!("ctxloom+local:{}", entry.name),
        path: entry.manifest_path.clone(),
        version: bundle.version.clone(),
        description: bundle.description.clone(),
        tags: bundle.tags.clone(),
        fragment_count: bundle.fragments.len() as u64,
        command_count: bundle.commands.len() as u64,
        mcp_count: bundle.mcp.len() as u64,
        profile_count: bundle.profiles.len() as u64,
        held: false,
    }
}

impl Project {
    fn bundle_list(&self) -> BundleList {
        BundleList {
            rows: self.catalog.entries().iter().map(row).collect(),
            no_root: !self.has_root,
            hint: self.listing_hint(),
        }
    }

    /// The bundle asked for, or why there is none: a bundle that is there and
    /// cannot be read says so, naming the fault.
    pub(crate) fn bundle(&self, ask: &str) -> Result<&Entry, AppError> {
        match self.catalog.lookup(ask) {
            Ok(entry) => Ok(entry),
            Err(CatalogError::BundleNotFound { .. }) => {
                if let Some(cause) = self.unreadable.get(ask) {
                    return Err(AppError::BundleUnreadable {
                        ask: ask.to_string(),
                        cause: Box::new(cause.clone()),
                    });
                }
                Err(AppError::cause(CatalogError::BundleNotFound {
                    ask: ask.to_string(),
                }))
            }
            Err(other) => Err(AppError::cause(other)),
        }
    }

    fn bundle_show(&self, ask: &str) -> Result<BundleShow, AppError> {
        if !self.has_root {
            return Err(AppError::NoBundlesDir);
        }
        let entry = self.bundle(ask)?;
        Ok(show_of(entry))
    }

    fn bundle_view(&self, ask: &str, distilled: bool) -> Result<BundleView, AppError> {
        let (name, item_path) = ask.split_once('#').unwrap_or((ask, ""));
        let entry = self.bundle(name)?;
        if item_path.is_empty() {
            let files = self
                .content
                .read_tree(&entry.name)
                .map_err(AppError::Cause)?;
            return Ok(BundleView {
                bundle: name.to_string(),
                path: String::new(),
                distilled: false,
                content: tree::dump(&files),
                item: false,
            });
        }
        Ok(BundleView {
            bundle: name.to_string(),
            path: item_path.to_string(),
            distilled,
            content: view_item(&entry.bundle, item_path, distilled)?,
            item: true,
        })
    }
}

fn show_of(entry: &Entry) -> BundleShow {
    let bundle = &entry.bundle;
    BundleShow {
        name: entry.name.clone(),
        path: entry.manifest_path.clone(),
        version: bundle.version.clone(),
        author: bundle.author.clone(),
        description: bundle.description.clone(),
        tags: bundle.tags.clone(),
        notes: bundle.notes.clone(),
        installation: bundle.installation.clone(),
        fragments: bundle
            .fragments
            .iter()
            .map(|(name, f)| {
                (
                    name.clone(),
                    ShownFragment {
                        tags: f.body.tags.clone(),
                        preview: library_preview(&f.body.content),
                        distilled: !f.body.distilled.is_empty(),
                        no_distill: f.body.no_distill,
                        premise: f.premise.clone(),
                    },
                )
            })
            .collect(),
        commands: bundle
            .commands
            .iter()
            .map(|(name, c)| {
                (
                    name.clone(),
                    ShownCommand {
                        tags: c.body.tags.clone(),
                        description: c.description.clone(),
                        preview: library_preview(&c.body.content),
                        distilled: !c.body.distilled.is_empty(),
                        no_distill: c.body.no_distill,
                    },
                )
            })
            .collect(),
        mcp: bundle
            .mcp
            .iter()
            .map(|(name, m)| {
                (
                    name.clone(),
                    ShownMcp {
                        command: m.command.clone(),
                        args: m.args.clone(),
                        env: m.env.clone(),
                        notes: m.notes.clone(),
                        installation: m.installation.clone(),
                    },
                )
            })
            .collect(),
        skills: bundle
            .skills
            .iter()
            .map(|(name, s)| {
                (
                    name.clone(),
                    ShownSkill {
                        path: s.path.clone(),
                        tags: s.tags.clone(),
                        notes: s.notes.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
        profiles: bundle.profiles.keys().cloned().collect(),
        hooks: bundle.hook_ids(),
    }
}

/// The first line of an item, trimmed and cut to 70 bytes.
pub(crate) fn library_preview(content: &str) -> String {
    let first = content.trim().split('\n').next().unwrap_or("").trim();
    if first.len() <= 70 {
        return first.to_string();
    }
    let mut end = 70 - '\u{2026}'.len_utf8();
    while !first.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\u{2026}", &first[..end])
}

/// One item of a bundle, as `bundle view` prints it.
fn view_item(bundle: &Bundle, path: &str, distilled: bool) -> Result<String, AppError> {
    if let Some(name) = path.strip_prefix("profiles/") {
        let profile = bundle
            .profiles
            .get(name)
            .ok_or_else(|| AppError::ViewMissing {
                kind: "profile".to_string(),
                name: name.to_string(),
            })?;
        return Ok(format!(
            "# Profile: {name}\n{}",
            crate::yamlout::profile(profile)
        ));
    }
    let (kind, name) = refs::parse_selector(path).map_err(|cause| AppError::ViewPath {
        cause: Box::new(cause),
    })?;
    match kind {
        ItemKind::Fragment => {
            let fragment = bundle
                .fragments
                .get(&name)
                .ok_or_else(|| missing("fragment", &name))?;
            Ok(view_content(
                &fragment.body.content,
                &fragment.body.distilled,
                distilled,
            ))
        }
        ItemKind::Prompt => {
            let command = bundle
                .commands
                .get(&name)
                .ok_or_else(|| missing("command", &name))?;
            Ok(view_content(
                &command.body.content,
                &command.body.distilled,
                distilled,
            ))
        }
        ItemKind::Mcp => {
            let (shown, server) = match bundle.mcp.get(&name) {
                Some(server) => (name.clone(), server),
                None if bundle.mcp.len() == 1 => {
                    let (only, server) = bundle
                        .mcp
                        .iter()
                        .next()
                        .ok_or_else(|| missing("mcp server", &name))?;
                    (only.clone(), server)
                }
                None => return Err(missing("mcp server", &name)),
            };
            Ok(format!(
                "# MCP Server: {shown}\n{}",
                crate::yamlout::mcp(server)
            ))
        }
        ItemKind::Skill => {
            let skill = bundle
                .skills
                .get(&name)
                .ok_or_else(|| missing("skill", &name))?;
            Ok(format!("# Skill: {name}\n{}", crate::yamlout::skill(skill)))
        }
        ItemKind::Hook => Err(AppError::NotViewable {
            dir: kind.dir().to_string(),
            path: path.to_string(),
        }),
    }
}

fn missing(kind: &str, name: &str) -> AppError {
    AppError::ViewMissing {
        kind: kind.to_string(),
        name: name.to_string(),
    }
}

fn view_content(raw: &str, distilled: &str, use_distilled: bool) -> String {
    let (content, header) = if use_distilled && !distilled.is_empty() {
        (distilled, "# (distilled version)\n")
    } else {
        (raw, "")
    };
    let tail = if content.ends_with('\n') { "" } else { "\n" };
    format!("{header}{content}{tail}")
}
