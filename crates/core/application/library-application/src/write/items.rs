//! `fragment|command create|remove|edit` (Go's `operations.AddItem`,
//! `DeleteItem`, `SetItemContent`, and the editor round trip of
//! `cli/item_crud.go`).

use library::write::item;

use crate::error::AppError;
use crate::items::Kind;
use crate::project::{Library, Outcome, Project};
use crate::write::editor::editor_command;
use crate::write::views::{ItemCreated, ItemEdited, ItemRemoved, Preview, Removal};
use crate::write::WriteError;
use crate::Flags;

impl Library {
    /// `fragment create <bundle> <name>`, `command create`: a placeholder in
    /// the bundle's tree.
    pub fn item_create(
        &self,
        flags: &Flags,
        kind: Kind,
        bundle: &str,
        name: &str,
    ) -> Outcome<ItemCreated> {
        self.run(flags, |p| p.item_create(kind, bundle, name))
    }

    /// `fragment remove <bundle>#fragments/<name> [--yes]`: the item and its
    /// distilled form; without `--yes` what would go, and nothing goes.
    pub fn item_remove(
        &self,
        flags: &Flags,
        kind: Kind,
        reference: &str,
        yes: bool,
    ) -> Outcome<Removal<ItemRemoved>> {
        self.run(flags, |p| p.item_remove(kind, reference, yes))
    }

    /// `fragment edit <ref> [--no-distill]`: the body through the editor. The
    /// front matter stays (its stale hash dropped) and so does the rest of
    /// the tree; a body that did not change, or came back empty, is not saved.
    /// Distilling is S7's: an item that would be distilled is saved raw and
    /// the command says so.
    pub fn item_edit(
        &self,
        flags: &Flags,
        kind: Kind,
        reference: &str,
        no_distill: bool,
    ) -> Outcome<ItemEdited> {
        self.run(flags, |p| p.item_edit(kind, reference, no_distill))
    }
}

impl Project {
    fn item_create(&self, kind: Kind, bundle: &str, name: &str) -> Result<ItemCreated, AppError> {
        self.require_project()?;
        let entry = self.bundle(bundle)?;
        if !item::valid_name(name) {
            return Err(WriteError::BadItemName {
                kind: kind.word().to_string(),
                name: name.to_string(),
            }
            .into());
        }
        let mut plan = self.plan_of(&entry.name)?;
        let path = item::item_path(kind.tree_dir(), name);
        if plan.has(&path) {
            return Err(WriteError::ItemExists {
                kind: kind.word().to_string(),
                name: name.to_string(),
            }
            .into());
        }
        plan.write(&path, item::placeholder_fragment(name));
        self.commit(&entry.name, plan)?;
        Ok(ItemCreated {
            status: "created".to_string(),
            bundle: bundle.to_string(),
            name: name.to_string(),
            path: entry.manifest_path.clone(),
            kind: kind.word().to_string(),
        })
    }

    fn item_remove(
        &self,
        kind: Kind,
        reference: &str,
        yes: bool,
    ) -> Result<Removal<ItemRemoved>, AppError> {
        self.require_project()?;
        let (bundle, name) = self.item_target(kind, reference)?;
        let entry = self.bundle(&bundle)?;
        let mut plan = self.plan_of(&entry.name)?;
        let path = item::item_path(kind.tree_dir(), &name);
        if !plan.has(&path) {
            return Err(WriteError::ItemNotFound {
                kind: kind.word().to_string(),
                name,
            }
            .into());
        }
        if !yes {
            return Ok(Removal::Preview(Preview::of(
                format!("{} \"{name}\" from bundle \"{bundle}\"", kind.word()),
                Vec::new(),
                format!("ctxloom {} remove {reference} --yes", kind.word()),
            )));
        }
        plan.remove(&path);
        plan.remove(&item::distilled_path(kind.tree_dir(), &name));
        self.commit(&entry.name, plan)?;
        Ok(Removal::Done(ItemRemoved {
            status: "deleted".to_string(),
            bundle,
            name,
            path: entry.manifest_path.clone(),
            kind: kind.word().to_string(),
        }))
    }

    fn item_edit(
        &self,
        kind: Kind,
        reference: &str,
        no_distill: bool,
    ) -> Result<ItemEdited, AppError> {
        self.require_project()?;
        let (bundle, name) = self.item_target(kind, reference)?;
        let entry = self.bundle(&bundle)?;
        let names: Vec<&str> = match kind {
            Kind::Fragment => entry.bundle.fragments.keys().map(String::as_str).collect(),
            Kind::Command => entry.bundle.commands.keys().map(String::as_str).collect(),
        };
        if !names.contains(&name.as_str()) {
            return Err(AppError::ItemMissing {
                kind: kind.word().to_string(),
                name,
                available: names.join(", "),
            });
        }
        let ports = self.write_ports()?;
        let mut plan = self.plan_of(&entry.name)?;
        let path = item::item_path(kind.tree_dir(), &name);
        let held = plan.text(&path).unwrap_or_default();
        let body = item::body_of(&held).to_string();
        let command = editor_command(&self.config, &self.environ);
        let edited = ports
            .editor
            .edit(&command, &format!("{name}.md"), &body)
            .map_err(AppError::Cause)?;
        let word = kind.word().to_string();
        let done = |status: &str, path: String, skipped: bool| ItemEdited {
            status: status.to_string(),
            bundle: bundle.clone(),
            name: name.clone(),
            path,
            distilled: false,
            kind: word.clone(),
            reference: reference.to_string(),
            distill_skipped: skipped,
        };
        if edited == body {
            return Ok(done("unchanged", String::new(), false));
        }
        if edited.trim().is_empty() {
            return Err(WriteError::EmptyEdit { kind: word, name }.into());
        }
        let marked = item::marked_no_distill(&held);
        plan.write(&path, item::replace_body(&held, &edited));
        plan.remove(&item::distilled_path(kind.tree_dir(), &name));
        self.commit(&entry.name, plan)?;
        if !no_distill && !marked {
            return Err(WriteError::DistillUnavailable {
                kind: word,
                name,
                reference: reference.to_string(),
            }
            .into());
        }
        Ok(done(
            "updated",
            entry.manifest_path.clone(),
            no_distill && !marked,
        ))
    }
}

#[cfg(test)]
#[path = "items.test.rs"]
mod tests;
