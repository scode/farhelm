//! Agent profiles as this UI handles them (PLAN_M6_75.md item 8): the
//! per-host catalog read every profile surface shares, the hosts panel's
//! profiles section (create, edit, delete), and the renderer-free rules the
//! create dialog's picker is built from.
//!
//! ## A catalog belongs to a HOST, and only ever to one
//!
//! Profiles are per-supervisor: an id minted on one machine names nothing on
//! another, and every fresh supervisor seeds the same starter profiles, so an
//! id carried across a host boundary does not merely go stale — it RESOLVES
//! over there, against a profile the user never chose. That is why
//! [`CatalogRead`] stores the host its answer describes and hands back
//! [`CatalogLookup::Pending`] for any other one, why a catalog for a
//! different host replaces rather than merges, and why the create dialog
//! drops a profile choice when its target host moves. The helm enforces the
//! same rule from its side (see farhelm-helm's `profiles` module); this is
//! the client half, and neither is trusted alone.
//!
//! ## Ask, do not guess
//!
//! SPEC.md's creation rule has two halves and the second is the one that is
//! easy to lose: the dialog defaults to the last-used profile, and when that
//! profile no longer exists it ASKS rather than substituting another. The
//! helm serves the remembered id raw — never filtered against the catalog
//! beside it — precisely so a client can tell "nothing was ever used here"
//! apart from "what you used is gone". [`resolve_agent`] is where those two
//! become different screens: the first preselects nothing quietly, the second
//! preselects nothing and says why.
//!
//! ## The snapshot rule, made visible
//!
//! SPEC.md: editing or deleting a profile affects future sessions only. That
//! promise is invisible unless something on screen keeps saying what a
//! session was created FROM — so a row renders its snapshotted profile name
//! ([`source_profile_label`]), marked when the catalog has since moved on,
//! and the profiles section says out loud what an edit does and does not
//! reach. Neither is decoration: without them a renamed profile looks like it
//! rewrote history, and a deleted one looks like it took its sessions with
//! it.
//!
//! ## One reader per surface, like everything else on this page
//!
//! [`use_catalog_surface`] wires a catalog read into the same discipline the
//! listing and hosts reads run under (`reader::SurfaceReader`): one read at a
//! time, every trigger coalesced into a single follow-up, a failed read
//! retried on its own. Its triggers are each ACTIVATION (the surface being
//! pointed at something), every feed notification (a profile edited in
//! another client must reach this one — PLAN_M6_75.md item 5 bumps the
//! revision for exactly that), the documented fallback poll while the feed is
//! unhealthy, and a mutation's own follow-up.
//!
//! What scopes these reads is the TARGET, not the mounting. Both hooks are
//! called by `list::ListView`, so the tasks live as long as that page does;
//! what makes a collapsed section and a closed dialog free is that every
//! trigger is a no-op while the target is `None`, and `ListView` clears the
//! target the moment the surface stops being shown. The two are worth telling
//! apart, because only the second is something this module can promise.
//!
//! An ACTIVATION is a surface being pointed at something — including at the
//! same host it held before. What it held then is not evidence about now (a
//! remembered default can have moved, profiles can have come and gone), so a
//! reopened surface reads as pending until its own read lands rather than
//! presenting the old answer as current.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::api::{
    self, ProfileCatalog, ProfileCommit, ProfileSpec, create_profile, delete_profile,
    update_profile,
};
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::hosts::host_incarnation;
use crate::ops::{OpLock, ReadGate};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::{ApiBase, Host, HostId, Profile, ProfileExistence, SourceProfile};

// ---------------------------------------------------------------------
// What a catalog belongs to
// ---------------------------------------------------------------------

/// The thing a profile catalog is ABOUT: a registry row together with the
/// incarnation behind it.
///
/// A `HostId` alone is not enough, and the gap is not theoretical. A host id
/// is a registry ROW that survives every edit made to it — a retarget points
/// it at another address, an adopt binds it to another install — while
/// profile ids are minted per supervisor AND every fresh supervisor seeds the
/// same starter profiles. So an id-keyed catalog does not merely go stale
/// when the row moves: the profile ids in it RESOLVE on the successor, to
/// different profiles that happen to share an id. Keyed on the id alone, an
/// open editor would save over the successor's starter profile, a delete
/// confirmation would delete the successor's, and a create dialog would offer
/// the predecessor's catalog for a launch on the new machine.
///
/// The incarnation is `hosts::host_incarnation`'s fingerprint — the same
/// value the create dialog's idempotency key is bound to, and for the same
/// reason. Compared, never parsed or displayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostTarget {
    pub(crate) host: HostId,
    /// The helm's CONNECTION token for this host (`Host::incarnation`), which
    /// is also what every mutation prepared here hands back as
    /// `expected_incarnation`.
    ///
    /// Part of the identity, so a reconnection re-activates the surface rather
    /// than letting a request prepared on one connection be sent on another.
    /// That costs an open editor its draft on a transient blip, which is the
    /// deliberate side of the trade: the helm compares connections, so binding
    /// this UI's state to anything coarser would mean the client-side check
    /// and the server-side check disagree about what "the same install" is.
    /// `0` is "never connected" and asserts nothing.
    pub(crate) connection: u64,
    /// The registry row's own fingerprint (`hosts::host_incarnation`): the
    /// destination, the install fields, the recorded identity.
    ///
    /// Kept ALONGSIDE the connection because they cover different windows. A
    /// host that is down has no connection to change, so a retarget while it
    /// is unreachable moves nothing above — but it changes what the row will
    /// reach, and a create dialog aimed at it must not carry a choice across
    /// that. Compared, never parsed or displayed.
    fingerprint: String,
}

impl HostTarget {
    /// The target one registry row currently names.
    pub(crate) fn of(host: &Host) -> HostTarget {
        HostTarget {
            host: host.id,
            connection: host.incarnation,
            fingerprint: host_incarnation(host),
        }
    }

    /// The target one create-dialog option currently names.
    ///
    /// Built from the option rather than re-derived from the registry because
    /// the option already carries the fingerprint the create's key is bound
    /// to, and two derivations of one value is one too many.
    pub(crate) fn new(host: HostId, connection: u64, fingerprint: String) -> HostTarget {
        HostTarget {
            host,
            connection,
            fingerprint,
        }
    }

    /// What a mutation prepared against this target asserts about the world.
    pub(crate) fn expectation(&self) -> api::Expectation<'static> {
        api::Expectation::on(self.connection)
    }

    /// Whether two targets name the same INSTALL — the registry row and
    /// everything about it that decides which machine and which binary answer,
    /// ignoring which connection to it is current.
    ///
    /// The distinction exists for one rule that would otherwise be violated
    /// quietly. A create's idempotency key must survive an ordinary
    /// reconnection: the case the key is FOR is a request that was accepted
    /// and whose reply was lost, and the most ordinary way to lose a reply is
    /// the connection dropping — so rotating the key on a reconnect would make
    /// the retry a second intended create, and the user would get two sessions
    /// from one press. A retarget or an adoption is the opposite: same id,
    /// different machine, and there the key MUST rotate, because replaying it
    /// there dedups against nothing.
    pub(crate) fn same_install(&self, other: &HostTarget) -> bool {
        self.host == other.host && self.fingerprint == other.fingerprint
    }
}

// ---------------------------------------------------------------------
// The catalog read, as four states
// ---------------------------------------------------------------------

/// What one surface currently knows about a host's profile catalog.
///
/// The same four-state shape `hosts::HostsRead` carries, for the same reason
/// — a failed refresh must not blank rows the user can still act on — plus
/// two rules that are specific to profiles:
///
/// - The answer is tied to a [`HostTarget`], not to an id. A catalog read
///   from a different install is not a stale answer to this question but an
///   answer to another question entirely, and answering with it would offer
///   profile ids that mean something else over here (see [`HostTarget`]).
/// - The answer is tied to the ACTIVATION that asked for it. A surface that
///   was closed and reopened on the same host is asking again, and what it
///   held from last time may predate a change made in between — a remembered
///   default moved by another client, a profile deleted from the panel. A
///   reopened surface therefore reads as pending until its OWN read lands,
///   rather than briefly presenting the old answer as current.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CatalogRead {
    /// Which target, and which activation of it, the stored answer belongs
    /// to. `None` before any read has completed.
    answering: Option<(HostTarget, u64)>,
    /// The last read that SUCCEEDED for `answering`, retained across later
    /// failures.
    catalog: Option<ProfileCatalog>,
    /// Set when the most recent read for `answering` failed; cleared by the
    /// next success.
    error: Option<String>,
}

impl CatalogRead {
    /// Fold one completed read in.
    ///
    /// A read for a different target OR a different activation discards
    /// everything held, and that discard is the load-bearing half: a retained
    /// catalog would otherwise be rendered under the new question for as long
    /// as its own read takes. Showing nothing for a moment is the honest
    /// alternative.
    fn record(
        &mut self,
        target: &HostTarget,
        activation: u64,
        outcome: Result<ProfileCatalog, String>,
    ) {
        self.rebind(target, activation);
        match outcome {
            Ok(catalog) => {
                self.catalog = Some(catalog);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    /// Drop whatever is held unless it already belongs to this question.
    fn rebind(&mut self, target: &HostTarget, activation: u64) {
        let current = self
            .answering
            .as_ref()
            .is_some_and(|(held, held_activation)| {
                held == target && *held_activation == activation
            });
        if !current {
            *self = CatalogRead {
                answering: Some((target.clone(), activation)),
                ..CatalogRead::default()
            };
        }
    }

    /// Fold a mutation this client PERFORMED into the held catalog.
    ///
    /// Not an optimization and not an optimistic paint: it closes a window
    /// where the page would otherwise hand out a definition it knows to be
    /// superseded. A save answers with the profile as the supervisor now
    /// holds it, but the authoritative re-read is a round trip away — and in
    /// between, the operation token is released and the row can be reopened,
    /// seeding an editor from the PRE-EDIT definition. Saving that would undo
    /// an update the supervisor already accepted, silently.
    ///
    /// Applied only when the held answer is still the one this mutation was
    /// made against; a surface that has since been re-pointed keeps its
    /// pending state rather than gaining a row from another install.
    fn absorb(&mut self, target: &HostTarget, activation: u64, change: CatalogChange) {
        let current = self
            .answering
            .as_ref()
            .is_some_and(|(held, held_activation)| {
                held == target && *held_activation == activation
            });
        let Some(catalog) = self.catalog.as_mut().filter(|_| current) else {
            return;
        };
        match change {
            CatalogChange::Upsert(profile, fingerprint) => {
                // The fingerprint moves with the definition, atomically. A
                // stale one left behind is worse than none at all: the next
                // edit would carry it as its precondition and be refused over
                // a change this client itself just made.
                match fingerprint {
                    Some(fingerprint) => {
                        catalog.definitions.insert(profile.id.clone(), fingerprint);
                    }
                    None => {
                        catalog.definitions.remove(&profile.id);
                    }
                }
                match catalog
                    .profiles
                    .iter_mut()
                    .find(|held| held.id == profile.id)
                {
                    Some(held) => *held = profile,
                    // A create lands at the end rather than in the
                    // supervisor's own order; the authoritative read that
                    // follows restores that order, and until it does the
                    // profile is at least THERE.
                    None => catalog.profiles.push(profile),
                }
            }
            CatalogChange::Remove(id) => {
                catalog.profiles.retain(|held| held.id != id);
                catalog.definitions.remove(&id);
            }
        }
    }

    /// Forget the held answer while staying bound to the same question, so the
    /// surface reads as pending until its next read lands. See
    /// [`CatalogSurface::invalidate`].
    fn forget(&mut self) {
        self.catalog = None;
        self.error = None;
    }

    /// What can be said about the question `target`/`activation` asks.
    fn answer_for(&self, target: Option<&HostTarget>, activation: u64) -> CatalogLookup<'_> {
        let answers = match (&self.answering, target) {
            (Some((held, held_activation)), Some(wanted)) => {
                held == wanted && *held_activation == activation
            }
            _ => false,
        };
        if !answers {
            return CatalogLookup::Pending;
        }
        match (&self.catalog, &self.error) {
            (Some(catalog), refresh_error) => CatalogLookup::Known {
                catalog,
                refresh_error: refresh_error.as_deref(),
            },
            (None, Some(error)) => CatalogLookup::Failed(error),
            (None, None) => CatalogLookup::Pending,
        }
    }
}

/// The question a mutation was dispatched under, carried to its completion.
///
/// A value rather than two loose fields because every completion-side write
/// checks the same pair, and a check that could be performed against only half
/// of it would be no check at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogLease {
    pub(crate) target: HostTarget,
    activation: u64,
}

/// One change this client made and has already been told succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CatalogChange {
    /// A profile as the supervisor now holds it — a create or an edit —
    /// together with the fingerprint of that committed definition, where the
    /// helm served one.
    ///
    /// The two travel together because they are folded in together: an editor
    /// reopened before the authoritative read is seeded from this pair, and a
    /// definition paired with the PREVIOUS fingerprint would send a
    /// precondition the helm refuses as stale — a conflict reported over this
    /// client's own change. A create whose fingerprint never arrives leaves
    /// the first edit of that profile unguarded instead, which is the lesser
    /// of the two and is what an absent value means.
    Upsert(Profile, Option<String>),
    /// A profile that is gone, by id.
    Remove(String),
}

