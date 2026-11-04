//! `profile create|modify|edit|export|import|remove` (Go's
//! `operations.CreateProfile`, `UpdateProfile`, `SetProfileContent`,
//! `ExportProfile`, `ImportProfile`, `DeleteProfile`). A profile is the file
//! `profiles/<name>.yaml` of a local bundle; the project's own live in the
//! `project` bundle, made the first time one is written.

use library::profile::{self, Profile};
use library::write::profile::{
    edit_profile, new_profile, profile_path, NewProfile, ProfileEdit, PROJECT_BUNDLE,
};
use library::ProfileError;

use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::write::editor::editor_command;
use crate::write::plan::Plan;
use crate::write::requests::ProfileCreateRequest;
use crate::write::views::{
    Preview, ProfileCreated, ProfileEdited, ProfileExported, ProfileImported, ProfileModified,
    ProfileRemoved, Removal,
};
use crate::write::WriteError;
use crate::Flags;

impl Library {
    /// `profile create <name> [--parent P]... [--include B]... [-d D] [--llm L]
    /// [--bundle B]`.
    pub fn profile_create(
        &self,
        flags: &Flags,
        request: &ProfileCreateRequest,
    ) -> Outcome<ProfileCreated> {
        self.run(flags, |p| p.profile_create(request))
    }

    /// `profile modify <name> ...`: only the lines of what changes change.
    pub fn profile_modify(
        &self,
        flags: &Flags,
        name: &str,
        edit: &ProfileEdit,
    ) -> Outcome<ProfileModified> {
        self.run(flags, |p| p.profile_modify(name, edit))
    }

    /// `profile edit <name>`: the file through the editor, kept only if it
    /// still reads as a profile.
    pub fn profile_edit(&self, flags: &Flags, name: &str) -> Outcome<ProfileEdited> {
        self.run(flags, |p| p.profile_edit(name))
    }

    /// `profile export <name> <dest-dir>`: the definition, as `<dest>/<name>.yaml`.
    pub fn profile_export(
        &self,
        flags: &Flags,
        name: &str,
        dest_dir: &str,
    ) -> Outcome<ProfileExported> {
        self.run(flags, |p| p.profile_export(name, dest_dir))
    }

    /// `profile import <path> [--force] [--bundle B]`.
    pub fn profile_import(
        &self,
        flags: &Flags,
        path: &str,
        force: bool,
        bundle: &str,
    ) -> Outcome<ProfileImported> {
        self.run(flags, |p| p.profile_import(path, force, bundle))
    }

    /// `profile remove <name> [--yes]`.
    pub fn profile_remove(
        &self,
        flags: &Flags,
        name: &str,
        yes: bool,
    ) -> Outcome<Removal<ProfileRemoved>> {
        self.run(flags, |p| p.profile_remove(name, yes))
    }
}

/// A profile found: the bundle it is in, its name there, the profile and the
/// path it is shown at.
struct Found {
    bundle: String,
    name: String,
    profile: Profile,
    shown: String,
}

impl Project {
    fn find_profile(&self, ask: &str) -> Result<Found, AppError> {
        let key = profile::canonical_key(ask).map_err(AppError::cause)?;
        let profiles = self.profiles();
        let not_found = || {
            AppError::cause(ProfileError::NotFound {
                name: ask.to_string(),
            })
        };
        let (found, shown) = profiles.by_key.get(&key).ok_or_else(not_found)?;
        let (bundle, name) = key
            .strip_prefix("ctxloom+local:")
            .and_then(|rest| rest.split_once("#profiles/"))
            .ok_or_else(not_found)?;
        Ok(Found {
            bundle: bundle.to_string(),
            name: name.to_string(),
            profile: found.clone(),
            shown: shown.clone(),
        })
    }

    /// The label is one the project can use.
    fn require_llm(&self, label: &str) -> Result<(), AppError> {
        let usable = self.available_llms();
        if label.is_empty() || usable.iter().any(|l| l == label) {
            return Ok(());
        }
        Err(AppError::UnknownLlm {
            label: label.to_string(),
            usable: usable.join(", "),
        })
    }

