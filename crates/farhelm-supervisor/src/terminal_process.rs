//! What the lifecycle verbs learn about the process behind a terminal.
//!
//! Stop, Restart, Delete (and its consent guard), prompt delivery and a
//! tab's reap all ask one question of a session's terminal before they act:
//! is its process running, dead, or gone? A terminal host's runtime answers
//! it (`crate::hosts::HostStatus::probe`), and the callers read the answer
//! in these terms rather than the runtime's own, so that each decides what
//! the answer means for its own operation.

/// What a recorded terminal's process is now.
///
/// A host that cannot be read is none of these: the probe answers with an
/// error, which every caller treats as "could not tell" and refuses on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalProbe {
    /// The terminal's ORIGINAL child runs, with this pid: the child the host
    /// forked, not whatever the agent turns into after its own exec chain.
    /// Every process the agent starts keeps that pid as an ancestor, which
    /// is exactly the property a tree-kill needs. A caller that kills or
    /// sweeps from it takes the root with its pinned start identity
    /// (`crate::hosts::HostTerminal::sweep_root`), never an identity read
    /// from the bare pid again.
    Running(u32),
    /// The terminal exists but its child has exited (reaped, or the host's
    /// exit recorded). No pid is reported: once the child is reaped its pid
    /// may already be recycled to something unrelated. Stop and Delete read
    /// this as "no process tree to touch".
    Dead,
    /// The terminal resolves to nothing: a host that has ended. Callers read
    /// this as "nothing is running", the same as a dead terminal.
    Gone,
}

impl TerminalProbe {
    /// The pid a kill or sweep may root at: the child's while it runs, and
    /// `None` for a dead or gone terminal.
    pub fn live_pid(&self) -> Option<u32> {
        match self {
            TerminalProbe::Running(pid) => Some(*pid),
            TerminalProbe::Dead | TerminalProbe::Gone => None,
        }
    }
}
