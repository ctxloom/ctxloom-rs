//! The text forms: what each command prints for a person (Go's renderers,
//! line for line). Every value a bundle's author wrote goes through
//! [`inert`](crate::inert) first.

use std::io::{self, Write};

use library_application::{
    AgentRow, AgentShow, BundleList, BundleRow, BundleShow, BundleView, CompanionRow, DryRun,
    ItemList, ItemShow, LlmRow, McpList, McpShow, Premises, ProfileList, ProfileShow, RemoteFound,
    SearchResults, SkillList, SkillShow,
};

use crate::inert::{body, field};

/// Text that stands for a body, and the notices that went with it.
pub struct Notes(pub Vec<String>);

fn f(s: &str) -> String {
    field(s, 256)
}

/// `bundle list`.
pub fn bundle_list(out: &mut dyn Write, list: &BundleList) -> io::Result<()> {
    if list.no_root {
        if list.hint.starts_with("No project here") {
            return writeln!(out, "{}", list.hint);
        }
        return writeln!(
            out,
            "No bundles directory found. Create one with: mkdir -p .ctxloom/content/bundles"
        );
    }
    if list.rows.is_empty() {
        writeln!(out, "No bundles installed.")?;
        return writeln!(out, "{}", list.hint);
    }
    writeln!(out, "Installed bundles ({}):\n", list.rows.len())?;
    for row in &list.rows {
        bundle_entry(out, row)?;
    }
    Ok(())
}

fn contents(row: &BundleRow) -> Vec<String> {
    let mut parts = Vec::new();
    for (n, singular, plural) in [
        (row.fragment_count, "fragment", "fragments"),
        (row.command_count, "command", "commands"),
        (row.mcp_count, "MCP server", "MCP servers"),
    ] {
        match n {
            0 => {}
            1 => parts.push(format!("1 {singular}")),
            n => parts.push(format!("{n} {plural}")),
        }
    }
    parts
}

fn bundle_entry(out: &mut dyn Write, row: &BundleRow) -> io::Result<()> {
    write!(out, "  {}", f(&row.name))?;
    if !row.version.is_empty() {
        write!(out, " (v{})", f(&row.version))?;
    }
    if row.held {
        write!(out, " [held]")?;
    }
    writeln!(out)?;
    if !row.description.is_empty() {
        writeln!(out, "    {}", f(&row.description))?;
    }
    let parts = contents(row);
    if !parts.is_empty() {
        writeln!(out, "    Contains: {}", parts.join(", "))?;
    }
    if !row.tags.is_empty() {
        writeln!(out, "    Tags: {}", f(&row.tags.join(", ")))?;
    }
    writeln!(out)
}

/// A body behind `indent`, its alteration noted in `notes`.
fn publisher_body(
    out: &mut dyn Write,
    notes: &mut Notes,
    indent: &str,
    collapse: bool,
    reference: &str,
    content: &str,
) -> io::Result<()> {
    let rendered = body(indent, "", collapse, reference, content);
    if !rendered.notice.is_empty() {
        notes.0.push(rendered.notice);
    }
    write!(out, "{}", rendered.text)
}

