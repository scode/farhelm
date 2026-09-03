//! One surface, one reader: the single-flight, retry-until-committed
//! discipline every feed-driven read on this page runs under.
//!
//! ## The problem this exists for
//!
//! The invalidation feed (`feed`) says only "something changed", and the
//! answer to that is a re-read. That leaves two failures the trigger itself
//! cannot solve, and both are silent:
//!
//! - **A spent notice.** A notification is consumed the moment a read is
//!   STARTED, not when one lands. If that read fails, the notice is gone,
//!   the feed is healthy (so the fallback poll is off by
//!   `feed::fallback_polls`), and nothing is left to ask again — the surface
//!   stays wrong until the fleet happens to change a second time. On a quiet
//!   fleet that is forever, and the page gives no sign of it.
//! - **Unbounded reads.** A notification per mutation and a fallback tick
//!   every few seconds, each spawning its own walk, means a helm that has
//!   stopped answering accumulates tasks and sockets for as long as the page
//!   is open. The reads are also unordered among themselves, so the surface
//!   is at the mercy of whichever walk happens to finish last.
//!
//! ## The discipline
//!
//! Demand is a FLAG, not a task. A notice records demand;
//! [`SurfaceReader::request`] starts a reader only if one is not already
//! running, so every notice arriving mid-read coalesces into exactly one
//! follow-up read rather than into N concurrent ones. A read that does not
//! come back with an answer leaves the demand standing and the reader
//! schedules another attempt itself, which is what makes a failed read
//! recoverable without a second notification.
//!
//! ## Where a read comes from decides two things about it
//!
//! Every [`Trigger`] answers two questions, and conflating either with the
//! other has already produced product bugs:
//!
//! - **Does it carry news?** A feed notice and a user's live filter edit do:
//!   something changed, or the user asked for something different, and the
//!   answer is owed now. A fallback tick does NOT — it is a clock going off,
//!   with no more information than the last one had. Neither does the
//!   reader's own [`Demand::Retry`], which knows only that the last attempt
//!   failed.
//! - **Is anyone watching?** A live filter edit, a host mutation's refresh and
//!   a mount are ATTENDED — a person did something and is waiting for the
//!   result. A feed notice, a fallback tick and a retry are unattended:
//!   nobody asked, the page is keeping itself current on its own.
//!
//! The first question decides whether a demand waits out a backoff. News
//! never does. A delivered notice is not just new information, it is
//! EVIDENCE THE TRANSPORT IS BACK — the socket that carried it is alive
//! right now — so making it inherit a dead period's backoff leaves a page
//! sitting on news it has already received for up to a probe interval. A
//! fallback tick carries no such evidence, and must not cancel a backoff
//! either: at a three-second cadence it would flatten the whole ladder into
//! a three-second poll against a helm that is down, which is precisely the
//! hammering the ladder exists to prevent. So a tick may START an idle
//! reader and otherwise does nothing at all.
//!
//! The second question decides what happens under a latched build mismatch.
//! SPEC_impl.md's withdrawal rule revokes UNATTENDED behavior — the page
//! must stop keeping itself current against a helm whose vocabulary it does
//! not share — while explicit user actions keep working, because refusing to
//! answer a person who just clicked something is a broken page rather than a
//! safe one. So under skew a live filter edit still reads and the feed, the
//! fallback and the retry ladder all stand down.
//!
//! The retry cadence is `reconnect`'s ladder and probe interval, reused
//! rather than re-tuned for the reason `feed` reuses them: a read that
//! cannot reach the helm and a socket that cannot reach the helm are the
//! same outage, and two cadences would be two things to keep in step. The
//! active window is bounded (~30s of doubling), and past it the reader
//! settles into low-frequency probing forever. Unbounded in COUNT is
//! deliberate: a reader that gave up would recreate exactly the permanent
//! staleness this module exists to remove. What is bounded is what matters —
//! the RATE at which this page retries on its own, and the number of reads
//! in flight, which is one.
//!
//! ## What the reader is NOT
//!
//! It is not an ordering mechanism. Which of several completed reads may be
//! believed is `ops::ReadGate`'s question, and it stays that way: reads
//! still overlap this reader (a mutation's own immediate refetch, the
//! session view's host-state follow-up), so the gate remains the single
//! answer to "is this reply still current". The reader bounds how many reads
//! this page STARTS; the gate decides which ones count.
//!
//! It is also not a poll, and the fallback poll is not one of its triggers
//! in the way the others are. That poll still exists and still runs only
//! while the feed is unhealthy; what it does here is ASK, weakly — see
//! [`Trigger::Scheduled`] — instead of spawning a walk of its own.

use dioxus::core::Task;
use dioxus::prelude::*;

use crate::reconnect::{PROBE_INTERVAL_MS, RETRY_LADDER_MS};
use crate::skew;