/// One host's catalog as a surface may currently describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CatalogLookup<'a> {
    /// Nothing has come back for THIS question yet — including the cases
    /// where something has come back for a different target, for a different
    /// install behind the same id, or for an earlier activation of this same
    /// surface.
    Pending,
    /// A catalog is in hand. `refresh_error` is set when the most recent read
    /// for it failed, so a surface can keep drawing rows while admitting they
    /// may be out of date.
    Known {
        catalog: &'a ProfileCatalog,
        refresh_error: Option<&'a str>,
    },
    /// Nothing in hand and the last read failed.
    Failed(&'a str),
}

// ---------------------------------------------------------------------
// The reader
// ---------------------------------------------------------------------

/// One catalog surface: its target host, its answer, and the single-flight
/// reader that keeps the two in step.
///
/// `Copy` and made entirely of signals so it can be handed to a component as
/// a prop and stay one surface — the alternative, each consumer holding its
/// own read of the same endpoint, is exactly the per-caller-counter shape the
/// page's one-door discipline exists to prevent (`reader`).
///
/// Constructed only by [`use_catalog_surface`], which is what guarantees the
/// triggers are wired: a hand-built surface would be a reader nothing ever
/// asks.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct CatalogSurface {
    /// The helm's origin, in a signal purely so this handle stays `Copy`.
    base: Signal<String>,
    /// Whose catalog is wanted right now, or `None` for "nothing is showing
    /// one" — the state a collapsed section and a closed dialog are in, and
    /// the one in which this surface performs no profile requests at all.
    ///
    /// Derived by `list::ListView` from what is on screen INTERSECTED with
    /// the registry as it currently stands, so a row that has been removed or
    /// re-pointed at another install cannot leave this aimed at it.
    target: Signal<Option<HostTarget>>,
    /// Which ACTIVATION of that target is current: bumped every time the
    /// surface is pointed at something, including at the same host it was
    /// pointed at before. See [`CatalogRead`] for why a reopened surface must
    /// not present what it held last time as current.
    activation: Signal<u64>,
    /// The answer every consumer renders from. Read through the guard and
    /// then asked through [`CatalogSurface::lookup`], never interrogated
    /// directly, so "which question does this answer" is decided in one
    /// place.
    pub(crate) catalog: Signal<CatalogRead>,
    /// Per-REQUEST ordering, so a slow read for the previous target cannot
    /// overwrite the current one's answer (`ops::ReadGate`).
    gate: Signal<ReadGate>,
    /// The single-flight, retry-until-answered reader (`reader`).
    reader: Signal<SurfaceReader>,
}

impl CatalogSurface {
    /// Ask for a read of whatever this surface is currently pointed at.
    ///
    /// Cheap to call from anywhere and safe to call repeatedly: with no
    /// target it does nothing, and with one it asks the surface reader, which
    /// starts a read only if none is running and otherwise records the demand
    /// as a single follow-up. The trigger classifies the demand's authority
    /// (`reader::Trigger`): pointing a surface somewhere and a mutation's
    /// follow-up are things a person did, feed notices and fallback ticks
    /// are not, and the reader withdraws only the latter under build skew.
    pub(crate) fn request(self, trigger: Trigger) {
        // Read here rather than only inside the closure so that an untargeted
        // surface never starts a reader at all — a reader that woke, found
        // nothing to do and retired would be indistinguishable in the logs
        // from one that read.
        if self.target.peek().is_none() {
            return;
        }
        request_read(self.reader, trigger, move || {
            let mut surface = self;
            // Sampled and CLAIMED synchronously, before any await, for
            // `list::ListView`'s reason: the generation has to order requests
            // by when they were asked for rather than by when their tasks
            // happened to be polled. Re-sampled per call because the reader
            // calls this again for a retry or a coalesced notice, and that
            // later read is a new request against whatever the surface is
            // asking then — target and activation alike.
            let wanted = surface.target.peek().clone();
            let activation = *surface.activation.peek();
            let started = wanted.map(|target| (target, activation, surface.gate.write().start()));
            let base = surface.base.peek().clone();
            async move { surface.complete(started, base).await }
        });
    }

    /// Perform one read and fold it in; the answer is whether the helm
    /// answered at all (`reader::SurfaceReader::finish`).
    ///
    /// A `None` target reports `true` — nothing was asked, so nothing is
    /// owed. Reporting a failure instead would put the reader on its retry
    /// ladder for a surface that has been closed, which is a loop nobody is
    /// waiting on.
    async fn complete(&mut self, started: Option<(HostTarget, u64, u64)>, base: String) -> bool {
        let Some((target, activation, generation)) = started else {
            return true;
        };
        // The READ carries the same expectation its mutations do. Without it
        // a catalog can be fetched from the successor of a host that was
        // adopted mid-flight and then rendered as this target's — the ids
        // would resolve, because starters collide across installs. A refusal
        // arrives as an ordinary failed read: the section says the catalog
        // could not be read, and the next activation asks again.
        let outcome = api::fetch_profiles(&base, target.host, target.expectation()).await;
        let answered = outcome.is_ok();
        // Superseded ACTIVATIONS are refused here, in addition to the
        // ordinary generation gate below. The two catch different things: the
        // gate orders reads that all answer the same question, while this
        // rejects a reply to a question nobody is asking anymore — a surface
        // that has been closed, re-pointed, or reopened since. Committing it
        // would put an answer under a question it does not belong to, which
        // for a catalog means offering profile ids from another install.
        if !self.asking(&target, activation) {
            return answered;
        }
        // Successes and failures are gated differently, exactly as the hosts
        // read gates them: an older success describes a catalog that has
        // since been changed by something this client did, while a failure
        // newer than what is on screen is worth reporting even though a later
        // read has already begun.
        let accepted = match &outcome {
            Ok(_) => self.gate.write().accept_success(generation),
            Err(_) => self.gate.peek().accept_failure(generation),
        };
        if accepted {
            self.catalog.write().record(&target, activation, outcome);
        }
        answered
    }

    /// Whether this surface is still asking exactly that question.
    fn asking(&self, target: &HostTarget, activation: u64) -> bool {
        *self.activation.peek() == activation && self.target.peek().as_ref() == Some(target)
    }

    /// Take a LEASE on the question this surface is currently asking.
    ///
    /// Every mutation runs under one, and every completion-side write checks
    /// it ([`CatalogSurface::holds`]) — not just the ones that touch the
    /// catalog. The failure that makes the wider rule necessary is quiet: a
    /// save dispatched before a cross-client adoption, answering after it,
    /// would otherwise repopulate a warning line, a form error, or a per-row
    /// refusal under the SUCCESSOR install — describing a machine nobody is
    /// looking at, on a surface that has already discarded everything else
    /// about the predecessor.
    pub(crate) fn lease(&self) -> Option<CatalogLease> {
        self.target.peek().clone().map(|target| CatalogLease {
            target,
            activation: *self.activation.peek(),
        })
    }

    /// Whether a lease is still the question being asked. `false` means every
    /// completion-side write belonging to it must be dropped.
    pub(crate) fn holds(&self, lease: &CatalogLease) -> bool {
        self.asking(&lease.target, lease.activation)
    }

    /// Drop the held catalog for `lease`, leaving the surface pending until
    /// its next read lands.
    ///
    /// For the one success this client cannot reconcile from: a 2xx whose body
    /// would not decode changed something the page cannot describe, so
    /// continuing to serve the pre-mutation catalog would let the next editor
    /// seed from a definition that is known to be superseded and save it back.
    /// Showing nothing until the authoritative read arrives is the honest
    /// state, and the read is already on its way.
    pub(crate) fn invalidate(mut self, lease: &CatalogLease) {
        if !self.holds(lease) {
            return;
        }
        self.catalog.write().forget();
    }

    /// The target this surface is pointed at right now, untracked — what a
    /// HANDLER must consult before sending a mutation, so a click cannot be
    /// routed by whatever the last render happened to draw.
    ///
    /// The residual race is stated rather than hidden: nothing can stop a
    /// host from being adopted between this read and the request landing,
    /// because the helm's profile routes take a host id and no incarnation.
    /// What this closes is the much larger window — a surface that has
    /// ALREADY been re-pointed still acting on the old catalog.
    pub(crate) fn target(&self) -> Option<HostTarget> {
        self.target.peek().clone()
    }

    /// The target as a TRACKED read, for a consumer that must reset its own
    /// state when the surface is re-pointed (see `ProfilesSection`).
    pub(crate) fn watch_target(&self) -> Option<HostTarget> {
        (self.target)()
    }

    /// What this surface can say right now, given what it is asking.
    ///
    /// The only door onto the stored answer: consumers hold the guard and ask
    /// through here, so the "does this answer the current question" rule lives
    /// in one place rather than at each render site.
    pub(crate) fn lookup<'a>(&self, read: &'a CatalogRead) -> CatalogLookup<'a> {
        let target = self.target.peek().clone();
        read.answer_for(target.as_ref(), *self.activation.peek())
    }

    /// Fold a mutation this client just performed into the held catalog, so
    /// the section stops handing out a definition it knows to be superseded
    /// (see [`CatalogRead::absorb`]). Called BEFORE the operation token is
    /// released, which is what makes the window it closes empty rather than
    /// merely short.
    pub(crate) fn absorb_change(mut self, lease: &CatalogLease, change: CatalogChange) {
        if !self.holds(lease) {
            return;
        }
        self.catalog
            .write()
            .absorb(&lease.target, lease.activation, change);
    }
}

/// Wire a catalog surface for whatever `target` names, with every trigger
/// this page's reads run under.
///
/// Called once, unconditionally, by `list::ListView`, which owns both catalog
/// surfaces and derives both targets. That placement is deliberate and is NOT
/// what scopes the reads: the hooks live on the page, so the tasks live as
/// long as the page does. What makes a closed dialog and a collapsed section
/// free is the TARGET — every trigger below is a no-op while it is `None`,
/// and `ListView` clears it the moment the surface stops being shown.
///
/// Four triggers, matching the session list's:
///
/// - every ACTIVATION (the effect below): the surface being pointed at
///   something, which includes being re-pointed at another install behind the
///   same id, and being reopened on the same host it held before;
/// - every feed notification, because a profile edited in another client
///   bumps the fleet revision and this surface is one of the things that
///   invalidates (PLAN_M6_75.md item 5 names the create dialog explicitly);
/// - the documented fallback poll, which runs only while the feed is
///   unhealthy and no build mismatch has been latched;
/// - a mutation's own follow-up, through [`CatalogSurface::request`].
pub(crate) fn use_catalog_surface(target: Signal<Option<HostTarget>>) -> CatalogSurface {
    let api_base = use_context::<ApiBase>().0;
    let mut activation = use_signal(|| 0_u64);
    let surface = CatalogSurface {
        base: use_signal(|| api_base),
        target,
        activation,
        catalog: use_signal(CatalogRead::default),
        gate: use_signal(ReadGate::default),
        reader: use_signal(SurfaceReader::default),
    };

    // What the current activation was minted for. Compared rather than
    // assumed, because this effect also re-runs for target changes that are
    // no change at all (a hosts refresh that rebuilt an identical row).
    let mut activated_for = use_signal(|| None::<HostTarget>);

    // The ONE tracked read is the target itself, so this runs at mount and
    // again whenever the surface is pointed somewhere else — never on an
    // answer landing, which would be a read triggering itself.
    use_effect(move || {
        let wanted = target();
        if *activated_for.peek() == wanted {
            return;
        }
        activated_for.set(wanted.clone());
        if wanted.is_none() {
            // Closing is not an activation and asks for nothing. What the
            // surface still HOLDS stops being current at the next opening,
            // because that opening mints a new activation — see
            // `CatalogRead`.
            return;
        }
        activation += 1;
        surface.request(Trigger::Explicit);
    });

    // Marked, not awaited: an effect is not a place to hold a round trip, and
    // a notice arriving mid-read becomes the reader's single follow-up rather
    // than a second walk.
    use_feed_reader(move || surface.request(Trigger::Notice));

    // The fallback ticks forever and the READ is what is gated, the same
    // arrangement the list uses and for the same reason: a fallback whose job
    // is to cover the moment the feed dies is the worst thing to have to spin
    // up at that moment.
    use_future(move || async move {
        loop {
            fallback_sleep().await;
            if fallback_polls_now() {
                surface.request(Trigger::Scheduled);
            }
        }
    });

    surface
}

