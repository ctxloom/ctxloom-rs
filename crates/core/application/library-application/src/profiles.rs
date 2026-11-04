//! `profile list` and `profile show`, and the profiles of the project's
//! bundles as a source for resolution.

use std::collections::BTreeMap;

use angzarr_diagnostic::Diagnostic;
use library::findings::Finding;
use library::profile::{self, LoadError, Profile, ProfileSource, ResolvedProfile};
use library::ProfileError;

use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::views::{ProfileList, ProfileRow, ProfileShow};
use crate::Flags;

/// The bundle every project's own profiles are items of.
const PROJECT_BUNDLE: &str = "ctxloom+local:project";

impl Library {
    /// `profile list`.
    pub fn profile_list(&self, flags: &Flags) -> Outcome<ProfileList> {
        self.run(flags, |p| Ok(p.profile_list()))
    }

    /// `profile show <name>`.
    pub fn profile_show(&self, flags: &Flags, name: &str) -> Outcome<ProfileShow> {
        self.run(flags, |p| p.profile_show(name))
    }
}

/// The profiles the project's local bundles ship, by their canonical key.
pub(crate) struct Profiles {
    pub(crate) by_key: BTreeMap<String, (Profile, String)>,
    /// The repository a remote profile was shipped in, by its key: it may
    /// name only that repository's content.
    sources: BTreeMap<String, String>,
}

impl ProfileSource for Profiles {
    fn load(&self, key: &str) -> Result<Profile, LoadError> {
        match self.by_key.get(key) {
            Some((profile, _)) => {
                self.own_repository(key, profile)?;
                Ok(profile.clone())
            }
            None if key.starts_with("ctxloom+local:") => Err(LoadError::LocalNotFound {
                name: key.to_string(),
            }),
            None => Err(LoadError::NotFound {
                name: key.to_string(),
            }),
        }
    }

    fn keys(&self) -> Vec<String> {
        self.by_key.keys().cloned().collect()
    }
}

impl Profiles {
    /// A remote profile that names content outside its own repository is
    /// refused: registering a remote admits that repository's content, never
    /// whatever its profiles name.
    fn own_repository(&self, key: &str, profile: &Profile) -> Result<(), LoadError> {
        let Some(url) = self.sources.get(key) else {
            return Ok(());
        };
        profile
            .check_own_repo(url)
            .map_err(|cause| LoadError::Unreadable {
                name: key.to_string(),
                detail: match cause {
                    ProfileError::CrossRepo { reference, .. } => format!(
                    "a remote profile may refer only to bundles in its own repository: {reference}"
                ),
                    other => other.code().to_string(),
                },
            })
    }
}

/// The name a user types to reach a profile: the bare name for the project
/// bundle's, else its full key.
fn ask_name(key: &str) -> String {
    match key.split_once("#profiles/") {
        Some((bundle, name)) if bundle == PROJECT_BUNDLE => name.to_string(),
        _ => key.to_string(),
    }
}

/// The bare profile name of a key.
fn display_name(key: &str) -> String {
    key.split_once("#profiles/")
        .map_or_else(|| key.to_string(), |(_, name)| name.to_string())
}

impl Project {
    pub(crate) fn profiles(&self) -> Profiles {
        let mut by_key = BTreeMap::new();
        let mut sources = BTreeMap::new();
        for entry in self.catalog.entries() {
            let dir = entry
                .manifest_path
                .strip_suffix("/bundle.yaml")
                .unwrap_or(&entry.manifest_path);
            let origin = self.catalog.origin(&entry.name);
            let bundle_key = origin.map_or_else(
                || format!("ctxloom+local:{}", entry.name),
                |origin| origin.reference.clone(),
            );
            for (name, profile) in &entry.bundle.profiles {
                let key = format!("{bundle_key}#profiles/{name}");
                let mut profile = profile.clone();
                profile.name = key.clone();
                if let (Some(origin), Some(remotes)) = (origin, &self.remotes) {
                    // A remote profile's short references are its own
                    // repository's, at the commit it was read at.
                    let within = |r: &String| remotes.within(r, &origin.url, &origin.sha);
                    profile.bundles = profile.bundles.iter().map(within).collect();
                    profile.parents = profile.parents.iter().map(within).collect();
                    profile.commands = profile.commands.iter().map(within).collect();
                    profile.skills = profile.skills.iter().map(within).collect();
                    profile.bundle_items = profile.bundle_items.iter().map(within).collect();
                    for fragment in &mut profile.fragments {
                        fragment.name = within(&fragment.name);
                    }
                    sources.insert(key.clone(), origin.url.clone());
                }
                by_key.insert(key, (profile, format!("{dir}/profiles/{name}.yaml")));
            }
        }
        Profiles { by_key, sources }
    }