/// Why a caller wants a read, as the call sites say it.
///
/// The three are exactly the answers to the module header's two questions,
/// and they are named for the SITUATION rather than for the behavior so a
/// call site cannot quietly pick the convenient one: "this is a fallback
/// tick" is a fact, while "this should not cancel the backoff" is a
/// conclusion someone might disagree with in the moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// A person did something and is waiting: a live filter edit, a host or
    /// profile mutation's follow-up, a mount, a restart's refresh. Carries
    /// news, and keeps working under a latched build mismatch.
    Explicit,
    /// A feed notification. Carries news — the fleet changed and the socket
    /// that said so is demonstrably alive — but nobody is watching, so it
    /// stands down under skew.
    Notice,
    /// The fallback poll's tick. No news and nobody watching: it may start
    /// an idle reader and must disturb nothing else.
    Scheduled,
}

/// What, if anything, this surface is owed — and on whose authority.
///
/// Ordered by strength, and the ordering is the rule: a stronger demand
/// landing on a weaker one replaces it and is never downgraded back (see
/// [`SurfaceReader::request`] and [`SurfaceReader::finish`]). The order is
/// not arbitrary — it falls out of the two questions in the module header,
/// with "carries news" outranking "waits" and "attended" outranking
/// everything, because a demand that loses its attended standing would be
/// withdrawn under skew after a person asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Demand {
    /// Nothing is owed: the last dispatched read answered and nothing has
    /// happened since.
    #[default]
    None,
    /// A fallback tick that found the surface idle. Weakest of all: it is
    /// only ever seen by the dispatch that immediately consumes it, and it
    /// exists as a value at all so that dispatch knows the read is
    /// unattended and must stand down under skew.
    Scheduled,
    /// The last read failed and nothing new has been heard since. The
    /// reader's own business, and the only demand that waits out a backoff.
    Retry,
    /// A feed notification: news, so it never waits — but unattended, so it
    /// is withdrawn under a latched build mismatch.
    Notice,
    /// A person asked. Never waits, and never withdrawn: SPEC_impl.md's
    /// withdrawal rule is about unattended behavior, and a page that stopped
    /// answering its own controls would be broken rather than careful.
    Explicit,
}

impl Demand {
    /// Rank, for the "a stronger demand wins" rule. Spelled out rather than
    /// derived from `PartialOrd` so that adding a variant forces a decision
    /// about where it sits instead of inheriting one from declaration order.
    fn strength(self) -> u8 {
        match self {
            Demand::None => 0,
            Demand::Scheduled => 1,
            Demand::Retry => 2,
            Demand::Notice => 3,
            Demand::Explicit => 4,
        }
    }

    /// Whether a person is waiting on this read — the one thing that
    /// survives the build-skew withdrawal.
    fn attended(self) -> bool {
        matches!(self, Demand::Explicit)
    }

    /// Whether this demand waits out the retry ladder. Only the reader's own
    /// blind retry does; see the module header for why a fallback tick must
    /// not (it would flatten the ladder into a poll).
    fn waits(self) -> bool {
        matches!(self, Demand::Retry)
    }
}

impl From<Trigger> for Demand {
    fn from(trigger: Trigger) -> Self {
        match trigger {
            Trigger::Explicit => Demand::Explicit,
            Trigger::Notice => Demand::Notice,
            Trigger::Scheduled => Demand::Scheduled,
        }
    }
}

/// A reader's task handle, as the state machine needs to use one: something
/// it can hold and later cancel.
///
/// The trait exists so the CANCELLATION lives inside
/// [`SurfaceReader::claim`] — the same function production calls — rather
/// than in the Dioxus-facing driver where no unit test can reach it. A
/// `Task` cannot be constructed outside a running runtime, so a rule
/// enforced out there is a rule asserted nowhere; behind this seam the tests
/// drive the real logic with a handle of their own and can say exactly what
/// was cancelled and when.
pub(crate) trait ReaderTask: Copy {
    /// Stop this task. Called exactly once per superseded sleeper.
    fn cancel_task(self);
}

impl ReaderTask for Task {
    fn cancel_task(self) {
        self.cancel();
    }
}