// ---------------------------------------------------------------------
// What a create dialog offers, and what it must ask about
// ---------------------------------------------------------------------

/// What a create would launch: a profile from the target host's catalog, or
/// the command line typed into the form.
///
/// The raw path survives alongside profiles deliberately (PLAN_M6_75.md item
/// 4): it is what the API, the e2e harness, and anyone running something no
/// profile describes uses, and removing it from the dialog would make the
/// only way to run an ad-hoc command a trip to `curl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentChoice {
    /// The invocation typed into the form's own field.
    Command,
    /// A profile, by id — always an id from the CURRENT target host, which is
    /// why a host change clears the choice rather than carrying it over.
    Profile(String),
}

/// The `<option>` value of the picker's placeholder — the entry that stands
/// for "you have not said yet", shown only while the selection is unresolved.
///
/// A sentinel is unavoidable (a `size=1` select always has exactly one option
/// selected, so "nothing chosen" needs something to select), and this one is
/// chosen to fail SAFE if a supervisor ever minted a profile id equal to it:
/// the picker would refuse to resolve that profile, blocking a submit, rather
/// than launching something the user did not pick. The command path's empty
/// value has the same property from the other side — a supervisor does not
/// mint an empty id, and an empty one would be unusable everywhere else too.
pub(crate) const UNRESOLVED_VALUE: &str = "__unresolved__";

impl AgentChoice {
    /// The `<option>` value this choice is selected by. The empty string is
    /// the command path, for the same reason the filter surface spells "any
    /// host" empty: a select has to name every choice with a string, and the
    /// absence of a profile id is what "no profile" IS on the wire.
    pub(crate) fn value(&self) -> &str {
        match self {
            AgentChoice::Command => "",
            AgentChoice::Profile(id) => id,
        }
    }

    /// The choice an `<option>` value selects, or `None` for the placeholder
    /// — which is a real answer ("still nothing chosen") rather than a
    /// degenerate one, and is what keeps re-selecting the placeholder from
    /// silently meaning "run the command below".
    pub(crate) fn from_value(value: &str) -> Option<AgentChoice> {
        match value {
            UNRESOLVED_VALUE => None,
            "" => Some(AgentChoice::Command),
            id => Some(AgentChoice::Profile(id.to_string())),
        }
    }
}

/// Why a dialog is preselecting nothing, when there is a reason worth saying.
///
/// Only ever produced for a profile that WAS available and is not anymore.
/// The ordinary "nothing has ever been created from a profile on this host"
/// case yields no note at all, and that asymmetry is the point: a first-time
/// dialog has nothing to explain, while a dialog that silently dropped a
/// remembered choice would look like it had forgotten it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentNote {
    /// The host's remembered last-used profile is gone from its catalog —
    /// SPEC.md's ask-don't-guess case, exactly.
    RememberedGone,
    /// A profile the user had explicitly picked left the catalog while the
    /// dialog was open (deleted from another client, or from the panel right
    /// beside it).
    ChoiceGone,
    /// A profile is chosen but this surface has no catalog to confirm it
    /// against — before the first read of a target, or after one that failed.
    /// Transient by construction, unlike the two above, and still blocking:
    /// acting on a choice nothing has confirmed is the same guess in a
    /// shorter window.
    ChoiceUnconfirmed,
}

impl AgentNote {
    /// The sentence to show. Each names what happened AND what to do, per
    /// SPEC.md's actionable-errors rule — a note saying only that something
    /// vanished leaves the user staring at a form with no next move. The
    /// first two say "nothing is selected" and mean it literally: the create
    /// is BLOCKED until one of the offered answers is chosen.
    pub(crate) fn text(self) -> &'static str {
        match self {
            AgentNote::RememberedGone => {
                "the profile you last used on this host no longer exists, so nothing is selected \
                 — choose a profile, or choose \"custom command\" to run the command below"
            }
            AgentNote::ChoiceGone => {
                "the profile you picked is no longer on this host, so nothing is selected — \
                 choose another, or choose \"custom command\" to run the command below"
            }
            AgentNote::ChoiceUnconfirmed => {
                "this host's profiles have not been read yet, so the profile you picked cannot be \
                 confirmed — nothing is selected until they arrive"
            }
        }
    }
}

/// What the picker shows and what a submit would send.
///
/// `choice` is an `Option` because "nothing is selected" is a real state the
/// dialog can be in, not a gap to be filled with the nearest plausible answer
/// — see [`resolve_agent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentSelection {
    /// What a submit would launch, or `None` when the dialog is waiting for
    /// the user to say. A `None` here BLOCKS the create.
    pub(crate) choice: Option<AgentChoice>,
    pub(crate) note: Option<AgentNote>,
}

/// Resolve the create dialog's agent selection: the user's explicit choice
/// where there is one, the host's remembered default where there is not, and
/// NOTHING — a blocked submit — rather than a substitute.
///
/// ## `seeded` is what makes the default a DECISION rather than a feed
///
/// The remembered default is consumed exactly once per target: the first
/// catalog that answers a dialog's own question decides, and `seeded` says
/// that has happened. Without it, "nothing chosen" would keep meaning "follow
/// whatever the helm currently remembers" — so another client creating a
/// session would move an open dialog's selection underneath whoever is filling
/// it in, which is the same silent retarget this file exists to prevent, with
/// a different cause.
///
/// ## Why a vanished profile is not the command path
///
/// It used to be: a chosen or remembered profile that had left the catalog
/// resolved to `Command` with a note beside it. That looked like a graceful
/// degradation and was a way to launch the wrong thing. The command field
/// keeps whatever was typed into it earlier — a user who typed a command,
/// switched to a profile, and then had that profile deleted under them would
/// submit the stale command while the note on screen said nothing was
/// selected. Blocking is the only answer that cannot mis-launch: the dialog
/// asks, and every way out of it (pick another profile, pick the command
/// path) is an act the user performs.
///
/// The distinction is against the ORDINARY unselected case, which is not a
/// question at all: a host nothing has ever been created from with a profile
/// has no forgotten choice to explain, so the command path is the honest
/// default there and the dialog stays immediately usable. The same goes for
/// everything before the first catalog read, where nothing is known either
/// way — with one exception, `chosen` naming a profile: a choice that cannot
/// be CONFIRMED must not be acted on either, and that state clears itself the
/// moment the read lands.
///
/// The rule this function exists to keep is the negative one: no branch here
/// may ever answer with a profile the user did not choose and the host did
/// not remember, and no branch may answer with the command path unless the
/// user asked for it or nothing was ever remembered.
/// What the FIRST catalog to answer a dialog's own question decides, once.
///
/// The consumption half of SPEC.md's creation rule, split out from the
/// component so the decision can be stated and tested without a runtime. Three
/// outcomes, and the middle one is why this returns an `Option` rather than a
/// choice:
///
/// - the remembered default still exists → that is the choice;
/// - a default is remembered but is GONE → no choice, which leaves the dialog
///   blocked and asking (SPEC.md's ask-don't-guess);
/// - nothing was ever remembered here → the command path, so a first create on
///   a host is usable immediately and has nothing to explain.
///
/// Consulted once per target, whatever it answers — see `list::CreateSessionForm`
/// for why "a choice exists" is not a usable record of that having happened.
pub(crate) fn seeded_choice(catalog: &ProfileCatalog) -> Option<AgentChoice> {
    match catalog.default_profile.as_deref() {
        Some(remembered) if catalog.profiles.iter().any(|p| p.id == remembered) => {
            Some(AgentChoice::Profile(remembered.to_string()))
        }
        Some(_) => None,
        None => Some(AgentChoice::Command),
    }
}

pub(crate) fn resolve_agent(
    chosen: Option<&AgentChoice>,
    catalog: Option<&ProfileCatalog>,
    seeded: bool,
) -> AgentSelection {
    let held = |id: &str| {
        catalog.is_some_and(|catalog| catalog.profiles.iter().any(|profile| profile.id == id))
    };
    let resolved = |choice: AgentChoice| AgentSelection {
        choice: Some(choice),
        note: None,
    };
    let blocked = |note: AgentNote| AgentSelection {
        choice: None,
        note: Some(note),
    };
    match (chosen, catalog) {
        // An explicit command choice is the one thing no catalog state can
        // disturb: it names the field below, which is always there.
        (Some(AgentChoice::Command), _) => resolved(AgentChoice::Command),
        (Some(AgentChoice::Profile(id)), Some(_)) if held(id) => {
            resolved(AgentChoice::Profile(id.clone()))
        }
        // Picked, then gone — or picked against a catalog this surface cannot
        // currently see. Both block; only the wording differs, because one is
        // a decision the user has to make and the other resolves itself.
        (Some(AgentChoice::Profile(_)), Some(_)) => blocked(AgentNote::ChoiceGone),
        (Some(AgentChoice::Profile(_)), None) => blocked(AgentNote::ChoiceUnconfirmed),
        // Nothing chosen, and this dialog has ALREADY been seeded from a
        // catalog: the only way to reach here is a remembered default that did
        // not resolve, because every other outcome writes a choice at seeding
        // time (see `list::CreateSessionForm`). It is SPEC.md's ask case and
        // it blocks.
        (None, _) if seeded => blocked(AgentNote::RememberedGone),
        // Nothing chosen and nothing seeded yet: the catalog is still being
        // read, or could not be. Blocking here is the conservative reading of
        // SPEC.md's ask-don't-guess rule — this dialog does not yet know
        // whether the host remembers a profile, so defaulting to the command
        // field would be answering a question nobody has asked yet, and the
        // field is not necessarily empty. It clears itself the moment the read
        // lands, and the user can always answer it directly.
        (None, _) => blocked(AgentNote::ChoiceUnconfirmed),
    }
}

// ---------------------------------------------------------------------
// The profile's own fields, as an editor has to handle them
// ---------------------------------------------------------------------

/// The integration kinds SPEC.md's built-in v1 catalog offers, in the wire's
/// own spelling.
///
/// Offered as a choice rather than typed, for `list::FILTERABLE_STATUSES`'s
/// reason: the supervisor refuses a kind it does not know, and a select
/// cannot produce a typo. `generic` is the explicit spelling of "no
/// integration" and is listed rather than implied — an absent kind and a
/// generic one would otherwise be two ways to say the same thing about a
/// value that decides whether capture and status sharpening run at all.
pub(crate) const AGENT_KINDS: [&str; 3] = ["generic", "claude", "codex"];

/// The kinds to offer for a profile whose kind is `current`.
///
/// [`AGENT_KINDS`] plus, when the stored kind is not among them, that kind
/// itself. A newer helm's vocabulary is the case this exists for: without the
/// extra option, editing such a profile would show an empty select and SAVE a
/// different kind than the one it was opened on — a silent rewrite of the one
/// field that decides which heuristics the supervisor applies.
pub(crate) fn kind_options(current: &str) -> Vec<String> {
    let mut kinds: Vec<String> = AGENT_KINDS.iter().map(|kind| kind.to_string()).collect();
    if !kinds.iter().any(|kind| kind == current) {
        kinds.push(current.to_string());
    }
    kinds
}

