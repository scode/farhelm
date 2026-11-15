//! The status-sound rules: one observation of the fleet in, at most one sound out.
//!
//! SPEC.md, "Status", the sounds paragraph, ported from the old app's `detectEvents` and `chooseSound`
//! (`assets/sounds.js`) and the history its Rust side kept between reads (`History`, `StatusScope` in `sounds.rs`).
//!
//! Here's the TLDR of the decision. An observation is compared with the one before it and yields events: every
//! approval id this UI session has never seen, every visible session that is now waiting and was not (including one
//! that was not visible before), and every visible session that went from running to idle. Of those, only the most
//! urgent kind whose switch is on and which has at least one event not about the quiet session plays. Everything else
//! is consumed silently: an observation always becomes the baseline for the next, whether or not it played, so turning
//! a switch on or moving the window's focus never replays an earlier event.
//!
//! Three things make an observation silent for statuses while approvals are still judged:
//!
//! - The first observation of a UI session is the snapshot the window started or reset with, and records everything
//!   it holds without a sound, approvals included.
//! - A change of the sidebar's view (its host selector or status filter) records the new view's statuses without a
//!   sound, because sessions entering the view were not seen changing. Approval requests are not filtered by the view
//!   (SPEC.md: "because their cards are always shown"), so a new one in the same observation still rings.
//! - A session that disappears raises nothing; only status edges of visible sessions do.
//!
//! The owner of a [`SoundRules`] keeps one per UI session and drops it on a reset, so the next connection's snapshot is
//! a fresh, silent first observation. Seen approval ids live as long as the [`SoundRules`] does.

use std::collections::{BTreeMap, HashSet};

use farhelm_proto::SessionStatus;
use farhelm_proto::ui_channel::Preferences;

use super::Sound;

// ---- The observation ------------------------------------------------------------------------------------------------

/// A session's status, reduced to what can raise a sound.
///
/// Only edges into Waiting, and from Running to Idle, ring, so every other status (unknown, exited, failed,
/// interrupted) is one [`Status::Other`]. That is the old `status_words` reduction: exit codes, error details and the
/// rest of a row cannot manufacture a transition, and a change between two such statuses is not an edge at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    Waiting,
    Idle,
    /// Any status that is not live-and-classified: unknown, exited, failed or interrupted.
    Other,
}

impl From<&SessionStatus> for Status {
    /// Listed out rather than caught by a wildcard so a status added to the protocol has to be placed here on purpose.
    fn from(status: &SessionStatus) -> Status {
        match status {
            SessionStatus::Running => Status::Running,
            SessionStatus::Waiting => Status::Waiting,
            SessionStatus::Idle => Status::Idle,
            SessionStatus::Unknown
            | SessionStatus::Exited { .. }
            | SessionStatus::Error { .. }
            | SessionStatus::Interrupted => Status::Other,
        }
    }
}

/// One pending approval request, as far as the rules care: its id and the session that asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Approval {
    /// The helm's request id, unique for the helm process's life. Once seen, it never rings again in this UI session.
    pub id: String,
    /// The id of the session that asked, so the quiet session's own request stays quiet.
    pub session: String,
}

/// The three sound switches, one per [`Sound`]: shared helm preferences, which the rules' owner reads when it observes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switches {
    pub approval: bool,
    pub waiting: bool,
    pub turn_finished: bool,
}

impl Switches {
    /// Whether `sound`'s switch is on.
    pub fn enabled(self, sound: Sound) -> bool {
        match sound {
            Sound::Approval => self.approval,
            Sound::Waiting => self.waiting,
            Sound::TurnFinished => self.turn_finished,
        }
    }

    /// The switches as the helm's shared preference row stores them, each unset field reading as its default
    /// ([`Switches::default`]); `None` (no row pushed) is all defaults.
    ///
    /// The one place an unset field is read, so the owner that plays and the Settings switches that show the choice can
    /// never disagree about a helm that never stored one.
    pub fn from_preferences(preferences: Option<&Preferences>) -> Switches {
        let defaults = Switches::default();
        let stored = |field: fn(&Preferences) -> Option<bool>, default: bool| {
            preferences.and_then(field).unwrap_or(default)
        };
        Switches {
            approval: stored(|row| row.sound_approval, defaults.approval),
            waiting: stored(|row| row.sound_waiting, defaults.waiting),
            turn_finished: stored(|row| row.sound_turn_finished, defaults.turn_finished),
        }
    }
}