/// One surface's read state: whether a read is running, what it is owed, and
/// how many attempts have failed in a row.
///
/// A value type with the transitions on it, rather than signals a caller
/// wires together, so the rules that matter can be stated and tested in one
/// place without a Dioxus runtime: at most one read runs at a time, demand
/// stands until a read ANSWERS it, and only the reader's own retries wait.
///
/// Generic over the task handle purely for testability (see [`ReaderTask`]);
/// production always uses the default, and the parameter is invisible at
/// every call site because of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SurfaceReader<T: ReaderTask = Task> {
    /// Whether a read is in flight (or about to be).
    ///
    /// Deliberately FALSE during a retry wait, which is what lets an
    /// explicit trigger claim the surface and read at once instead of
    /// inheriting the outage's rung. The reader that stepped aside for the
    /// wait comes back holding a ticket and finds itself superseded — see
    /// [`SurfaceReader::pause`] and [`SurfaceReader::resume`].
    running: bool,
    /// What the surface is owed, and by whom.
    ///
    /// Cleared by [`SurfaceReader::begin`], which is to say at the instant a
    /// read is actually DISPATCHED for it. It is therefore "demand not yet
    /// dispatched" rather than "the surface is stale" — the read in flight
    /// is what covers the gap between the two.
    ///
    /// Clearing it at dispatch rather than at scheduling is what makes the
    /// coalescing exact: a trigger arriving during a read owes a FURTHER
    /// read, because the one in flight had already started when the trigger
    /// landed and cannot be said to have answered it.
    demand: Demand,
    /// Consecutive reads that failed to produce an answer, which is what
    /// picks the retry rung. Reset by any answer, so an intermittent helm
    /// does not inherit an earlier outage's backoff.
    failures: u32,
    /// How many retry waits this surface has entered, ever.
    ///
    /// The ticket a paused reader carries across its wait. Comparing it on
    /// the way back is what makes stepping aside safe: if anything has
    /// claimed the surface in the meantime, the woken reader is holding a
    /// stale ticket and retires rather than starting a second read beside
    /// the one that superseded it.
    ///
    /// Kept even though a superseded sleeper is also CANCELLED outright
    /// ([`SurfaceReader::claim`]), and the redundancy is deliberate rather
    /// than leftover. The two answer different questions and only one of
    /// them can be answered here: cancellation bounds the RESOURCE and
    /// depends on what the runtime does with a cancelled task, while the
    /// ticket keeps "one read at a time" true as a property of this type
    /// alone. If a future Dioxus made cancellation lazy — a cancelled task
    /// polled once more before being dropped, say — the ticket is the
    /// difference between a bounded leak and two concurrent walks writing
    /// the same surface. A guarantee that costs one `u64` and one
    /// comparison, and that a test can hold this type to on its own terms,
    /// is worth keeping even where today's runtime makes it unreachable.
    waits: u64,
    /// The live reader's task, so a sleeper can be cancelled rather than
    /// left to wake up and find itself obsolete.
    ///
    /// Recorded by [`SurfaceReader::attach`] the moment a reader is spawned
    /// and cancelled by the claim that supersedes it. `None` between
    /// readers, and deliberately cleared when a paused reader is resumed or
    /// retires: a handle is only worth keeping while there is something to
    /// cancel.
    reader: Option<T>,
}

/// Hand-written rather than derived: a derived `Default` would demand
/// `T: Default` of the task handle, and a task id has no meaningful zero —
/// the field it lives in is an `Option` precisely because "no reader yet" is
/// its own state.
impl<T: ReaderTask> Default for SurfaceReader<T> {
    fn default() -> Self {
        SurfaceReader {
            running: false,
            demand: Demand::None,
            failures: 0,
            waits: 0,
            reader: None,
        }
    }
}

/// What the reader should do once a read pass is over.
///
/// An enum rather than an `Option<u64>` because the two outcomes are
/// decisions, not a value and its absence: a caller reading `None` as "no
/// delay" instead of "stop" would spin the reader forever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Next {
    /// Nothing more is wanted; the reader ends and the surface is clean.
    Idle,
    /// Read again after this many milliseconds. Zero means the demand is a
    /// FRESH one that landed while the last read was in flight, so the
    /// follow-up is owed immediately whatever the backoff would have said —
    /// a coalesced trigger, not a retry.
    Again(u64),
}

impl<T: ReaderTask> SurfaceReader<T> {
    /// Record a demand and, if the surface is free, claim it for a new
    /// reader — cancelling the sleeper that was holding it, if any.
    ///
    /// The whole claim is one operation because its two halves are one
    /// decision: the only moment a reader may start is the moment the
    /// previous one stops mattering, and the sleeper it displaces must go
    /// with it. Calling [`Self::request`] without the cancellation is what a
    /// caller would eventually do by accident, and the cost is a live timer
    /// per superseded rung — so the accident is made impossible instead of
    /// documented against.
    ///
    /// Cancels EXACTLY the handle recorded for the reader being superseded,
    /// and only when this call is what supersedes it: a claim refused
    /// because a read is already running never touches that read's handle.
    pub(crate) fn claim(&mut self, trigger: Trigger) -> bool {
        if !self.request(trigger) {
            return false;
        }
        // The paused reader this claim just stepped over. `None` when the
        // previous reader retired cleanly (nothing to cancel) or when this
        // is the surface's first read.
        if let Some(superseded) = self.reader.take() {
            superseded.cancel_task();
        }
        true
    }

    /// Record a demand for a read; the answer is whether the CALLER must
    /// start the reader loop.
    ///
    /// The test-and-set is the single-flight rule in one operation, for
    /// `ops::OpLock`'s reason: it is called synchronously from a handler, an
    /// effect or a timer tick, with no await and no render able to
    /// interleave between the check and the claim. A caller told `false` has
    /// still been heard — its demand is recorded and the running reader will
    /// honor it.
    ///
    /// Two rules, both from the module header:
    ///
    /// - A [`Trigger::Scheduled`] tick acts ONLY on a surface that is idle
    ///   with nothing owed. It must not upgrade a standing retry (that would
    ///   cancel the backoff and turn the ladder into a three-second poll),
    ///   and it must not queue a follow-up behind a running read (that would
    ///   make a slow helm produce back-to-back walks forever). A tick that
    ///   finds work already in progress has nothing to add: the read that is
    ///   happening is the refresh it wanted.
    /// - Anything else records the stronger of what stands and what arrived,
    ///   so an attended demand is never downgraded into one the skew rule
    ///   would withdraw.
    ///
    /// A call during a retry WAIT finds `running` false — the waiting reader
    /// stepped aside, see [`Self::pause`] — and therefore dispatches at
    /// once, which is the point of stepping aside.
    fn request(&mut self, trigger: Trigger) -> bool {
        let arriving = Demand::from(trigger);
        if arriving == Demand::Scheduled && (self.running || self.demand != Demand::None) {
            return false;
        }
        if arriving.strength() > self.demand.strength() {
            self.demand = arriving;
        }
        if self.running {
            return false;
        }
        self.running = true;
        true
    }

