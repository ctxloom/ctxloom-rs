//! The write verbs: arguments in, the use case's answer out as json or as the
//! text Go printed. A verb Go answers in text only (its format debt) refuses
//! an explicit `--format json` before it asks the use case anything.

use std::collections::BTreeMap;
use std::io::{self, Write};

use angzarr_presentation::Format;
use clap::ArgMatches;
use library_application::{
    AgentRequest, AppError, BundleEditRequest, DefaultAgent, ItemKind, LlmDefault, LlmRequest,
    McpEdited, McpPatch, Outcome, Preview, ProfileCreateRequest, ProfileEdit, ProfileEdited,
    Removal, WriteError, LOADOUT,
};
use serde::Serialize;

use crate::{emit, failed, flag, many, text_arg, Context, Failure, IntoFailure};

/// A flag's value, `None` when the flag was not given.
fn given(sub: &ArgMatches, name: &str) -> Option<String> {
    sub.try_get_one::<String>(name).ok().flatten().cloned()
}

/// A repeatable flag's values, `None` when the flag was not given; an empty
/// value clears (it contributes no entry).
fn given_list(sub: &ArgMatches, name: &str) -> Option<Vec<String>> {
    sub.try_get_many::<String>(name)
        .ok()
        .flatten()
        .map(|values| values.filter(|v| !v.is_empty()).cloned().collect())
}

fn pair(flag_name: &str, value: &str) -> Result<(String, String), Failure> {
    value
        .split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| {
            failed(AppError::from(WriteError::FlagPair {
                flag: format!("--{flag_name}"),
                got: value.to_string(),
            }))
        })
}

fn given_pairs(sub: &ArgMatches, name: &str) -> Result<Option<Vec<(String, String)>>, Failure> {
    given_list(sub, name)
        .map(|values| values.iter().map(|v| pair(name, v)).collect())
        .transpose()
}

fn given_map(sub: &ArgMatches, name: &str) -> Result<Option<BTreeMap<String, String>>, Failure> {
    Ok(given_pairs(sub, name)?.map(|pairs| pairs.into_iter().collect()))
}

/// A verb that answers in text only: json asked for by name is refused.
fn text_only(cx: &Context<'_>, command: &str) -> Result<(), Failure> {
    if cx.explicit && cx.format == Format::Json {
        return Err(AppError::UnsupportedFormat {
            command: command.to_string(),
            format: "json".to_string(),
        }
        .into_failure());
    }
    Ok(())
}

/// Said and written: json, or `text`.
fn finish<T: Serialize>(
    cx: &Context<'_>,
    outcome: Outcome<T>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    text: impl FnOnce(&T, &mut dyn Write, &mut Vec<String>) -> io::Result<()>,
) -> Result<u8, Failure> {
    crate::answer(cx, outcome, out, err, |value, out, notes| {
        emit(cx.format, out, &value, |out| text(&value, out, notes))
    })
}

/// Said and written as text, whatever the format.
fn finish_text<T>(
    cx: &Context<'_>,
    outcome: Outcome<T>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    text: impl FnOnce(&T, &mut dyn Write, &mut Vec<String>) -> io::Result<()>,
) -> Result<u8, Failure> {
    crate::answer(cx, outcome, out, err, |value, out, notes| {
        text(&value, out, notes).map(|()| 0)
    })
}

/// A removal: the preview or the removed.
fn removal<T: Serialize>(
    cx: &Context<'_>,
    outcome: Outcome<Removal<T>>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    done: impl FnOnce(&T, &mut dyn Write) -> io::Result<()>,
) -> Result<u8, Failure> {
    finish(cx, outcome, out, err, |value, out, _| match value {
        Removal::Preview(preview) => preview_text(out, preview),
        Removal::Done(removed) => done(removed, out),
    })
}

fn preview_text(out: &mut dyn Write, preview: &Preview) -> io::Result<()> {
    if preview.detail.is_empty() {
        writeln!(out, "Would remove {}.", preview.target)?;
    } else {
        writeln!(out, "Would remove {}:", preview.target)?;
        for line in &preview.detail {
            writeln!(out, "  {line}")?;
        }
    }
    writeln!(out)?;
    writeln!(out, "Nothing was removed. Re-run with --yes to apply:")?;
    writeln!(out, "  {}", preview.apply)
}

fn nothing(out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "No changes made.")
}