/// `bundle show`.
pub fn bundle_show(out: &mut dyn Write, notes: &mut Notes, show: &BundleShow) -> io::Result<()> {
    writeln!(out, "Bundle: {}", f(&show.name))?;
    if !show.version.is_empty() {
        writeln!(out, "Version: {}", f(&show.version))?;
    }
    if !show.author.is_empty() {
        writeln!(out, "Author: {}", f(&show.author))?;
    }
    if !show.description.is_empty() {
        writeln!(out, "Description: {}", f(&show.description))?;
    }
    if !show.tags.is_empty() {
        writeln!(out, "Tags: {}", f(&show.tags.join(", ")))?;
    }
    writeln!(out, "Path: {}\n", show.path)?;
    if !show.mcp.is_empty() {
        writeln!(out, "MCP Servers ({}):", show.mcp.len())?;
        for (name, mcp) in &show.mcp {
            writeln!(out, "  - {}", f(name))?;
            writeln!(out, "      Command: {}", f(&mcp.command))?;
            if !mcp.args.is_empty() {
                writeln!(out, "      Args: {}", f(&mcp.args.join(" ")))?;
            }
            if !mcp.env.is_empty() {
                writeln!(out, "      Env:")?;
                for (key, value) in &mcp.env {
                    writeln!(out, "        {}={}", f(key), f(value))?;
                }
            }
            if !mcp.notes.is_empty() {
                writeln!(out, "      Notes:")?;
                publisher_body(
                    out,
                    notes,
                    "        ",
                    true,
                    &format!("{name} notes"),
                    &mcp.notes,
                )?;
            }
            if !mcp.installation.is_empty() {
                writeln!(out, "      Installation:")?;
                publisher_body(
                    out,
                    notes,
                    "        ",
                    true,
                    &format!("{name} installation"),
                    &mcp.installation,
                )?;
            }
        }
        writeln!(out)?;
    }
    if !show.fragments.is_empty() {
        writeln!(out, "Fragments ({}):", show.fragments.len())?;
        for (name, fragment) in &show.fragments {
            item_line(
                out,
                name,
                &fragment.tags,
                fragment.distilled,
                fragment.no_distill,
            )?;
            writeln!(out, "      {}", f(&fragment.preview))?;
        }
        writeln!(out)?;
    }
    if !show.commands.is_empty() {
        writeln!(out, "Commands ({}):", show.commands.len())?;
        for (name, command) in &show.commands {
            item_line(
                out,
                name,
                &command.tags,
                command.distilled,
                command.no_distill,
            )?;
            if !command.description.is_empty() {
                writeln!(out, "      {}", f(&command.description))?;
            }
        }
        writeln!(out)?;
    }
    if !show.notes.is_empty() {
        writeln!(out, "Notes:")?;
        publisher_body(
            out,
            notes,
            "  ",
            true,
            &format!("{} notes", show.name),
            &show.notes,
        )?;
    }
    Ok(())
}

fn item_line(
    out: &mut dyn Write,
    name: &str,
    tags: &[String],
    distilled: bool,
    no_distill: bool,
) -> io::Result<()> {
    write!(out, "  - {}", f(name))?;
    if !tags.is_empty() {
        write!(out, " [{}]", f(&tags.join(", ")))?;
    }
    if distilled {
        write!(out, " (distilled)")?;
    } else if no_distill {
        write!(out, " (no_distill)")?;
    }
    writeln!(out)
}

/// `bundle view`.
pub fn bundle_view(
    out: &mut dyn Write,
    notes: &mut Notes,
    view: &BundleView,
    ask: &str,
) -> io::Result<()> {
    publisher_body(out, notes, "", view.item, ask, &view.content)
}