    /// Record the reader task that was just spawned for this surface, so the
    /// next claim can cancel it if it ends up asleep.
    pub(crate) fn attach(&mut self, task: T) {
        self.reader = Some(task);
    }

    /// A read is being dispatched right now, satisfying whatever demand
    /// stands — or, under a latched build mismatch, the point at which an
    /// UNATTENDED reader gives up instead.
    ///
    /// The withdrawal check lives here rather than in the driver so the
    /// whole rule is one testable transition: `withdrawn` is passed in
    /// rather than read from `skew`, which is what lets SPEC_impl.md's
    /// "explicit actions keep working" be asserted without a Dioxus runtime
    /// or a global. The demand is left standing when the reader stands down,
    /// because the surface really is unread and saying otherwise in the
    /// state would be a lie a later reader could act on.
    fn begin(&mut self, withdrawn: bool) -> bool {
        if withdrawn && !self.demand.attended() {
            self.running = false;
            self.reader = None;
            return false;
        }
        self.demand = Demand::None;
        true
    }

    /// One read pass finished; decide what the reader does next.
    ///
    /// `answered` means THE DEMAND WAS DISCHARGED, which is a stronger claim
    /// than "a reply arrived" and a weaker one than "something was painted".
    /// The three rejections a commit path can make do not agree with each
    /// other, so the callers have to tell them apart:
    ///
    /// - **Refused as older** (`ops::ReadGate`): discharged. The gate only
    ///   refuses a read that a NEWER read has already committed, so the
    ///   surface holds a fresher answer than this reply carried.
    /// - **Refused for describing a stale filter** (`list::accepts_listing`):
    ///   discharged. The submit that changed the filter recorded its own
    ///   demand in the same instant, so a read for the applied filter is
    ///   already owed and reporting a failure here would only make it wait
    ///   out a backoff.
    /// - **Discarded by a restart epoch** (`session_view::admit_detail`):
    ///   NOT discharged, and this is the one an earlier version got wrong.
    ///   Nothing was applied, no newer read is guaranteed to be coming, and
    ///   the notification that prompted this one is already spent — so
    ///   reporting it as answered leaves the view describing a run that has
    ///   ended, with nothing scheduled to correct it.
    ///
    /// What `false` means is that: no answer this surface can use arrived,
    /// so it is still showing whatever it showed before and somebody has to
    /// ask again.
    ///
    /// A failure records a [`Demand::Retry`] only where nothing stronger
    /// stands. A trigger that landed while this read was in flight keeps its
    /// standing and is dispatched at once even though the read it overlapped
    /// failed — the failure is a fact about the request that just ended, not
    /// about the news that arrived during it.
    pub(crate) fn finish(&mut self, answered: bool) -> Next {
        if answered {
            self.failures = 0;
        } else {
            self.failures = self.failures.saturating_add(1);
            if Demand::Retry.strength() > self.demand.strength() {
                self.demand = Demand::Retry;
            }
        }
        if self.demand == Demand::None {
            self.running = false;
            self.reader = None;
            return Next::Idle;
        }
        Next::Again(if self.demand.waits() {
            retry_delay_ms(self.failures)
        } else {
            0
        })
    }

    /// Step aside for a retry's backoff, taking a ticket to come back with.
    ///
    /// Releasing the claim across the wait is the whole point rather than an
    /// implementation detail: a reader that held it would make every
    /// explicit trigger queue behind the outage's rung, so a feed notice
    /// arriving one second into a thirty-second probe wait would take
    /// twenty-nine seconds to produce a read — a page sitting on news it has
    /// already received, over an outage the notice itself proves is over.
    ///
    /// What the release costs is that the surface is momentarily
    /// unclaimed, which is what [`Self::resume`] exists to settle.
    fn pause(&mut self) -> u64 {
        self.running = false;
        self.waits += 1;
        self.waits
    }

    /// A backoff elapsed: reclaim the surface, or report that this reader
    /// has been superseded and should retire.
    ///
    /// Three ways to lose, and all three mean the same thing — somebody else
    /// is taking care of it: another reader is running, another wait has
    /// been entered since this one began (so this ticket is stale), or the
    /// demand has since been answered and nothing is owed. Refusing in all
    /// three is what keeps "one read at a time" true across the gap
    /// [`Self::pause`] opens.
    fn resume(&mut self, ticket: u64) -> bool {
        if self.running || self.waits != ticket || self.demand == Demand::None {
            return false;
        }
        self.running = true;
        true
    }
}

