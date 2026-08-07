//! The supervisor service: farhelm-proto over a unix socket.
//!
//! This is the only doorway to sessions — the helm (local or via the ssh
//! stdio proxy) and every future caller speak the same protocol to the
//! same handlers, which is what keeps "CLI flags bypass the creation UI,
//! never the creation API" true. The supervisor listens on no network
//! port (SPEC.md): the unix socket plus ssh exec is the entire reachable
//! surface.
//!
//! State model: SQLite (`crate::store`) is the truth that a session exists
//! and what its metadata is, written at creation and reloaded at startup
//! so sessions survive a supervisor restart. tmux stays the truth for
//! whether a session's terminal is currently alive; a session whose own
//! tmux session no longer exists — whether because the whole private tmux
//! server did not survive a restart, or because just that one session was
//! killed independently — is still listed (metadata came back from the
//! DB) but loses its terminal handle — see `core::SessionEntry`'s `terminal`
//! field and `terminals::resolve_terminal`, where that `None` becomes the `NotFound`
//! an `Attach` is refused with. Nothing else has to re-derive it: a
//! terminal-less session can never hold an attachment, so `Resize` and
//! the input path simply find none.
//! PLAN_M2.md's "restart gap" paragraph is the contract.
//!
//! The restart gap still cannot say how a vanished session ENDED. A
//! durable last-known outcome per session — written wherever this process
//! actually WITNESSES a transition — plus the host's boot id turn "the
//! terminal is gone" into two distinguishable answers (PLAN_M3.md item
//! 2) — the agent exited
//! (with the code, when something still holds it) versus the host rebooted
//! and took every terminal with it, which is **interrupted**. The
//! classification precedence lives on `status::session_status`, the
//! recording rules on `Supervisor::record_outcome`/`record_stop`, and the
//! boot comparison on `Supervisor::reload_sessions`.
//!
//! Every session carries an integration snapshot and, for the two
//! integrated kinds, a captured conversation identity (PLAN_M3.md items 7
//! and 8; the per-kind knowledge itself lives in `crate::agent_kind`).
//! What this module owns is the plumbing around it: resolving the snapshot
//! during create validation, recording the first-input timestamp capture
//! correlates on, and running the rescan that claims an identity. The
//! rescan deliberately has no watcher thread and no inotify — it is a
//! POLLED pass (see `core::capture_pass` for the cost envelope and why
//! polling is sufficient here), driven from two cadences that answer
//! different questions: the `ticker` task, which guarantees progress with
//! nobody connected, and the `ListSessions`, restart, and reload passes,
//! which guarantee a reply is fresh as of the request it answers. Which
//! kind a given call is, and what each is allowed to skip, is
//! `Supervisor::capture_pass_for`'s single scheduling rule; `ticker`'s own
//! module doc owns the argument for why both cadences exist and what
//! running both actually costs.
//!
//! A data channel has a second meaning beyond terminal bytes: attachment
//! bytes flowing client-toward-supervisor (PLAN_M4.md item 4). Each accepted
//! `BeginUpload` gets a task that owns the transfer end to end — staging,
//! credit acks, the progress timeout, and the commit that publishes —
//! because every one of a transfer's endings (a client disconnecting, a
//! session being deleted, a sender that simply stops) happens where no
//! request handler is present to clean up. The storage half (layout,
//! naming, the delete and startup sweeps) lives in `crate::attachments`
//! and the write atomicity in `crate::files`; what belongs here is the
//! protocol, the lifecycle, and the serialization against delete.
//!
//! This directory is the M4.5 split of what used to be one ~20k-line
//! `service.rs` (a functional no-op — see PLAN.md's milestone ladder): each
//! submodule below owns one cohesive slice, `core` holds the `Supervisor`
//! struct, its central lifecycle impl, and the state every other slice
//! reads and writes, and this file is a re-export shell so nothing outside
//! `service` has to know the split happened. A submodule may EXTEND
//! `Supervisor` with an impl block of its own where the operation is
//! genuinely a method on it (`teardown`'s `teardown_session` is the one
//! that does today); `core` owns the type and the common lifecycle, not a
//! monopoly on its methods.
//!
//! Three of those slices were carved out of `core` and `handlers` later,
//! on the same functional-no-op terms, once each had grown into a contract
//! of its own: `status` (how a session's liveness, restart offer, and
//! reply-shaped `SessionInfo` are derived), `listing` (the `ListSessions`
//! order, cursor, page cuts, and the walk itself), and `teardown` (the
//! whole of what `DeleteSession` does, minus the reply). Each owns a
//! sequence or a precedence whose parts have to agree with each other; see
//! their own module docs for what each one is holding together.

mod connection;
mod core;
mod handlers;
mod launch_artifacts;
mod listing;
mod snapshots;
mod status;
mod sweep;
mod teardown;
mod terminals;
mod ticker;
mod uploads;

pub use connection::handle_connection;
#[cfg(test)]
pub(crate) use core::recovered_archive_flag;
pub use core::{
    ArchiveGate, ArchiveStage, BootIdSource, CaptureStoreFault, CaptureWrite, CreateCrashSeam,
    CreateStage, STALL_DETACH_TIMEOUT, SampleFault, SampleRead, SessionSnapshot, StateDirOwnership,
    Supervisor, SupervisorSeams, SupervisorTimeouts, TabOpenFault, TabOpenStage,
    UPLOAD_DISK_STAGE_TIMEOUT, UPLOAD_PROGRESS_TIMEOUT, WRITER_STALL_TIMEOUT, connect, run,
};
pub use listing::LIST_SESSION_CAP;
