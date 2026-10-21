//! The tables and columns, as sea-query identifiers: every query names them
//! through these, never as strings. The tables themselves are created by the
//! migrations (`migrations/`).

use sea_query::Iden;

#[derive(Iden)]
pub(crate) enum Events {
    Table,
    Domain,
    Edition,
    Root,
    Sequence,
    CommandId,
    Page,
}

#[derive(Iden)]
pub(crate) enum Snapshots {
    Table,
    Domain,
    Edition,
    Root,
    Snapshot,
}