pub(crate) fn config(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    match verb {
        "create" => {
            text_only(cx, "config create")?;
            let outcome = cx.library.config_create(&text_arg(sub, "engine"));
            finish_text(cx, outcome, out, err, |made, out, _| {
                writeln!(out, "Wrote {}", made.path)
            })
            .map(Some)
        }
        "edit" => {
            text_only(cx, "config edit")?;
            let outcome = cx.library.config_edit();
            finish_text(cx, outcome, out, err, |(), _, _| Ok(())).map(Some)
        }
        _ => Ok(None),
    }
}

pub(crate) fn bundle(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    let name = text_arg(sub, "name");
    let code = match verb {
        "create" => {
            let outcome = cx
                .library
                .bundle_create(&cx.flags, &name, &text_arg(sub, "description"));
            finish(cx, outcome, out, err, |made, out, _| {
                writeln!(out, "Created bundle: {}", made.path)?;
                writeln!(
                    out,
                    "Edit its item files to add your fragments and prompts."
                )
            })?
        }
        "edit" => {
            let request = BundleEditRequest {
                name,
                description: given(sub, "description"),
                version: given(sub, "version"),
                add_tags: many(sub, "add-tag"),
                remove_tags: many(sub, "remove-tag"),
                add_fragments: many(sub, "add-fragment"),
                remove_fragments: many(sub, "remove-fragment"),
                add_prompts: many(sub, "add-prompt"),
                remove_prompts: many(sub, "remove-prompt"),
                add_mcp: many(sub, "add-mcp"),
                remove_mcp: many(sub, "remove-mcp"),
            };
            let outcome = cx.library.bundle_edit(&cx.flags, &request);
            finish(cx, outcome, out, err, |edited, out, _| {
                if edited.changes.is_empty() {
                    return nothing(out);
                }
                for change in &edited.changes {
                    writeln!(out, "{change}")?;
                }
                writeln!(out, "Updated bundle: {}", edited.path)
            })?
        }
        "remove" => {
            let outcome = cx.library.bundle_remove(&cx.flags, &name, flag(sub, "yes"));
            removal(cx, outcome, out, err, |gone, out| {
                writeln!(out, "Removed bundle: {}", gone.path)
            })?
        }
        "export" => {
            let outcome = cx.library.bundle_export(
                &cx.flags,
                &name,
                &text_arg(sub, "dest"),
                &text_arg(sub, "output"),
            );
            finish(cx, outcome, out, err, |done, out, _| {
                writeln!(out, "Exported: {} -> {}", done.source, done.dest)
            })?
        }
        "import" => {
            let outcome =
                cx.library
                    .bundle_import(&cx.flags, &text_arg(sub, "path"), flag(sub, "force"));
            finish(cx, outcome, out, err, |done, out, _| {
                writeln!(out, "Imported: {} -> {}", done.source, done.dest)?;
                writeln!(out, "  Version: {}", done.version)?;
                writeln!(
                    out,
                    "  Fragments: {}, Commands: {}, MCP: {}",
                    done.fragments, done.commands, done.mcp
                )
            })?
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

pub(crate) fn item(
    cx: &Context<'_>,
    kind: ItemKind,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    let reference = text_arg(sub, "ref");
    let code = match verb {
        "create" => {
            let outcome = cx.library.item_create(
                &cx.flags,
                kind,
                &text_arg(sub, "bundle"),
                &text_arg(sub, "name"),
            );
            finish(cx, outcome, out, err, |made, out, _| {
                writeln!(
                    out,
                    "Created {} \"{}\" in bundle \"{}\"",
                    made.kind, made.name, made.bundle
                )?;
                writeln!(
                    out,
                    "Edit with: ctxloom {} edit {}#{}/{}",
                    made.kind,
                    made.bundle,
                    dir_of(&made.kind),
                    made.name
                )
            })?
        }
        "remove" => {
            let outcome = cx
                .library
                .item_remove(&cx.flags, kind, &reference, flag(sub, "yes"));
            removal(cx, outcome, out, err, |gone, out| {
                writeln!(
                    out,
                    "Removed {} \"{}\" from bundle \"{}\"",
                    gone.kind, gone.name, gone.bundle
                )
            })?
        }
        "edit" => {
            let no_distill = flag(sub, "no-distill");
            let outcome = cx
                .library
                .item_edit(&cx.flags, kind, &reference, no_distill);
            finish(cx, outcome, out, err, |edited, out, notes| {
                if edited.status == "unchanged" {
                    return nothing(out);
                }
                if edited.distill_skipped {
                    notes.push("distilled form not refreshed (--no-distill)".to_string());
                }
                writeln!(
                    out,
                    "Updated {} \"{}\" in bundle \"{}\"",
                    edited.kind, edited.name, edited.bundle
                )?;
                writeln!(out)?;
                writeln!(out, "Bundle modified. To publish changes:")?;
                writeln!(out, "  ctxloom bundle push {} [remote]", edited.bundle)
            })?
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

/// The directory a kind's items live under in a reference.
fn dir_of(kind: &str) -> &'static str {
    if kind == "fragment" {
        "fragments"
    } else {
        "commands"
    }
}

pub(crate) fn skill(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    let reference = text_arg(sub, "ref");
    let code = match verb {
        "create" => {
            let outcome = cx.library.skill_create(
                &cx.flags,
                &text_arg(sub, "bundle"),
                &text_arg(sub, "name"),
                &text_arg(sub, "description"),
            );
            finish(cx, outcome, out, err, |made, out, _| {
                writeln!(
                    out,
                    "Created skill \"{}\" in bundle \"{}\"",
                    made.name, made.bundle
                )?;
                writeln!(out, "  {}", made.dir)?;
                writeln!(out, "Edit {}/SKILL.md to describe the skill", made.dir)
            })?
        }
        "remove" => {
            let outcome = cx
                .library
                .skill_remove(&cx.flags, &reference, flag(sub, "yes"));
            removal(cx, outcome, out, err, |gone, out| {
                writeln!(
                    out,
                    "Removed skill \"{}\" from bundle \"{}\"",
                    gone.name, gone.bundle
                )?;
                writeln!(out, "  {}", gone.dir)
            })?
        }
        "export" => {
            let outcome = cx
                .library
                .skill_export(&cx.flags, &reference, &text_arg(sub, "output"));
            finish(cx, outcome, out, err, |done, out, _| {
                writeln!(
                    out,
                    "Exported skill \"{}\" to {} ({} bytes)",
                    done.name, done.zip_path, done.bytes
                )
            })?
        }
        "import" => {
            let outcome = cx.library.skill_import(
                &cx.flags,
                &text_arg(sub, "archive"),
                &text_arg(sub, "bundle"),
                flag(sub, "force"),
            );
            finish(cx, outcome, out, err, |done, out, _| {
                writeln!(
                    out,
                    "Imported skill \"{}\" into bundle \"{}\" ({} file(s))",
                    done.name, done.bundle, done.file_count
                )?;
                writeln!(out, "  {}", done.dir)
            })?
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

pub(crate) fn mcp(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    let reference = text_arg(sub, "ref");
    match verb {
        "edit" => {
            text_only(cx, "mcp server edit")?;
            let outcome = cx.library.mcp_edit(&cx.flags, &reference);
            finish_text(cx, outcome, out, err, |edited, out, _| match edited {
                McpEdited::Unchanged => nothing(out),
                McpEdited::Updated { name, bundle } => {
                    writeln!(out, "Updated MCP server \"{name}\" in bundle \"{bundle}\"")
                }
            })
            .map(Some)
        }
        "set" => {
            let patch = McpPatch {
                command: given(sub, "command"),
                args: given_list(sub, "arg"),
                env: given_map(sub, "env")?,
                url: given(sub, "url"),
                headers: given_map(sub, "header")?,
                tags: given_list(sub, "tag"),
                served_by: given(sub, "served-by"),
                notes: given(sub, "notes"),
                installation: given(sub, "installation"),
            };
            let outcome = cx.library.mcp_set(&cx.flags, &reference, &patch);
            finish(cx, outcome, out, err, |set, out, _| {
                writeln!(
                    out,
                    "Set MCP server \"{}\" in bundle \"{}\"",
                    set.name, set.bundle
                )
            })
            .map(Some)
        }
        _ => Ok(None),
    }
}

/// A bare `mcp`: no stdio server to point a client at.
pub(crate) fn mcp_bare(cx: &Context<'_>) -> Failure {
    if cx.explicit && cx.format == Format::Json {
        return AppError::from(WriteError::McpBareJson {}).into_failure();
    }
    AppError::from(WriteError::McpBare {}).into_failure()
}

pub(crate) fn profile(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    let name = text_arg(sub, "name");
    let code = match verb {
        "create" => {
            text_only(cx, "profile create")?;
            let request = ProfileCreateRequest {
                name,
                description: text_arg(sub, "description"),
                parents: many(sub, "parent"),
                bundles: many(sub, "include"),
                llm: text_arg(sub, "llm"),
                target: text_arg(sub, "bundle"),
            };
            let outcome = cx.library.profile_create(&cx.flags, &request);
            finish_text(cx, outcome, out, err, |made, out, _| {
                let mut parts = Vec::new();
                if !made.parents.is_empty() {
                    parts.push(format!("parents: {}", made.parents.join(", ")));
                }
                if !made.bundles.is_empty() {
                    parts.push(format!("bundles: {}", made.bundles.join(", ")));
                }
                let with = if parts.is_empty() {
                    String::new()
                } else {
                    format!(" with {}", parts.join("; "))
                };
                writeln!(out, "Created profile \"{}\"{with}", made.name)?;
                writeln!(out, "Saved to: {}", made.path)
            })?
        }
        "modify" => {
            text_only(cx, "profile modify")?;
            let edit = ProfileEdit {
                description: given(sub, "description"),
                llm: given(sub, "llm"),
                add_parents: many(sub, "add-parent"),
                remove_parents: many(sub, "remove-parent"),
                add_bundles: many(sub, "add-bundle"),
                remove_bundles: many(sub, "remove-bundle"),
                exclude_fragments: many(sub, "exclude-fragment"),
                include_fragments: many(sub, "include-fragment"),
                exclude_mcp: many(sub, "exclude-mcp"),
                include_mcp: many(sub, "include-mcp"),
            };
            let outcome = cx.library.profile_modify(&cx.flags, &name, &edit);
            finish_text(cx, outcome, out, err, |done, out, _| {
                if done.changes.is_empty() {
                    return nothing(out);
                }
                for change in &done.changes {
                    writeln!(out, "{change}")?;
                }
                writeln!(out, "Modified profile \"{}\"", done.name)
            })?
        }
        "edit" => {
            text_only(cx, "profile edit")?;
            let outcome = cx.library.profile_edit(&cx.flags, &name);
            finish_text(cx, outcome, out, err, |done, out, _| match done {
                ProfileEdited::Unchanged => nothing(out),
                ProfileEdited::Updated { path } => writeln!(out, "Updated profile: {path}"),
            })?
        }
        "export" => {
            text_only(cx, "profile export")?;
            let outcome = cx
                .library
                .profile_export(&cx.flags, &name, &text_arg(sub, "dest"));
            finish_text(cx, outcome, out, err, |done, out, _| {
                writeln!(out, "Exported: {} -> {}", done.source, done.dest)
            })?
        }
        "import" => {
            text_only(cx, "profile import")?;
            let outcome = cx.library.profile_import(
                &cx.flags,
                &text_arg(sub, "path"),
                flag(sub, "force"),
                &text_arg(sub, "bundle"),
            );
            finish_text(cx, outcome, out, err, |done, out, _| {
                writeln!(out, "Imported: {} -> {}", done.source, done.dest)
            })?
        }
        "remove" => {
            let outcome = cx
                .library
                .profile_remove(&cx.flags, &name, flag(sub, "yes"));
            removal(cx, outcome, out, err, |gone, out| {
                writeln!(out, "Removed profile \"{}\"", gone.profile)
            })?
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

fn agent_request(sub: &ArgMatches) -> Result<AgentRequest, Failure> {
    Ok(AgentRequest {
        llm: given(sub, "llm"),
        profiles: given_list(sub, "profiles"),
        runtime: given(sub, "runtime"),
        surfaces: given_pairs(sub, "surface")?,
        roots: given_pairs(sub, "root")?,
        permissions: given(sub, "permissions"),
        engine_home: given(sub, "engine-home"),
        env_host: sub.try_get_one::<bool>("env-host").ok().flatten().copied(),
        env: given_list(sub, "env"),
    })
}

pub(crate) fn agent(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    let name = text_arg(sub, "name");
    let code = match verb {
        "create" | "edit" => {
            let request = agent_request(sub)?;
            let created = verb == "create";
            let outcome = cx.library.agent_write(&cx.flags, &name, &request, !created);
            finish(cx, outcome, out, err, |row, out, _| {
                let mut parts = Vec::new();
                if !row.llm.is_empty() {
                    parts.push(format!("llm: {}", row.llm));
                }
                if !row.profiles.is_empty() {
                    parts.push(format!("profiles: {}", row.profiles.join(", ")));
                }
                if !row.runtime.is_empty() {
                    parts.push(format!("runtime: {}", row.runtime));
                }
                if !row.permissions_line.is_empty() {
                    parts.push(format!("permissions: {}", row.permissions_line));
                }
                if !row.home_mode.is_empty() {
                    parts.push(format!("engine_home: {}", row.home_mode));
                }
                if let Some(host) = row.env_host {
                    parts.push(format!("env_host: {host}"));
                }
                if !row.env.is_empty() {
                    parts.push(format!("env: {}", row.env.join(", ")));
                }
                let verb = if created { "Created" } else { "Updated" };
                writeln!(out, "{verb} agent \"{}\" ({})", row.name, parts.join(", "))
            })?
        }
        "default" => {
            text_only(cx, "agent default")?;
            let asked = given(sub, "name");
            let outcome = cx.library.agent_default(&cx.flags, asked.as_deref());
            finish_text(cx, outcome, out, err, |done, out, notes| match done {
                DefaultAgent::Current(current) if current.is_empty() => {
                    writeln!(out, "No default agent set.")
                }
                DefaultAgent::Current(current) => writeln!(out, "Default agent: {current}"),
                DefaultAgent::Set { name, defined } => {
                    if !defined {
                        notes.push(format!(
                            "agent \"{name}\" is not defined yet; define it with `ctxloom agent create {name}`"
                        ));
                    }
                    writeln!(out, "Set default agent to \"{name}\".")
                }
            })?
        }
        "remove" => {
            let outcome = cx.library.agent_remove(&cx.flags, &name, flag(sub, "yes"));
            removal(cx, outcome, out, err, |gone, out| {
                writeln!(out, "Removed agent \"{}\"", gone.name)
            })?
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

pub(crate) fn llm(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    let label = text_arg(sub, "label");
    let home = flag(sub, "home");
    let code = match verb {
        "create" | "edit" => {
            let request = LlmRequest {
                kind: given(sub, "type"),
                model: given(sub, "model"),
                permissions: given(sub, "permissions"),
                set: many(sub, "set"),
            };
            let outcome = cx
                .library
                .llm_write(&cx.flags, &label, &request, home, verb == "edit");
            finish(cx, outcome, out, err, |done, out, _| {
                let verb = if done.edited { "Updated" } else { "Created" };
                let mut parts = Vec::new();
                if !done.kind.is_empty() {
                    parts.push(format!("type: {}", done.kind));
                }
                if !done.model.is_empty() {
                    parts.push(format!("model: {}", done.model));
                }
                if !done.permissions_line.is_empty() {
                    parts.push(format!("permissions: {}", done.permissions_line));
                }
                writeln!(out, "{verb} llm \"{}\" ({})", done.label, parts.join(", "))
            })?
        }
        "default" => {
            let asked = given(sub, "name");
            let outcome = cx.library.llm_default(&cx.flags, asked.as_deref());
            finish(cx, outcome, out, err, |done, out, _| match done {
                LlmDefault::Shown(shown) => writeln!(out, "{}", shown.default),
                LlmDefault::Set(set) if set.status == "unchanged" => {
                    writeln!(out, "Default LLM is already {}", set.name)
                }
                LlmDefault::Set(set) => writeln!(out, "Default LLM set to: {}", set.name),
            })?
        }
        "remove" => {
            let outcome = cx
                .library
                .llm_remove(&cx.flags, &label, home, flag(sub, "yes"));
            removal(cx, outcome, out, err, |gone, out| {
                writeln!(out, "Removed llm \"{}\"", gone.label)
            })?
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

pub(crate) fn companion(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<u8>, Failure> {
    let name = text_arg(sub, "name");
    let code = match verb {
        "add" => {
            let command: Vec<String> = sub
                .try_get_many::<String>("command")
                .ok()
                .flatten()
                .into_iter()
                .chain(sub.try_get_many::<String>("args").ok().flatten())
                .flatten()
                .cloned()
                .collect();
            let outcome = cx.library.companion_add(&cx.flags, &name, &command);
            finish(cx, outcome, out, err, |done, out, _| {
                if done.added {
                    writeln!(
                        out,
                        "Registered companion '{}' (answers: {})",
                        done.name,
                        done.command.join(" ")
                    )
                } else {
                    writeln!(
                        out,
                        "Companion '{}' is already registered ({})",
                        done.name,
                        done.command.join(" ")
                    )
                }
            })?
        }
        "remove" => {
            let outcome = cx
                .library
                .companion_remove(&cx.flags, &name, flag(sub, "yes"));
            removal(cx, outcome, out, err, |gone, out| {
                writeln!(out, "Removed companion '{}'", gone.name)
            })?
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

/// `loadout`: the one fixed YAML document. The value of `--format` is a
/// document format here, and yaml is the only one there is.
pub(crate) fn loadout(sub: &ArgMatches, out: &mut dyn Write) -> Result<u8, Failure> {
    let asked = given(sub, "format");
    if let Some(format) = asked.filter(|f| f != "yaml") {
        return Err(AppError::from(WriteError::LoadoutFormat { format }).into_failure());
    }
    out.write_all(LOADOUT.as_bytes())
        .map_err(crate::output_failure)?;
    Ok(0)
}
