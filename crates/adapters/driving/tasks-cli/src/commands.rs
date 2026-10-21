//! Each task command: its arguments, the use case they become, and how
//! its answer and notices are written.

use std::io::{self, Write};

use clap::Args;
use presentation::{Catalog, Format};
use tasks_application::{
    ListQuery, Listing, NewTask, PruneRequest, PruneService, Reach, Scope, ShowQuery, StatusChange,
    TagChange, TaskService, TextEdit, DEFAULT_OLDER_THAN, DEFAULT_PRUNE_STATUSES,
};

use crate::prune::{apply_command, PruneReport};
use crate::render::{Added, Counted, Detailed, Linted, Listed, Summed, Tagged, Taxonomy};
use crate::{notify, output_failed, wrote, CliError, Notice};

/// What every task command is handed besides its own arguments: the
/// global flags, the format, the catalog and the session.
pub(crate) struct Ctx<'a> {
    pub project: Option<String>,
    pub config_set: Vec<String>,
    pub format: Format,
    pub catalog: &'a Catalog,
    pub session: &'a str,
}

#[derive(Args, Debug)]
pub(crate) struct AddArgs {
    /// The task's text; several words are joined with spaces.
    #[arg(required = true, num_args = 1..)]
    text: Vec<String>,
    /// Initial status (default: "To Do").
    #[arg(long, default_value = "", hide_default_value = true)]
    status: String,
    /// Revive condition for a Deferred task (required when --status Deferred).
    #[arg(long, default_value = "", hide_default_value = true)]
    trigger: String,
    /// Tag to set at creation (repeatable).
    #[arg(long = "tag")]
    tags: Vec<String>,
}

pub(crate) async fn add(
    args: AddArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let task = NewTask {
        project: ctx.project,
        text: args.text.join(" "),
        status: args.status,
        trigger: args.trigger,
        tags: args.tags,
        origin_session: ctx.session.to_string(),
        config_set: ctx.config_set,
    };
    let written = service.add(task).await.map_err(CliError::App)?;
    wrote(
        &written,
        &Added(&written.task),
        ctx.format,
        ctx.catalog,
        out,
        err,
    )
}

#[derive(Args, Debug)]
pub(crate) struct StatusArgs {
    /// The task's id.
    harp_id: String,
    /// The status: any name not starting with '@' (see `taskloom statuses`).
    status: String,
    /// Revive condition, when the status is Deferred; a task re-deferred
    /// keeps the one it has.
    #[arg(long, default_value = "", hide_default_value = true)]
    trigger: String,
}

pub(crate) async fn status(
    args: StatusArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let change = StatusChange {
        project: ctx.project,
        harp_id: args.harp_id,
        status: args.status,
        trigger: args.trigger,
        config_set: ctx.config_set,
    };
    let written = service.set_status(change).await.map_err(CliError::App)?;
    wrote(
        &written,
        &Added(&written.task),
        ctx.format,
        ctx.catalog,
        out,
        err,
    )
}

#[derive(Args, Debug)]
pub(crate) struct EditArgs {
    /// The task's id.
    harp_id: String,
    /// The new text, whole; several words are joined with spaces. Text
    /// starting with '-' goes after `--`.
    #[arg(required = true, num_args = 1..)]
    text: Vec<String>,
}

pub(crate) async fn edit(
    args: EditArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let edit = TextEdit {
        project: ctx.project,
        harp_id: args.harp_id,
        text: args.text.join(" "),
        config_set: ctx.config_set,
    };
    let written = service.edit(edit).await.map_err(CliError::App)?;
    wrote(
        &written,
        &Added(&written.task),
        ctx.format,
        ctx.catalog,
        out,
        err,
    )
}

#[derive(Args, Debug)]
pub(crate) struct TagArgs {
    /// The task's id.
    harp_id: String,
    /// Tag to add (repeatable).
    #[arg(long)]
    add: Vec<String>,
    /// Tag to remove (repeatable).
    #[arg(long)]
    remove: Vec<String>,
}

pub(crate) async fn tag(
    args: TagArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let change = TagChange {
        project: ctx.project,
        harp_id: args.harp_id,
        add: args.add,
        remove: args.remove,
        config_set: ctx.config_set,
    };
    let written = service.tag(change).await.map_err(CliError::App)?;
    let view = Tagged(&written.task, &written.shown);
    wrote(&written, &view, ctx.format, ctx.catalog, out, err)
}