    /// The canonical keys of the profiles the default agent composes.
    fn default_keys(&self, profiles: &Profiles) -> Vec<String> {
        self.config
            .default_agent_profiles()
            .iter()
            .filter_map(|name| profile::canonical_key(name).ok())
            .filter(|key| profiles.by_key.contains_key(key))
            .collect()
    }

    pub(crate) fn profile_list(&self) -> ProfileList {
        let profiles = self.profiles();
        let defaults = self.default_keys(&profiles);
        let mut rows: Vec<ProfileRow> = profiles
            .by_key
            .iter()
            .map(|(key, (p, path))| ProfileRow {
                name: ask_name(key),
                display_name: display_name(key),
                description: p.description.clone(),
                parents: p.parents.clone(),
                tags: p.tags.clone(),
                bundles: p.bundles.clone(),
                default: defaults.contains(key),
                path: path.clone(),
                is_remote: profiles.sources.contains_key(key),
                bundle: key
                    .split_once("#profiles/")
                    .map(|(b, _)| b.to_string())
                    .unwrap_or_default(),
            })
            .collect();
        rows.sort_by_key(|r| r.name.to_lowercase());
        ProfileList {
            rows,
            hint: if self.no_project() {
                "No project here: run 'ctxloom init' to set one up with its default content."
                    .to_string()
            } else {
                "Use 'ctxloom profile create <name> --include <bundle>...' to create one."
                    .to_string()
            },
        }
    }

    fn profile_show(&self, ask: &str) -> Result<ProfileShow, AppError> {
        let profiles = self.profiles();
        let key = profile::canonical_key(&self.spelled_out(ask)).map_err(AppError::cause)?;
        let Some((found, path)) = profiles.by_key.get(&key) else {
            return Err(AppError::cause(ProfileError::NotFound {
                name: ask.to_string(),
            }));
        };
        if let Some(url) = profiles.sources.get(&key) {
            found.check_own_repo(url).map_err(AppError::cause)?;
        }
        let defaults = self.default_keys(&profiles);
        Ok(ProfileShow {
            name: ask_name(&key),
            description: found.description.clone(),
            llm: found.llm.clone(),
            parents: found.parents.clone(),
            tags: found.tags.clone(),
            bundles: found.bundles.clone(),
            variables: found.variables.clone(),
            exclude_fragments: found.exclude_fragments.clone(),
            exclude_mcp: found.exclude_mcp.clone(),
            path: path.clone(),
            bundle: key
                .split_once("#profiles/")
                .map(|(b, _)| b.to_string())
                .unwrap_or_default(),
            default: defaults.contains(&key),
        })
    }

    /// The named profiles, each resolved with its parents, in order. A name
    /// that does not resolve is an error naming it.
    pub(crate) fn resolve_profiles(
        &self,
        names: &[String],
        findings: &mut Vec<Finding>,
    ) -> Result<Vec<ResolvedProfile>, AppError> {
        let profiles = self.profiles();
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let mut resolved = profile::resolve(&profiles, &self.spelled_out(name), findings)
                .map_err(|cause| AppError::Compose {
                    profile: name.clone(),
                    cause: Box::new(cause),
                })?;
            resolved.name.clone_from(name);
            out.push(resolved);
        }
        Ok(out)
    }
}