/// A stored resume template as the editor's single-line field shows it —
/// QUOTED where an element needs it, so what is displayed can be typed back.
///
/// The quoting is what makes the field authorable rather than merely
/// displayable. SPEC.md's promise is that a user controls the invocations
/// completely, and a bare space-join breaks that in both directions: an
/// element containing a space renders as two, and there is no spelling the
/// user could type to produce one. Round-tripping through [`parse_resume`] is
/// the contract this function keeps, not an incidental property of it.
pub(crate) fn resume_text(template: Option<&[String]>) -> String {
    template
        .unwrap_or_default()
        .iter()
        .map(|arg| quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One argv element as the field spells it: bare where it can be, single
/// quoted where it cannot.
///
/// Single quotes are the outer form because their contents need no escaping —
/// with one exception, a literal single quote, spelled by leaving the quoted
/// run, escaping it, and re-entering (`'it'\''s'`), which is the shell's own
/// idiom and what [`parse_resume`] reads back. An EMPTY element is quoted too:
/// it is a real argument, and unquoted it would simply vanish at the next
/// parse.
fn quote_arg(arg: &str) -> String {
    let plain = !arg.is_empty()
        && !arg
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '\'' | '"' | '\\'));
    if plain {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('\'');
    for c in arg.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Parse the resume field into argv the way a command line is parsed:
/// whitespace separates, quotes group, a backslash escapes.
///
/// Deliberately the vocabulary the INVOCATION field is already subject to on
/// the far side (the supervisor shell-splits it), rather than a rule invented
/// here — a user who can write `claude --note='two words'` in one field of
/// this form and not the other would be entitled to call that a bug, and
/// SPEC.md's promise that they control the invocations completely is not kept
/// by a field that can only display what it cannot express.
///
/// `Err` carries a sentence to show. An unclosed quote is a typo somebody can
/// fix; guessing at it, or dropping the remainder, would save an argv they did
/// not write — and a resume template is what a restart executes.
///
/// An empty or whitespace-only field is `Ok(None)`: an explicit "no resume
/// template", which is a value the supervisor acts on (an integrated kind
/// derives its own, a generic one gets none) rather than a missing field.
pub(crate) fn parse_resume(text: &str) -> Result<Option<Vec<String>>, String> {
    let mut argv: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            // A backslash escapes the next character anywhere except inside
            // single quotes, where it is literal — the shell's own rule, and
            // what makes `'it'\''s'` one argument containing a quote.
            (q, '\\') if q != Some('\'') => {
                let Some(escaped) = chars.next() else {
                    return Err(
                        "the resume command ends in a backslash, which escapes nothing — remove \
                         it, or double it for a literal backslash"
                            .to_string(),
                    );
                };
                current.push(escaped);
                started = true;
            }
            (None, '\'') | (None, '"') => {
                quote = Some(c);
                started = true;
            }
            (Some(open), c) if c == open => quote = None,
            (None, c) if c.is_whitespace() => {
                if started {
                    argv.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (_, c) => {
                current.push(c);
                started = true;
            }
        }
    }
    if let Some(open) = quote {
        return Err(format!(
            "the resume command has an unclosed {open} quote, so it cannot be split into \
             arguments — close it, or escape it with a backslash if it is meant literally"
        ));
    }
    if started {
        argv.push(current);
    }
    Ok((!argv.is_empty()).then_some(argv))
}

/// How a session's snapshotted profile reads on its row.
///
/// The NAME is always the snapshotted one — never the profile's current name,
/// which this UI deliberately is not told (see [`SourceProfile`]) — and the
/// qualifier is what keeps that from reading as a claim about the catalog as
/// it stands today. Both halves say the same thing from different sides:
/// SPEC.md's rule that editing or deleting a profile leaves existing sessions
/// alone.
///
/// An existence this build does not know is qualified as unknown rather than
/// silently rendered as `present`: claiming a profile is still there is a
/// statement, and a word this build cannot read is not grounds for it.
pub(crate) fn source_profile_label(source: &SourceProfile) -> String {
    let name = display_peer(&source.name);
    match source.existence {
        ProfileExistence::Present => format!("profile: {name}"),
        ProfileExistence::Renamed => format!("profile: {name} (renamed since)"),
        ProfileExistence::Deleted => format!("profile: {name} (deleted since)"),
        ProfileExistence::Unrecognized => format!("profile: {name} (state unknown to this build)"),
    }
}

/// One word for an existence, for a machine reading the DOM.
///
/// The wire's own spellings, so a browser test asserting on a row and a
/// developer reading the helm's JSON are looking at the same vocabulary — the
/// same argument the hosts panel's chip makes for using the helm's phase
/// words verbatim. Kept apart from [`source_profile_label`] because that one
/// is prose for a person and this one is an attribute value; folding them
/// together would make either a rewording or a new variant break the other.
pub(crate) fn existence_word(existence: ProfileExistence) -> &'static str {
    match existence {
        ProfileExistence::Present => "present",
        ProfileExistence::Renamed => "renamed",
        ProfileExistence::Deleted => "deleted",
        ProfileExistence::Unrecognized => "unrecognized",
    }
}

// ---------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------

/// Which profile the section's form is editing, if any.
///
/// One form at a time, and one draft behind it — the same interaction the
/// rename field and the host destination field keep, for the same reason: two
/// open editors are two half-finished decisions competing for one submit.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Editing {
    /// A profile that does not exist yet.
    New,
    /// An existing profile, by id.
    Existing(String),
}

/// The editor's fields, plus what the resume field was seeded from.
///
/// Owned by the SECTION rather than by the form component, the discipline
/// `rename::RenameForm` records: this form is unmounted by re-renders the
/// user did not cause (a catalog refresh landing, a host row rebuilding), and
/// a draft owned by the form would be silently discarded with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ProfileDraft {
    /// The EDITING representations — what the fields show, which for a seeded
    /// draft starts as the escaped form (see [`ProfileDraft::of`]).
    name: String,
    invocation: String,
    agent_kind: String,
    resume: String,
    /// Which text fields the user has actually TYPED IN, as recorded by their
    /// input events.
    ///
    /// Tracked rather than inferred by comparing the field against its seed,
    /// and the difference is a real hole rather than a nicety: the seed is
    /// displayed escaped, so "unchanged" would have to mean "equal to the
    /// escaped spelling" — and a user who deliberately retypes that spelling
    /// (replacing an active right-to-left override with the visible
    /// `<U+202E>` text, which is exactly what someone cleaning up a hostile
    /// name would do) would silently have the control characters put back.
    /// Equality cannot tell those two apart; an input event can.
    edited: EditedFields,
    /// The definition this draft was seeded from, RAW, or `None` for a new
    /// profile.
    ///
    /// Every field the user never typed in is sent from here rather than from
    /// the text above — see [`ProfileDraft::spec`]. That is what makes the
    /// escaped display safe: an untouched field round-trips byte for byte,
    /// however exotic.
    seed: Option<Profile>,
    /// The fingerprint the seed was served with (`ProfileCatalog::definitions`),
    /// echoed back as this update's `expected_definition`. `None` for a create,
    /// and for a helm that does not serve fingerprints.
    definition: Option<String>,
}

/// Which of the editor's text fields have been typed in.
///
/// Three booleans rather than a set, because the fields are fixed and named:
/// a new one would have to be added here deliberately, which is the point —
/// a field nobody remembered to track would silently inherit the "send the
/// raw seed" behavior and ignore what the user typed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct EditedFields {
    name: bool,
    invocation: bool,
    resume: bool,
}

impl ProfileDraft {
    /// A blank draft for a NEW profile.
    ///
    /// `generic` is the starting kind because it is the only one that claims
    /// nothing: a new profile defaulting to an integration would apply that
    /// kind's status heuristics and conversation capture to whatever the user
    /// then typed, which is a decision they never made.
    fn blank() -> ProfileDraft {
        ProfileDraft {
            agent_kind: "generic".to_string(),
            ..ProfileDraft::default()
        }
    }

    /// A draft seeded from an existing profile — every field, because a save
    /// replaces the whole definition and anything not shown here would be
    /// cleared by it.
    ///
    /// The text fields are seeded ESCAPED (`peer::display_peer`), which is the
    /// one place this UI puts peer-supplied text into an editable control and
    /// therefore the one place the usual "render it isolated" answer does not
    /// reach: a right-to-left override or a zero-width joiner pasted into a
    /// profile name stays ACTIVE inside an `<input>`, so the command a person
    /// reads while editing can differ from the bytes they save — on a value
    /// that is about to be executed. The escaped form makes the field say what
    /// is actually stored.
    ///
    /// Nothing is lost by it: [`ProfileDraft::spec`] sends the RAW seed for
    /// every field nobody typed in, so an untouched exotic name survives a
    /// save exactly.
    fn of(profile: &Profile, definition: Option<&str>) -> ProfileDraft {
        ProfileDraft {
            name: display_peer(&profile.name),
            invocation: display_peer(&profile.invocation),
            agent_kind: profile.agent_kind.clone(),
            resume: display_peer(&resume_text(profile.resume_template.as_deref())),
            edited: EditedFields::default(),
            seed: Some(profile.clone()),
            definition: definition.map(str::to_string),
        }
    }

    /// This draft as the request body: the raw seed for every field nobody
    /// typed in, and the typed text for every field they did.
    ///
    /// `Err` is the resume field failing to parse — the only local validation
    /// this form performs, and it is not a duplicate of a supervisor rule: the
    /// far side receives argv, so a quoting mistake has to be caught on this
    /// side or the field would silently send something else. Everything else
    /// (names, sizes, the placeholder rule) is refused by the authority that
    /// owns it, whose sentence is what a user acts on.
    fn spec(&self) -> Result<ProfileSpec, String> {
        let seed = self.seed.as_ref();
        let resume_template = if self.edited.resume {
            parse_resume(&self.resume)?
        } else {
            seed.and_then(|profile| profile.resume_template.clone())
        };
        Ok(ProfileSpec {
            name: submitted_field(
                &self.name,
                self.edited.name,
                seed.map(|profile| profile.name.as_str()),
            ),
            invocation: submitted_field(
                &self.invocation,
                self.edited.invocation,
                seed.map(|profile| profile.invocation.as_str()),
            ),
            agent_kind: self.agent_kind.clone(),
            resume_template,
        })
    }
}

/// What an edit should SEND for one text field.
///
/// A field nobody typed in sends the RAW seed — what it displays is the
/// escaped rendering of exactly that, so sending the rendering would rewrite a
/// value the user never touched. A field they DID type in sends what they
/// typed, whatever it looks like: someone who replaces a live right-to-left
/// override with its visible spelling means the visible spelling, and no
/// comparison against the seed can tell that apart from not having edited at
/// all (which is why this takes an `edited` flag rather than comparing).
fn submitted_field(text: &str, edited: bool, seed: Option<&str>) -> String {
    match seed {
        Some(seed) if !edited => seed.to_string(),
        _ => text.to_string(),
    }
}

