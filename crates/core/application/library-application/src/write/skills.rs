//! `skill create|remove|export|import` (Go's `operations` skill leaves). A
//! skill is the directory `skills/<n>/` of a bundle; the sidecar
//! `skills/.<n>.meta.yaml` says which of its files are executable (the plan's
//! rule: an exec bit comes from the sidecar and from nowhere else).

use library::frontmatter::split_front_matter;
use library::write::archive::{self, Entry, Limits};
use library::write::item::valid_name;
use library::write::skill::{self, DEFAULT_DESCRIPTION};

use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::write::plan::Plan;
use crate::write::views::{
    Preview, Removal, SkillCreated, SkillExported, SkillImported, SkillRemoved,
};
use crate::write::WriteError;
use crate::Flags;

impl Library {
    /// `skill create <bundle> <name> [-d description]`.
    pub fn skill_create(
        &self,
        flags: &Flags,
        bundle: &str,
        name: &str,
        description: &str,
    ) -> Outcome<SkillCreated> {
        self.run(flags, |p| p.skill_create(bundle, name, description))
    }

    /// `skill remove <bundle>#skills/<name> [--yes]`: the package and its sidecar.
    pub fn skill_remove(
        &self,
        flags: &Flags,
        reference: &str,
        yes: bool,
    ) -> Outcome<Removal<SkillRemoved>> {
        self.run(flags, |p| p.skill_remove(reference, yes))
    }

    /// `skill export <bundle>#skills/<name> [-o file.zip]`: `<name>.zip` by
    /// default; a file that is there is not replaced.
    pub fn skill_export(
        &self,
        flags: &Flags,
        reference: &str,
        output: &str,
    ) -> Outcome<SkillExported> {
        self.run(flags, |p| p.skill_export(reference, output))
    }

    /// `skill import <archive> --bundle B [--force]`: the archive is adversarial
    /// input and every byte of it passes the unpacker's checks before anything
    /// is written; over a skill that is there it refuses unless forced.
    pub fn skill_import(
        &self,
        flags: &Flags,
        archive: &str,
        bundle: &str,
        force: bool,
    ) -> Outcome<SkillImported> {
        self.run(flags, |p| p.skill_import(archive, bundle, force))
    }
}

/// The bundle and skill a `<bundle>#skills/<name>` reference names.
fn split_reference(reference: &str) -> Result<(String, String), AppError> {
    let bad = || WriteError::SkillReference {
        got: reference.to_string(),
    };
    let (bundle, selector) = reference.split_once('#').ok_or_else(bad)?;
    let name = selector.strip_prefix("skills/").ok_or_else(bad)?;
    if bundle.is_empty() || name.is_empty() {
        return Err(bad().into());
    }
    Ok((bundle.to_string(), name.to_string()))
}

fn invalid(name: &str, detail: &str) -> AppError {
    WriteError::SkillInvalid {
        name: name.to_string(),
        detail: detail.to_string(),
    }
    .into()
}

impl Project {
    /// A skill's package directory in the plan: its name must be there.
    fn skill_dir(&self, plan: &Plan, name: &str) -> Result<String, AppError> {
        let dir = skill::package_dir(name);
        if plan.below(&dir).is_empty() {
            return Err(WriteError::SkillNotFound {
                name: name.to_string(),
            }
            .into());
        }
        Ok(dir)
    }

    fn skill_create(
        &self,
        bundle: &str,
        name: &str,
        description: &str,
    ) -> Result<SkillCreated, AppError> {
        self.require_project()?;
        if !valid_name(name) {
            return Err(WriteError::BadSkillName {
                name: name.to_string(),
            }
            .into());
        }
        let entry = self.bundle(bundle)?;
        let mut plan = self.plan_of(&entry.name)?;
        let dir = skill::package_dir(name);
        if !plan.below(&dir).is_empty() {
            return Err(WriteError::SkillExists {
                name: name.to_string(),
            }
            .into());
        }
        let description = if description.is_empty() {
            DEFAULT_DESCRIPTION
        } else {
            description
        };
        plan.write(
            &format!("{dir}/SKILL.md"),
            skill::skill_md(name, description),
        );
        self.commit(&entry.name, plan)?;
        Ok(SkillCreated {
            status: "created".to_string(),
            bundle: bundle.to_string(),
            name: name.to_string(),
            dir: format!("{}/{dir}", self.content.bundle_dir(&entry.name)),
        })
    }

