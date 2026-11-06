//! What the lifecycle verbs learn about the process behind a terminal, in
//! terms both kinds of terminal answer in.
//!
//! Stop, Restart, Delete (and its consent guard), prompt delivery and a
//! tab's reap all ask one question of a session's terminal before they act:
//! is its process running, dead, gone, or (for tmux) no longer this
//! session's? A tmux pane answers it from tmux (`TmuxDriver::pane_process`),
//! a terminal host from its runtime (`crate::hosts::HostStatus::probe`), and
//! the callers read one answer whichever kind it came from, so these types
//! belong to neither. They used to be the tmux module's `PaneProcess` and
//! `PaneProbe`, which made a host's state travel in tmux types; the names
//! changed with the move, the meaning did not.
//!
//! [`TabScope`] is the same kind of shared answer for a terminal tab: whether
//! its shell was launched in a cgroup scope of its own, which a tab's close
//! needs whichever kind the tab is. An old tmux tab records it as a window
//! option, a host tab in its host's client tag.

/// A terminal's root process as stop and delete need it: the OS pid
/// `kill_process_tree` reaps from, and whether the terminal has already seen
/// that process exit.
///
/// Anything that signals, walks or anchors a kill reads the pid through
/// [`Self::live_pid`], never the raw field: the raw field may be a stale
/// number left over from a process that is long gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalProcess {
    /// The terminal's ORIGINAL child: for tmux the one `new-session` or
    /// `split-window` forked (a shell, or the exec'd argv when tmux runs it
    /// directly), for a host the child the host forked; not whatever the
    /// agent turns into after its own exec chain. Every process the agent
    /// starts keeps that pid as an ancestor, which is exactly the property a
    /// tree-kill needs.
    ///
    /// `None` only for a dead terminal that reported no pid. tmux 3.8
    /// stopped expanding `#{pane_pid}` once it has closed the pane's
    /// terminal, which every dead pane has; tmux 3.7c, the pinned build,
    /// always expands it. Assuming it was always there is what made every
    /// restart of an exited agent fail on tmux 3.8 with "cannot parse
    /// integer from empty string". A terminal that is NOT dead always has a
    /// pid here: the tmux parser refuses the other 3.8 shape (terminal
    /// closed, exit status not yet collected) rather than letting a live
    /// pane through with nothing to kill.
    ///
    /// When `dead` is `true` and this is `Some`, it is the LAST pid the
    /// terminal remembers: stale, and it must never be walked or signaled,
    /// since a dead terminal is kept around (tmux's `remain-on-exit`, a
    /// host's final screen) after that process and everything under it is
    /// long gone, and the number may already be recycled to something
    /// unrelated by the time anyone reads it.
    pub pid: Option<u32>,
    /// Set once the process backing this terminal has exited (tmux's own
    /// `#{pane_dead}` flag, a host's recorded exit). A dead terminal means
    /// nothing is running in it: stop and delete both treat this as "no
    /// process tree to touch" rather than attempting to signal the stale
    /// `pid` above.
    pub dead: bool,
}

impl TerminalProcess {
    /// The pid a kill or sweep may root at: the terminal's process while it
    /// is not dead, and `None` for a dead terminal, whose pid (when one is
    /// still reported) is stale.
    ///
    /// Whether the agent is running is still decided by `dead`, so stop,
    /// restart and delete pick the same path on every tmux version; this
    /// only keeps a dead terminal's leftover number away from code that
    /// signals or walks processes.
    pub fn live_pid(&self) -> Option<u32> {
        if self.dead { None } else { self.pid }
    }
}

