//! What this process knows about the private tmux server: whether one runs
//! at all, and which tmux sessions it holds.
//!
//! The private server exists only for old terminals, and a supervisor whose
//! sessions are all new terminals must run no tmux server and issue no tmux
//! command at all (SPEC_impl.md, "Terminal substrate: private tmux server").
//! Old tmux tabs are recorded nowhere but in tmux itself, so every place that
//! lists a session's tabs, probes its liveness or tears it down used to ask
//! tmux, for every session. This model is what lets those places stop asking
//! without a durable record of their own: the server's existence is the
//! signal. A supervisor probes the socket once at startup
//! ([`super::TmuxDriver::adopt_running_server`]; startup now ends the server
//! instead and sets no server, `super::TmuxDriver::no_server_after_startup`). No server means no old
//! terminals this run, and the driver then answers every query as tmux
//! answers for an absent server without spawning anything. A running server
//! is adopted, and its session names become the set of tmux sessions this
//! process may ask about. From then on the set only shrinks (a listing that
//! no longer shows a name, a kill, a server that went away) and grows only
//! when this process itself creates a tmux session, which the supervisor no
//! longer does (only the driver's own tests create sessions).
//!
//! An empty set means no server: the configuration sets `exit-empty on`, so
//! tmux ends the server with its last session (verified against tmux 3.7c,
//! including that a pending `wait-for` client does not keep it alive: tmux
//! releases the waiter, successfully, as the server exits).
//!
//! NOTE: this is knowledge, not a lock. It can be stale in the direction
//! that costs one command: a server that died since the last observation is
//! still believed to run until a command reports it gone. It is never stale
//! in the other direction while this process is the only one creating tmux
//! sessions on the socket, which the state directory's claim makes true.

use std::collections::{HashMap, HashSet};

/// The shared presence model behind every clone of one [`super::TmuxDriver`].
#[derive(Debug)]
pub(super) struct ServerPresence {
    state: std::sync::Mutex<State>,
    /// Whether a server may run (anything but [`Knowledge::Absent`]), for a
    /// waiter that must not spawn tmux while none does
    /// ([`ServerPresence::wait_until_may_run`]).
    may_run: tokio::sync::watch::Sender<bool>,
}

#[derive(Debug)]
struct State {
    knowledge: Knowledge,
    /// Bumped by every finished creation, so that a listing can tell a
    /// session it should have seen from one created while it ran (see
    /// [`ServerPresence::listed`]).
    epoch: u64,
}

/// Where a known tmux session's creation stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Noted {
    /// Its `new-session` is running. Nothing can show yet whether it will
    /// exist: a listing may run before it does, and a command racing it may
    /// find no server because this very create has not started one yet.
    /// Neither may drop it.
    Creating,
    /// Known to exist, or finished trying to, at this epoch (0 for the
    /// sessions found at adoption). A listing that began later and does
    /// not show it proves it gone.
    Settled(u64),
}

#[derive(Debug)]
enum Knowledge {
    /// No claim either way: every command asks the socket, exactly as the
    /// driver did before this model existed. Drivers built by tests and
    /// tools start here; a supervisor leaves it at startup.
    Unprobed,
    /// No server runs. Nothing is asked until this process creates a tmux
    /// session.
    Absent,
    /// A server answers, but this process cannot drive it (the tmux binary
    /// is missing or below the floor, or the server is), for this reason.
    /// Queries are answered as for no server, so its sessions read as not
    /// alive; but the agents in it may well be running, so the questions
    /// that decide whether to discard one (does its session exist, kill it,
    /// create beside it) are refused rather than answered "absent"
    /// (`TmuxDriver::refuse_if_unusable`). Kept for the life of the process.
    Unusable(String),
    /// A server runs, or is being started, holding these sessions.
    Running(HashMap<String, Noted>),
}

