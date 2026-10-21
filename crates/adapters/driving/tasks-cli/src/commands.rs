//! Each task command: its arguments, the use case they become, and how
//! its answer and notices are written.

use std::io::{self, Write};

use clap::Args;
use presentation::{Catalog, Format};
use tasks_application::{ListQuery, Listing, NewTask, Reach, StatusChange, TagChange, TaskService};

use crate::render::{Added, Listed, Tagged};
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
    #[arg(long, default_value = "")]
    status: String,
    /// Revive condition for a Deferred task (required when --status Deferred).
    #[arg(long, default_value = "")]
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
    harp_id: String,
    /// The status: any name not starting with '@' (see `taskloom statuses`).
    status: String,
    /// Revive condition, when the status is Deferred; a task re-deferred
    /// keeps the one it has.
    #[arg(long, default_value = "")]
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
pub(crate) struct TagArgs {
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
    #[arg(long, default_value = "")]
    term: String,
    /// Include the tasks hidden by default: Done, Archived and Deferred.
    #[arg(long)]
    all: bool,
    /// Every project's tasks, not just this project's.
    #[arg(long)]
    global: bool,
    /// The order: priority, Go's one sort, waits on priority, which is
    /// not computed yet; the default is the order added (across projects,
    /// by project and then id).
    #[arg(long, default_value = "")]
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
    #[arg(long = "tag-query", default_value = "")]
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

/// Where the listing was read, if not here; what the default view hid of
/// a search's matches; and what the limit left out.
fn listing_notices(listing: &Listing, catalog: &Catalog, err: &mut dyn Write) -> io::Result<()> {
    reached(&listing.reach, catalog, err)?;
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