    fn skill_remove(&self, reference: &str, yes: bool) -> Result<Removal<SkillRemoved>, AppError> {
        self.require_project()?;
        let (bundle, name) = split_reference(reference)?;
        let entry = self.bundle(&bundle)?;
        let mut plan = self.plan_of(&entry.name)?;
        let dir = self.skill_dir(&plan, &name)?;
        if !yes {
            return Ok(Removal::Preview(Preview::of(
                format!("skill \"{name}\" from bundle \"{bundle}\""),
                Vec::new(),
                format!("ctxloom skill remove {reference} --yes"),
            )));
        }
        plan.remove_dir(&dir);
        plan.remove(&skill::sidecar_path(&name));
        self.commit(&entry.name, plan)?;
        Ok(Removal::Done(SkillRemoved {
            status: "removed".to_string(),
            bundle,
            dir: format!("{}/{dir}", self.content.bundle_dir(&entry.name)),
            name,
        }))
    }

    fn skill_export(&self, reference: &str, output: &str) -> Result<SkillExported, AppError> {
        let (bundle, name) = split_reference(reference)?;
        let entry = self.bundle(&bundle)?;
        let plan = self.plan_of(&entry.name)?;
        let dir = self.skill_dir(&plan, &name)?;
        let sidecar = plan.text(&skill::sidecar_path(&name));
        let executable = skill::executable_files(sidecar.as_deref());
        let prefix = format!("{dir}/");
        let entries: Vec<Entry> = plan
            .below(&dir)
            .into_iter()
            .map(|path| {
                let rel = path.strip_prefix(&prefix).unwrap_or(&path).to_string();
                Entry {
                    bytes: plan.files().get(&path).cloned().unwrap_or_default(),
                    executable: executable.contains(&rel),
                    path: rel,
                }
            })
            .collect();
        let packed = archive::pack(&name, &entries).map_err(AppError::cause)?;
        let ports = self.write_ports()?;
        let zip = if output.is_empty() {
            format!("{name}.zip")
        } else {
            output.to_string()
        };
        if !ports
            .user
            .write_file(&zip, &packed, false)
            .map_err(AppError::Cause)?
        {
            return Err(WriteError::ExportExists { path: zip }.into());
        }
        Ok(SkillExported {
            name,
            zip_path: zip,
            bytes: packed.len() as u64,
        })
    }

    fn skill_import(
        &self,
        archive_path: &str,
        bundle: &str,
        force: bool,
    ) -> Result<SkillImported, AppError> {
        self.require_project()?;
        let ports = self.write_ports()?;
        let bytes = ports
            .user
            .read_file(archive_path)
            .map_err(AppError::Cause)?;
        let unpacked = archive::unpack(&bytes, &Limits::default()).map_err(AppError::cause)?;
        let name = unpacked.top.clone();
        if !valid_name(&name) {
            return Err(WriteError::BadSkillName { name }.into());
        }
        let Some(descriptor) = unpacked.entries.iter().find(|e| e.path == "SKILL.md") else {
            return Err(invalid(&name, "SKILL.md is required"));
        };
        let text = String::from_utf8_lossy(&descriptor.bytes).into_owned();
        let front = split_front_matter(&text).ok_or_else(|| {
            invalid(
                &name,
                "SKILL.md must have YAML frontmatter delimited by `---` lines",
            )
        })?;
        if let Err(cause) = angzarr_yaml::parse(front.meta) {
            return Err(invalid(
                &name,
                &format!("invalid SKILL.md frontmatter: {cause}"),
            ));
        }
        let entry = self.bundle(bundle)?;
        let mut plan = self.plan_of(&entry.name)?;
        let dir = skill::package_dir(&name);
        let dest = format!("{}/{dir}", self.content.bundle_dir(&entry.name));
        if !plan.below(&dir).is_empty() {
            if !force {
                return Err(WriteError::ImportExists {
                    what: "skill".to_string(),
                    path: dest,
                }
                .into());
            }
            plan.remove_dir(&dir);
        }
        for file in &unpacked.entries {
            plan.put(
                &format!("{dir}/{}", file.path),
                file.bytes.clone(),
                file.executable,
            );
        }
        let executable: Vec<String> = unpacked
            .entries
            .iter()
            .filter(|e| e.executable)
            .map(|e| e.path.clone())
            .collect();
        let sidecar = plan.text(&skill::sidecar_path(&name));
        match skill::with_executable(sidecar.as_deref(), &executable).map_err(AppError::cause)? {
            Some(text) => plan.write(&skill::sidecar_path(&name), text),
            None => {
                plan.remove(&skill::sidecar_path(&name));
            }
        }
        self.commit(&entry.name, plan)?;
        Ok(SkillImported {
            status: "imported".to_string(),
            bundle: bundle.to_string(),
            name,
            dir: dest,
            file_count: unpacked.entries.len() as u64,
        })
    }
}

#[cfg(test)]
#[path = "skills.test.rs"]
mod tests;