#[derive(Args, Debug)]
pub(crate) struct ListArgs {
    /// Only tasks in this status (repeatable, or comma-separated); the
    /// classes @open (every status not terminal) and @terminal (Done,
    /// Archived) mix with statuses.
    #[arg(long, value_delimiter = ',')]
    status: Vec<String>,
    /// Only tasks whose text holds this, in any case.
    #[arg(long, default_value = "", hide_default_value = true)]
    term: String,
    /// Include the tasks hidden by default: Done, Archived and Deferred.
    #[arg(long)]
    all: bool,
    /// Every project's tasks, not just this project's.
    #[arg(long)]
    global: bool,
    /// The order: priority, highest first, each project's tasks ranked
    /// against all of its own; the default is the order added (across
    /// projects, by project and then id).
    #[arg(long, default_value = "", hide_default_value = true)]
    sort: String,
    /// In JSON, each task as its id, status, checked, tags and headline;
    /// text is one line per task already.
    #[arg(long)]
    compact: bool,
    /// At most this many tasks (0: no limit); how many more there were is
    /// said on stderr.
    #[arg(long, default_value_t = 0)]
    limit: usize,
    /// Only the tasks this tag query selects: postfix, tags first and the
    /// operator after (urgent/release/and, urgent/not), or infix (urgent
    /// and not release). Operators: and, or, not; atoms [ns:]key[op value]
    /// with op one of = != > >= < <= ~, and * (any) or + (present) for a
    /// namespace, key or value.
    #[arg(long = "tag-query", default_value = "", hide_default_value = true)]
    tag_query: String,
}

pub(crate) async fn list(
    args: ListArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let query = ListQuery {
        project: ctx.project,
        statuses: args.status,
        all: args.all,
        tag_query: args.tag_query,
        config_set: ctx.config_set,
        term: args.term,
        global: args.global,
        limit: args.limit,
        sort: args.sort,
    };
    let listing = service.list(query).await.map_err(CliError::App)?;
    listing_notices(&listing, ctx.catalog, err)
        .and_then(|()| {
            let view = Listed {
                listing: &listing,
                compact: args.compact,
            };
            presentation::render(out, ctx.format, &view)
        })
        .map_err(output_failed)
}

#[derive(Args, Debug)]
pub(crate) struct ShowArgs {
    /// The tasks' ids: every one must be found.
    #[arg(required = true, num_args = 1..)]
    harp_ids: Vec<String>,
    /// Find them in every project, not just this one.
    #[arg(long)]
    global: bool,
}

pub(crate) async fn show(
    args: ShowArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let query = ShowQuery {
        project: ctx.project,
        global: args.global,
        harp_ids: args.harp_ids,
        config_set: ctx.config_set,
    };
    let details = service.show(query).await.map_err(CliError::App)?;
    read_in(&details.reach, ctx.catalog, err)
        .and_then(|()| presentation::render(out, ctx.format, &Detailed(&details)))
        .map_err(output_failed)
}

#[derive(Args, Debug)]
pub(crate) struct GlobalArgs {
    /// Every project, not just this one.
    #[arg(long)]
    global: bool,
}

impl GlobalArgs {
    fn scope(&self, ctx: Ctx<'_>) -> Scope {
        Scope {
            project: ctx.project,
            global: self.global,
            config_set: ctx.config_set,
        }
    }
}

pub(crate) async fn summary(
    args: GlobalArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let (format, catalog) = (ctx.format, ctx.catalog);
    let summary = service
        .summary(args.scope(ctx))
        .await
        .map_err(CliError::App)?;
    reached(&summary.reach, catalog, err)
        .and_then(|()| presentation::render(out, format, &Summed(&summary)))
        .map_err(output_failed)
}

pub(crate) async fn statuses(
    args: GlobalArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let (format, catalog) = (ctx.format, ctx.catalog);
    let statuses = service
        .statuses(args.scope(ctx))
        .await
        .map_err(CliError::App)?;
    // With no project the defaults are still the answer: only a read of
    // every project for want of one is worth saying.
    let noticed = match statuses.reach {
        Reach::Every { .. } => reached(&statuses.reach, catalog, err),
        Reach::Project(_) | Reach::NoProject => Ok(()),
    };
    noticed
        .and_then(|()| presentation::render(out, format, &Taxonomy(&statuses)))
        .map_err(output_failed)
}

#[derive(Args, Debug)]
pub(crate) struct TagsArgs {
    /// Count only the tasks in this status (repeatable, or
    /// comma-separated); @open and @terminal as list takes them.
    #[arg(long, value_delimiter = ',')]
    status: Vec<String>,
    /// Count only the tasks whose text holds this, in any case (it narrows
    /// the tasks, never the tags' names).
    #[arg(long, default_value = "", hide_default_value = true)]
    term: String,
    /// Count only the tasks this tag query selects (see `list --help`).
    #[arg(long = "tag-query", default_value = "", hide_default_value = true)]
    tag_query: String,
    /// Every project's tags, not just this project's.
    #[arg(long)]
    global: bool,
}

