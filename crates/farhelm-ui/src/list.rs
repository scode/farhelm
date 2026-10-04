//! The session list: `ListView` (the flat listing, its filter and search
//! surface, and its stop/delete/create/rename actions), `SessionRow`
//! (one row, including the inline lifecycle confirmations), and
//! `CreateSessionForm` (the "new session" inline form). All three are
//! `ListView`'s own concern — none of them is meaningful mounted outside
//! it — so only `ListView` itself is `pub(crate)`; `SessionRow` and
//! `CreateSessionForm` stay private to this module. The rename editor is
//! the one visibility exception: `rename::RenameDialog` is the list-owned
//! modal the sidebar's rename contract lives in (the edit survives the
//! menu, the row, and listing churn, which the popup's geometry could
//! never guarantee), and the list view submodule is what mounts it.
//!
//! ## The list is multi-host (PLAN_M6.md item 6)
//!
//! Every row names the host it lives on, and a row whose host is not
//! connected is marked stale rather than hidden — SPEC.md: sessions on an
//! unreachable host "stay in the list from the helm's last-known
//! knowledge, clearly marked". Their lifecycle controls stay live too, and
//! deliberately: the helm refuses such an operation with the host's state
//! in the message, which is a far more useful answer than a disabled button
//! that explains nothing.
//!
//! This view also owns the hosts READ (`hosts::HostsPanel` renders it),
//! because two consumers need one read: the panel, and the create dialog's
//! host selector.
//!
//! ## The list is the WHOLE list
//!
//! `api::fetch_sessions` makes one request for the whole list and this view
//! renders what comes back in the helm's own order, unsorted. The helm's cap
//! is the only thing that can leave the list short, and it says so with
//! `truncated`; a client-side sort would only ever rearrange an order the
//! helm already produced for the sort control's own setting.
//!
//! The one exception is display-only and temporary: while the pointer is over
//! the list, `view`'s order hold keeps rows where they were so none moves
//! under the pointer (SPEC.md, Session list). It never re-sorts anything; it
//! only delays the helm's order, and `rows::held_display_order` is the whole
//! rule for what a held list shows.

mod create_form;
mod row;
mod shared;
/// The Templates panel beside New (SPEC.md, Launch templates).
mod templates;
mod view;

pub(crate) use shared::{OpenDestination, with_source_host};
pub(crate) use view::{
    DeleteNotice, HeaderDeleteRequest, HeaderPrefillRequest, ListView, SharedPreferences,
    remember_selection,
};