    /// A local parent is one the project has; a remote one is taken as given.
    fn require_parent(&self, parent: &str) -> Result<(), AppError> {
        let missing = || WriteError::ParentNotFound {
            name: parent.to_string(),
        };
        let key = profile::canonical_key(parent).map_err(|_| missing())?;
        if key.starts_with("ctxloom+local:") && !self.profiles().by_key.contains_key(&key) {
            return Err(missing().into());
        }
        Ok(())
    }

    /// The plan over a bundle that may not be there yet: the project's own is
    /// made the first time. Whether it has to be claimed.
    fn plan_for_profile(&self, bundle: &str) -> Result<(Plan, bool), AppError> {
        let there = self.catalog.entries().iter().any(|e| e.name == bundle)
            || self.unreadable.contains_key(bundle);
        if there {
            return Ok((self.plan_of(bundle)?, false));
        }
        if bundle != PROJECT_BUNDLE {
            return Err(AppError::cause(library::CatalogError::BundleNotFound {
                ask: bundle.to_string(),
            }));
        }
        let mut plan = Plan::of(library::tree::Files::new(), Default::default());
        plan.write(
            "bundle.yaml",
            library::write::envelope::new_envelope("1.0.0", ""),
        );
        Ok((plan, true))
    }

    /// Hand the plan over; a bundle made for it is claimed first and given
    /// back if the write fails.
    fn commit_profile(&self, bundle: &str, plan: Plan, claim: bool) -> Result<(), AppError> {
        if claim && !self.content.claim(bundle).map_err(AppError::Cause)? {
            return Err(WriteError::BundleExists {
                path: self.content.manifest_path(bundle),
            }
            .into());
        }
        let done = self.commit(bundle, plan);
        if done.is_err() && claim {
            let _ = self.content.remove_bundle(bundle);
        }
        done
    }

    fn check_profile_name(&self, name: &str) -> Result<(), AppError> {
        if name.contains('#') {
            return Err(WriteError::BadProfileName {
                name: name.to_string(),
            }
            .into());
        }
        if !library::write::item::valid_name(name) {
            return Err(WriteError::ProfileNameSegment {
                name: name.to_string(),
            }
            .into());
        }
        Ok(())
    }

    fn profile_create(&self, request: &ProfileCreateRequest) -> Result<ProfileCreated, AppError> {
        self.require_project()?;
        if request.parents.is_empty() && request.bundles.is_empty() {
            return Err(WriteError::ProfileEmpty {}.into());
        }
        self.require_llm(&request.llm)?;
        self.check_profile_name(&request.name)?;
        let target = if request.target.is_empty() {
            PROJECT_BUNDLE.to_string()
        } else {
            self.bundle(&request.target)?.name.clone()
        };
        let key = format!("ctxloom+local:{target}#profiles/{}", request.name);
        if self.profiles().by_key.contains_key(&key) {
            return Err(WriteError::ProfileExists {
                name: request.name.clone(),
            }
            .into());
        }
        for parent in &request.parents {
            self.require_parent(parent)?;
        }
        let (mut plan, claim) = self.plan_for_profile(&target)?;
        let text = new_profile(&NewProfile {
            description: request.description.clone(),
            parents: request.parents.clone(),
            llm: request.llm.clone(),
            bundles: request.bundles.clone(),
        });
        plan.write(&profile_path(&request.name), text);
        self.commit_profile(&target, plan, claim)?;
        Ok(ProfileCreated {
            name: request.name.clone(),
            parents: request.parents.clone(),
            bundles: request.bundles.clone(),
            path: format!(
                "{}/{}",
                self.content.bundle_dir(&target),
                profile_path(&request.name)
            ),
        })
    }

    fn profile_modify(&self, name: &str, edit: &ProfileEdit) -> Result<ProfileModified, AppError> {
        self.require_project()?;
        let found = self.find_profile(name)?;
        if let Some(llm) = &edit.llm {
            self.require_llm(llm)?;
        }
        for parent in &edit.add_parents {
            self.require_parent(parent)?;
        }
        let mut plan = self.plan_of(&found.bundle)?;
        let path = profile_path(&found.name);
        let text = plan.text(&path).unwrap_or_default();
        let (next, changes) = edit_profile(&text, edit).map_err(AppError::cause)?;
        if changes.is_empty() {
            return Ok(ProfileModified {
                name: name.to_string(),
                changes,
            });
        }
        Profile::decode(&next).map_err(|e| WriteError::ProfileInvalid {
            cause: e.to_string(),
        })?;
        plan.write(&path, next);
        self.commit(&found.bundle, plan)?;
        Ok(ProfileModified {
            name: name.to_string(),
            changes,
        })
    }