pub(crate) async fn tags(
    args: TagsArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let query = ListQuery {
        project: ctx.project,
        statuses: args.status,
        tag_query: args.tag_query,
        config_set: ctx.config_set,
        term: args.term,
        global: args.global,
        ..ListQuery::default()
    };
    let vocabulary = service.tags(query).await.map_err(CliError::App)?;
    reached(&vocabulary.reach, ctx.catalog, err)
        .and_then(|()| presentation::render(out, ctx.format, &Counted(&vocabulary)))
        .map_err(output_failed)
}

/// Lint, then refuse when it found a violation or checked no target, so
/// that CI sees it in the exit code.
pub(crate) async fn lint(
    args: GlobalArgs,
    ctx: Ctx<'_>,
    service: &mut impl TaskService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let (format, catalog) = (ctx.format, ctx.catalog);
    let report = service.lint(args.scope(ctx)).await.map_err(CliError::App)?;
    let view = Linted {
        report: &report,
        catalog,
    };
    reached(&report.reach, catalog, err)
        .and_then(|()| presentation::render(out, format, &view))
        .map_err(output_failed)?;
    if report.checked_targets == 0 {
        return Err(CliError::LintCheckedNothing);
    }
    match report.violations.len() {
        0 => Ok(()),
        count => Err(CliError::LintViolations { count }),
    }
}

/// `prune`'s flags.
#[derive(Args, Debug)]
pub(crate) struct PruneArgs {
    /// Only tasks whose last status change is older than this age (30d,
    /// 12w, 720h) or date (2026-01-01).
    #[arg(long = "older-than", default_value = DEFAULT_OLDER_THAN)]
    older_than: String,
    /// The statuses that qualify a task (repeatable, or comma-separated).
    #[arg(long, value_delimiter = ',', default_values = DEFAULT_PRUNE_STATUSES)]
    status: Vec<String>,
    /// Only tasks matching this tag query (see list --tag-query).
    #[arg(long = "tag-query", default_value = "", hide_default_value = true)]
    tag_query: String,
    /// Remove and purge; without it prune only reports what it would.
    #[arg(long)]
    yes: bool,
}

/// Run the prune the flags ask for. Say on stderr when there is no
/// project here, then render the report, and after a report that would
/// take something, say nothing was taken and the command that takes it.
pub(crate) async fn prune(
    args: PruneArgs,
    ctx: Ctx<'_>,
    service: &mut impl PruneService,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let apply = apply_command(&args.older_than, &args.status, &args.tag_query);
    let request = PruneRequest {
        project: ctx.project,
        statuses: args.status,
        older_than: args.older_than,
        tag_query: args.tag_query,
        apply: args.yes,
        config_set: ctx.config_set,
    };
    let (format, catalog) = (ctx.format, ctx.catalog);
    let pruning = service.prune(request).await.map_err(CliError::App)?;
    if pruning.project.is_none() {
        notify(&Notice::NothingToPrune, catalog, err).map_err(output_failed)?;
    }
    presentation::render(out, format, &PruneReport(&pruning)).map_err(output_failed)?;
    if pruning.applied || pruning.pruned.is_empty() {
        return Ok(());
    }
    let report_only = Notice::ReportOnly { command: apply };
    notify(&report_only, catalog, err).map_err(output_failed)
}

/// Say on stderr which project a read was in: the one here, by name, or
/// why it was not.
pub(crate) fn read_in(reach: &Reach, catalog: &Catalog, err: &mut dyn Write) -> io::Result<()> {
    match reach {
        Reach::Project(project) => notify(&Notice::project(project), catalog, err),
        Reach::NoProject | Reach::Every { .. } => reached(reach, catalog, err),
    }
}

/// Say on stderr where a read went when it is not the project here: no
/// project, or every project for want of one.
pub(crate) fn reached(reach: &Reach, catalog: &Catalog, err: &mut dyn Write) -> io::Result<()> {
    match reach {
        Reach::NoProject => notify(&Notice::NoProject, catalog, err),
        Reach::Every {
            fallback: Some(directory),
            ..
        } => notify(
            &Notice::EveryProject {
                directory: directory.clone(),
            },
            catalog,
            err,
        ),
        Reach::Project(_) | Reach::Every { fallback: None, .. } => Ok(()),
    }
}

/// Where the listing was read, if not here; why its ranking by priority
/// means little; what the default view hid of a search's matches; and
/// what the limit left out.
fn listing_notices(listing: &Listing, catalog: &Catalog, err: &mut dyn Write) -> io::Result<()> {
    reached(&listing.reach, catalog, err)?;
    for note in &listing.priority_notes {
        notify(note, catalog, err)?;
    }
    let hidden = listing.hidden;
    if listing.searched && hidden.total() > 0 {
        let notice = Notice::Hidden {
            hidden: hidden.total(),
            completed: hidden.completed,
            deferred: hidden.deferred,
        };
        notify(&notice, catalog, err)?;
    }
    if listing.omitted > 0 {
        let notice = Notice::Omitted {
            omitted: listing.omitted,
        };
        notify(&notice, catalog, err)?;
    }
    Ok(())
}