/// One host's profiles, inside the hosts panel (PLAN_M6_75.md item 8).
///
/// ## Why it lives here rather than on a page of its own
///
/// A profile belongs to a supervisor, and the hosts panel is where this UI
/// talks about supervisors: the row that says whether a host is reachable is
/// also the row whose catalog can only be read while it is. A separate page
/// would have to re-state the host's identity and its connection state to be
/// usable at all, and would leave a user reading two surfaces to answer one
/// question.
///
/// ## What it refuses to decide
///
/// Everything about validity. Names, sizes, the catalog bound and the resume
/// template's placeholder rule are the supervisor's, and its refusals are
/// rendered as it wrote them (through `peer::PeerLine`, since a supervisor
/// under `--ssh` is a machine this helm does not control).
///
/// `ops` is the page's live-operation token: every mutation claims it
/// synchronously at handler entry, which excludes profile edits against each
/// other AND against a host mutation or a create — a delete landing while a
/// host is being removed under it is precisely the interleaving that token
/// exists to prevent. See the `ops` module for why the `disabled` attributes
/// are cosmetic and the claim is the guard.
#[component]
pub(crate) fn ProfilesSection(
    host: HostId,
    /// The host's display name, for the delete prompt — a profile and a host
    /// are both named things, and a confirmation that did not say which
    /// machine it was about would be ambiguous on a fleet.
    host_name: String,
    surface: CatalogSurface,
    mut ops: OpLock,
) -> Element {
    // Held in a signal rather than as a captured `String` so every handler
    // below stays `Copy` — the hosts panel clones its handlers into each row
    // instead, and here they are reused by both the row's editor and the
    // section's own form, where a clone per use would be noise.
    let api_base = use_context::<ApiBase>().0;
    let base = use_signal(|| api_base);
    let mut editing = use_signal(|| None::<Editing>);
    let mut confirming = use_signal(|| None::<String>);
    let mut draft = use_signal(ProfileDraft::blank);
    // The open form's own refusal. One slot, unlike the hosts panel's
    // per-row map, because only one form is ever open here — a second slot
    // would be a place for a message about a form that no longer exists.
    let mut form_error = use_signal(|| None::<String>);
    // Delete failures, per profile: a refusal on one profile must not blank
    // out a refusal on another the user has not read yet.
    let mut errors = use_signal(HashMap::<String, String>::new);
    // A mutation the helm ACCEPTED whose reply this build could not read.
    // Distinct from an error because it means the opposite thing — the change
    // happened — and the catalog re-read beside it is the authoritative
    // account of what it did.
    let mut warning = use_signal(|| None::<String>);
    // Something happened to this catalog that the user did not do and has to
    // be told about: a precondition refusal (the world moved under an editor),
    // or a profile disappearing from under an open form. Distinct from
    // `form_error`, which belongs to a form that is still open and still
    // holding a draft.
    let mut notice = use_signal(|| None::<String>);

    // Everything above is bound to ONE install, and this is what enforces
    // that. A registry row survives a retarget and an adopt, so the host id
    // this section is mounted under can start naming a different machine
    // without the section moving at all — and every piece of state here would
    // then be about a catalog that no longer exists: an open editor would
    // save its draft over whatever profile now holds that id (starter ids
    // collide across installs by construction), a delete confirmation would
    // delete the successor's, and an unread refusal would describe a machine
    // nobody is looking at. Discarding all of it the moment the target moves
    // is the only answer that cannot act on the wrong supervisor.
    let mut bound_to = use_signal(|| None::<HostTarget>);
    use_effect(move || {
        let current = surface.watch_target();
        if *bound_to.peek() == current {
            return;
        }
        bound_to.set(current);
        editing.set(None);
        confirming.set(None);
        draft.set(ProfileDraft::blank());
        form_error.set(None);
        errors.write().clear();
        warning.set(None);
        notice.set(None);
    });

    let on_submit = move |_| {
        // The claim IS the guard and it happens synchronously: the rerender
        // that disables these controls is not synchronous with the event that
        // queued this submit, so a second submit would otherwise create two
        // profiles for one intent.
        if !ops.claim() {
            return;
        }
        // Every mutation runs under a LEASE on the question this surface is
        // asking — the target AND the activation — rather than merely against
        // the id this component was rendered with. The lease is what the
        // completion below checks before it writes anything at all, so a reply
        // that arrives after a cross-client adoption cannot repopulate a form
        // error, a warning, or a row refusal under the successor install.
        let (Some(lease), Some(editing_now)) = (surface.lease(), editing.peek().clone()) else {
            ops.release();
            return;
        };
        let spec = match draft.peek().spec() {
            Ok(spec) => spec,
            // The one thing this form validates locally, because it is the one
            // thing the far side cannot: it receives argv, so a quoting
            // mistake here would send a different command rather than be
            // refused. The form stays open with the draft intact.
            Err(reason) => {
                form_error.set(Some(reason));
                ops.release();
                return;
            }
        };
        // UPDATE ONLY, and only where the helm served one: the fingerprint the
        // editor was seeded from, handed back so the far side refuses an edit
        // that would silently overwrite somebody else's change. A create
        // carrying it is refused outright, which is why it is dropped here
        // rather than sent as `None`.
        let definition = match &editing_now {
            Editing::New => None,
            Editing::Existing(_) => draft.peek().definition.clone(),
        };
        let base = base.peek().clone();
        form_error.set(None);
        // A warning describes the mutation that produced it and nothing else,
        // so it is cleared where the next one starts rather than left to
        // accumulate over a surface that has since moved on.
        warning.set(None);
        notice.set(None);
        spawn(async move {
            // Each verb states exactly the expectations it may: a create has
            // no prior definition (and the helm refuses one claiming a
            // definition it cannot have), an update names both.
            let sent = match &editing_now {
                Editing::New => {
                    create_profile(&base, lease.target.host, &spec, lease.target.expectation())
                        .await
                }
                Editing::Existing(id) => {
                    update_profile(
                        &base,
                        lease.target.host,
                        id,
                        &spec,
                        api::Expectation::replacing(lease.target.connection, definition.as_deref()),
                    )
                    .await
                }
            };
            // NOTHING below writes UI state unless this surface is still
            // asking the question this mutation was dispatched for.
            if !surface.holds(&lease) {
                ops.release();
                return;
            }
            match sent {
                Ok(ProfileCommit::Confirmed(profile, fingerprint)) => {
                    // Folded in BEFORE the token is released, which is what
                    // makes the window empty rather than merely short: until
                    // the authoritative re-read lands, reopening this row
                    // would otherwise seed the editor from the PRE-EDIT
                    // definition, and saving that would undo an update the
                    // supervisor has already accepted.
                    surface.absorb_change(&lease, CatalogChange::Upsert(profile, fingerprint));
                    editing.set(None);
                    surface.request(Trigger::Explicit);
                }
                Ok(ProfileCommit::Unvalidated(unread)) => {
                    // Accepted, and this build cannot say what it produced. The
                    // held catalog is therefore known to be superseded and
                    // cannot be reconciled from, so it is dropped rather than
                    // left to seed the next editor — the authoritative read
                    // asked for below is what fills the section back in.
                    warning.set(Some(unread));
                    surface.invalidate(&lease);
                    editing.set(None);
                    surface.request(Trigger::Explicit);
                }
                Err(refusal) => {
                    let (stale, prose) = api::precondition_of(&refusal);
                    match stale {
                        // The world moved under this editor. Never retried
                        // automatically — a resubmit would be this client
                        // insisting on a definition that is no longer the one
                        // it was shown — so the form closes, the reason stands,
                        // and the re-read below re-seeds whatever comes next.
                        true => {
                            editing.set(None);
                            notice.set(Some(prose));
                            // The refusal is PROOF the held catalog is stale —
                            // it is what the precondition compared against. So
                            // it is dropped rather than left reopenable until
                            // the authoritative read lands: an editor seeded
                            // from it would send the same doomed precondition,
                            // and the user would be told twice about one
                            // change they cannot see.
                            surface.invalidate(&lease);
                            surface.request(Trigger::Explicit);
                        }
                        // An ordinary refusal: the form STAYS open with what
                        // was typed still in it — a refused name is usually one
                        // keystroke from an accepted one, and closing it would
                        // throw the draft away with the reason still on screen.
                        false => form_error.set(Some(prose)),
                    }
                }
            }
            // Released on every path: a leaked token leaves the whole page
            // inert with nothing on screen to explain why.
            ops.release();
        });
    };

    let on_delete_confirm = move |id: String| {
        // Only ever proceeds while this profile is still the one being
        // confirmed, which is what keeps a confirm click queued behind a
        // cancel (both fired in one burst) from deleting something the user
        // just backed out of.
        if confirming.peek().as_deref() != Some(id.as_str()) {
            return;
        }
        if !ops.claim() {
            return;
        }
        let Some(lease) = surface.lease() else {
            ops.release();
            return;
        };
        confirming.set(None);
        errors.write().remove(&id);
        warning.set(None);
        notice.set(None);
        let base = base.peek().clone();
        spawn(async move {
            let sent =
                delete_profile(&base, lease.target.host, &id, lease.target.expectation()).await;
            if !surface.holds(&lease) {
                ops.release();
                return;
            }
            match sent {
                Ok(()) => {
                    // Same pre-unlock reconciliation as an edit's: the row is
                    // gone from this client's view of the catalog before
                    // anything else can act on it.
                    surface.absorb_change(&lease, CatalogChange::Remove(id));
                    surface.request(Trigger::Explicit);
                }
                Err(refusal) => {
                    let (stale, prose) = api::precondition_of(&refusal);
                    match stale {
                        true => {
                            notice.set(Some(prose));
                            // Same argument as the editor's: what this delete
                            // was refused against is the catalog on screen.
                            surface.invalidate(&lease);
                            surface.request(Trigger::Explicit);
                        }
                        false => {
                            errors.write().insert(id, prose);
                        }
                    }
                }
            }
            ops.release();
        });
    };

    // The editor and the confirmation are reconciled against every catalog
    // this surface accepts, not only against a change of target.
    //
    // The case that is otherwise invisible: a refresh for the SAME target that
    // no longer holds the profile being edited (deleted from another client,
    // or from another tab). The row and its form disappear with the row, and
    // the draft, the error line and the confirmation would go on existing with
    // nothing on screen to show for them — so the next save or confirm would
    // act on a profile that is gone. Cleared, with a line saying why: a
    // disappearance the user did not cause is exactly the kind of thing this
    // surface has to say out loud.
    use_effect(move || {
        let read = surface.catalog.read();
        let CatalogLookup::Known { catalog, .. } = surface.lookup(&read) else {
            return;
        };
        let holds = |id: &str| catalog.profiles.iter().any(|profile| profile.id == id);
        // A per-row refusal belongs to a row. Once the row is gone there is
        // nothing left to render it under, so keeping the entry would grow
        // this map for as long as the section stays open — on a surface whose
        // whole point is to be left open while a fleet changes around it.
        errors.write().retain(|id, _| holds(id));
        let open_editor = match editing.peek().clone() {
            Some(Editing::Existing(id)) => Some(id),
            _ => None,
        };
        if let Some(id) = open_editor
            && !holds(&id)
        {
            editing.set(None);
            notice.set(Some(
                "the profile you were editing is no longer on this host, so the editor was \
                 closed — nothing was saved"
                    .to_string(),
            ));
        }
        let open_prompt = confirming.peek().clone();
        if let Some(id) = open_prompt
            && !holds(&id)
        {
            confirming.set(None);
            notice.set(Some(
                "the profile you were about to delete is already gone from this host".to_string(),
            ));
        }
    });

    let read = surface.catalog.read();
    let lookup = surface.lookup(&read);
    // Cosmetic only — every handler above claims the token for itself.
    let busy = ops.busy();
    let shown_host = display_peer(&host_name);

    rsx! {
        section {
            class: "profiles-section",
            "data-profiles-host": "{host}",
            div { class: "profiles-header",
                span { class: "profiles-title", "profiles" }
                button {
                    r#type: "button",
                    class: "btn new-profile-button",
                    // This control UNMOUNTS an open form, so it must not act
                    // while anything is in flight: dropping the component
                    // mid-request strands the response with nothing left to
                    // act on it.
                    disabled: busy,
                    onclick: move |_| {
                        if ops.busy_now() {
                            return;
                        }
                        confirming.set(None);
                        form_error.set(None);
                        draft.set(ProfileDraft::blank());
                        editing.set(Some(Editing::New));
                    },
                    "new profile"
                }
            }
            // SPEC.md's snapshot rule, said where the editing happens. It is
            // the one thing about profiles that is not obvious from the
            // surface: without it, renaming a profile looks like it rewrote
            // the history of every session made from it.
            div { class: "profiles-snapshot-note",
                "editing or deleting a profile changes what FUTURE sessions launch — sessions \
                 already created keep the invocation and resume command they snapshotted"
            }
            if let Some(unread) = warning() {
                PeerLine {
                    class: "profiles-warning",
                    parts: vec![DetailPart::Peer(unread)],
                }
            }
            // A conflict or a disappearance, in the helm's own words where it
            // has any (its precondition refusals name what moved and what to
            // do). Rendered through the peer discipline like every other
            // sentence this UI did not write.
            if let Some(said) = notice() {
                PeerLine {
                    class: "profiles-notice",
                    parts: vec![DetailPart::Peer(said)],
                }
            }
            match lookup {
                CatalogLookup::Pending => rsx! {
                    div { class: "status profiles-status", "loading profiles…" }
                },
                // A catalog read fails for the ordinary reason a host
                // operation fails — the host is not connected — so the helm's
                // own sentence, which names the phase, is strictly more
                // useful than anything this side could compose.
                CatalogLookup::Failed(error) => rsx! {
                    PeerLine {
                        class: "status error profiles-error",
                        parts: vec![
                            DetailPart::text("this host's profiles could not be read: "),
                            DetailPart::peer(error),
                        ],
                    }
                },
                CatalogLookup::Known { catalog, refresh_error } => rsx! {
                    // Rows the user can still SEE, marked as possibly out of
                    // date, beat an empty section — the same choice the hosts
                    // panel makes about a failed registry refresh.
                    if let Some(error) = refresh_error {
                        PeerLine {
                            class: "status error profiles-refresh-error",
                            parts: vec![
                                DetailPart::text(
                                    "showing the last catalog this client read; the refresh \
                                     failed: ",
                                ),
                                DetailPart::peer(error),
                            ],
                        }
                    }
                    if catalog.profiles.is_empty() {
                        div { class: "status profiles-empty",
                            "this host has no profiles; sessions here are created from a typed \
                             command until one is defined"
                        }
                    }
                    div { class: "profile-list",
                        for profile in catalog.profiles.iter().cloned() {
                            ProfileRow {
                                key: "{profile.id}",
                                default_profile: catalog.default_profile.as_deref()
                                    == Some(profile.id.as_str()),
                                busy,
                                confirming: confirming.read().as_deref() == Some(profile.id.as_str()),
                                editing: *editing.read() == Some(Editing::Existing(profile.id.clone())),
                                error: errors.read().get(&profile.id).cloned(),
                                form_error: form_error.read().clone(),
                                host_name: shown_host.clone(),
                                // The fingerprint this row was SERVED with,
                                // handed to the editor so a save can say which
                                // definition it means to replace. Taken from
                                // the same read as the profile beside it, so
                                // the two cannot describe different moments.
                                definition: catalog.definitions.get(&profile.id).cloned(),
                                draft,
                                on_edit_start: move |(profile, definition): (Profile, Option<String>)| {
                                    if ops.busy_now() {
                                        return;
                                    }
                                    confirming.set(None);
                                    form_error.set(None);
                                    notice.set(None);
                                    draft.set(ProfileDraft::of(&profile, definition.as_deref()));
                                    editing.set(Some(Editing::Existing(profile.id)));
                                },
                                on_submit,
                                on_cancel: move |_| {
                                    // The LIVE token, not the `busy` prop the
                                    // form renders with: a cancel queued in
                                    // the same frame as a submit reaches this
                                    // handler with the page still looking
                                    // idle to anything computed at render
                                    // time, and closing the form there would
                                    // orphan the refusal the submit is about
                                    // to produce — leaving the user with a
                                    // failed save and nowhere it is reported.
                                    if ops.busy_now() {
                                        return;
                                    }
                                    editing.set(None);
                                    form_error.set(None);
                                },
                                on_delete_start: move |id: String| {
                                    if ops.busy_now() {
                                        return;
                                    }
                                    editing.set(None);
                                    confirming.set(Some(id));
                                },
                                on_delete_confirm,
                                on_delete_cancel: move |_| {
                                    // Same live-token guard: a cancel racing
                                    // the confirm it follows must not close a
                                    // prompt whose delete is already out.
                                    if ops.busy_now() {
                                        return;
                                    }
                                    confirming.set(None);
                                },
                                profile,
                            }
                        }
                    }
                },
            }
            // The NEW-profile form sits below the list rather than inside it:
            // it describes nothing that exists yet, so there is no row for it
            // to take the place of.
            if *editing.read() == Some(Editing::New) {
                ProfileForm {
                    draft,
                    busy,
                    error: form_error.read().clone(),
                    submit_label: "create profile",
                    on_submit,
                    on_cancel: move |_| {
                        // The live token, for the reason the row editor's own
                        // cancel gives.
                        if ops.busy_now() {
                            return;
                        }
                        editing.set(None);
                        form_error.set(None);
                    },
                }
            }
        }
    }
}