    fn profile_edit(&self, name: &str) -> Result<ProfileEdited, AppError> {
        self.require_project()?;
        let found = self.find_profile(name)?;
        let ports = self.write_ports()?;
        let mut plan = self.plan_of(&found.bundle)?;
        let path = profile_path(&found.name);
        let text = plan.text(&path).unwrap_or_default();
        let command = editor_command(&self.config, &self.environ);
        let next = ports
            .editor
            .edit(&command, &format!("{}.yaml", found.name), &text)
            .map_err(AppError::Cause)?;
        if next == text {
            return Ok(ProfileEdited::Unchanged);
        }
        Profile::decode(&next).map_err(AppError::cause)?;
        plan.write(&path, next);
        self.commit(&found.bundle, plan)?;
        Ok(ProfileEdited::Updated { path: found.shown })
    }

    fn profile_export(&self, name: &str, dest_dir: &str) -> Result<ProfileExported, AppError> {
        let found = self.find_profile(name)?;
        let ports = self.write_ports()?;
        let plan = self.plan_of(&found.bundle)?;
        let bytes = plan
            .files()
            .get(&profile_path(&found.name))
            .cloned()
            .unwrap_or_default();
        let dest = format!("{}/{}.yaml", dest_dir.trim_end_matches('/'), found.name);
        ports
            .user
            .write_file(&dest, &bytes, true)
            .map_err(AppError::Cause)?;
        Ok(ProfileExported {
            source: found.shown,
            dest,
        })
    }

    fn profile_import(
        &self,
        path: &str,
        force: bool,
        bundle: &str,
    ) -> Result<ProfileImported, AppError> {
        self.require_project()?;
        let ports = self.write_ports()?;
        let bytes = ports.user.read_file(path).map_err(AppError::Cause)?;
        let text = String::from_utf8(bytes).map_err(|_| WriteError::ProfileInvalid {
            cause: "the file is not UTF-8".to_string(),
        })?;
        Profile::decode(&text).map_err(AppError::cause)?;
        let file = path.rsplit('/').next().unwrap_or(path);
        let name = file
            .strip_suffix(".yaml")
            .or_else(|| file.strip_suffix(".yml"))
            .unwrap_or(file);
        self.check_profile_name(name)?;
        let target = if bundle.is_empty() {
            PROJECT_BUNDLE.to_string()
        } else {
            self.bundle(bundle)?.name.clone()
        };
        let dest = format!(
            "{}/{}",
            self.content.bundle_dir(&target),
            profile_path(name)
        );
        let key = format!("ctxloom+local:{target}#profiles/{name}");
        if self.profiles().by_key.contains_key(&key) && !force {
            return Err(WriteError::ImportExists {
                what: "profile".to_string(),
                path: dest,
            }
            .into());
        }
        let (mut plan, claim) = self.plan_for_profile(&target)?;
        plan.write(&profile_path(name), text);
        self.commit_profile(&target, plan, claim)?;
        Ok(ProfileImported {
            source: path.to_string(),
            dest,
        })
    }

    fn profile_remove(&self, name: &str, yes: bool) -> Result<Removal<ProfileRemoved>, AppError> {
        self.require_project()?;
        let found = self.find_profile(name)?;
        if !yes {
            let n = found.profile.bundles.len();
            let detail = if n > 0 {
                vec![format!("{n} bundle(s)")]
            } else {
                Vec::new()
            };
            return Ok(Removal::Preview(Preview::of(
                format!("profile \"{name}\""),
                detail,
                format!("ctxloom profile remove {name} --yes"),
            )));
        }
        let mut plan = self.plan_of(&found.bundle)?;
        plan.remove(&profile_path(&found.name));
        self.commit(&found.bundle, plan)?;
        Ok(Removal::Done(ProfileRemoved {
            status: "deleted".to_string(),
            profile: name.to_string(),
        }))
    }
}

#[cfg(test)]
#[path = "profiles.test.rs"]
mod tests;
