//! Retired containerized sessions: what the list and the open session's view withdraw from a session of the earlier
//! container model, which supports Delete only.
//!
//! SPEC.md, "Retired containerized sessions": the earlier model, whose supervisor stayed outside the container, is
//! retired; "Existing sessions support Delete only. Opening terminals or tabs, exec, Stop, Restart, Clone, Replace and
//! Replace with are refused." The helm refuses all of them, so a session that offered them would look ordinary while
//! every action failed. Ported from the old app's handling of `Session::is_retired_container` (`lib.rs`, `list/row.rs`,
//! `list/view.rs` and `session_view.rs` at the reference commit, and `retired-container-sessions.spec.ts`). Here's the
//! TLDR of the rules:
//!
//! - **What makes a session retired** is its stored launch alone ([`is`]): an agent launch whose selection still names
//!   the old container (`SessionLaunch::is_retired_container`, the one answer the launcher's Restart with reads too).
//!   Never the host or its provider: a session in a new recipe-backed container runs an ordinary launch and keeps every
//!   ordinary control.
//! - **The row** reads "Retired container · <title>" in every density ([`TITLE_PREFIX`], `super::row`), so the
//!   retirement shows on a compact row too, where no other line could say it. Its accessible name starts the same
//!   way. It draws no bell (closing a bell writes a read mark) and its status dot offers no Mark read or Mark unread
//!   (`super::seen::offered`).
//! - **The marked bar** leaves it out of every metadata batch (`super::marking::MetadataAction::target`): Pin, Unpin,
//!   Mark read and Mark unread neither count it nor send it anything, as the old bar's batches skipped it.
//! - **The menu** keeps Delete alone, the ordinary helm Delete with its ordinary question, and its header's summary
//!   ends with " · " and the retired reason (`super::menu`). Rename, Mark read and Mark unread, Pin, Move up and Move
//!   down, Clone, Replace with, Replace and Stop are not drawn, as the old menu drew none of them. The Rename shortcut
//!   opens nothing for such a session either (`super::rename`), as the old list's metadata fence refused it.
//! - **No seen stamp.** Opening the session never marks it seen (`super::selection::seen_due`): the old app kept a
//!   retired session's stored metadata unchanged, even on selection.
//! - **The open session's view** mounts no terminal and no tab strip and opens no stream, its header draws Delete
//!   alone, and a card in the terminal's place says why (`crate::session_pane::retired`).
//!
//! NOTE: Dragging pins and the marked bar's Stop and Delete are not here, since neither exists yet. The old app refused
//! a retired row's pin drag and left retired sessions out of bulk Stop; whoever ports those reads [`is`].

use farhelm_proto::ui_channel::SessionRow;

/// The words before a retired session's title in its row and in the row's accessible name: the old
/// `session_identity_copy`'s prefix, word for word.
pub(crate) const TITLE_PREFIX: &str = "Retired container · ";

/// Whether `row` is a retired containerized session's (the module docs' first rule), which supports Delete only.
pub(crate) fn is(row: &SessionRow) -> bool {
    row.info.launch.is_retired_container()
}