impl Default for Switches {
    /// SPEC.md's defaults, which an unset preference means: waiting and approval on, turn finished off.
    fn default() -> Switches {
        Switches {
            approval: true,
            waiting: true,
            turn_finished: false,
        }
    }
}

/// Everything one decision needs, built by the rules' owner from the store at the moment it observes.
///
/// The rules read nothing else, so this is also the whole contract with that owner. Session ids are the raw ids the
/// helm sends, never a display spelling, so two sessions cannot merge into one history entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    /// Names the view the statuses below answer: the sidebar's host selector and status filter. A value different from
    /// the previous observation's says the view changed, so the statuses are recorded without being judged. The owner
    /// moves it on every observation while it cannot tell which view the listing answers (a filter change not yet
    /// answered, a sessions collection the helm could not read), and once more when that is known again, so the first
    /// observation of the new view's listing is recorded rather than judged against the old view's.
    ///
    /// It is a counter rather than the query itself so that leaving a view and coming back between two observations
    /// still counts as a change: the old app's `StatusScope` kept a generation for the same reason, because statuses
    /// remembered from the first visit to a view say nothing about what happened while it was not shown.
    pub view: u64,
    /// The visible sessions' statuses, keyed by session id: exactly the sessions the sidebar's current query lists.
    pub statuses: BTreeMap<String, Status>,
    /// Every pending approval request, whatever the view shows.
    pub approvals: Vec<Approval>,
    /// The session that stays quiet: the open session while its window is active, or `None`. Its waiting, its turn
    /// finishing and its own approval requests do not ring (they are still consumed).
    pub quiet: Option<String>,
    /// The switches as they are now. A disabled event is consumed like a played one.
    pub switches: Switches,
}

// ---- The rules ------------------------------------------------------------------------------------------------------

/// The history one UI session's decisions need: what the previous observation saw, and every approval id ever seen.
///
/// Approval ids are the UI session's whole seen history, not the requests still pending: a request that leaves the
/// list and is presented again (a later listing repeating it) must not ring a second time.
#[derive(Debug, Default)]
pub struct SoundRules {
    /// The previous observation's view and statuses; `None` until the first observation, which is silent.
    previous: Option<(u64, BTreeMap<String, Status>)>,
    /// Every approval id observed in this UI session, pending or not.
    seen_approvals: HashSet<String>,
}

/// One status or approval change that may raise a sound, and the session it concerns (for the quiet check).
struct Event<'a> {
    sound: Sound,
    session: &'a str,
}

impl SoundRules {
    /// Rules with no history: the next observation is a first observation and makes no sound.
    pub fn new() -> SoundRules {
        SoundRules::default()
    }

    /// Records `current` as the baseline for the next observation and returns the one sound it calls for, if any.
    ///
    /// The sound is the most urgent kind ([`Sound::PRIORITY`]) that is enabled and has at least one event not about
    /// the quiet session. A quiet or disabled event never hides a less urgent audible one, and however many sessions
    /// change at once, one observation plays at most one sound.
    pub fn observe(&mut self, current: Observation) -> Option<Sound> {
        let sound = self.previous.as_ref().and_then(|(view, statuses)| {
            // A changed view has no status history to compare against: its statuses are recorded, not judged.
            let statuses = (*view == current.view).then_some(statuses);
            let events = self.events(statuses, &current);
            choose(&events, &current)
        });
        self.seen_approvals
            .extend(current.approvals.into_iter().map(|approval| approval.id));
        self.previous = Some((current.view, current.statuses));
        sound
    }

    /// The events between `previous` statuses (`None` when the view changed) and `current`.
    ///
    /// A session absent from `previous` counts as not waiting and not running, so a newly visible session rings only
    /// when it is already waiting: idle alone is no evidence this client saw a turn finish.
    fn events<'a>(
        &self,
        previous: Option<&BTreeMap<String, Status>>,
        current: &'a Observation,
    ) -> Vec<Event<'a>> {
        let approvals = current
            .approvals
            .iter()
            .filter(|approval| !self.seen_approvals.contains(&approval.id))
            .map(|approval| Event {
                sound: Sound::Approval,
                session: &approval.session,
            });
        let statuses = previous.into_iter().flat_map(|previous| {
            current
                .statuses
                .iter()
                .filter_map(move |(session, &status)| {
                    let before = previous.get(session).copied();
                    let sound = match status {
                        Status::Waiting if before != Some(Status::Waiting) => Sound::Waiting,
                        Status::Idle if before == Some(Status::Running) => Sound::TurnFinished,
                        _ => return None,
                    };
                    Some(Event { sound, session })
                })
        });
        approvals.chain(statuses).collect()
    }
}