/// What a recorded terminal is now: still this session's, gone, or
/// demonstrably somebody else's.
///
/// A terminal host is never [`TerminalProbe::ForeignOwner`]: its owner is
/// its tag, not a name that could be recycled, and a host that cannot be
/// read is an error, which every caller treats as "could not tell". The
/// third case is a tmux one. tmux pane ids (`%N`) come from a server-wide
/// counter that is monotonic only WITHIN one server's lifetime: it restarts
/// at `%0` with the server. Several unrelated things therefore produce a
/// foreign owner, and the list is not closed: the recorded id may predate
/// the current server generation and have been handed to a session that
/// started later (the 2026-08-16 incident: the private tmux server
/// segfaulted out from under a running supervisor, whose in-memory
/// `Terminal` rows kept pointing at the dead server's `%0`); the pane's own
/// session may have been renamed out-of-band, so the pane is still ours but
/// no longer answers to the recorded name; or panes may have been rearranged
/// across sessions (`move-pane`/`break-pane`). Nothing in farhelm does the
/// latter two on its private server, but the socket is reachable by whoever
/// owns the account.
///
/// Those causes do NOT share a safety profile, which is why this type
/// deliberately stops at "the recorded name is not what tmux reports" and
/// leaves the verdict to callers. A recycled id means the old terminal
/// provably died with its server; a rename or a move may mean the recorded
/// pane is still a LIVE agent of ours under another name, and tearing it
/// down would kill an agent without consent. The discriminator — is `owner`
/// the tmux name of another session this supervisor knows? — and the
/// reasoning behind it live at `Supervisor::known_session_tmux_name`, which
/// every lifecycle call site consults.
///
/// Modelled as an outcome rather than an error because a foreign owner is
/// not a failure to ask tmux anything. It is a decisive answer about the
/// NAME, and each caller decides what that means for its own operation: the
/// lifecycle verbs proceed on other kill mechanisms for a recycled id and
/// refuse otherwise, while a pane this process created moments ago being
/// foreign is an invariant violation under any cause. What NO caller may do
/// is read the foreign pane's pid: it anchors kills and descendant sweeps,
/// and pointing those at a stranger is precisely the cross-wiring the
/// session scoping exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalProbe {
    /// The terminal exists and is this session's (for tmux, the pane's
    /// `#{session_name}` matches the session it was recorded under; for a
    /// host, its runtime reads it): its [`TerminalProcess::live_pid`], when
    /// there is one, may anchor a kill.
    Owned(TerminalProcess),
    /// The terminal resolves to nothing: for tmux an unassigned pane id, a
    /// session killed out from under us, or no server at all; for a host, a
    /// host that has ended. Callers read this as "nothing is running", the
    /// same as a positively dead terminal.
    Gone,
    /// The tmux pane exists but `#{session_name}` is `owner`, not the name
    /// it was recorded under. `owner` is the COMPLETE name including any
    /// spaces, because callers compare it against their own session names
    /// to tell a recycled id from a rename (see the type's docs); a
    /// truncated name would silently fail that comparison. The pid is
    /// deliberately not carried, because no caller has any business using
    /// it.
    ForeignOwner { owner: String },
}

/// Whether a terminal tab's shell was launched in a cgroup scope of its own,
/// as its close needs to know: a close treats a scope it cannot confirm gone
/// as a failure exactly when the tab really had one.
///
/// An old tmux tab answers from its window's scope option
/// (`crate::tmux::TAB_SCOPED_WINDOW_OPTION`, read back by
/// `TmuxDriver::tab_scope_marker`). A host tab answers from its host's tag
/// (`HostRole::Tab { scoped }`); a startup placeholder whose tag could not be
/// read answers [`TabScope::Unscoped`], not [`TabScope::Unknown`], so a host
/// tab's `Unscoped` is not always a real answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabScope {
    /// The tab's shell was launched in its own cgroup scope.
    Scoped,
    /// The tab's shell was launched without a scope.
    Unscoped,
    /// No usable answer, which only an old tmux tab gives today: its window
    /// predates the scope option, holds a value this supervisor never
    /// writes, or is no longer the pane and tab the caller asked about. The
    /// caller has to decide from other evidence.
    Unknown,
}
