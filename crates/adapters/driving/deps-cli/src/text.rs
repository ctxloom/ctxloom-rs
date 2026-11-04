//! The text forms: what each command prints for a person (Go's renderers,
//! line for line). What a bundle's author wrote goes through [`field`] first,
//! so it cannot drive the terminal.

use std::io::{self, Write};

use angzarr_presentation::text;
use deps_application::{
    Browsed, CheckReport, CheckedRef, DepsListing, Discovered, EditedRemote, HoldResult, Moved,
    PullReport, Pushed, RemoteListing, RemoveResult, UpgradeReport,
};
use pin_changes::{new_pins_text, pin_changes_text};

/// A value for a terminal: control characters shown as escapes.
pub fn field(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// A commit shortened to seven characters.
fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// `remote list`.
pub fn remote_list(out: &mut dyn Write, listing: &RemoteListing) -> io::Result<()> {
    if listing.count == 0 {
        writeln!(out, "No remotes configured.")?;
        writeln!(
            out,
            "Use 'ctxloom remote create <name> <url>' to add a remote."
        )?;
        return writeln!(
            out,
            "Use 'ctxloom remote discover' to find public repositories."
        );
    }
    writeln!(out, "Configured remotes:")?;
    for remote in &listing.remotes {
        let mark = if remote.name == listing.default {
            " (default)"
        } else {
            ""
        };
        writeln!(out, "  {:<15} {}{mark}", remote.name, remote.url)?;
    }
    Ok(())
}

/// `remote edit`.
pub fn remote_edit(out: &mut dyn Write, edited: &EditedRemote) -> io::Result<()> {
    if edited.changed.is_empty() {
        return writeln!(out, "remote {:?} unchanged", edited.name);
    }
    writeln!(
        out,
        "Edited remote {:?} ({})",
        edited.name,
        edited.changed.join(", ")
    )?;
    let (before, after) = (&edited.before, &edited.after);
    if before.name != after.name {
        writeln!(out, "  name:  {} \u{2192} {}", before.name, after.name)?;
    }
    if before.url != after.url {
        writeln!(out, "  url:   {} \u{2192} {}", before.url, after.url)?;
    }
    if before.forge != after.forge {
        writeln!(
            out,
            "  forge: {} \u{2192} {}",
            forge_or_host(&before.forge),
            forge_or_host(&after.forge)
        )?;
    }
    if edited.default_pointer_updated {
        writeln!(out, "  the default remote followed the rename")?;
    }
    Ok(())
}

/// An empty forge as what it MEANS, so a cleared binding reads as a decision.
fn forge_or_host(label: &str) -> &str {
    if label.is_empty() {
        "(resolved from URL host)"
    } else {
        label
    }
}

/// `remote remove`.
pub fn remote_remove(out: &mut dyn Write, removed: &RemoveResult) -> io::Result<()> {
    match removed {
        RemoveResult::Preview { target, apply, .. } => {
            writeln!(out, "Would remove {target}.")?;
            writeln!(out)?;
            writeln!(out, "Nothing was removed. Re-run with --yes to apply:")?;
            writeln!(out, "  {apply}")
        }
        RemoveResult::Removed { name, .. } => writeln!(out, "Removed remote '{name}'"),
    }
}

/// `remote show`.
pub fn remote_show(out: &mut dyn Write, browsed: &Browsed) -> io::Result<()> {
    if browsed.count == 0 {
        return writeln!(out, "No bundles found in {}", browsed.remote);
    }
    writeln!(
        out,
        "Bundles in {} ({}):\n",
        browsed.remote,
        field(&browsed.url)
    )?;
    for item in &browsed.items {
        writeln!(out, "  {}", field(&item.pull_ref))?;
    }
    writeln!(out)?;
    writeln!(
        out,
        "Use one: add its ref to a profile (ctxloom profile create/edit), then ctxloom deps pull"
    )
}

/// `deps list`.
pub fn deps_list(out: &mut dyn Write, listing: &DepsListing) -> io::Result<()> {
    if listing.deps.is_empty() {
        writeln!(out, "No dependencies installed.")?;
        return writeln!(
            out,
            "A profile that names a remote bundle gets one; run 'ctxloom deps pull' to install it."
        );
    }
    writeln!(out, "Installed dependencies:")?;
    for dep in &listing.deps {
        let origin = [&dep.origin, &dep.url, &dep.reference]
            .into_iter()
            .find(|text| !text.is_empty())
            .map_or("", String::as_str);
        let mut marks = String::new();
        if !dep.constraint.is_empty() {
            marks.push_str(&format!("  ({})", field(&dep.constraint)));
        }
        if dep.held {
            marks.push_str("  [held]");
        }
        writeln!(
            out,
            "  {:<20} {:<10} {}{marks}",
            field(&dep.name),
            short(&dep.sha),
            field(origin)
        )?;
    }
    Ok(())
}

/// `deps pull`.
pub fn pull(out: &mut dyn Write, report: &PullReport) -> io::Result<()> {
    if report.total == 0 {
        writeln!(out, "No remote dependencies to pull.")?;
    } else {
        pull_summary(out, report)?;
    }
    if let Some(reconcile) = &report.reconcile {
        reconciled(out, reconcile)?;
    }
    Ok(())
}

fn pull_summary(out: &mut dyn Write, report: &PullReport) -> io::Result<()> {
    writeln!(out, "\nDependencies ({}):", report.total)?;
    if report.installed > 0 {
        writeln!(out, "  Installed: {}", report.installed)?;
    }
    if report.reinstalled > 0 {
        writeln!(out, "  Reinstalled at their pin: {}", report.reinstalled)?;
    }
    if !report.skipped.is_empty() {
        writeln!(
            out,
            "  Skipped (kept at their locked commit): {}",
            report.skipped.len()
        )?;
        writeln!(
            out,
            "    Pull never moves an existing pin and does not ask upstream whether one could move."
        )?;
        writeln!(out, "    Run 'ctxloom deps check' to find out.")?;
    }
    for identity in &report.removed {
        writeln!(
            out,
            "  Removed {} from the lockfile: nothing this project composes depends on it any more.",
            field(identity)
        )?;
    }
    for dir in &report.pruned_checkouts {
        writeln!(
            out,
            "  Pruned the checkout at {}: the lockfile no longer names its bundle.",
            field(dir)
        )?;
    }
    if report.errors > 0 {
        writeln!(out, "  Failed: {}", report.errors)?;
        for item in &report.failed {
            writeln!(
                out,
                "    - {}: {}",
                field(&item.reference),
                field(&item.error)
            )?;
            writeln!(out, "      fix: {}", field(&item.fix))?;
        }
    }
    if report.incomplete {
        let names: Vec<String> = report.unreachable.iter().map(|n| field(n)).collect();
        writeln!(
            out,
            "  Lock incomplete: could not reach {}; their previous lock entries were kept.",
            names.join(", ")
        )?;
    }
    constraint_changes(out, report)?;
    write!(out, "{}", new_pins_text(&report.changes))
}

/// Pins kept at their commit though the manifest now asks for something else.
fn constraint_changes(out: &mut dyn Write, report: &PullReport) -> io::Result<()> {
    let label = |expression: &str| {
        if expression.is_empty() {
            "the default branch".to_string()
        } else {
            expression.to_string()
        }
    };
    for change in &report.constraint_changes {
        writeln!(
            out,
            "  {}: the manifest now asks for {}; the pin stays at {} (resolved from {}).",
            change.identity,
            label(&change.declared),
            short(&change.sha),
            label(&change.pinned)
        )?;
    }
    if !report.constraint_changes.is_empty() {
        writeln!(
            out,
            "  Only 'ctxloom deps upgrade' moves a pin: run it to see the change, then 'ctxloom deps upgrade --yes' to apply it."
        )?;
    }
    Ok(())
}

/// What the reconcile did, and what it could not do: both halves are
/// load-bearing, and neither substitutes for the other.
fn reconciled(out: &mut dyn Write, plan: &deps_application::ReconcileView) -> io::Result<()> {
    if !plan.gone.is_empty() {
        writeln!(
            out,
            "\nRemoved {} dependency(ies) no longer published by their remote:",
            plan.gone.len()
        )?;
        for reference in &plan.gone {
            writeln!(out, "  - {}", field(reference))?;
        }
        writeln!(
            out,
            "  Re-adding them upstream and pulling again restores them; nothing authored here was touched."
        )?;
    }
    for unchecked in &plan.unreachable {
        let place = if unchecked.url.is_empty() {
            "an unidentifiable repository".to_string()
        } else {
            field(&unchecked.url)
        };
        writeln!(
            out,
            "\n{place} could not be reached, so its dependencies were left exactly as they are ({}).",
            field(&unchecked.reason)
        )?;
        for reference in &unchecked.refs {
            writeln!(out, "  - kept: {}", field(reference))?;
        }
        writeln!(
            out,
            "  Nothing is removed on the strength of a remote that could not be read."
        )?;
    }
    Ok(())
}

/// `deps check`.
pub fn check(out: &mut dyn Write, report: &CheckReport) -> io::Result<()> {
    if report.entries == 0 {
        writeln!(out, "Nothing is installed, so there is nothing to check.")?;
        return writeln!(
            out,
            "Install this project's closure with: ctxloom deps pull"
        );
    }
    for row in report
        .unchecked
        .iter()
        .filter(|r| r.reason == "not_refreshed")
    {
        writeln!(
            out,
            "  {}: not checked \u{2014} {} could not be fetched (see warning above)",
            field(&row.reference),
            field(&row.url)
        )?;
    }
    if report.skipped_empty > 0 {
        writeln!(
            out,
            "Skipped {} entries with empty SHA (run 'ctxloom deps pull' to clean up)\n",
            report.skipped_empty
        )?;
    }
    if report.updates.is_empty() {
        return if report.unchecked.is_empty() {
            writeln!(out, "All items are up to date!")
        } else {
            writeln!(
                out,
                "No updates found among the entries that could be checked \u{2014} {} entry(ies) could not be checked (see warnings above).",
                report.unchecked.len()
            )
        };
    }
    writeln!(
        out,
        "Found {} items with updates available:\n",
        report.updates.len()
    )?;
    writeln!(out, "Bundles:")?;
    for update in &report.updates {
        writeln!(
            out,
            "  {}  ({})",
            field(&update.reference),
            selector_label(update)
        )?;
        writeln!(
            out,
            "    Current: {} \u{2192} Latest: {}",
            short(&update.current_sha),
            short(&update.latest_sha)
        )?;
    }
    writeln!(out, "\nRun 'ctxloom deps upgrade' to advance these pins.")
}

/// What an entry tracks, for the update listing.
fn selector_label(update: &deps_application::UpdateRow) -> String {
    match update.selector.as_str() {
        "branch" if update.requested.is_empty() => "tracking branch default branch".to_string(),
        "branch" => format!("tracking branch {}", field(&update.requested)),
        "version" if update.resolved.is_empty() => format!("range {}", field(&update.requested)),
        "version" => format!(
            "range {} \u{2192} {}",
            field(&update.requested),
            field(&update.resolved)
        ),
        other => other.to_string(),
    }
}

/// `deps check <reference>`.
pub fn check_reference(out: &mut dyn Write, checked: &CheckedRef) -> io::Result<()> {
    if checked.current_sha.is_empty() {
        writeln!(
            out,
            "{} not found in lockfile, checking latest version...",
            field(&checked.reference)
        )?;
    } else if checked.up_to_date {
        writeln!(
            out,
            "{} is up to date (SHA: {})",
            field(&checked.reference),
            short(&checked.latest_sha)
        )?;
    } else {
        writeln!(out, "{} has update available:", field(&checked.reference))?;
        writeln!(out, "  Current: {}", short(&checked.current_sha))?;
        writeln!(out, "  Latest:  {}", short(&checked.latest_sha))?;
    }
    if !checked.up_to_date {
        writeln!(out, "\nRun 'ctxloom deps upgrade' to advance it.")?;
    }
    Ok(())
}

/// `deps upgrade`: removals, each pin that moves with what it brings in, and
/// what to do next.
pub fn upgrade(out: &mut dyn Write, report: &UpgradeReport) -> io::Result<()> {
    let verb = if report.applied {
        "Removed"
    } else {
        "Would remove"
    };
    for identity in &report.removed {
        writeln!(
            out,
            "{verb} {} from the lockfile: nothing this project composes depends on it any more.",
            field(identity)
        )?;
    }
    write!(out, "{}", pin_changes_text(&report.changes))?;
    if report.changes.is_empty() {
        nothing_moves(out, report)?;
    }
    let moves = report.changes.len();
    if report.applied && moves > 0 {
        writeln!(out, "Applied {moves} pin(s).")
    } else if !report.applied && moves + report.removed.len() > 0 {
        writeln!(
            out,
            "{moves} pin(s) would move. Re-run with --yes to apply."
        )
    } else {
        Ok(())
    }
}

/// The line for a round in which no pin moves; which depends on WHY.
fn nothing_moves(out: &mut dyn Write, report: &UpgradeReport) -> io::Result<()> {
    if report.incomplete {
        writeln!(
            out,
            "No pins advanced among what could be resolved \u{2014} part of the dependency closure was unreachable this round (see warning above); re-run once it's reachable to get a complete picture."
        )
    } else if report.nothing_declared {
        writeln!(
            out,
            "Nothing to upgrade: no dependencies are declared here, so there was nothing to check and no lockfile was written."
        )?;
        writeln!(
            out,
            "  If you expected some, you are probably not in the project you meant: run 'ctxloom deps upgrade' from your project root."
        )?;
        writeln!(
            out,
            "  'ctxloom profile list' shows what this directory actually resolves, and 'ctxloom init' sets a new project up."
        )
    } else {
        writeln!(out, "Everything is up to date.")
    }
}

/// `deps hold` and `deps unhold`.
pub fn hold(out: &mut dyn Write, held: &HoldResult) -> io::Result<()> {
    match held.status {
        "held" => writeln!(out, "Held {:?} at its locked SHA.", held.name),
        "released" => writeln!(
            out,
            "Released hold on {:?}; the next 'ctxloom deps upgrade' may advance it.",
            held.name
        ),
        _ => Ok(()),
    }
}

/// `remote discover`: the repositories as a table, and how to register one.
pub fn discovered(out: &mut dyn Write, found: &Discovered, query: &str) -> io::Result<()> {
    if found.count == 0 {
        writeln!(out, "No ctxloom repositories found.")?;
        if !query.is_empty() {
            writeln!(out, "Try a different search term or remove the filter.")?;
        }
        return Ok(());
    }
    let names: Vec<String> = found
        .repositories
        .iter()
        .map(|repo| field(&format!("{}/{}", repo.owner, repo.name)))
        .collect();
    let width = names
        .iter()
        .map(|name| text::width(name))
        .fold(text::width("Repository"), usize::max);
    let rule = |n: usize| "\u{2500}".repeat(n);
    writeln!(
        out,
        "  # \u{2502} Forge  \u{2502} {} \u{2502} Stars \u{2502} Description",
        text::pad("Repository", width)
    )?;
    writeln!(
        out,
        "{}\u{253c}{}\u{253c}{}\u{253c}{}\u{253c}{}",
        rule(4),
        rule(8),
        rule(width + 2),
        rule(7),
        rule(DESCRIPTION_CAP + 2)
    )?;
    for (at, (repo, name)) in found.repositories.iter().zip(&names).enumerate() {
        writeln!(
            out,
            "{:>3} \u{2502} {:<6} \u{2502} {} \u{2502} {:>5} \u{2502} {}",
            at + 1,
            "GitHub",
            text::pad(name, width),
            repo.stars,
            field(&ellipsize(&repo.description, DESCRIPTION_CAP))
        )?;
    }
    writeln!(out)?;
    writeln!(out, "Add one with: ctxloom remote create <name> <url>")
}

/// The most bytes of a description the table shows.
const DESCRIPTION_CAP: usize = 35;

/// `text` within `max` bytes: cut at a character, with an ellipsis, where it
/// is longer (Go's `textutil.Ellipsize`).
fn ellipsize(text: &str, max: usize) -> String {
    const MARK: &str = "\u{2026}";
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max - MARK.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{MARK}", &text[..end])
}

/// `bundle push`.
pub fn pushed(out: &mut dyn Write, pushed: &Pushed) -> io::Result<()> {
    if !pushed.pr_url.is_empty() {
        return writeln!(out, "Created pull request: {}", field(&pushed.pr_url));
    }
    writeln!(out, "Pushed {} to {}", pushed.target_path, pushed.remote)?;
    writeln!(out, "Commit: {}", short(&pushed.commit_sha))
}

/// `bundle move`.
pub fn moved(out: &mut dyn Write, moved: &Moved) -> io::Result<()> {
    if moved.dest_kind == "remote" {
        writeln!(
            out,
            "Moved {} -> {} (remote {}, commit {})",
            moved.source,
            moved.dest,
            moved.remote,
            short(&moved.commit_sha)
        )
    } else {
        writeln!(out, "Moved {} -> {}", moved.source, moved.dest)
    }
}

#[cfg(test)]
#[path = "text.test.rs"]
mod tests;
