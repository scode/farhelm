//! The session list: `ListView` (the flat listing, its filter and search
//! surface, and its stop/delete/archive/create/rename actions), `SessionRow`
//! (one row, including the inline lifecycle confirmations and the inline
//! rename field), and
//! `CreateSessionForm` (the "new session" inline form). All three are
//! `ListView`'s own concern — none of them is meaningful mounted outside
//! it — so only `ListView` itself is `pub(crate)`; `SessionRow` and
//! `CreateSessionForm` stay private to this module. The rename FIELD is
//! the one exception: `rename::RenameForm` is shared with the session
//! view, since SPEC.md puts the same operation on both surfaces.
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
//! ## Profiles reach the list in two places (PLAN_M6_75.md item 8)
//!
//! The create dialog gains an agent picker over the target host's catalog,
//! and a row created from a profile names the profile it SNAPSHOTTED. Both
//! rules — what a fresh dialog preselects, and how a snapshot reads once the
//! catalog has moved on — live in `profiles` rather than being re-derived by
//! the list. `create_form` owns the picker state and submit handlers, `row`
//! owns snapshot presentation, and `view` composes both with the list-level
//! state, as it does for the rename overlay and the count banner.
//!
//! ## The list is the WHOLE list
//!
//! `api::fetch_sessions` makes one request for the whole list and this view
//! renders what comes back in the helm's own order, unsorted. The helm's cap
//! is the only thing that can leave the list short, and it says so with
//! `truncated`; a client-side sort would only ever rearrange an order the
//! helm already produced for the sort control's own setting.

mod create_form;
mod row;
mod shared;
mod view;

pub(crate) use shared::OpenHost;
pub(crate) use view::{ListView, SharedPreferences};