/// The most urgent enabled sound with an event outside the quiet session, or `None`.
fn choose(events: &[Event<'_>], current: &Observation) -> Option<Sound> {
    Sound::PRIORITY.into_iter().find(|&sound| {
        current.switches.enabled(sound)
            && events.iter().any(|event| {
                event.sound == sound && current.quiet.as_deref() != Some(event.session)
            })
    })
}

// ---- Tests ----------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! The rules' acceptance cases, ported from the old app's `e2e/tests/sounds.spec.ts` at the reference commit (its
    //! status, view, quiet and first-read cases) and from the rules it exercised in `sounds.js`. The old spec's touch
    //! and Web Audio cases are playback, which is not here; its "initial status baseline does not wait for the first
    //! approval reply" case answered approvals and sessions arriving in separate reads, which the pushed snapshot does
    //! not do, so it has no counterpart.

    use super::*;
    /// An observation in view 0 with the default switches, nothing quiet, and the given statuses and approvals.
    fn obs(statuses: &[(&str, Status)], approvals: &[(&str, &str)]) -> Observation {
        Observation {
            view: 0,
            statuses: statuses
                .iter()
                .map(|(id, status)| ((*id).to_owned(), *status))
                .collect(),
            approvals: approvals
                .iter()
                .map(|(id, session)| Approval {
                    id: (*id).to_owned(),
                    session: (*session).to_owned(),
                })
                .collect(),
            quiet: None,
            switches: Switches::default(),
        }
    }

    /// Rules that have already observed `first`, so the next observation is judged.
    fn after(first: Observation) -> SoundRules {
        let mut rules = SoundRules::new();
        assert_eq!(
            rules.observe(first),
            None,
            "premise: the first observation is silent"
        );
        rules
    }

    /// Every switch on, so tests about priority and quiet are not shaped by the turn-finished default.
    fn all_on() -> Switches {
        Switches {
            approval: true,
            waiting: true,
            turn_finished: true,
        }
    }

    use Status::{Idle, Other, Running, Waiting};

    /// SPEC.md: "the state loaded when the window starts or resets makes no sound". A window that opens onto a waiting
    /// session and a pending approval must not ring for either, and must still ring for an approval that arrives later.
    #[test]
    fn first_observation_records_existing_attention_silently() {
        let mut rules = SoundRules::new();
        let first = obs(&[("s", Waiting)], &[("already", "s")]);
        assert_eq!(rules.observe(first), None);
        let later = obs(&[("s", Waiting)], &[("already", "s"), ("new", "s")]);
        assert_eq!(rules.observe(later), Some(Sound::Approval));
    }

    /// Specifies the status edges: into Waiting from anything else rings, Running to Idle rings turn finished, and
    /// staying put, Waiting to Idle, and Other to Idle ring nothing. These are the whole status vocabulary of sounds.
    #[test]
    fn status_edges() {
        let switches = all_on();
        let cases = [
            (Running, Waiting, Some(Sound::Waiting)),
            (Idle, Waiting, Some(Sound::Waiting)),
            (Other, Waiting, Some(Sound::Waiting)),
            (Waiting, Waiting, None),
            (Running, Idle, Some(Sound::TurnFinished)),
            (Waiting, Idle, None),
            (Other, Idle, None),
            (Idle, Idle, None),
            (Idle, Running, None),
            (Running, Other, None),
        ];
        for (before, now, expected) in cases {
            let mut rules = after(Observation {
                switches,
                ..obs(&[("s", before)], &[])
            });
            let sound = rules.observe(Observation {
                switches,
                ..obs(&[("s", now)], &[])
            });
            assert_eq!(sound, expected, "{before:?} -> {now:?}");
        }
    }

    /// A session that appears (created, or matching a status filter now) has no witnessed history: it rings only when
    /// already waiting, never turn finished for arriving idle, and a session that disappears rings nothing.
    #[test]
    fn appearing_and_disappearing_sessions() {
        let switches = all_on();
        let mut rules = after(Observation {
            switches,
            ..obs(&[("gone", Waiting)], &[])
        });
        let idle = Observation {
            switches,
            ..obs(&[("new", Idle)], &[])
        };
        assert_eq!(rules.observe(idle), None);
        let waiting = Observation {
            switches,
            ..obs(&[("new", Idle), ("newer", Waiting)], &[])
        };
        assert_eq!(rules.observe(waiting), Some(Sound::Waiting));
    }

    /// Approval ids are remembered for the UI session, not only while pending: a request that leaves the list and is
    /// presented again must not ring twice, which a comparison against the previous observation alone would miss.
    #[test]
    fn a_seen_approval_never_rings_again() {
        let mut rules = after(obs(&[], &[]));
        assert_eq!(
            rules.observe(obs(&[], &[("a", "s")])),
            Some(Sound::Approval)
        );
        assert_eq!(rules.observe(obs(&[], &[("a", "s")])), None);
        assert_eq!(rules.observe(obs(&[], &[])), None);
        assert_eq!(rules.observe(obs(&[], &[("a", "s")])), None);
    }

    /// SPEC.md: new approval requests sound regardless of the sidebar's filters. A request from a session the view
    /// does not list still rings.
    #[test]
    fn approvals_ring_for_sessions_outside_the_view() {
        let mut rules = after(obs(&[("visible", Running)], &[]));
        let ask = obs(&[("visible", Running)], &[("a", "hidden")]);
        assert_eq!(rules.observe(ask), Some(Sound::Approval));
    }

    /// SPEC.md: changing the sidebar's view silently records its existing statuses, but approvals are not filtered.
    /// Ported from the old spec's status-segment and host-selector cases: a newly visible waiting session is quiet on
    /// the change, an approval in the same observation still rings, and later edges inside the new view ring again.
    #[test]
    fn a_view_change_records_statuses_silently_but_judges_approvals() {
        let mut rules = after(obs(&[("s", Running)], &[]));
        let changed = Observation {
            view: 1,
            ..obs(&[("other", Waiting)], &[])
        };
        assert_eq!(
            rules.observe(changed),
            None,
            "newly visible waiting session"
        );

        let with_ask = Observation {
            view: 2,
            ..obs(&[("s", Waiting)], &[("a", "s")])
        };
        assert_eq!(rules.observe(with_ask), Some(Sound::Approval));

        let running = Observation {
            view: 2,
            ..obs(&[("s", Running)], &[("a", "s")])
        };
        assert_eq!(rules.observe(running), None);
        let waiting = Observation {
            view: 2,
            ..obs(&[("s", Waiting)], &[("a", "s")])
        };
        assert_eq!(rules.observe(waiting), Some(Sound::Waiting));
    }

    /// SPEC.md: the open session stays quiet while its window is active. Its waiting and its own approval request
    /// are quiet, and once another session is the quiet one, the first session's next edge rings. Ported from the old
    /// spec's "active open session is quiet" case.
    #[test]
    fn the_quiet_session_stays_quiet() {
        let mut rules = after(obs(&[("s", Running), ("o", Running)], &[]));
        let quiet = Observation {
            quiet: Some("s".to_owned()),
            ..obs(&[("s", Waiting), ("o", Running)], &[("a", "s")])
        };
        assert_eq!(rules.observe(quiet), None);

        let moved = |status| Observation {
            quiet: Some("o".to_owned()),
            ..obs(&[("s", status), ("o", Running)], &[("a", "s")])
        };
        assert_eq!(rules.observe(moved(Running)), None);
        assert_eq!(rules.observe(moved(Waiting)), Some(Sound::Waiting));
    }

    /// SPEC.md: each observation plays only its most urgent enabled, non-quiet event. Specifies the priority, and
    /// that a quiet or disabled event never hides a less urgent audible one (the old `chooseSound`).
    #[test]
    fn priority_skips_quiet_and_disabled_events() {
        let start = || {
            after(Observation {
                switches: all_on(),
                ..obs(&[("w", Running), ("f", Running)], &[])
            })
        };
        let everything = |quiet: Option<&str>, switches| Observation {
            quiet: quiet.map(str::to_owned),
            switches,
            ..obs(&[("w", Waiting), ("f", Idle)], &[("a", "q")])
        };

        assert_eq!(
            start().observe(everything(None, all_on())),
            Some(Sound::Approval)
        );
        assert_eq!(
            start().observe(everything(Some("q"), all_on())),
            Some(Sound::Waiting)
        );
        let no_approval = Switches {
            approval: false,
            ..all_on()
        };
        assert_eq!(
            start().observe(everything(None, no_approval)),
            Some(Sound::Waiting)
        );
        let only_finished = Switches {
            approval: false,
            waiting: false,
            turn_finished: true,
        };
        assert_eq!(
            start().observe(everything(None, only_finished)),
            Some(Sound::TurnFinished)
        );
        assert_eq!(
            start().observe(everything(None, Switches::default())),
            Some(Sound::Approval)
        );
        let none = Switches {
            approval: false,
            waiting: false,
            turn_finished: false,
        };
        assert_eq!(start().observe(everything(None, none)), None);
    }

    /// The defaults are SPEC.md's: waiting and approval on, turn finished off. A running-to-idle edge under the
    /// default switches is therefore silent.
    #[test]
    fn turn_finished_is_off_by_default() {
        assert_eq!(
            Switches::default(),
            Switches {
                approval: true,
                waiting: true,
                turn_finished: false,
            }
        );
        let mut rules = after(obs(&[("s", Running)], &[]));
        assert_eq!(rules.observe(obs(&[("s", Idle)], &[])), None);
    }

    /// The switches are the helm's shared preference row, and a helm that never stored a choice (every helm at the
    /// switch to the new app) must get SPEC.md's defaults rather than everything off. Specified: no row and an empty
    /// row read as the defaults; each stored field wins over its default, in both directions.
    #[test]
    fn the_preference_row_reads_unset_fields_as_the_defaults() {
        assert_eq!(Switches::from_preferences(None), Switches::default());
        assert_eq!(
            Switches::from_preferences(Some(&Preferences::default())),
            Switches::default()
        );
        let flipped = Preferences {
            sound_approval: Some(false),
            sound_waiting: Some(false),
            sound_turn_finished: Some(true),
            ..Preferences::default()
        };
        assert_eq!(
            Switches::from_preferences(Some(&flipped)),
            Switches {
                approval: false,
                waiting: false,
                turn_finished: true,
            }
        );
    }

    /// A disabled or quiet event is consumed like a played one (the old "silent playback refusal still consumes the
    /// transition"): turning the switch on, or the session stopping being quiet, must not replay it.
    #[test]
    fn suppressed_events_are_consumed() {
        let off = Switches {
            waiting: false,
            ..Switches::default()
        };
        let mut rules = after(obs(&[("s", Running)], &[]));
        let disabled = Observation {
            switches: off,
            ..obs(&[("s", Waiting)], &[])
        };
        assert_eq!(rules.observe(disabled), None);
        assert_eq!(rules.observe(obs(&[("s", Waiting)], &[])), None);

        let mut rules = after(obs(&[], &[]));
        let quiet = Observation {
            quiet: Some("s".to_owned()),
            ..obs(&[], &[("a", "s")])
        };
        assert_eq!(rules.observe(quiet), None);
        assert_eq!(rules.observe(obs(&[], &[("a", "s")])), None);
    }

    /// One observation consumes every event it raises, played or not: twenty sessions starting to wait at once ring
    /// once, and the same observation repeated rings nothing, so a busy fleet can neither stack sounds nor replay one.
    #[test]
    fn many_events_play_once_and_are_all_consumed() {
        let ids: Vec<String> = (0..20).map(|n| format!("s{n}")).collect();
        let all = |status| Observation {
            statuses: ids.iter().map(|id| (id.clone(), status)).collect(),
            ..obs(&[], &[])
        };
        let mut rules = after(all(Running));
        assert_eq!(rules.observe(all(Waiting)), Some(Sound::Waiting));
        assert_eq!(rules.observe(all(Waiting)), None);
    }

    /// The protocol's statuses reduce to the three that can ring and one for everything else, so an exit code or
    /// error detail can never look like a waiting or idle session.
    #[test]
    fn protocol_statuses_reduce_to_sound_words() {
        assert_eq!(Status::from(&SessionStatus::Running), Running);
        assert_eq!(Status::from(&SessionStatus::Waiting), Waiting);
        assert_eq!(Status::from(&SessionStatus::Idle), Idle);
        for other in [
            SessionStatus::Unknown,
            SessionStatus::Exited { exit_code: Some(0) },
            SessionStatus::Error {
                detail: "x".to_owned(),
            },
            SessionStatus::Interrupted,
        ] {
            assert_eq!(Status::from(&other), Other, "{other:?}");
        }
    }
}