/// How long before retry number `failures` (one-based).
///
/// Two regimes, `reconnect`'s: the ladder covers the outage that fixes
/// itself, and everything past it is unbounded low-frequency probing, so a
/// helm that comes back overnight is re-read without anyone watching.
///
/// Consulted for [`Demand::Retry`] only, and that demand is recorded by the
/// same statement that increments the count — so `failures` is one or more
/// here by construction, and the debug assertion says so rather than a
/// branch pretending otherwise. A zero-argument answer would be dead code
/// whose only effect is to make a future caller think this handles a case it
/// does not.
fn retry_delay_ms(failures: u32) -> u64 {
    debug_assert!(
        failures > 0,
        "a retry delay is only ever asked for after a failure"
    );
    match RETRY_LADDER_MS.get(failures.saturating_sub(1) as usize) {
        Some(rung) => *rung as u64,
        None => PROBE_INTERVAL_MS as u64,
    }
}

/// Sleep, on whichever renderer this is.
///
/// A per-target `cfg` pair rather than a call: `tokio::time::sleep` is
/// unavailable on wasm32 (there is no reactor in the browser) while
/// `gloo-timers`' `TimeoutFuture` only works there. The desktop build runs
/// inside the tokio multi-thread runtime `dioxus-desktop` constructs for
/// itself, so the native half needs no setup of its own.
pub(crate) async fn sleep_ms(millis: u64) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::TimeoutFuture::new(millis as u32).await;
    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
}