/// `fragment list` and `command list`.
pub fn item_list(out: &mut dyn Write, list: &ItemList) -> io::Result<()> {
    let kind = &list.kind;
    if list.rows.is_empty() {
        if !list.bundle_filter.is_empty() {
            return writeln!(
                out,
                "No {kind}s in bundle {:?} ({} {kind}s exist in other bundles).",
                list.bundle_filter, list.total
            );
        }
        writeln!(out, "No {kind}s found.")?;
        return writeln!(out, "{}", list.hint);
    }
    let title = format!("{}{}s", kind[..1].to_uppercase(), &kind[1..]);
    writeln!(out, "{title} ({}):\n", list.rows.len())?;
    let mut current = "";
    for row in &list.rows {
        if row.bundle != current {
            if !current.is_empty() {
                writeln!(out)?;
            }
            writeln!(out, "  {}:", f(&row.bundle_label))?;
            current = &row.bundle;
        }
        write!(out, "    - {}", f(&row.name))?;
        if !row.tags.is_empty() {
            write!(out, " [{}]", f(&row.tags.join(", ")))?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// `fragment show` and `command show`.
pub fn item_show(out: &mut dyn Write, notes: &mut Notes, show: &ItemShow) -> io::Result<()> {
    if show.distilled {
        writeln!(out, "# (distilled version)")?;
    }
    writeln!(out, "{}\n", f(&show.name))?;
    publisher_body(out, notes, "", true, &show.reference, &show.content)
}

/// `fragment premises`.
pub fn premises(out: &mut dyn Write, premises: &Premises) -> io::Result<()> {
    if premises.fragments.is_empty() {
        return writeln!(out, "{}", premises.instruction);
    }
    write!(out, "{}", premises.instruction)?;
    for row in &premises.fragments {
        write!(out, "\n- {}: {}", row.name, row.premise)?;
    }
    writeln!(out)
}

/// `skill list`.
pub fn skill_list(out: &mut dyn Write, list: &SkillList) -> io::Result<()> {
    if list.rows.is_empty() {
        if !list.bundle_filter.is_empty() && list.total > 0 {
            writeln!(out, "No skills found in bundle {:?}.", list.bundle_filter)?;
            return writeln!(
                out,
                "Skills exist in other bundles \u{2014} run `ctxloom skill list` without --bundle to see them all."
            );
        }
        writeln!(out, "No skills found.")?;
        return writeln!(out, "Create one with: ctxloom skill create <bundle> <name>");
    }
    writeln!(out, "Skills ({}):\n", list.rows.len())?;
    let mut current = "";
    for row in &list.rows {
        if row.source != current {
            if !current.is_empty() {
                writeln!(out)?;
            }
            writeln!(out, "  {}:", f(&row.source))?;
            current = &row.source;
        }
        write!(out, "    - {}", f(&row.name))?;
        if !row.description.is_empty() {
            write!(out, ": {}", f(&row.description))?;
        }
        write!(out, " [{} file(s)]", row.file_count)?;
        if !row.tags.is_empty() {
            write!(out, " ({})", f(&row.tags.join(", ")))?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// `skill show`.
pub fn skill_show(out: &mut dyn Write, notes: &mut Notes, show: &SkillShow) -> io::Result<()> {
    writeln!(out, "{}#skills/{}\n", f(&show.bundle), f(&show.name))?;
    writeln!(out, "description: {}", f(&show.description))?;
    if !show.license.is_empty() {
        writeln!(out, "license: {}", f(&show.license))?;
    }
    if !show.compatibility.is_empty() {
        writeln!(out, "compatibility: {}", f(&show.compatibility))?;
    }
    if !show.allowed_tools.is_empty() {
        writeln!(out, "allowed-tools: {}", f(&show.allowed_tools.join(", ")))?;
    }
    writeln!(out)?;
    let reference = format!("{}#skills/{}", show.bundle, show.name);
    publisher_body(out, notes, "", true, &reference, &show.body)?;
    writeln!(out, "\nFiles ({}):", show.files.len())?;
    for file in &show.files {
        writeln!(out, "  {}  {}  {}", file.mode, file.sha256, f(&file.path))?;
    }
    Ok(())
}

/// `profile list`.
pub fn profile_list(out: &mut dyn Write, list: &ProfileList) -> io::Result<()> {
    if list.rows.is_empty() {
        writeln!(out, "No profiles defined.")?;
        return writeln!(out, "{}", list.hint);
    }
    writeln!(out, "Profiles ({}):", list.rows.len())?;
    for row in &list.rows {
        write!(out, "  {}", row.name)?;
        if row.default {
            write!(out, " (default)")?;
        }
        writeln!(out)?;
        if !row.description.is_empty() {
            writeln!(out, "    {}", row.description)?;
        }
        let mut parts = Vec::new();
        if !row.bundle.is_empty() {
            parts.push(format!("from bundle: {}", row.bundle));
        }
        if !row.parents.is_empty() {
            parts.push(format!("parents: {}", row.parents.join(", ")));
        }
        if !row.bundles.is_empty() {
            parts.push(format!("{} bundles", row.bundles.len()));
        }
        if !parts.is_empty() {
            writeln!(out, "    {}", parts.join(", "))?;
        }
    }
    Ok(())
}

fn bullets(out: &mut dyn Write, heading: &str, items: &[String]) -> io::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    writeln!(out, "{heading}:")?;
    for item in items {
        writeln!(out, "  - {}", f(item))?;
    }
    Ok(())
}

/// `profile show`.
pub fn profile_show(out: &mut dyn Write, show: &ProfileShow) -> io::Result<()> {
    writeln!(out, "Profile: {}", f(&show.name))?;
    writeln!(out, "Path: {}", show.path)?;
    if !show.bundle.is_empty() {
        writeln!(out, "Bundle: {}", f(&show.bundle))?;
    }
    if show.default {
        writeln!(out, "Default: yes")?;
    }
    if !show.description.is_empty() {
        writeln!(out, "Description: {}", f(&show.description))?;
    }
    if !show.llm.is_empty() {
        writeln!(out, "LLM: {}", f(&show.llm))?;
    }
    bullets(out, "Parents", &show.parents)?;
    bullets(out, "Bundles", &show.bundles)?;
    bullets(out, "Tags", &show.tags)?;
    if !show.variables.is_empty() {
        writeln!(out, "Variables:")?;
        for (key, value) in &show.variables {
            writeln!(out, "  {}: {}", f(key), f(value))?;
        }
    }
    bullets(out, "Excluded fragments", &show.exclude_fragments)?;
    bullets(out, "Excluded MCP servers", &show.exclude_mcp)
}

/// `agent list`.
pub fn agent_list(out: &mut dyn Write, rows: &[AgentRow]) -> io::Result<()> {
    if rows.is_empty() {
        writeln!(out, "No agents defined.")?;
        return writeln!(out, "Define one under 'agents:' in .ctxloom/config.yaml.");
    }
    writeln!(out, "Agents ({}):", rows.len())?;
    for row in rows {
        write!(out, "  {}", row.name)?;
        if row.llm.is_empty() {
            write!(out, " (llm: project default)")?;
        } else {
            write!(out, " (llm: {})", row.llm)?;
        }
        writeln!(out)?;
        if !row.profiles.is_empty() {
            writeln!(out, "    profiles: {}", row.profiles.join(", "))?;
        }
        for (key, value) in [
            ("runtime", &row.runtime),
            ("permissions", &row.permissions_line),
            ("driving", &row.driving),
            ("engine_home", &row.home_mode),
        ] {
            if !value.is_empty() {
                writeln!(out, "    {key}: {value}")?;
            }
        }
    }
    Ok(())
}

fn plain_bullets(out: &mut dyn Write, heading: &str, items: &[String]) -> io::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    writeln!(out, "{heading}:")?;
    for item in items {
        writeln!(out, "  - {item}")?;
    }
    Ok(())
}

/// How a headless run of the agent meets a call its posture leaves open.
fn headless(backend: &str, posture: &str, approver: &str) -> String {
    let lead = if posture.is_empty() {
        format!("approver {approver}")
    } else {
        format!("{posture}, approver {approver}")
    };
    let asks_nobody = backend == "claude-code" && posture == "bypassPermissions";
    if asks_nobody {
        format!("{lead} \u{2014} nothing is asked")
    } else if approver == "none" {
        format!("{lead} \u{2014} asks are denied at once")
    } else if approver == "reviewer" {
        format!("{lead} \u{2014} the engine's reviewer decides asks")
    } else {
        format!(
            "{lead} \u{2014} asks go to the root's approvals modal; denied at once when the root is unattended (one-shot, --plain-terminal, headless)"
        )
    }
}

/// `agent show`.
pub fn agent_show(out: &mut dyn Write, show: &AgentShow) -> io::Result<()> {
    let def = &show.definition;
    writeln!(out, "Agent: {}", def.name)?;
    if def.llm.is_empty() {
        writeln!(out, "Engine (declared): (project default)")?;
    } else {
        writeln!(out, "Engine (declared): {}", def.llm)?;
    }
    if !def.runtime.is_empty() {
        writeln!(out, "Runtime: {}", def.runtime)?;
    }
    if !def.permissions_line.is_empty() {
        writeln!(out, "Permissions: {}", def.permissions_line)?;
    }
    if !def.driving.is_empty() {
        writeln!(out, "Driving: {}", def.driving)?;
    }
    if !def.home_mode.is_empty() {
        writeln!(out, "Config home (declared): {}", def.home_mode)?;
    }
    if let Some(env_host) = def.env_host {
        writeln!(out, "Env host: {env_host}")?;
    }
    plain_bullets(out, "Env", &def.env)?;
    plain_bullets(out, "Profiles", &def.profiles)?;
    match &show.resolved {
        None => writeln!(out, "Resolved llm: unavailable ({})", show.resolution_error)?,
        Some(resolved) => {
            write!(out, "Resolved llm: {}", resolved.label)?;
            if !resolved.backend.is_empty() {
                write!(out, " (backend: {}", resolved.backend)?;
                if !resolved.model.is_empty() {
                    write!(out, ", model: {}", resolved.model)?;
                }
                write!(out, ")")?;
            }
            writeln!(out)?;
            if !resolved.effective_permissions.is_empty() {
                writeln!(
                    out,
                    "Resolved permissions: {}",
                    resolved.effective_permissions
                )?;
            }
            if !resolved.approver.is_empty() {
                writeln!(
                    out,
                    "Headless posture: {}",
                    headless(
                        &resolved.backend,
                        &resolved.effective_permissions,
                        &resolved.approver
                    )
                )?;
            }
            if !resolved.home_mode.is_empty() {
                writeln!(out, "Resolved config home: {}", resolved.home_mode)?;
            }
            writeln!(out, "Composed fragments: {}", resolved.fragments.len())?;
        }
    }
    if !show.requirements_error.is_empty() {
        return writeln!(out, "Requires: unavailable ({})", show.requirements_error);
    }
    if show.requirements.is_empty() {
        return Ok(());
    }
    writeln!(out, "Requires:")?;
    for requirement in &show.requirements {
        let sources: Vec<String> = requirement
            .sources
            .iter()
            .map(|s| format!("{} {}", s.kind, s.reference))
            .collect();
        writeln!(out, "  - {} ({})", requirement.command, sources.join(", "))?;
    }
    Ok(())
}

/// `llm list`.
pub fn llm_list(out: &mut dyn Write, rows: &[LlmRow]) -> io::Result<()> {
    writeln!(out, "Available LLMs:")?;
    for row in rows {
        let mut label = row.label.clone();
        if row.default {
            label.push_str(" (default)");
        }
        let origin = if row.authored {
            "[configured]"
        } else {
            "[built-in]"
        };
        writeln!(out, "  {label} {origin}")?;
        if !row.runtimes.is_empty() {
            writeln!(out, "      runtimes: {}", row.runtimes.join(", "))?;
        }
    }
    writeln!(
        out,
        "\n[configured] declared in your config.yaml   [built-in] supplied by ctxloom"
    )?;
    writeln!(
        out,
        "runtimes: the `agent create --runtime` values that engine can be given. There is"
    )?;
    writeln!(
        out,
        "no default \u{2014} an agent created without one inherits whatever the project says,"
    )?;
    writeln!(out, "which is a default rather than a decision.")
}

/// `companion list`.
pub fn companion_list(out: &mut dyn Write, rows: &[CompanionRow]) -> io::Result<()> {
    if rows.is_empty() {
        return writeln!(
            out,
            "No companions registered.\nUse 'ctxloom companion add <name>' to register one."
        );
    }
    for row in rows {
        let where_ = if row.resolves {
            row.path.clone()
        } else {
            format!(
                "NOT ON PATH \u{2014} install it, or: ctxloom companion remove {} --yes",
                row.name
            )
        };
        writeln!(out, "{:<12} {:<28} {}", row.name, row.bin, where_)?;
    }
    Ok(())
}

fn mcp_target(
    out: &mut dyn Write,
    indent: &str,
    served_by: &str,
    url: &str,
    command: &str,
    args: &[String],
) -> io::Result<()> {
    if served_by == "session-endpoint" {
        return writeln!(out, "{indent}Served by: the running session's endpoint");
    }
    if !url.is_empty() {
        return writeln!(out, "{indent}URL: {}", f(url));
    }
    writeln!(out, "{indent}Command: {}", f(command))?;
    if !args.is_empty() {
        writeln!(out, "{indent}Args: {}", f(&args.join(" ")))?;
    }
    Ok(())
}

/// `mcp server list`.
pub fn mcp_list(out: &mut dyn Write, list: &McpList) -> io::Result<()> {
    if list.servers.is_empty() {
        writeln!(out, "No MCP servers registered.")?;
        return writeln!(
            out,
            "\nMCP servers ship in bundles \u{2014} compose one that declares an `mcp:` server, or check whether a profile's `exclude_mcp` withholds it."
        );
    }
    writeln!(out, "MCP Servers:")?;
    for server in &list.servers {
        writeln!(out, "  {}", f(&server.name))?;
        mcp_target(
            out,
            "    ",
            &server.served_by,
            &server.url,
            &server.command,
            &server.args,
        )?;
        writeln!(out, "    Bundle: {}", f(&server.source))?;
    }
    Ok(())
}

/// `mcp server show`.
pub fn mcp_show(out: &mut dyn Write, show: &McpShow) -> io::Result<()> {
    for entry in &show.entries {
        writeln!(out, "MCP Server: {}", f(&entry.name))?;
        writeln!(out, "Bundle: {}", f(&entry.source))?;
        mcp_target(
            out,
            "",
            &entry.served_by,
            &entry.url,
            &entry.command,
            &entry.args,
        )?;
        if !entry.env.is_empty() {
            writeln!(out, "Environment:")?;
            for (key, value) in &entry.env {
                writeln!(out, "  {}={}", f(key), f(value))?;
            }
        }
    }
    Ok(())
}

/// `search`.
pub fn search(out: &mut dyn Write, results: &SearchResults) -> io::Result<()> {
    if results.count == 0 {
        return writeln!(out, "No results found.");
    }
    writeln!(out, "Results ({}):\n", results.count)?;
    for (kind, title) in [
        ("fragment", "Fragments"),
        ("command", "Commands"),
        ("skill", "Skills"),
        ("profile", "Profiles"),
        ("mcp_server", "MCP Servers"),
    ] {
        let hits: Vec<_> = results.local.iter().filter(|h| h.kind == kind).collect();
        if hits.is_empty() {
            continue;
        }
        writeln!(out, "{title}:")?;
        for hit in hits {
            write!(out, "  - {}", f(&hit.name))?;
            if !hit.tags.is_empty() {
                write!(out, " [{}]", f(&hit.tags.join(", ")))?;
            }
            if !hit.source.is_empty() {
                write!(out, " ({})", f(&hit.source))?;
            }
            writeln!(out)?;
        }
    }
    if !results.remote.is_empty() {
        if !results.local.is_empty() {
            writeln!(out)?;
        }
        remote_table(out, &results.remote)?;
    }
    Ok(())
}

/// What the remotes publish, as a table whose Remote and Name columns are as
/// wide as their widest entry, so a name is never cut.
fn remote_table(out: &mut dyn Write, hits: &[RemoteFound]) -> io::Result<()> {
    let remote_width = hits
        .iter()
        .map(|h| h.remote.chars().count())
        .max()
        .unwrap_or(0)
        .max("Remote".len());
    let name_width = hits
        .iter()
        .map(|h| f(&h.name).chars().count())
        .max()
        .unwrap_or(0)
        .max("Name".len());
    let row = |kind: &str, remote: &str, name: &str, tags: &str| {
        format!(
            "  {kind:<8} \u{2502} {remote:<remote_width$} \u{2502} {name:<name_width$} \u{2502} {tags}"
        )
    };
    let rule = |width: usize| "\u{2500}".repeat(width);
    writeln!(out, "Remote:")?;
    writeln!(out, "{}", row("Type", "Remote", "Name", "Tags"))?;
    writeln!(
        out,
        "  {}\u{253c}{}\u{253c}{}\u{253c}{}",
        rule(9),
        rule(remote_width + 2),
        rule(name_width + 2),
        rule(12)
    )?;
    for hit in hits {
        let tags = f(&hit.tags.join(", "));
        let tags = if tags.chars().count() > 20 {
            format!("{}...", tags.chars().take(17).collect::<String>())
        } else {
            tags
        };
        writeln!(out, "{}", row(&hit.kind, &hit.remote, &f(&hit.name), &tags))?;
    }
    writeln!(out)?;
    writeln!(
        out,
        "Use one: add its ref to a profile (ctxloom profile create/modify), then ctxloom deps pull"
    )
}

/// `run --dry-run`: the sections this build can show.
pub fn dry_run(out: &mut dyn Write, plan: &DryRun) -> io::Result<()> {
    if !plan.agent.is_empty() {
        let (workspace, runtime) = &plan.axes;
        writeln!(out, "=== Agent ===")?;
        writeln!(
            out,
            "{} (workspace: {workspace}, runtime: {runtime})",
            plan.agent
        )?;
    }
    writeln!(out, "=== LLM ===")?;
    writeln!(out, "{} ({})", plan.llm, plan.backend)?;
    writeln!(out, "\n=== Profiles ===")?;
    listed(out, &plan.profiles, "(no profiles)")?;
    writeln!(out, "\n=== Fragments Loaded ===")?;
    listed(out, &plan.fragments, "(no fragments)")?;
    writeln!(out, "\n=== Assembled Context (~{} tokens) ===", plan.tokens)?;
    if plan.context.is_empty() {
        writeln!(out, "(no context)")?;
    } else {
        writeln!(out, "{}", plan.context)?;
    }
    writeln!(out, "\n=== Prompt ===")?;
    if plan.prompt.is_empty() {
        writeln!(out, "(interactive mode)")
    } else {
        writeln!(out, "{}", plan.prompt)
    }
}

fn listed(out: &mut dyn Write, items: &[String], none: &str) -> io::Result<()> {
    if items.is_empty() {
        return writeln!(out, "{none}");
    }
    for item in items {
        writeln!(out, "  {item}")?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "text.test.rs"]
mod tests;
