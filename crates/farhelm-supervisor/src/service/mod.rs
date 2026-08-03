//! The supervisor service: farhelm-proto over a unix socket.
//!
//! This is the only doorway to sessions — the helm (local or via the ssh
//! stdio proxy) and every future caller speak the same protocol to the
//! same handlers, which is what keeps "CLI flags bypass the creation UI,
//! never the creation API" true. The supervisor listens on no network
//! port (SPEC.md): the unix socket plus ssh exec is the entire reachable
//! surface.
//!
//! M2 scope: SQLite (`crate::store`) is the truth that a session exists
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
//! M3 adds the half M2 could not answer (PLAN_M3.md item 2): a durable
//! last-known outcome per session, written wherever this process actually
//! WITNESSES a transition, plus the host's boot id. Together they turn "the
//! terminal is gone" into two distinguishable answers — the agent exited
//! (with the code, when something still holds it) versus the host rebooted
//! and took every terminal with it, which is **interrupted**. The
//! classification precedence lives on `core::session_status`, the recording
//! rules on `Supervisor::record_outcome`/`record_stop`, and the boot
//! comparison on `Supervisor::reload_sessions`.
//!
//! M3 also gives every session an integration snapshot and, for the two
//! integrated kinds, a captured conversation identity (PLAN_M3.md items 7
//! and 8; the per-kind knowledge itself lives in `crate::agent_kind`).
//! What this module owns is the plumbing around it: resolving the snapshot
//! during create validation, recording the first-input timestamp capture
//! correlates on, and running the rescan that claims an identity. The
//! rescan deliberately has no watcher thread and no inotify: it rides the
//! `ListSessions` and reload passes this supervisor already performs (see
//! `core::capture_pass` for the cost envelope and why polling is sufficient
//! here), so nothing new has to be started, supervised, or torn down.
//!
//! M4 gives a data channel a second meaning: attachment bytes flowing
//! client-toward-supervisor (PLAN_M4.md item 4). Each accepted
//! `BeginUpload` gets a task that owns the transfer end to end — staging,
//! credit acks, the progress timeout, and the commit that publishes —
//! because every one of a transfer's endings (a client disconnecting, a
//! session being deleted, a sender that simply stops) happens where no
//! request handler is present to clean up. The storage half (layout,
//! naming, the delete and startup sweeps) lives in `crate::attachments`
//! and the write atomicity in `crate::files`; what belongs here is the
//! protocol, the lifecycle, and the serialization against delete.
//!
//! M4.5 splits what used to be one ~20k-line `service.rs` into this
//! directory (functional no-op — see PLAN.md's milestone ladder): each
//! submodule below owns one cohesive slice, `core` holds the `Supervisor`
//! struct/impl and the session-status/list-building logic that reads its
//! state back out, and this file is a re-export shell so nothing outside
//! `service` has to know the split happened.

mod connection;
mod core;
mod handlers;
mod launch_artifacts;
mod snapshots;
mod sweep;
mod terminals;
mod uploads;

pub use connection::handle_connection;
pub use core::{
    BootIdSource, CaptureStoreFault, CaptureWrite, CreateCrashSeam, CreateStage, LIST_SESSION_CAP,
    STALL_DETACH_TIMEOUT, SessionSnapshot, StateDirOwnership, Supervisor, SupervisorSeams,
    SupervisorTimeouts, TabOpenFault, TabOpenStage, UPLOAD_DISK_STAGE_TIMEOUT,
    UPLOAD_PROGRESS_TIMEOUT, WRITER_STALL_TIMEOUT, connect, run,
};