/// Ask `state`'s surface for a read, starting the reader if it is idle.
///
/// `read` performs ONE read and reports whether the helm answered (see
/// [`SurfaceReader::finish`] for what that does and does not claim). It is
/// called again by the reader itself, so it must sample everything that
/// describes a request — the generation, the filter, the read-order index —
/// at each call rather than capturing one request's values once.
///
/// The task belongs to the CALLING component's scope, which is what makes
/// "only the mounted page reads" a lifecycle property: navigating away drops
/// the reader mid-wait along with everything else the page owns. That is
/// also why this is a free function taking a signal rather than a hook —
/// every trigger (mount, feed notice, fallback tick, live filter edit) calls it
/// from a different place in the same component.
///
/// A news-carrying trigger that lands while a retry is BACKING OFF reads
/// immediately rather than waiting the rung out: the waiting reader has
/// released its claim, so this one claims the surface, CANCELS the sleeper
/// and dispatches. The two therefore never read at once, and a trigger never
/// pays for an outage it has already disproved.
///
/// Cancelling matters as much as claiming. Leaving the superseded sleeper to
/// wake up and notice its ticket is stale is correct but not bounded: a page
/// taking a notification every few seconds against a failing helm would
/// stack one live timer per superseded rung, each holding up to a probe
/// interval. The ticket stays as the state machine's own guarantee — it is
/// what makes "one read at a time" provable in a unit test rather than a
/// property of the runtime's cancellation semantics.
///
/// Under a latched build mismatch only ATTENDED reads proceed
/// ([`SurfaceReader::begin`]). The feed, the fallback and the retry ladder
/// stand down — that is SPEC_impl.md's withdrawal rule — while a filter
/// submit or a mutation's refresh still reads, because the page must keep
/// answering the person using it.
pub(crate) fn request_read<F, Fut>(mut state: Signal<SurfaceReader>, trigger: Trigger, mut read: F)
where
    F: FnMut() -> Fut + 'static,
    Fut: std::future::Future<Output = bool> + 'static,
{
    // Claims the surface AND cancels whatever sleeper it displaced — one
    // call, because they are one decision (see `SurfaceReader::claim`).
    if !state.write().claim(trigger) {
        return;
    }
    let task = spawn(async move {
        loop {
            if !state.write().begin(skew::build_skew_detected_now()) {
                return;
            }
            let answered = read().await;
            let delay = match state.write().finish(answered) {
                Next::Idle => return,
                Next::Again(delay) => delay,
            };
            if delay == 0 {
                continue;
            }
            // The claim is released for the duration of the backoff and
            // reclaimed on the far side, unless a trigger got there first —
            // in which case this task has already been cancelled and the
            // ticket check is the belt to that suspender.
            let ticket = state.write().pause();
            sleep_ms(delay).await;
            if !state.write().resume(ticket) {
                return;
            }
        }
    });
    state.write().attach(task);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    thread_local! {
        /// Every handle `cancel_task` was called on, in order — the record
        /// the supersession tests read.
        ///
        /// Thread-local rather than passed around because [`ReaderTask`] is
        /// deliberately a `Copy` handle with no context: production hands the
        /// state machine a task id and nothing else, and a test handle that
        /// carried a channel or a counter would be testing a different shape
        /// from the one that ships.
        static CANCELLED: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    /// A task handle for tests: numbered, `Copy`, and it records its own
    /// cancellation.
    ///
    /// This is what closes the gap a real `Task` leaves. `Task` cannot be
    /// constructed outside a running Dioxus runtime, so with it the
    /// supersession tests could only ever watch `None` go past and the whole
    /// cancel-exactly-once rule went unasserted — an implementation that
    /// simply never cancelled anything passed every one of them.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct FakeTask(u32);

    impl ReaderTask for FakeTask {
        fn cancel_task(self) {
            CANCELLED.with(|log| log.borrow_mut().push(self.0));
        }
    }

    /// A reader with a clean cancellation log.
    fn fresh() -> SurfaceReader<FakeTask> {
        CANCELLED.with(|log| log.borrow_mut().clear());
        SurfaceReader::default()
    }

    /// Everything cancelled since the last [`fresh`].
    fn cancelled() -> Vec<u32> {
        CANCELLED.with(|log| log.borrow().clone())
    }

    /// Drive one reader through a scripted sequence of answers, returning
    /// what it decided after each pass — the state machine without a
    /// runtime, a socket, or a helm.
    ///
    /// `answers` is what each successive read reports; `notices_during` says
    /// how many notifications land while that read is in flight, which is the
    /// only way the coalescing rule can be exercised at all (a notice that
    /// arrives between reads is just another `request`).
    fn drive(script: &[(bool, usize)]) -> (SurfaceReader<FakeTask>, Vec<Next>) {
        let mut reader = fresh();
        let mut decisions = Vec::new();
        assert!(
            reader.claim(Trigger::Notice),
            "an idle surface starts its reader"
        );
        for (answered, notices_during) in script {
            // The dispatch the driver performs before every read, which is
            // what the notices below then land AFTER.
            assert!(reader.begin(false));
            for _ in 0..*notices_during {
                assert!(
                    !reader.claim(Trigger::Notice),
                    "a notice during a read must never start a second reader"
                );
            }
            decisions.push(reader.finish(*answered));
        }
        (reader, decisions)
    }

    /// One full backoff cycle as the DRIVER runs it: dispatch, fail, step
    /// aside, come back. Returns the rung that was waited out.
    fn fail_once(reader: &mut SurfaceReader<FakeTask>) -> u64 {
        assert!(reader.begin(false));
        let Next::Again(delay) = reader.finish(false) else {
            panic!("a failed read always owes another");
        };
        let ticket = reader.pause();
        assert!(
            reader.resume(ticket),
            "an undisturbed wait ends with the same reader carrying on"
        );
        delay
    }

    /// The happy path: one read per notice, and the reader stops when the
    /// surface is clean.
    ///
    /// The stopping half is what keeps this from being a poll. A reader that
    /// looped regardless of what was owed would re-read at the ladder's
    /// cadence forever, which is exactly the periodic loop M6.75 removed.
    #[test]
    fn a_satisfied_reader_stops() {
        let (reader, decisions) = drive(&[(true, 0)]);
        assert_eq!(decisions, vec![Next::Idle]);
        assert!(!reader.running, "nothing is left running");
        assert_eq!(reader.demand, Demand::None);
    }

    /// Notices arriving DURING a read coalesce into exactly one follow-up.
    ///
    /// The bug this pins is the one a naive "spawn a read per notice" has: a
    /// burst of five mutations against a slow helm produces five concurrent
    /// walks, all describing very nearly the same fleet, and the surface
    /// takes whichever lands last. One follow-up read is both cheaper and
    /// more correct — it is guaranteed to have started after every notice in
    /// the burst.
    #[test]
    fn notices_during_a_read_coalesce_into_one_follow_up() {
        let (reader, decisions) = drive(&[(true, 5), (true, 0)]);
        assert_eq!(
            decisions,
            vec![Next::Again(0), Next::Idle],
            "five notices owe exactly one immediate re-read, and then nothing"
        );
        assert!(!reader.running);
    }

    /// A read that never answered leaves the demand standing and schedules a
    /// retry — the whole reason this module exists.
    ///
    /// Without it a notice consumed by a failed read is spent: the feed is
    /// healthy, so the fallback poll is off, and no further notification is
    /// owed until the fleet changes again. The surface would then be wrong
    /// for as long as the fleet is quiet, with nothing on screen admitting
    /// it. The retry is what turns that into a delay.
    ///
    /// The `pause`/`resume` pair around each wait is the driver's own
    /// sequence, replayed here because it is where the reader is at its most
    /// exposed: the claim is released for the duration of the backoff, and
    /// an undisturbed wait has to end with the SAME reader picking the
    /// surface back up.
    #[test]
    fn a_failed_read_retries_and_eventually_settles() {
        let mut reader = fresh();
        assert!(reader.claim(Trigger::Notice));

        assert_eq!(fail_once(&mut reader), 500);
        assert_eq!(fail_once(&mut reader), 1_000);

        assert!(reader.begin(false));
        assert_eq!(reader.finish(true), Next::Idle);
        assert_eq!(
            reader.failures, 0,
            "an answer clears the backoff, so the next outage starts at the first rung"
        );
    }

    /// The retry cadence is `reconnect`'s ladder followed by unbounded
    /// probing, and the boundary between the two is the assertion that
    /// matters.
    ///
    /// Pinned as literals rather than derived from the same constants,
    /// because the point is the SHAPE — a bounded active window and then a
    /// cadence cheap enough to run forever. A reader that stopped at the end
    /// of the ladder would leave the surface permanently stale, which is the
    /// failure this whole module answers; one that kept hammering at half a
    /// second would be a poll wearing a retry's name.
    #[test]
    fn the_retry_ladder_gives_way_to_low_frequency_probing() {
        let rungs: Vec<u64> = (1..=6).map(retry_delay_ms).collect();
        assert_eq!(rungs, vec![500, 1_000, 2_000, 4_000, 8_000, 15_000]);
        assert_eq!(retry_delay_ms(7), 30_000);
        assert_eq!(retry_delay_ms(70), 30_000, "and it never gives up");
    }

    /// A trigger landing during a retry's backoff reads AT ONCE, and the
    /// waiting reader retires rather than reading a second time.
    ///
    /// The bug this pins was found in the browser: after an outage long
    /// enough to walk the ladder out to its probe rung, a feed notice could
    /// take up to thirty seconds to produce a read, because it inherited a
    /// backoff belonging to a dead period the notice itself proves is over.
    /// The delivered notification came down a working socket — there is
    /// nothing left to back off from, and a page that has been told the
    /// fleet changed must not sit on the news.
    ///
    /// The second half is what makes the first safe: the reader that stepped
    /// aside wakes holding a stale ticket and must NOT start a read beside
    /// the one that superseded it, or "one read at a time" would hold
    /// everywhere except across a backoff.
    #[test]
    fn a_trigger_during_a_retry_wait_reads_at_once() {
        let mut reader = fresh();
        assert!(reader.claim(Trigger::Notice));

        // Six failures in a row: the ladder is spent and the reader is out at
        // its probe cadence, which is where the delay is long enough to be a
        // user-visible bug.
        for expected in [500, 1_000, 2_000, 4_000, 8_000, 15_000] {
            assert_eq!(fail_once(&mut reader), expected);
        }
        assert!(reader.begin(false));
        assert_eq!(reader.finish(false), Next::Again(30_000));
        let stranded = reader.pause();

        assert!(
            reader.claim(Trigger::Notice),
            "a notice claims the paused surface and dispatches immediately"
        );
        assert!(
            !reader.resume(stranded),
            "and the reader that was waiting out the probe interval retires instead of reading \
             a second time"
        );
    }

    /// Repeated news arriving during repeated backoffs leaves exactly one
    /// live reader, cycle after cycle.
    ///
    /// The shape is the one a real outage produces: a mutating fleet keeps
    /// delivering notices while the helm keeps failing reads, so every rung
    /// is interrupted. Each interruption strands the reader that was waiting
    /// it out, and both mechanisms that keep that bounded are asserted —
    /// every stranded sleeper is CANCELLED exactly once (so live timers
    /// cannot stack up), and every stranded ticket is refused (so no two
    /// readers can ever read, whatever cancellation does or does not do).
    ///
    /// The cancellation half runs through the same `claim` production calls,
    /// with a test handle standing in for the runtime's `Task` (see
    /// [`FakeTask`]). Without that seam this test could only watch `None` go
    /// past, and an implementation that cancelled nothing at all would pass
    /// it — which is exactly the state it was in before.
    #[test]
    fn repeated_news_during_backoffs_leaves_exactly_one_reader() {
        let mut reader = fresh();
        assert!(reader.claim(Trigger::Explicit));
        assert_eq!(
            cancelled(),
            Vec::<u32>::new(),
            "the first claim supersedes nothing"
        );
        reader.attach(FakeTask(0));

        let mut stranded = Vec::new();
        for cycle in 1..=5 {
            assert!(reader.begin(false));
            assert!(matches!(reader.finish(false), Next::Again(delay) if delay > 0));
            stranded.push(reader.pause());

            assert!(
                reader.claim(Trigger::Notice),
                "news claims the surface the sleeper stepped off"
            );
            assert_eq!(
                cancelled(),
                (0..cycle).collect::<Vec<u32>>(),
                "each superseded sleeper is cancelled once, and only when superseded"
            );
            // What the driver does next: the task it just spawned becomes
            // the one a later claim will supersede.
            reader.attach(FakeTask(cycle));
        }

        // A claim REFUSED while a read is running must never cancel that
        // read's task — the failure mode with the worst symptom, since the
        // surface would then have a demand recorded and nothing left running
        // to serve it.
        assert!(!reader.claim(Trigger::Notice));
        assert_eq!(
            cancelled(),
            (0..5).collect::<Vec<u32>>(),
            "a refused claim cancels nothing"
        );

        // Every sleeper that was interrupted stays retired, including the
        // oldest: tickets are never reused, so a wake-up arriving late — a
        // cancellation that did not take, a timer that fired first — still
        // cannot start a read beside the live one.
        for ticket in stranded {
            assert!(
                !reader.resume(ticket),
                "a superseded reader must stay retired however late it wakes"
            );
        }
        assert!(reader.running, "and exactly one reader is left holding it");
    }

    /// A reader that retires on its own leaves nothing for the next claim to
    /// cancel.
    ///
    /// The counterpart to the test above, and the reason `finish` and
    /// `begin` clear the handle: a finished task's id must not survive into
    /// the next cycle, or the following claim would cancel a task that has
    /// already ended — harmless today, and exactly the kind of stale handle
    /// that becomes a live bug the moment ids are reused.
    #[test]
    fn a_retired_reader_leaves_no_handle_behind() {
        let mut reader = fresh();
        assert!(reader.claim(Trigger::Notice));
        reader.attach(FakeTask(7));
        assert!(reader.begin(false));
        assert_eq!(reader.finish(true), Next::Idle);

        assert!(reader.claim(Trigger::Notice));
        assert_eq!(
            cancelled(),
            Vec::<u32>::new(),
            "the reader that went idle is gone; there is nothing to cancel"
        );

        // The withdrawal path clears it too: a reader that stood down under
        // skew has returned, so its handle is equally spent.
        reader.attach(FakeTask(8));
        assert!(!reader.begin(true));
        assert!(reader.claim(Trigger::Explicit));
        assert_eq!(cancelled(), Vec::<u32>::new());
    }

    /// A fallback tick starts an idle reader and disturbs nothing else.
    ///
    /// The bug this pins: classified as news, a three-second tick cancels
    /// whatever backoff is pending, so a helm that is down is re-read every
    /// three seconds forever and the ladder might as well not exist. A tick
    /// carries no information the last one did not — it is a clock, not an
    /// event — so the only thing it may do is set a stopped reader going.
    #[test]
    fn a_fallback_tick_never_shortens_a_backoff() {
        let mut reader = fresh();
        assert!(
            reader.claim(Trigger::Scheduled),
            "an idle surface is exactly what a tick is for"
        );

        assert!(reader.begin(false));
        assert_eq!(reader.finish(false), Next::Again(500));
        let ticket = reader.pause();
        assert!(
            !reader.claim(Trigger::Scheduled),
            "a tick during a backoff must not start a second reader"
        );
        assert_eq!(
            reader.demand,
            Demand::Retry,
            "nor upgrade the demand into one that skips the wait"
        );
        assert!(
            reader.resume(ticket),
            "and the waiting reader still owns its ticket"
        );

        // Mid-READ is the other half: a tick then has nothing to add, since
        // the read in flight is the refresh it wanted. Queuing a follow-up
        // would make a slow helm produce back-to-back walks forever.
        assert!(reader.begin(false), "the retry is dispatched");
        assert!(!reader.claim(Trigger::Scheduled));
        assert_eq!(reader.finish(true), Next::Idle);
    }

    /// Under a latched build mismatch the page keeps answering its own
    /// controls and stops keeping itself current.
    ///
    /// SPEC_impl.md's withdrawal rule is about UNATTENDED behavior: polling
    /// a helm whose vocabulary this bundle does not share is what gets
    /// revoked. Withdrawing explicit reads too was a real regression — a
    /// live filter edit under skew produced no read at all, so the control
    /// silently did nothing — and it is the kind of bug that reads as
    /// correct in the diff, because "stop reading" sounds like the safe
    /// direction until it is a person's click that stops working.
    #[test]
    fn only_unattended_reads_are_withdrawn_under_skew() {
        for unattended in [Trigger::Notice, Trigger::Scheduled] {
            let mut reader = fresh();
            assert!(reader.claim(unattended));
            assert!(
                !reader.begin(true),
                "{unattended:?} keeps the page current on its own, which is what is revoked"
            );
            assert!(
                !reader.running,
                "and the reader retires rather than reading"
            );
        }

        let mut reader = fresh();
        assert!(reader.claim(Trigger::Explicit));
        assert!(
            reader.begin(true),
            "a person asked, and the page must still answer"
        );

        // The retry that a failed explicit read owes is the reader's own
        // work, not the person's, so it stands down like any other
        // unattended read. The user's next action reads again.
        assert_eq!(reader.finish(false), Next::Again(500));
        let ticket = reader.pause();
        assert!(reader.resume(ticket));
        assert!(!reader.begin(true), "the retry ladder is unattended");
    }

    /// A trigger that lands while a read is IN FLIGHT is dispatched
    /// immediately once that read ends — even if it ended in failure.
    ///
    /// The distinction is the module's core rule seen from the other side: a
    /// failure is a fact about the request that just ended, not about the
    /// news that arrived during it. Backing the follow-up off would make an
    /// explicit trigger pay for a request it never had anything to do with,
    /// which is the same product bug as the one above with different timing.
    #[test]
    fn a_trigger_during_a_failed_read_is_not_backed_off() {
        let mut reader = fresh();
        assert!(reader.claim(Trigger::Notice));
        assert!(reader.begin(false));
        assert!(!reader.claim(Trigger::Notice), "the notice lands mid-read");

        assert_eq!(
            reader.finish(false),
            Next::Again(0),
            "the read failed, but the notice is news and is owed at once"
        );
        assert_eq!(
            reader.failures, 1,
            "the failure is still counted, so a retry that follows with no news backs off"
        );

        // With nothing new, the next failure does back off — the two demands
        // differ only in where they came from, and that is exactly the
        // difference this module turns on.
        assert!(reader.begin(false));
        assert_eq!(reader.finish(false), Next::Again(1_000));
    }

    /// An attended demand is never downgraded by an unattended one landing
    /// on top of it.
    ///
    /// The failure this forbids is quiet and specific: a user submits a
    /// filter, a feed notice lands in the same instant, and the demand the
    /// page acts on is now the notice's — which the withdrawal rule revokes
    /// under skew. The user's click would then be answered on a helm that
    /// matches and silently dropped on one that does not, which is the worst
    /// kind of intermittent.
    #[test]
    fn an_attended_demand_survives_unattended_ones_landing_on_it() {
        let mut reader = fresh();
        assert!(reader.claim(Trigger::Explicit));
        assert!(!reader.claim(Trigger::Notice));
        assert!(!reader.claim(Trigger::Scheduled));
        assert_eq!(reader.demand, Demand::Explicit);
        assert!(
            reader.begin(true),
            "the person's read still happens under skew"
        );
    }
}