/// One profile: its definition, whether it is this host's remembered default,
/// and whichever of edit / delete / the open editor belongs there.
///
/// Every user-supplied value renders through the peer discipline
/// (`peer::display_peer` plus an isolated run), because a profile is written
/// on a machine this helm does not control under `--ssh`: a name able to
/// reorder the row around it could make a delete button appear to belong to a
/// different profile than it does.
///
/// `data-profile-id` is the browser suite's handle, on the wrapper so a test
/// can find a profile and then assert about anything inside it.
#[component]
fn ProfileRow(
    profile: Profile,
    default_profile: bool,
    busy: bool,
    confirming: bool,
    editing: bool,
    error: Option<String>,
    /// The open form's refusal, passed down so an edit's error lands under
    /// the fields it is about rather than at the bottom of the section. Only
    /// one form is ever open, so the rows that are not editing ignore it.
    form_error: Option<String>,
    /// The host's name, ALREADY escaped by the section (every row shows the
    /// same one, so escaping it once above beats escaping it per row).
    host_name: String,
    /// This profile's definition fingerprint as served, or `None` from a helm
    /// that does not serve them. Carried into the editor and back out as an
    /// update's `expected_definition`.
    definition: Option<String>,
    draft: Signal<ProfileDraft>,
    on_edit_start: EventHandler<(Profile, Option<String>)>,
    on_submit: EventHandler<()>,
    on_cancel: EventHandler<()>,
    on_delete_start: EventHandler<String>,
    on_delete_confirm: EventHandler<String>,
    on_delete_cancel: EventHandler<()>,
) -> Element {
    let id = profile.id.clone();
    let shown_name = display_peer(&profile.name);
    let shown_invocation = display_peer(&profile.invocation);
    let shown_resume = display_peer(&resume_text(profile.resume_template.as_deref()));
    let edit_target = (profile.clone(), definition.clone());

    rsx! {
        div { class: "profile-row", "data-profile-id": "{profile.id}",
            div { class: "profile-row-main",
                span { class: "profile-name peer-value", dir: "ltr", "{shown_name}" }
                span { class: "profile-kind", "{profile.agent_kind}" }
                // The remembered default is marked rather than sorted to the
                // top: the catalog's order is the supervisor's and stays
                // stable across renames, and a list that reordered itself
                // after every create would move options out from under
                // whoever was reading it.
                if default_profile {
                    span { class: "profile-default", "last used" }
                }
                if confirming {
                    // Consequence first and never truncated, then the thing
                    // it is about — the reading order every other
                    // confirmation here keeps, so a long name cannot clip the
                    // sentence that says what the button does. The
                    // consequence is also the snapshot rule stated at the
                    // exact moment it matters most.
                    span { class: "confirm-consequence",
                        "deleting a profile leaves every session already created from it running \
                         and unchanged; it only stops being offered for new ones on"
                    }
                    span { class: "confirm-title peer-value", dir: "ltr", "\"{host_name}\"" }
                    button {
                        r#type: "button",
                        class: "btn confirm-delete profile-confirm-delete",
                        disabled: busy,
                        onclick: {
                            let id = id.clone();
                            move |_| on_delete_confirm.call(id.clone())
                        },
                        "confirm delete"
                    }
                    button {
                        r#type: "button",
                        class: "btn confirm-cancel profile-cancel-delete",
                        // Focus lands on the way OUT of the destructive
                        // action, through the plain HTML attribute rather
                        // than a fallible `set_focus` whose discarded
                        // `Result` could drop the safety behavior silently.
                        autofocus: true,
                        onclick: move |_| on_delete_cancel.call(()),
                        "cancel"
                    }
                } else if !editing {
                    button {
                        r#type: "button",
                        class: "btn profile-edit",
                        disabled: busy,
                        onclick: move |_| on_edit_start.call(edit_target.clone()),
                        "edit"
                    }
                    button {
                        r#type: "button",
                        class: "btn profile-delete",
                        disabled: busy,
                        onclick: {
                            let id = id.clone();
                            move |_| on_delete_start.call(id.clone())
                        },
                        "delete"
                    }
                }
            }
            // The definition itself, on its own line: an invocation is
            // unbounded text and the row above is a controls row.
            div { class: "profile-detail",
                span { class: "profile-invocation peer-value", dir: "ltr", "{shown_invocation}" }
                if !shown_resume.is_empty() {
                    span { class: "profile-resume peer-value", dir: "ltr",
                        "resume: {shown_resume}"
                    }
                }
            }
            if editing {
                ProfileForm {
                    draft,
                    busy,
                    error: form_error,
                    submit_label: "save profile",
                    on_submit,
                    on_cancel,
                }
            }
            if let Some(error) = error {
                PeerLine {
                    class: "action-error profile-error",
                    parts: vec![DetailPart::Peer(error)],
                }
            }
        }
    }
}