impl ServerPresence {
    /// A model that claims nothing ([`Knowledge::Unprobed`]).
    pub(super) fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(State {
                knowledge: Knowledge::Unprobed,
                epoch: 0,
            }),
            may_run: tokio::sync::watch::Sender::new(true),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("tmux presence mutex poisoned")
    }

    /// Publish the knowledge's "may a server run" bit to waiters. Called
    /// with the state locked, so two transitions cannot publish out of
    /// order.
    fn publish(&self, state: &State) {
        let may_run = !matches!(state.knowledge, Knowledge::Absent | Knowledge::Unusable(_));
        self.may_run.send_if_modified(|current| {
            let changed = *current != may_run;
            *current = may_run;
            changed
        });
    }

    /// Whether this process will not ask the server anything: none runs,
    /// or the one that does cannot be used. Only then may the driver answer
    /// without asking.
    pub(super) fn known_absent(&self) -> bool {
        matches!(
            self.lock().knowledge,
            Knowledge::Absent | Knowledge::Unusable(_)
        )
    }

    /// Why the server that answers cannot be used, when that is the case.
    pub(super) fn unusable_reason(&self) -> Option<String> {
        match &self.lock().knowledge {
            Knowledge::Unusable(reason) => Some(reason.clone()),
            _ => None,
        }
    }

    /// Whether this process knows the tmux session `name` does not exist:
    /// no server runs, or the one it adopted or started does not hold it.
    /// Unlike `!may_hold`, false for an unusable server, whose sessions are
    /// unknown rather than absent.
    pub(super) fn knows_absent(&self, name: &str) -> bool {
        match &self.lock().knowledge {
            Knowledge::Absent => true,
            Knowledge::Running(sessions) => !sessions.contains_key(name),
            Knowledge::Unprobed | Knowledge::Unusable(_) => false,
        }
    }

    /// Whether this process knows a server runs (it adopted or started one
    /// and has not seen it go).
    #[cfg(test)]
    pub(super) fn known_running(&self) -> bool {
        matches!(self.lock().knowledge, Knowledge::Running(_))
    }

    /// Whether the tmux session `name` may exist: false only when this
    /// process knows it does not (no server, or a server without it).
    pub(super) fn may_hold(&self, name: &str) -> bool {
        match &self.lock().knowledge {
            Knowledge::Unprobed => true,
            Knowledge::Absent | Knowledge::Unusable(_) => false,
            Knowledge::Running(sessions) => sessions.contains_key(name),
        }
    }

    /// Claim nothing again ([`Knowledge::Unprobed`]); tests only, see
    /// `TmuxDriver::forget_presence_for_test`.
    #[cfg(test)]
    pub(super) fn forget(&self) {
        let mut state = self.lock();
        state.knowledge = Knowledge::Unprobed;
        self.publish(&state);
    }

    /// The startup probe found a server this process cannot use, for
    /// `reason`.
    pub(super) fn set_unusable(&self, reason: String) {
        let mut state = self.lock();
        state.knowledge = Knowledge::Unusable(reason);
        self.publish(&state);
    }

    /// The startup probe found no server.
    pub(super) fn set_absent(&self) {
        let mut state = self.lock();
        state.knowledge = Knowledge::Absent;
        self.publish(&state);
    }

    /// The startup probe adopted a server holding `names`. An empty list is
    /// a server about to exit (adoption turned `exit-empty` on), so it reads
    /// as none.
    pub(super) fn adopted(&self, names: impl IntoIterator<Item = String>) {
        let sessions: HashMap<String, Noted> = names
            .into_iter()
            .map(|name| (name, Noted::Settled(0)))
            .collect();
        let mut state = self.lock();
        state.knowledge = if sessions.is_empty() {
            Knowledge::Absent
        } else {
            Knowledge::Running(sessions)
        };
        self.publish(&state);
    }

    /// This process is about to run `new-session` for `name`, which may
    /// start a server. From now until [`Self::create_finished`], a server
    /// may run whatever any other command reports. An unprobed model stays
    /// unprobed: it makes no claims.
    pub(super) fn create_started(&self, name: &str) {
        let mut state = self.lock();
        match &mut state.knowledge {
            // A create refuses first while the server is unusable.
            Knowledge::Unprobed | Knowledge::Unusable(_) => {}
            Knowledge::Running(sessions) => {
                sessions.insert(name.to_owned(), Noted::Creating);
            }
            Knowledge::Absent => {
                state.knowledge =
                    Knowledge::Running(HashMap::from([(name.to_owned(), Noted::Creating)]));
            }
        }
        self.publish(&state);
    }

    /// The `new-session` for `name` returned, whether or not it succeeded:
    /// a failed one may still have created the session, so the name stays
    /// until a listing that began after now does not show it.
    pub(super) fn create_finished(&self, name: &str) {
        let mut state = self.lock();
        state.epoch += 1;
        let epoch = state.epoch;
        if let Knowledge::Running(sessions) = &mut state.knowledge {
            sessions.insert(name.to_owned(), Noted::Settled(epoch));
        }
        self.publish(&state);
    }

    /// The tmux session `name` is gone (this process killed it).
    pub(super) fn ended(&self, name: &str) {
        let mut state = self.lock();
        if let Knowledge::Running(sessions) = &mut state.knowledge {
            sessions.remove(name);
            if sessions.is_empty() {
                state.knowledge = Knowledge::Absent;
            }
        }
        self.publish(&state);
    }

    /// Where a listing starts, for [`Self::listed`].
    pub(super) fn listing_started(&self) -> u64 {
        self.lock().epoch
    }

    /// A complete listing that began at `started` showed exactly the
    /// sessions in `present` (empty for an empty or absent server). Every
    /// known session noted no later than `started` that it did not show has
    /// ended; one created after the listing began is kept, since the
    /// listing may have run before it existed.
    pub(super) fn listed(&self, started: u64, present: &HashSet<&str>) {
        let mut state = self.lock();
        if let Knowledge::Running(sessions) = &mut state.knowledge {
            sessions.retain(|name, noted| match noted {
                Noted::Creating => true,
                Noted::Settled(at) => *at > started || present.contains(name.as_str()),
            });
            if sessions.is_empty() {
                state.knowledge = Knowledge::Absent;
            }
        }
        self.publish(&state);
    }

    /// A command that began at `started` ([`Self::listing_started`])
    /// reported that no server runs on the socket: every session settled
    /// no later than `started` is gone. A creation in progress keeps the
    /// model running, since its `new-session` may not have started the
    /// server yet when the command asked, and so does one that finished
    /// after the command began, whose server the command may have missed.
    /// An unprobed model stays unprobed.
    pub(super) fn no_server(&self, started: u64) {
        let mut state = self.lock();
        if let Knowledge::Running(sessions) = &mut state.knowledge {
            sessions.retain(|_, noted| match noted {
                Noted::Creating => true,
                Noted::Settled(at) => *at > started,
            });
            if sessions.is_empty() {
                state.knowledge = Knowledge::Absent;
            }
        }
        self.publish(&state);
    }

    /// Wait until a server may run. Returns at once unless this process
    /// knows none does; then it waits for this process to create one, which
    /// for a supervisor without the test-only switch is forever.
    pub(super) async fn wait_until_may_run(&self) {
        let mut may_run = self.may_run.subscribe();
        // The sender lives as long as `self`, which outlives this borrow.
        let _ = may_run.wait_for(|may_run| *may_run).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(present: &[&'static str]) -> HashSet<&'static str> {
        present.iter().copied().collect()
    }

    /// An unprobed model claims nothing, whatever it is told.
    ///
    /// Why: drivers built by tests and tools never run the startup probe,
    /// and they must keep asking tmux exactly as before; only a supervisor
    /// that probed may answer without asking. Spec: unprobed, every name may
    /// be held and a server may run, and neither a creation, a kill, a
    /// listing nor a "no server" report changes that.
    #[farhelm_testtrace::test]
    fn an_unprobed_model_claims_nothing() {
        let presence = ServerPresence::new();
        presence.create_started("fh-a");
        presence.create_finished("fh-a");
        presence.ended("fh-a");
        presence.listed(presence.listing_started(), &names(&[]));
        presence.no_server(presence.listing_started());
        assert!(!presence.known_absent());
        assert!(!presence.known_running());
        assert!(presence.may_hold("fh-anything"));
    }

    /// An adopted server's sessions are the only ones that may exist, and
    /// the set shrinks to nothing, which reads as no server.
    ///
    /// Why: this is what keeps host sessions from asking tmux about tabs
    /// they never had while old sessions remain, and what stops all tmux
    /// traffic once the last old session goes (tmux ends the server then).
    /// Spec: after adopting `fh-a` and `fh-b`, only those may be held; a
    /// listing without `fh-a` drops it; killing `fh-b` leaves none, and the
    /// model then knows no server runs. Adopting an empty server is the same
    /// as finding none.
    #[farhelm_testtrace::test]
    fn the_adopted_set_only_shrinks_and_empty_means_no_server() {
        let presence = ServerPresence::new();
        presence.adopted(["fh-a".to_owned(), "fh-b".to_owned()]);
        assert!(presence.known_running());
        assert!(presence.may_hold("fh-a") && presence.may_hold("fh-b"));
        assert!(!presence.may_hold("fh-host"));

        presence.listed(presence.listing_started(), &names(&["fh-b", "foreign"]));
        assert!(
            !presence.may_hold("fh-a"),
            "a listing that lacks it ends it"
        );
        assert!(
            !presence.may_hold("foreign"),
            "a listing never adds a name: only this process's creates do"
        );
        presence.ended("fh-b");
        assert!(presence.known_absent());

        let empty = ServerPresence::new();
        empty.adopted(Vec::new());
        assert!(empty.known_absent());
    }

    /// A session being created, or created while a listing ran, survives
    /// that listing and a racing "no server" report.
    ///
    /// Why: `list-panes` can run before a concurrent `new-session` lands,
    /// and the first old terminal's `new-session` is what starts the
    /// server, so a command racing it sees no server (observed under a real
    /// supervisor process, whose ticker and listings run beside a create:
    /// the create's next command was then refused). Dropping the name would
    /// make the driver refuse the create's own follow-up commands and its
    /// session stop asking tmux while its agent runs there. Spec: while a
    /// creation is in progress, neither a listing nor a "no server" report
    /// drops its name or makes the model absent; a name whose creation
    /// finished after `listing_started` is kept by a listing that does not
    /// show it, and one that finished before is dropped.
    #[farhelm_testtrace::test]
    fn a_creation_during_a_listing_is_kept() {
        let presence = ServerPresence::new();
        presence.set_absent();
        presence.create_started("fh-starting");
        presence.no_server(presence.listing_started());
        presence.listed(presence.listing_started(), &names(&[]));
        assert!(presence.may_hold("fh-starting") && !presence.known_absent());
        presence.create_finished("fh-starting");

        let started = presence.listing_started();
        presence.create_started("fh-new");
        presence.create_finished("fh-new");
        presence.listed(started, &names(&[]));
        assert!(presence.may_hold("fh-new"));
        assert!(!presence.may_hold("fh-starting"));
    }

    /// A "no server" report ends what a running model knows, and only a
    /// creation brings a server back.
    ///
    /// Why: once the server has gone, every later query would only spawn
    /// tmux to hear the same answer, and the pane-death waiter would spin;
    /// the test-only tmux switch is the one way a new server starts. Spec:
    /// running, a report of no server makes the model absent and a waiter
    /// waits; a creation makes it running again and releases the waiter.
    #[farhelm_testtrace::test]
    async fn no_server_stops_everything_until_a_creation() {
        let presence = std::sync::Arc::new(ServerPresence::new());
        presence.adopted(["fh-a".to_owned()]);
        presence.no_server(presence.listing_started());
        assert!(presence.known_absent());
        let waiter = tokio::spawn({
            let presence = std::sync::Arc::clone(&presence);
            async move { presence.wait_until_may_run().await }
        });
        tokio::task::yield_now().await;
        assert!(
            !waiter.is_finished(),
            "nothing may run, so the waiter waits"
        );
        presence.create_started("fh-b");
        tokio::time::timeout(std::time::Duration::from_secs(10), waiter)
            .await
            .expect("a creation releases the waiter")
            .expect("the waiter task");
        assert!(presence.may_hold("fh-b"));
    }

    /// A "no server" answer to a command that began before a create
    /// finished does not drop that create's session.
    ///
    /// Why: the model can believe a server runs after it died unseen (a
    /// last tab's window closing ends its session without a listing), and a
    /// command spawned then answers "no server" while a create's
    /// `new-session` starts a fresh one. Dropping the new session would make
    /// the driver refuse everything about a live old terminal until the
    /// next create. Spec: with `fh-old` adopted, a "no server" answer to a
    /// command that began before `fh-new`'s create finished drops `fh-old`,
    /// keeps `fh-new`, and leaves the model running.
    #[farhelm_testtrace::test]
    fn a_stale_no_server_answer_keeps_a_later_creation() {
        let presence = ServerPresence::new();
        presence.adopted(["fh-old".to_owned()]);
        let started = presence.listing_started();
        presence.create_started("fh-new");
        presence.create_finished("fh-new");
        presence.no_server(started);
        assert!(presence.known_running());
        assert!(presence.may_hold("fh-new"));
        assert!(!presence.may_hold("fh-old"));
    }

    /// An unusable server is never asked, yet its sessions are unknown, not
    /// absent.
    ///
    /// Why: its old agents may still run, so Delete and Restart must not
    /// read "not there" from it and discard or duplicate them; listings
    /// still read it as empty. Spec: after `set_unusable`, the model will
    /// not ask (`known_absent`), holds no session for gating purposes, does
    /// not know any session absent, reports its reason, and a pane-death
    /// waiter waits.
    #[farhelm_testtrace::test]
    async fn an_unusable_server_is_not_asked_and_not_known_empty() {
        let presence = ServerPresence::new();
        presence.set_unusable("tmux 3.4 is below the floor".to_owned());
        assert!(presence.known_absent());
        assert!(!presence.may_hold("fh-old"));
        assert!(!presence.knows_absent("fh-old"));
        assert_eq!(
            presence.unusable_reason().as_deref(),
            Some("tmux 3.4 is below the floor")
        );
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(50),
                presence.wait_until_may_run()
            )
            .await
            .is_err(),
            "nothing may be asked, so a waiter waits"
        );
    }
}