/// The profile editor: one form for both creating and editing, because a
/// profile has no partial state — an edit REPLACES the whole definition
/// (`api::update_profile`), so every field has to be present in both cases or
/// saving would clear whatever was left out.
///
/// The draft belongs to the section (see [`ProfileDraft`]); this component
/// owns nothing but the layout and the submit.
///
/// Every field opts out of browser text mangling for the create form's
/// reason: a name is matched exactly by the filter surface, and an invocation
/// and a resume command are literal text that gets EXECUTED — an
/// autocapitalized command runs something the user did not type.
#[component]
fn ProfileForm(
    mut draft: Signal<ProfileDraft>,
    busy: bool,
    error: Option<String>,
    /// What the submit button reads, which is the only thing that differs
    /// between defining a profile and replacing one.
    submit_label: &'static str,
    on_submit: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    // The draft's kind, and the options to offer for it, sampled together so
    // the `selected` marks below and the list they mark cannot disagree.
    let current_kind = draft.read().agent_kind.clone();
    let kinds = kind_options(&current_kind);

    rsx! {
        form {
            class: "profile-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                if busy {
                    return;
                }
                on_submit.call(());
            },
            label {
                "name"
                input {
                    r#type: "text",
                    class: "profile-name-input",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{draft.read().name}",
                    disabled: busy,
                    oninput: move |evt| {
                        let mut draft = draft.write();
                        draft.name = evt.value();
                        // Typed in, so this field now sends what it
                        // shows rather than the raw seed behind it
                        // (`ProfileDraft::edited`).
                        draft.edited.name = true;
                    },
                }
            }
            label {
                // The kind clause is here because the failure it prevents is
                // silent: a wrapper profile left at the `generic` kind still
                // launches fine, but nothing captures its conversation, so
                // the row never offers to resume the exact conversation (a
                // placeholder-free template only gets the generic fallback
                // replay) — and nothing else in this form would tell the
                // user why.
                //
                // `{{cwd}}` is doubled to render the literal text `{cwd}`:
                // rsx! text is a format string, so a single-braced `{cwd}`
                // would be parsed as an interpolation of a `cwd` binding
                // (and fail to compile, since none exists here).
                "invocation ({{cwd}} is replaced with the session's directory; set the agent kind when the first word is a wrapper rather than the agent)"
                input {
                    r#type: "text",
                    class: "profile-invocation-input",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{draft.read().invocation}",
                    disabled: busy,
                    oninput: move |evt| {
                        let mut draft = draft.write();
                        draft.invocation = evt.value();
                        // Typed in, so this field now sends what it
                        // shows rather than the raw seed behind it
                        // (`ProfileDraft::edited`).
                        draft.edited.invocation = true;
                    },
                }
            }
            label {
                "agent kind"
                select {
                    class: "profile-kind-select",
                    disabled: busy,
                    value: "{current_kind}",
                    onchange: move |evt| draft.write().agent_kind = evt.value(),
                    // Marked on the option, not only through the select's
                    // `value`, and here the stakes are the highest of the
                    // three pickers in this UI: a select's `value` is a DOM
                    // property a browser ignores when it is applied before
                    // the matching option exists, so an editor opened on a
                    // `claude` profile could show `generic` — and a save
                    // replaces the whole definition, so it would rewrite the
                    // kind the user never touched. `kind_options` makes sure
                    // there is always an option to mark, including for a kind
                    // this build does not know.
                    for kind in kinds {
                        option {
                            key: "{kind}",
                            value: "{kind}",
                            selected: kind == current_kind,
                            "{kind}"
                        }
                    }
                }
            }
            label {
                // The rule is stated in the label rather than left to be
                // discovered: this field is argv written as one line, and
                // quoting is how an argument containing spaces is written —
                // the same vocabulary the invocation above is subject to on
                // the far side (`parse_resume`). A quote that is not closed is
                // refused with a sentence rather than guessed at.
                "resume command (optional; quote arguments containing spaces)"
                input {
                    r#type: "text",
                    class: "profile-resume-input",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{draft.read().resume}",
                    disabled: busy,
                    oninput: move |evt| {
                        let mut draft = draft.write();
                        draft.resume = evt.value();
                        // Typed in, so this field now sends what it
                        // shows rather than the raw seed behind it
                        // (`ProfileDraft::edited`).
                        draft.edited.resume = true;
                    },
                }
            }
            button {
                r#type: "submit",
                class: "btn profile-save",
                disabled: busy,
                "{submit_label}"
            }
            button {
                r#type: "button",
                class: "btn profile-cancel",
                disabled: busy,
                onclick: move |_| {
                    if busy {
                        return;
                    }
                    on_cancel.call(());
                },
                "cancel"
            }
            // The supervisor's own words — the name rule, the size cap, the
            // catalog bound, the placeholder rule — rendered through the same
            // escaping and isolation every peer string gets.
            if let Some(error) = error {
                PeerLine {
                    class: "create-session-error profile-form-error",
                    parts: vec![DetailPart::Peer(error)],
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile with the given id and name; everything else is filler,
    /// because every assertion below is about identity or existence.
    fn profile(id: &str, name: &str) -> Profile {
        Profile {
            id: id.to_string(),
            name: name.to_string(),
            invocation: "agent".to_string(),
            agent_kind: "generic".to_string(),
            resume_template: None,
        }
    }

    /// A catalog holding `profiles`, with `default_profile` remembered.
    fn catalog(profiles: Vec<Profile>, default_profile: Option<&str>) -> ProfileCatalog {
        ProfileCatalog {
            profiles,
            default_profile: default_profile.map(str::to_string),
            definitions: Default::default(),
        }
    }

    /// A target for the tests below: a host id plus an incarnation, which is
    /// what a catalog actually belongs to.
    fn target(host: HostId, incarnation: &str) -> HostTarget {
        HostTarget::new(host, 1, incarnation.to_string())
    }

    /// A catalog read for one target says NOTHING about another, however
    /// recent it is — and a HOST ID is not a target.
    ///
    /// The failure this pins is the expensive one, and the incarnation half is
    /// the sharper of the two: profile ids are minted per supervisor and every
    /// fresh supervisor seeds the same starters, so a row that has been
    /// retargeted or adopted onto another install keeps its id while its
    /// catalog becomes a different catalog with colliding ids. Answering the
    /// new question with the old answer would offer profiles that RESOLVE over
    /// there, to definitions nobody chose.
    #[test]
    fn a_catalog_answers_only_for_the_target_it_was_read_from() {
        let first = target(1, "install-a");
        let mut read = CatalogRead::default();
        read.record(
            &first,
            1,
            Ok(catalog(vec![profile("p-1", "Claude Code")], None)),
        );

        assert!(matches!(
            read.answer_for(Some(&first), 1),
            CatalogLookup::Known { .. }
        ));
        assert_eq!(
            read.answer_for(Some(&target(2, "install-b")), 1),
            CatalogLookup::Pending,
            "another host's catalog is not a stale answer to this question; it is an answer to a \
             different one"
        );
        assert_eq!(
            read.answer_for(Some(&target(1, "install-b")), 1),
            CatalogLookup::Pending,
            "and the SAME registry row on another install is another question too — this is what \
             a retarget or an adopt does, with the id unchanged"
        );

        // And re-pointing DISCARDS rather than merges: the previous install's
        // profiles must not be on screen for even one frame under the new one.
        let second = target(2, "install-b");
        read.record(
            &second,
            2,
            Err("host 2 is unreachable-reprobing".to_string()),
        );
        assert_eq!(
            read.answer_for(Some(&second), 2),
            CatalogLookup::Failed("host 2 is unreachable-reprobing")
        );
        assert_eq!(read.answer_for(Some(&first), 1), CatalogLookup::Pending);
    }

    /// A surface reopened on the SAME target reads as pending until its own
    /// read lands.
    ///
    /// What it held is not evidence about now: a remembered default can have
    /// moved (another client created from a different profile) and profiles
    /// can have come and gone while this surface was closed. Presenting the
    /// old answer for the moment before the new read lands is exactly long
    /// enough for a create to be submitted against it.
    #[test]
    fn a_reopened_surface_does_not_present_what_it_held_as_current() {
        let host = target(1, "install-a");
        let mut read = CatalogRead::default();
        read.record(
            &host,
            1,
            Ok(catalog(vec![profile("p-1", "Codex")], Some("p-1"))),
        );

        assert_eq!(
            read.answer_for(Some(&host), 2),
            CatalogLookup::Pending,
            "a later activation of the same target has not been answered yet"
        );
        // Nothing is lost by that: the activation's own read replaces it.
        read.record(
            &host,
            2,
            Ok(catalog(vec![profile("p-2", "Claude Code")], None)),
        );
        match read.answer_for(Some(&host), 2) {
            CatalogLookup::Known { catalog, .. } => {
                assert_eq!(catalog.profiles[0].id, "p-2");
                assert_eq!(catalog.default_profile, None);
            }
            other => panic!("the new activation's read must land, got {other:?}"),
        }
    }

    /// A failed refresh keeps the catalog on screen and says so; it does not
    /// blank the section.
    ///
    /// The same choice the hosts panel makes about the registry, for the same
    /// reason: rows the user can still act on, marked as possibly out of
    /// date, beat an empty box — and a profile section that emptied itself
    /// whenever one read dropped would look like the host had lost its
    /// profiles.
    #[test]
    fn a_failed_refresh_keeps_the_catalog_and_reports_itself() {
        let host = target(1, "install-a");
        let mut read = CatalogRead::default();
        read.record(
            &host,
            1,
            Ok(catalog(vec![profile("p-1", "Codex")], Some("p-1"))),
        );
        read.record(&host, 1, Err("the helm did not answer".to_string()));

        match read.answer_for(Some(&host), 1) {
            CatalogLookup::Known {
                catalog,
                refresh_error,
            } => {
                assert_eq!(catalog.profiles.len(), 1);
                assert_eq!(refresh_error, Some("the helm did not answer"));
            }
            other => panic!("a held catalog must survive a failed refresh, got {other:?}"),
        }

        // And the next success clears the line rather than leaving a
        // permanent warning over rows that were just refreshed.
        read.record(
            &host,
            1,
            Ok(catalog(vec![profile("p-1", "Codex")], Some("p-1"))),
        );
        assert!(matches!(
            read.answer_for(Some(&host), 1),
            CatalogLookup::Known {
                refresh_error: None,
                ..
            }
        ));
    }

    /// A mutation this client performed is folded into the held catalog at
    /// once — and only into the catalog it was made against.
    ///
    /// The window this closes is small and the failure inside it is durable: a
    /// save answers with the new definition, the operation token is released,
    /// and until the authoritative re-read lands the section would otherwise
    /// still seed an editor from the definition that was just replaced.
    /// Saving THAT would undo an update the supervisor accepted, with nothing
    /// on screen to suggest anything went wrong.
    #[test]
    fn a_committed_mutation_is_folded_in_before_the_authoritative_read() {
        let host = target(1, "install-a");
        let mut read = CatalogRead::default();
        read.record(&host, 1, Ok(catalog(vec![profile("p-1", "Before")], None)));

        read.absorb(
            &host,
            1,
            CatalogChange::Upsert(profile("p-1", "After"), Some("after-fp".to_string())),
        );
        read.absorb(
            &host,
            1,
            CatalogChange::Upsert(profile("p-2", "Fresh"), Some("fresh-fp".to_string())),
        );
        // The fingerprint moves WITH the definition: a stale one left behind
        // is what the next editor would send as its precondition, and the helm
        // would refuse it over a change this client itself just made.
        match read.answer_for(Some(&host), 1) {
            CatalogLookup::Known { catalog, .. } => {
                assert_eq!(
                    catalog.definitions.get("p-1").map(String::as_str),
                    Some("after-fp")
                );
                assert_eq!(
                    catalog.definitions.get("p-2").map(String::as_str),
                    Some("fresh-fp"),
                    "a created profile is guarded from its FIRST edit, which needs a fingerprint \
                     it never had before"
                );
            }
            other => panic!("the catalog must still be readable, got {other:?}"),
        }
        match read.answer_for(Some(&host), 1) {
            CatalogLookup::Known { catalog, .. } => {
                assert_eq!(
                    catalog.profiles[0].name, "After",
                    "an edit replaces in place"
                );
                assert_eq!(catalog.profiles[1].id, "p-2", "a create appears at once");
            }
            other => panic!("the catalog must still be readable, got {other:?}"),
        }

        read.absorb(&host, 1, CatalogChange::Remove("p-1".to_string()));
        match read.answer_for(Some(&host), 1) {
            CatalogLookup::Known { catalog, .. } => {
                assert_eq!(catalog.profiles.len(), 1);
                assert_eq!(catalog.profiles[0].id, "p-2");
                assert!(
                    !catalog.definitions.contains_key("p-1"),
                    "a deleted profile's fingerprint goes with it"
                );
            }
            other => panic!("the catalog must still be readable, got {other:?}"),
        }

        // A change made against a question this surface is no longer asking
        // is dropped rather than applied to whatever it holds now — the same
        // rule the read path keeps, from the write side.
        read.absorb(
            &target(1, "install-b"),
            1,
            CatalogChange::Upsert(profile("p-9", "Elsewhere"), None),
        );
        read.absorb(
            &host,
            2,
            CatalogChange::Upsert(profile("p-8", "Later"), None),
        );
        match read.answer_for(Some(&host), 1) {
            CatalogLookup::Known { catalog, .. } => assert_eq!(catalog.profiles.len(), 1),
            other => panic!("the catalog must be untouched, got {other:?}"),
        }
    }

    /// The first catalog to answer decides, once — and a remembered default
    /// that is GONE decides nothing, which is what leaves the dialog asking.
    ///
    /// The three outcomes are SPEC.md's creation rule in one table, and the
    /// middle one is the whole point: substituting another profile there would
    /// launch an agent nobody chose from a dialog that looks like it
    /// remembered a preference, and substituting the command field would run
    /// whatever was typed into it earlier.
    #[test]
    fn the_first_catalog_decides_once_and_a_deleted_default_decides_nothing() {
        let held = catalog(
            vec![profile("p-1", "Claude Code"), profile("p-2", "Codex")],
            Some("p-2"),
        );
        assert_eq!(
            seeded_choice(&held),
            Some(AgentChoice::Profile("p-2".to_string()))
        );

        let gone = catalog(vec![profile("p-1", "Claude Code")], Some("p-deleted"));
        assert_eq!(
            seeded_choice(&gone),
            None,
            "a deleted default must be asked about — never replaced by another profile, and never \
             by the command field, which is not empty"
        );

        // Nothing has ever been created from a profile here: there is no
        // forgotten choice, so there is nothing to explain and the dialog
        // stays immediately usable.
        let fresh = catalog(vec![profile("p-1", "Claude Code")], None);
        assert_eq!(seeded_choice(&fresh), Some(AgentChoice::Command));
    }

    /// Once a dialog has been seeded, "nothing chosen" means the ask —
    /// permanently, until the user answers or the target moves.
    ///
    /// The failure this pins is a silent retarget with a different cause than
    /// the host ones: another client creating a session moves the helm's
    /// remembered id, this page re-reads on the notification, and a dialog
    /// that still consulted the default would change its selection under
    /// whoever was filling it in. The latch is what makes the default a
    /// decision this dialog made rather than a value it follows.
    #[test]
    fn a_seeded_dialog_never_follows_a_later_remembered_default() {
        let moved = catalog(
            vec![profile("p-1", "Claude Code"), profile("p-2", "Codex")],
            Some("p-2"),
        );
        assert_eq!(
            resolve_agent(None, Some(&moved), true),
            AgentSelection {
                choice: None,
                note: Some(AgentNote::RememberedGone),
            },
            "a seeded dialog holding no choice is the ask case, whatever the catalog now \
             remembers"
        );
    }

    /// An explicit choice outranks the remembered default, and survives every
    /// re-read that still holds it.
    #[test]
    fn an_explicit_choice_outranks_the_remembered_default() {
        let held = catalog(
            vec![profile("p-1", "Claude Code"), profile("p-2", "Codex")],
            Some("p-2"),
        );
        assert_eq!(
            resolve_agent(
                Some(&AgentChoice::Profile("p-1".to_string())),
                Some(&held),
                true
            ),
            AgentSelection {
                choice: Some(AgentChoice::Profile("p-1".to_string())),
                note: None,
            }
        );
        // Including the choice to type a command, which must never be
        // overridden by a remembered profile — a user who selected the
        // command path has said what they want.
        assert_eq!(
            resolve_agent(Some(&AgentChoice::Command), Some(&held), true),
            AgentSelection {
                choice: Some(AgentChoice::Command),
                note: None,
            }
        );
    }

    /// A chosen profile that leaves the catalog BLOCKS the create, visibly.
    ///
    /// A real race rather than a hypothetical: the profiles section sits
    /// directly above this dialog, and the feed re-reads the catalog in both,
    /// so a delete made in the panel — or in another browser — reaches an open
    /// dialog within one notification. Two failures are ruled out at once: a
    /// picker still displaying a profile the create would not use, and a
    /// picker that silently reverts to the command field, which still holds
    /// whatever was typed there before the profile was chosen.
    #[test]
    fn a_chosen_profile_that_leaves_the_catalog_blocks_rather_than_falling_back() {
        let without = catalog(vec![profile("p-1", "Claude Code")], None);
        assert_eq!(
            resolve_agent(
                Some(&AgentChoice::Profile("p-gone".to_string())),
                Some(&without),
                true
            ),
            AgentSelection {
                choice: None,
                note: Some(AgentNote::ChoiceGone),
            }
        );
    }

    /// Before any catalog has been read, NOTHING is selected — whether or not
    /// a profile has been picked.
    ///
    /// SPEC.md's ask-don't-guess rule read conservatively: a dialog that has
    /// not yet learned whether its host remembers a profile cannot honestly
    /// default to the command field, which may already hold text typed for a
    /// different intention. The state is transient by construction (it clears
    /// when the read lands) and always escapable — choosing "custom command"
    /// is an answer, and so is typing into the command field, which records
    /// the same choice.
    #[test]
    fn an_unread_catalog_confirms_nothing_at_all() {
        assert_eq!(
            resolve_agent(None, None, false),
            AgentSelection {
                choice: None,
                note: Some(AgentNote::ChoiceUnconfirmed),
            },
            "before the catalog answers, this dialog does not know whether the host remembers a \
             profile — so it asks rather than defaulting to a command field that may not be empty"
        );
        assert_eq!(
            resolve_agent(Some(&AgentChoice::Profile("p-1".to_string())), None, false),
            AgentSelection {
                choice: None,
                note: Some(AgentNote::ChoiceUnconfirmed),
            },
            "a choice nothing has confirmed must not be acted on, in either direction"
        );
        // An explicit command choice needs no catalog at all: it names the
        // field below, which is always there.
        assert_eq!(
            resolve_agent(Some(&AgentChoice::Command), None, false),
            AgentSelection {
                choice: Some(AgentChoice::Command),
                note: None,
            }
        );
    }

    /// The picker's option values round-trip, and the placeholder is not a
    /// choice.
    ///
    /// A select names every choice with a string, so this pairing is what
    /// stands between a chosen profile and a create that sends the wrong mode.
    /// The placeholder decoding to `None` is the load-bearing half: it is what
    /// a blocked dialog SHOWS, and reading it as the command path would turn
    /// "nothing is selected" back into a silent launch of the field below.
    #[test]
    fn every_agent_choice_round_trips_through_its_option_value() {
        for choice in [AgentChoice::Command, AgentChoice::Profile("p-1".into())] {
            assert_eq!(AgentChoice::from_value(choice.value()), Some(choice));
        }
        assert_eq!(AgentChoice::from_value(UNRESOLVED_VALUE), None);
    }

    /// A kind this build does not know must still be offered — and therefore
    /// preserved — when its profile is edited.
    ///
    /// Without the extra option the select renders empty and the save writes
    /// whichever kind the browser then reports, silently changing the one
    /// field that decides whether capture and status sharpening run. The
    /// known kinds must not be duplicated when the current one is among them.
    #[test]
    fn an_unknown_agent_kind_is_offered_so_an_edit_cannot_rewrite_it() {
        assert_eq!(kind_options("claude"), vec!["generic", "claude", "codex"]);
        assert_eq!(
            kind_options("novel-agent"),
            vec!["generic", "claude", "codex", "novel-agent"]
        );
    }

    /// The resume field is AUTHORABLE, not merely displayable: an argument
    /// containing a space can be WRITTEN, and what is written comes back the
    /// same way.
    ///
    /// SPEC.md's promise is that a user controls the invocations completely,
    /// and the earlier whitespace split did not keep it — such an argument
    /// could be preserved from a seed but never typed, so the only way to have
    /// one was to have had it already. The round trip is the contract: what
    /// the field shows parses back to exactly what it shows.
    #[test]
    fn a_resume_argument_containing_spaces_can_be_written_and_read_back() {
        let authored = parse_resume("claude --resume {conversation} --note='two words'")
            .expect("a quoted argument parses");
        assert_eq!(
            authored,
            Some(vec![
                "claude".to_string(),
                "--resume".to_string(),
                "{conversation}".to_string(),
                "--note=two words".to_string(),
            ])
        );
        assert_eq!(
            parse_resume(&resume_text(authored.as_deref())).expect("the displayed form parses"),
            authored,
            "the field has to round-trip, or a save would rewrite an argv nobody edited"
        );

        // The awkward ones round-trip too: a literal quote, an EMPTY argument
        // (a real one, which an unquoted join would lose), a backslash, and
        // surrounding whitespace.
        for argv in [
            vec!["it's".to_string()],
            vec!["a".to_string(), String::new(), "b".to_string()],
            vec!["back\\slash".to_string()],
            vec!["  padded  ".to_string()],
        ] {
            let shown = resume_text(Some(&argv));
            assert_eq!(
                parse_resume(&shown).expect("every displayed form parses"),
                Some(argv.clone()),
                "{shown:?} must read back as what it displays"
            );
        }
    }

    /// A quoting mistake is REFUSED with a sentence rather than guessed at.
    ///
    /// The alternative is worse than an error: an unclosed quote silently
    /// swallowing the rest of the line would save a resume command the user
    /// did not write, and a resume command is what a restart executes.
    #[test]
    fn an_unparseable_resume_command_is_refused_rather_than_guessed_at() {
        let unclosed = parse_resume("claude --note='two words").expect_err("an unclosed quote");
        assert!(unclosed.contains("unclosed"), "got {unclosed:?}");
        let trailing = parse_resume("claude \\").expect_err("a dangling escape");
        assert!(trailing.contains("backslash"), "got {trailing:?}");

        // An empty field is not a mistake: it is an explicit "no resume
        // template", which the supervisor acts on.
        assert_eq!(parse_resume("").expect("empty is a value"), None);
        assert_eq!(parse_resume("   ").expect("blank is a value"), None);
    }

    /// A draft seeded from a profile round-trips the whole definition, so
    /// saving an untouched form changes nothing.
    ///
    /// The failure this rules out is silent and total: a save replaces the
    /// definition, so any field the editor did not carry would be cleared by
    /// the first rename anyone performs.
    #[test]
    fn an_untouched_draft_saves_the_profile_it_was_opened_on() {
        let stored = Profile {
            resume_template: Some(vec![
                "claude".into(),
                "--resume".into(),
                "{conversation}".into(),
            ]),
            agent_kind: "claude".to_string(),
            invocation: "claude --dangerously-skip-permissions".to_string(),
            ..profile("p-1", "Claude Code")
        };
        let spec = ProfileDraft::of(&stored, Some("fingerprint"))
            .spec()
            .expect("an untouched draft needs no parsing");
        assert_eq!(spec.name, stored.name);
        assert_eq!(spec.invocation, stored.invocation);
        assert_eq!(spec.agent_kind, stored.agent_kind);
        assert_eq!(spec.resume_template, stored.resume_template);
    }

    /// An UNTOUCHED field saves the bytes it was seeded with; an EDITED one
    /// saves exactly what the user typed — decided by input events, never by
    /// comparing the field against its seed.
    ///
    /// The comparison approach has a hole this pins shut. The seed is
    /// DISPLAYED escaped, so equality would have to be against the escaped
    /// spelling — and someone who deliberately retypes that spelling (turning
    /// an active right-to-left override into the visible `<U+202E>` text,
    /// which is exactly what cleaning up a hostile name looks like) would have
    /// the control characters silently restored. Two intentions, one string;
    /// only an input event tells them apart.
    #[test]
    fn a_deliberately_retyped_escape_is_saved_as_typed() {
        let stored = Profile {
            name: "Claude\u{202E}Code".to_string(),
            invocation: "claude --flag\u{200B}x".to_string(),
            resume_template: Some(vec!["claude".into(), "--note=a b".into()]),
            ..profile("p-1", "unused")
        };
        let draft = ProfileDraft::of(&stored, Some("fingerprint"));
        assert_eq!(
            draft.name, "Claude<U+202E>Code",
            "the field has to say what is stored"
        );

        // Untouched: the raw bytes survive, however exotic — including the
        // resume argv, which the display quotes.
        let untouched = draft.spec().expect("an untouched draft needs no parsing");
        assert_eq!(untouched.name, stored.name);
        assert_eq!(untouched.invocation, stored.invocation);
        assert_eq!(untouched.resume_template, stored.resume_template);

        // Retyped to EXACTLY the escaped spelling — the case equality cannot
        // see. What the user typed is what is saved.
        let mut retyped = draft.clone();
        retyped.name = "Claude<U+202E>Code".to_string();
        retyped.edited.name = true;
        let sent = retyped.spec().expect("nothing to parse");
        assert_eq!(
            sent.name, "Claude<U+202E>Code",
            "a field the user typed in saves as typed, even when it happens to equal the escaped \
             rendering of what was there"
        );
        assert_eq!(
            sent.invocation, stored.invocation,
            "and editing one field must not rewrite the others"
        );
    }

    /// A new profile starts GENERIC, claiming no integration.
    ///
    /// Defaulting to an integrated kind would apply that kind's status
    /// heuristics and conversation capture to whatever the user typed next —
    /// a decision they never made, and one whose effects (a wrong resume
    /// command on restart) show up long afterwards.
    #[test]
    fn a_new_profile_claims_no_integration_until_one_is_chosen() {
        let blank = ProfileDraft::blank();
        assert_eq!(blank.agent_kind, "generic");
        assert!(blank.name.is_empty());
        assert!(blank.invocation.is_empty());
        assert_eq!(
            blank.spec().expect("a blank draft parses").resume_template,
            None
        );
    }

    /// A row names the profile a session was created FROM, as snapshotted,
    /// with what has since become of it said out loud.
    ///
    /// Both halves are SPEC.md's snapshot rule seen from the list: the name
    /// never changes under an existing session (so a rename cannot rewrite
    /// history), and the qualifier is what stops that name from reading as a
    /// claim about today's catalog. An unknown existence is qualified rather
    /// than rounded to "present", because claiming a profile still exists is
    /// a statement and a word this build cannot read is not grounds for it.
    #[test]
    fn a_snapshotted_profile_reads_as_a_snapshot() {
        let source = |existence| SourceProfile {
            id: "p-1".to_string(),
            name: "Claude Code".to_string(),
            existence,
        };
        assert_eq!(
            source_profile_label(&source(ProfileExistence::Present)),
            "profile: Claude Code"
        );
        assert_eq!(
            source_profile_label(&source(ProfileExistence::Renamed)),
            "profile: Claude Code (renamed since)"
        );
        assert_eq!(
            source_profile_label(&source(ProfileExistence::Deleted)),
            "profile: Claude Code (deleted since)"
        );
        assert_eq!(
            source_profile_label(&source(ProfileExistence::Unrecognized)),
            "profile: Claude Code (state unknown to this build)"
        );

        // The name is peer-supplied and is escaped like every other rendering
        // of one: a directional override inside a profile name could
        // otherwise reorder the row it sits in.
        let hostile = SourceProfile {
            id: "p-2".to_string(),
            name: "Claude\u{202E}Code".to_string(),
            existence: ProfileExistence::Present,
        };
        assert_eq!(
            source_profile_label(&hostile),
            "profile: Claude<U+202E>Code"
        );
    }

    /// The DOM's existence vocabulary is the WIRE's, one word per state and
    /// no two alike.
    ///
    /// A browser test keys off these, and so does anyone comparing a row
    /// against the helm's JSON — the same reason the hosts panel chips the
    /// helm's own phase words rather than friendlier synonyms. Two states
    /// sharing a word would make an assertion about a renamed profile pass
    /// for a deleted one.
    #[test]
    fn every_existence_has_its_own_wire_word() {
        let words = [
            existence_word(ProfileExistence::Present),
            existence_word(ProfileExistence::Renamed),
            existence_word(ProfileExistence::Deleted),
            existence_word(ProfileExistence::Unrecognized),
        ];
        assert_eq!(words, ["present", "renamed", "deleted", "unrecognized"]);
    }
}
