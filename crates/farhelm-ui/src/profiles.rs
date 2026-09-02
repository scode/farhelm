//! Agent profiles as the UI handles the helm's one catalog: the shared
//! feed-driven read, the sidebar popup that manages definitions, and the
//! renderer-free rules the create dialog's picker uses.
//!
//! ## One catalog, one reader
//!
//! Profiles belong to the helm. Every host uses the same ids and definitions,
//! so neither the catalog nor a chosen profile follows the create dialog's
//! host selector. [`CatalogSurface`] is mounted once for the authenticated
//! session-list page and stays active while that page is mounted. The picker
//! and popup consume that same answer; neither owns a second request path.
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
//! ([`source_profile_label`]), marking a rename while treating a missing row
//! as an ordinary historical label, and the profiles popup says out loud what
//! an edit does and does not reach. Neither is decoration: without them a
//! renamed profile looks like it rewrote history.
//!
//! ## The reader follows the fleet feed
//!
//! [`use_catalog_surface`] wires the catalog read into the same discipline the
//! listing and hosts reads run under (`reader::SurfaceReader`): one read at a
//! time, every trigger coalesced into a single follow-up, a failed read
//! retried on its own. Its triggers are mount, every feed notification, the
//! documented fallback poll while the feed is unhealthy, and a mutation's own
//! follow-up. Keeping the reader alive while both consumers are closed
//! matters: another client's edit still advances the one answer the next
//! popup or dialog will render.

use std::collections::HashMap;
use std::time::Duration;

use dioxus::prelude::*;
use web_time::Instant;

use crate::api::{
    self, ProfileCatalog, ProfileCommit, ProfileSpec, create_profile, delete_profile,
    update_profile,
};
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::ops::{OpLock, ReadGate};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::reader::{SurfaceReader, Trigger, finish_before, request_read, sleep_ms};
use crate::{ApiBase, Profile, ProfileExistence, SourceProfile};

// ---------------------------------------------------------------------
// The catalog read, as four states
// ---------------------------------------------------------------------

/// What this client currently knows about the helm's profile catalog.
///
/// The same four-state shape `hosts::HostsRead` carries, for the same reason:
/// a failed refresh must not blank rows the user can still act on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CatalogRead {
    /// The last read that succeeded, retained across later failures.
    catalog: Option<ProfileCatalog>,
    /// Set when the most recent read failed; cleared by the next success.
    error: Option<String>,
}

impl CatalogRead {
    /// Report whether recording this reply would change either rendered fact.
    ///
    /// Mutation replies are absorbed before their confirming read. Skipping an
    /// identical confirmation keeps keyed profile controls mounted, which in
    /// turn preserves the explicit focus the mutation just established.
    fn differs_from(&self, outcome: &Result<ProfileCatalog, String>) -> bool {
        match outcome {
            Ok(catalog) => self.catalog.as_ref() != Some(catalog) || self.error.is_some(),
            Err(error) => self.error.as_ref() != Some(error),
        }
    }

    /// Fold one completed read into the shared answer.
    fn record(&mut self, outcome: Result<ProfileCatalog, String>) {
        match outcome {
            Ok(catalog) => {
                self.catalog = Some(catalog);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    /// Fold a mutation this client PERFORMED into the held catalog.
    ///
    /// Not an optimization and not an optimistic paint: it closes a window
    /// where the page would otherwise hand out a definition it knows to be
    /// superseded. A save answers with the profile as the helm now
    /// holds it, but the authoritative re-read is a round trip away — and in
    /// between, the operation token is released and the row can be reopened,
    /// seeding an editor from the PRE-EDIT definition. Saving that would undo
    /// an update the helm already accepted, silently.
    ///
    /// The result says whether a complete catalog was available to absorb
    /// into. It does not say whether the change altered the catalog: removing
    /// an already-absent id still returns `true` when a catalog was held.
    fn absorb(&mut self, change: CatalogChange) -> bool {
        let Some(catalog) = self.catalog.as_mut() else {
            return false;
        };
        match change {
            CatalogChange::Upsert(profile) => {
                match catalog
                    .profiles
                    .iter_mut()
                    .find(|held| held.id == profile.id)
                {
                    Some(held) => *held = profile,
                    // A create lands at the end rather than in the
                    // helm's own order; the authoritative read that
                    // follows restores that order, and until it does the
                    // profile is at least THERE.
                    None => catalog.profiles.push(profile),
                }
            }
            CatalogChange::Remove(id) => {
                catalog.profiles.retain(|held| held.id != id);
            }
        }
        true
    }

    /// Forget the held answer so the surface reads as pending until its next
    /// read lands. See [`CatalogSurface::invalidate`].
    fn forget(&mut self) {
        self.catalog = None;
        self.error = None;
    }

    /// What can be said about the helm catalog now.
    pub(crate) fn answer(&self) -> CatalogLookup<'_> {
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

/// One change this client made and has already been told succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CatalogChange {
    /// A profile as the helm now holds it — a create or an edit.
    Upsert(Profile),
    /// A profile that is gone, by id.
    Remove(String),
}

/// The four states in which the shared catalog can be rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CatalogLookup<'a> {
    /// Nothing has come back yet, or an unreadable successful mutation made
    /// the prior answer unsafe to keep serving.
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

/// Keep the strongest catalog trigger waiting for the page-owned driver.
///
/// A signal is a mailbox here, not a queue: several requests can arrive
/// before the owning effect runs, and the existing single-flight reader will
/// coalesce everything after that. Preserving the strongest trigger keeps an
/// attended request from being replaced by a fallback tick that would stand
/// down under build skew.
fn strongest_catalog_trigger(standing: Option<Trigger>, arriving: Trigger) -> Trigger {
    let strength = |trigger| match trigger {
        Trigger::Scheduled => 0,
        Trigger::Notice => 1,
        Trigger::Explicit => 2,
    };
    match standing {
        Some(current) if strength(current) >= strength(arriving) => current,
        Some(_) | None => arriving,
    }
}

/// The helm catalog's answer and the single-flight reader that keeps it fresh.
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
    /// The answer both consumers render from.
    pub(crate) catalog: Signal<CatalogRead>,
    /// Per-request ordering, so a slow read cannot overwrite a newer answer.
    gate: Signal<ReadGate>,
    /// The single-flight, retry-until-answered reader (`reader`).
    reader: Signal<SurfaceReader>,
    /// The strongest trigger the page-owned driver has not dispatched yet.
    pending: Signal<Option<Trigger>>,
}

impl CatalogSurface {
    /// Ask the page-owned catalog driver for a refresh.
    ///
    /// Consumer components only write this bounded mailbox. The driver lives
    /// in [`use_catalog_surface`], so a popup or create form unmount cannot
    /// cancel the shared reader it requested and leave its state claimed.
    pub(crate) fn request(mut self, trigger: Trigger) {
        let trigger = strongest_catalog_trigger(*self.pending.peek(), trigger);
        self.pending.set(Some(trigger));
    }

    /// Perform one read and fold it in; the answer is whether the helm
    /// answered at all (`reader::SurfaceReader::finish`).
    async fn complete(&mut self, generation: u64, base: String) -> bool {
        let outcome = api::fetch_profiles(&base).await;
        let answered = outcome.is_ok();
        // Successes and failures are gated differently, exactly as the hosts
        // read gates them: an older success describes a catalog that has
        // since been changed by something this client did, while a failure
        // newer than what is on screen is worth reporting even though a later
        // read has already begun.
        let accepted = match &outcome {
            Ok(_) => self.gate.write().accept_success(generation),
            Err(_) => self.gate.peek().accept_failure(generation),
        };
        if accepted && self.catalog.peek().differs_from(&outcome) {
            self.catalog.write().record(outcome);
        }
        answered
    }

    /// Drop the held catalog, leaving the surface pending until its next read.
    ///
    /// For the one success this client cannot reconcile from: a 2xx whose body
    /// would not decode changed something the page cannot describe, so
    /// continuing to serve the pre-mutation catalog would let the next editor
    /// seed from a definition that is known to be superseded and save it back.
    /// Showing nothing until the authoritative read arrives is the honest
    /// state, and the read is already on its way.
    pub(crate) fn invalidate(mut self) {
        self.gate.write().fence();
        self.catalog.write().forget();
    }

    /// Fold a mutation this client just performed into the held catalog, so
    /// consumers stop receiving a definition known to be superseded
    /// (see [`CatalogRead::absorb`]). Called BEFORE the operation token is
    /// released, which is what makes the window it closes empty rather than
    /// merely short.
    ///
    /// The result says whether an existing complete catalog received the
    /// change, not whether applying the change altered its contents. `false`
    /// therefore means a caller cannot yet assume the changed row is rendered.
    pub(crate) fn absorb_change(mut self, change: CatalogChange) -> bool {
        self.gate.write().fence();
        self.catalog.write().absorb(change)
    }
}

/// Wire the page's one catalog surface with every trigger its reads use.
///
/// Called once by `list::ListView` and shared by the popup and picker. The
/// mount read is deliberately unconditional: a closed consumer does not turn
/// the helm catalog back into a surface-local resource.
///
/// Four triggers, matching the session list's reader discipline:
///
/// - the page mount;
/// - every feed notification, because a profile edited in another client
///   bumps the fleet revision and this surface is one of the things that
///   invalidates (PLAN_M6_75.md item 5 names the create dialog explicitly);
/// - the documented fallback poll, which runs only while the feed is
///   unhealthy and no build mismatch has been latched;
/// - a mutation's own follow-up, through [`CatalogSurface::request`].
pub(crate) fn use_catalog_surface() -> CatalogSurface {
    let api_base = use_context::<ApiBase>().0;
    let mut surface = CatalogSurface {
        base: use_signal(|| api_base),
        catalog: use_signal(CatalogRead::default),
        gate: use_signal(ReadGate::default),
        reader: use_signal(SurfaceReader::default),
        pending: use_signal(|| None),
    };

    // This is the only place a catalog reader task starts, so the task belongs
    // to the page that owns the catalog rather than whichever short-lived
    // consumer happened to ask. Clearing the mailbox before dispatch lets a
    // request made by the dispatch itself schedule a later pass normally.
    use_effect(move || {
        let Some(trigger) = (surface.pending)() else {
            return;
        };
        surface.pending.set(None);
        request_read(surface.reader, trigger, move || {
            let mut surface = surface;
            // Claimed synchronously so generations order requests by when
            // they were asked for, not when their tasks happened to be polled.
            let generation = surface.gate.write().start();
            let base = surface.base.peek().clone();
            async move { surface.complete(generation, base).await }
        });
    });

    // Effects are not places to await round trips. Mark the initial demand;
    // the shared reader owns retries and coalescing from here onward.
    use_effect(move || {
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

/// What a create would launch: a profile from the helm catalog, or the command
/// line typed into the form.
///
/// The raw path survives alongside profiles deliberately (PLAN_M6_75.md item
/// 4): it is what the API, the e2e harness, and anyone running something no
/// profile describes uses, and removing it from the dialog would make the
/// only way to run an ad-hoc command a trip to `curl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentChoice {
    /// The invocation typed into the form's own field.
    Command,
    /// A helm profile by id. The choice survives target-host changes because
    /// the same catalog applies to every host.
    Profile(String),
}

/// The `<option>` value of the picker's placeholder — the entry that stands
/// for "you have not said yet", shown only while the selection is unresolved.
///
/// A sentinel is unavoidable (a `size=1` select always has exactly one option
/// selected, so "nothing chosen" needs something to select), and this one is
/// chosen to fail SAFE if the helm ever minted a profile id equal to it:
/// the picker would refuse to resolve that profile, blocking a submit, rather
/// than launching something the user did not pick. The command path's empty
/// value has the same property from the other side — the helm does not
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
/// The ordinary "nothing has ever been created from a profile on this helm"
/// case yields no note at all, and that asymmetry is the point: a first-time
/// dialog has nothing to explain, while a dialog that silently dropped a
/// remembered choice would look like it had forgotten it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentNote {
    /// The helm's remembered last-used profile is gone from its catalog —
    /// SPEC.md's ask-don't-guess case, exactly.
    RememberedGone,
    /// A profile the user had explicitly picked left the catalog while the
    /// dialog was open (deleted from another client, or from the popup).
    ChoiceGone,
    /// A profile is chosen but this surface has no catalog to confirm it
    /// against — before the first helm catalog read, or after one that failed.
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
                "the profile you last used in this helm no longer exists, so nothing is selected \
                 — choose a profile, or choose \"custom command\" to run the command below"
            }
            AgentNote::ChoiceGone => {
                "the profile you picked is no longer in this helm, so nothing is selected — \
                 choose another, or choose \"custom command\" to run the command below"
            }
            AgentNote::ChoiceUnconfirmed => {
                "this helm's profiles have not been read yet, so the profile you picked cannot be \
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

/// What the first catalog to answer a dialog's own question decides, once.
///
/// The consumption half of SPEC.md's creation rule, split out from the
/// component so the decision can be stated and tested without a runtime. Three
/// outcomes, and the middle one is why this returns an `Option` rather than a
/// choice:
///
/// - the remembered default still exists → that is the choice;
/// - a default is remembered but is GONE → no choice, which leaves the dialog
///   blocked and asking (SPEC.md's ask-don't-guess);
/// - nothing was ever remembered here → the command path, so the helm is
///   usable immediately and has nothing to explain.
///
/// Consulted once per dialog, whatever it answers — see
/// `list::CreateSessionForm` for why "a choice exists" is not a usable record
/// of that having happened.
pub(crate) fn seeded_choice(catalog: &ProfileCatalog) -> Option<AgentChoice> {
    match catalog.default_profile.as_deref() {
        Some(remembered) if catalog.profiles.iter().any(|p| p.id == remembered) => {
            Some(AgentChoice::Profile(remembered.to_string()))
        }
        Some(_) => None,
        None => Some(AgentChoice::Command),
    }
}

/// Resolve the create dialog's current agent choice without guessing.
///
/// An explicit choice wins. A chosen or remembered profile that is absent or
/// unconfirmed blocks rather than falling back to the command field, whose
/// retained text may describe a different intention. `seeded` makes the
/// remembered default a one-time dialog decision instead of a value that an
/// open form follows when another client changes it.
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
        // whether the helm remembers a profile, so defaulting to the command
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
/// reason: the helm refuses a kind it does not know, and a select
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
/// rename qualifier keeps that case from reading as a claim about the catalog
/// as it stands today. A missing row is intentionally plain: old sessions and
/// sessions imported from another helm still have a useful immutable label,
/// and absence from this helm is not a warning about the session.
///
/// An existence this build does not know is qualified as unknown rather than
/// silently rendered as `present`: claiming a profile is still there is a
/// statement, and a word this build cannot read is not grounds for it.
pub(crate) fn source_profile_label(source: &SourceProfile) -> String {
    let name = display_peer(&source.name);
    match source.existence {
        ProfileExistence::Present | ProfileExistence::Deleted => format!("profile: {name}"),
        ProfileExistence::Renamed => format!("profile: {name} (renamed since)"),
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
// The popup
// ---------------------------------------------------------------------

/// Which profile the popup's form is editing, if any.
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

/// The control that should receive focus after a popup state transition.
///
/// Rows and forms mount and unmount as the state changes, so the destination
/// is recorded as intent and resolved after Dioxus has committed the next DOM.
/// Profile ids are compared as data in the generated script rather than
/// interpolated into a selector, keeping peer text out of selector syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FocusDestination {
    /// The stable entry point and fallback when no target row survives.
    NewProfile,
    /// The first field of the one open create or edit form.
    FirstFormField,
    /// The edit button belonging to a specific profile row.
    Edit(String),
    /// The safe way out of a specific row's delete confirmation.
    DeleteCancel(String),
}

/// Wall-clock budget for a popup control created by a render to become usable.
///
/// Focus-out dismissal waits this whole budget plus
/// [`FOCUS_TRANSIT_GRACE_MS`]. These constants describe one handoff and must
/// be tuned together: dismissal may not conclude that `body` is the user's
/// destination while the popup still owns a live placement request.
pub(crate) const FOCUS_SETTLE_MS: u64 = 250;

/// Extra time reserved for the final focus-out classification after placement.
pub(crate) const FOCUS_TRANSIT_GRACE_MS: u64 = 120;

/// Whether a focus request may replace the current active element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Replace {
    /// Opening may replace its trigger or document transit, and nothing else.
    Opening,
    /// An internal transition may replace transit or focus still in the popup.
    Internal,
    /// Async completion has the internal boundary but yields to prior focus-out.
    Completion,
}

/// One consumable request to place focus in the popup.
#[derive(Debug, Clone, PartialEq)]
struct FocusRequest {
    destination: FocusDestination,
    generation: u64,
    may_replace: Replace,
    /// Rust bounds every bridge await against this one request-wide budget.
    rust_deadline: Instant,
    /// JavaScript checks this immediately before its only focus side effect.
    /// It never lies later than `rust_deadline` on the browser's clock (see
    /// `request_focus` for how the two clocks are aligned), so a commit that
    /// resumes after Rust gave up on the request cannot move focus.
    browser_deadline_ms: f64,
}

/// One browser-side attempt to resolve and focus a rendered destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusAttempt {
    /// The target has not rendered yet, so the bounded worker should retry.
    Missing,
    /// The exact enabled node must survive one retry interval before focus.
    Found,
    /// Observation proved the same enabled node on consecutive attempts.
    Ready,
    /// The target accepted focus and the request is complete.
    Focused,
    /// The renderer reached the commit only after its absolute deadline.
    Expired,
    /// A deliberate active element is outside this request's replacement set.
    Refused,
    /// The renderer did not return evidence, so dismissal must not infer transit.
    Unknown,
}

/// Page-lived coordination shared by the popup worker and its focus-out owner.
///
/// The popup-local request contains the destination; this handle exposes only
/// invalidation and pending state so the app bar can wait for a render handoff
/// without inspecting popup DOM or starting another focus worker.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FocusCoordinator {
    generation: Signal<u64>,
    pending: Signal<bool>,
    unknown: Signal<bool>,
    /// The trusted outside obligation sequence mutation completion must yield to.
    outside_obligation: Signal<Option<u64>>,
}

impl FocusCoordinator {
    /// Build the page-lived half of the popup's focus state machine.
    pub(crate) fn new(
        generation: Signal<u64>,
        pending: Signal<bool>,
        unknown: Signal<bool>,
        outside_obligation: Signal<Option<u64>>,
    ) -> Self {
        Self {
            generation,
            pending,
            unknown,
            outside_obligation,
        }
    }

    /// Invalidate every older request and report the new generation.
    pub(crate) fn advance(mut self) -> u64 {
        self.generation += 1;
        *self.generation.peek()
    }

    /// Invalidate outstanding work at an opening or closing boundary.
    pub(crate) fn invalidate(mut self) {
        self.advance();
        self.pending.set(false);
        self.unknown.set(false);
        self.outside_obligation.set(None);
    }

    /// Whether focus-out must still allow a popup render to place focus.
    pub(crate) fn pending(self) -> bool {
        *self.pending.peek()
    }

    /// Whether the last consumed request ended without renderer evidence.
    pub(crate) fn unknown(self) -> bool {
        *self.unknown.peek()
    }

    /// Publish the trusted outside sequence currently owned by the app bar.
    pub(crate) fn set_outside_obligation(mut self, sequence: Option<u64>) {
        self.outside_obligation.set(sequence);
    }

    /// Clear an outside sequence only if the caller still owns that exact fact.
    pub(crate) fn clear_outside_obligation(mut self, sequence: u64) {
        if *self.outside_obligation.peek() == Some(sequence) {
            self.outside_obligation.set(None);
        }
    }

    /// Whether mutation completion must yield to a trusted outside choice.
    fn outside_obligation_pending(self) -> bool {
        self.outside_obligation.peek().is_some()
    }

    /// Sample ownership immediately before an asynchronous focus attempt.
    fn generation_now(self) -> u64 {
        *self.generation.peek()
    }
}

/// Create one generation-tagged request with matching Rust and browser clocks.
///
/// `may_replace` separates opening, synchronous in-popup transitions, and
/// asynchronous completion. Completion yields when the app bar has published
/// a sequence-tagged trusted outside obligation; ordinary focus-out alone is
/// not evidence that the user chose that destination. A failed or timed-out
/// browser-clock guard consumes the request as `Unknown` before observation.
fn request_focus(
    mut focus_request: Signal<Option<FocusRequest>>,
    mut coordinator: FocusCoordinator,
    destination: FocusDestination,
    may_replace: Replace,
) {
    if may_replace == Replace::Completion && coordinator.outside_obligation_pending() {
        return;
    }
    let generation = coordinator.advance();
    coordinator.pending.set(true);
    coordinator.unknown.set(false);
    focus_request.set(None);
    spawn(async move {
        let rust_deadline = Instant::now() + Duration::from_millis(FOCUS_SETTLE_MS);
        let clock = document::eval(
            "const test = window.__farhelmTestProfiles; \
             if (test?.focusClockDelayMs > 0) \
                 await new Promise((resolve) => setTimeout(resolve, test.focusClockDelayMs)); \
             if (test?.focusClockErrors > 0) { \
                 test.focusClockErrors -= 1; \
                 throw new Error('held focus clock'); \
             } \
             return [performance.now(), test?.focusBrowserBudgetMs ?? null];",
        );
        let browser_deadline_ms =
            match finish_before(rust_deadline, clock.join::<(f64, Option<f64>)>()).await {
                // The browser deadline is derived from the Rust budget that is
                // STILL LEFT when the sample arrives, not from a full budget
                // added at sampling time: the sample was taken some unknown
                // dispatch delay before this line runs, so `sample + remaining`
                // can only fall at or before the moment the Rust deadline
                // passes. That is what keeps a late commit from focusing after
                // Rust has already consumed the request as unknown. A test may
                // shrink the browser budget further; it can never widen it.
                Some(Ok((sample, budget_override))) => {
                    let remaining = rust_deadline
                        .saturating_duration_since(Instant::now())
                        .as_secs_f64()
                        * 1000.0;
                    sample + budget_override.map_or(remaining, |budget| budget.min(remaining))
                }
                Some(Err(_)) | None => {
                    if coordinator.generation_now() == generation {
                        document::eval(
                            "if (window.__farhelmTestProfiles) \
                         window.__farhelmTestProfiles.focusSettled = 'unknown';",
                        );
                        coordinator.unknown.set(true);
                        coordinator.pending.set(false);
                    }
                    return;
                }
            };
        if coordinator.generation_now() != generation {
            return;
        }
        focus_request.set(Some(FocusRequest {
            destination,
            generation,
            may_replace,
            rust_deadline,
            browser_deadline_ms,
        }));
    });
}

/// Render the JavaScript expression for one popup-owned focus destination.
///
/// Profile ids are serialized as data before entering the expression, so peer
/// text cannot change selector syntax in either observation or commit.
fn focus_target_expression(destination: &FocusDestination) -> String {
    match destination {
        FocusDestination::NewProfile => "popup.querySelector('.new-profile-button')".to_string(),
        FocusDestination::FirstFormField => {
            "popup.querySelector('.profile-name-input')".to_string()
        }
        FocusDestination::Edit(id) => format!(
            "[...popup.querySelectorAll('.profile-row')].find((row) => row.dataset.profileId === {})?.querySelector('.profile-edit')",
            serde_json::to_string(&id).expect("a profile id always serializes as JSON")
        ),
        FocusDestination::DeleteCancel(id) => format!(
            "[...popup.querySelectorAll('.profile-row')].find((row) => row.dataset.profileId === {})?.querySelector('.profile-cancel-delete')",
            serde_json::to_string(&id).expect("a profile id always serializes as JSON")
        ),
    }
}

/// Observe one rendered destination without committing browser focus.
///
/// A browser evaluation failure is `Unknown`, not transit or refusal. The Rust
/// worker can retry it without turning a renderer fault into dismissal evidence.
/// `attempt` makes the stable-node handshake genuinely consecutive: an error
/// or any skipped ordinal cannot reuse an older `Found` observation. This eval
/// has no focus side effect, so a renderer that finishes after Rust's timeout
/// can at worst leave an unusable generation-tagged candidate behind.
async fn observe_focus_destination(request: &FocusRequest, attempt: u64) -> FocusAttempt {
    let generation = request.generation;
    let replace_mode = match request.may_replace {
        Replace::Opening => "opening",
        Replace::Internal => "internal",
        Replace::Completion => "completion",
    };
    let target = focus_target_expression(&request.destination);
    let script = format!(
        "const test = window.__farhelmTestProfiles; \
         if (test) {{ \
             test.focusAttempts = (test.focusAttempts || 0) + 1; \
             if (test.focusAttempts === 1) test.focusStartedAt = Date.now(); \
             if (test.focusEvalDelayMs > 0) \
                 await new Promise((resolve) => setTimeout(resolve, test.focusEvalDelayMs)); \
         }} \
         if (test?.focusEvalErrorAttempts?.includes({attempt}) || test?.focusEvalErrors > 0) {{ \
             if (test?.focusEvalErrors > 0) test.focusEvalErrors -= 1; \
             throw new Error('held focus evaluation'); \
         }} \
         if (test?.hideFocusTarget) return 'missing'; \
         const popup = document.querySelector('.profiles-popover'); \
         if (!popup) return 'refused'; \
         const active = document.activeElement; \
         const replaceable = '{replace_mode}' === 'opening' \
             ? (!active || active === document.body || active === document.querySelector('.profiles-toggle')) \
             : (!active || active === document.body || popup.contains(active)); \
         if (!replaceable) return 'refused'; \
         const target = {target}; \
         if (!target || target.disabled) {{ \
             if (window.__farhelmProfilesFocusCandidate?.generation === {generation}) \
                 delete window.__farhelmProfilesFocusCandidate; \
             return 'missing'; \
         }} \
         const candidate = window.__farhelmProfilesFocusCandidate; \
         if (!candidate || candidate.generation !== {generation} || candidate.target !== target || \
             candidate.attempt + 1 !== {attempt}) {{ \
             window.__farhelmProfilesFocusCandidate = {{ generation: {generation}, target, attempt: {attempt} }}; \
             return 'found'; \
         }} \
         candidate.attempt = {attempt}; \
         return 'ready';"
    );
    match document::eval(&script).join::<String>().await.as_deref() {
        Ok("missing") => FocusAttempt::Missing,
        Ok("found") => FocusAttempt::Found,
        Ok("ready") => FocusAttempt::Ready,
        Ok("refused") => FocusAttempt::Refused,
        Err(_) | Ok(_) => FocusAttempt::Unknown,
    }
}

/// Commit a previously observed node only while both absolute deadlines hold.
///
/// Rust races this bridge call against the same request-wide budget. The
/// renderer separately checks its absolute deadline immediately before
/// `focus()`, because dropping an overdue eval future cannot stop JavaScript
/// that has already started running.
async fn commit_focus_destination(request: &FocusRequest, attempt: u64) -> FocusAttempt {
    let generation = request.generation;
    let replace_mode = match request.may_replace {
        Replace::Opening => "opening",
        Replace::Internal => "internal",
        Replace::Completion => "completion",
    };
    let target = focus_target_expression(&request.destination);
    let browser_deadline = serde_json::to_string(&request.browser_deadline_ms)
        .expect("performance.now() returns a finite deadline");
    let script = format!(
        "const test = window.__farhelmTestProfiles; \
         if (test) {{ \
             test.focusCommitAttempts = (test.focusCommitAttempts || 0) + 1; \
             if (test.focusCommitDelayMs > 0) \
                 await new Promise((resolve) => setTimeout(resolve, test.focusCommitDelayMs)); \
         }} \
         if (test?.focusCommitErrors > 0) {{ \
             test.focusCommitErrors -= 1; \
             throw new Error('held focus commit'); \
         }} \
         const popup = document.querySelector('.profiles-popover'); \
         if (!popup) return 'refused'; \
         const active = document.activeElement; \
         const replaceable = '{replace_mode}' === 'opening' \
             ? (!active || active === document.body || active === document.querySelector('.profiles-toggle')) \
             : (!active || active === document.body || popup.contains(active)); \
         if (!replaceable) return 'refused'; \
         const target = {target}; \
         const candidate = window.__farhelmProfilesFocusCandidate; \
         if (!target || target.disabled || !candidate || candidate.generation !== {generation} || \
             candidate.target !== target || candidate.attempt !== {attempt}) return 'missing'; \
         if (performance.now() > {browser_deadline}) {{ \
             if (test) test.focusCommitExpired = true; \
             return 'expired'; \
         }} \
         target.focus({{ preventScroll: true }}); \
         if (document.activeElement === target && test) test.focusedAt = Date.now(); \
         return document.activeElement === target ? 'focused' : 'missing';"
    );
    match document::eval(&script).join::<String>().await.as_deref() {
        Ok("missing") => FocusAttempt::Missing,
        Ok("focused") => FocusAttempt::Focused,
        Ok("expired") => FocusAttempt::Expired,
        Ok("refused") => FocusAttempt::Refused,
        Err(_) | Ok(_) => FocusAttempt::Unknown,
    }
}

/// Forget a stable-node observation after an attempt produced no evidence.
///
/// The attempt ordinal independently prevents reuse if this cleanup eval also
/// fails, but removing the reference keeps the browser state honest and avoids
/// retaining a detached control until the bounded worker finishes.
async fn clear_focus_candidate(generation: u64) {
    let _ = document::eval(&format!(
        "if (window.__farhelmProfilesFocusCandidate?.generation === {generation}) \
         delete window.__farhelmProfilesFocusCandidate;"
    ))
    .await;
}

/// Pick the row after a deleted profile in the catalog's stable order.
///
/// The id is captured before the local removal changes the list. Deleting the
/// last row deliberately returns no target so focus falls back to the popup's
/// stable `new profile` control instead of jumping backward.
fn edit_target_after_delete(catalog: &ProfileCatalog, deleted: &str) -> Option<String> {
    let index = catalog
        .profiles
        .iter()
        .position(|profile| profile.id == deleted)?;
    catalog
        .profiles
        .get(index + 1)
        .map(|profile| profile.id.clone())
}

/// The editor's fields, plus what the resume field was seeded from.
///
/// Owned by the popup rather than by the form component, the discipline
/// `rename::RenameForm` records: a catalog refresh can replace the row that
/// holds an editor, and a draft owned by that row would be silently discarded
/// with it.
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
    fn of(profile: &Profile) -> ProfileDraft {
        ProfileDraft {
            name: display_peer(&profile.name),
            invocation: display_peer(&profile.invocation),
            agent_kind: profile.agent_kind.clone(),
            resume: display_peer(&resume_text(profile.resume_template.as_deref())),
            edited: EditedFields::default(),
            seed: Some(profile.clone()),
        }
    }

    /// This draft as the request body: the raw seed for every field nobody
    /// typed in, and the typed text for every field they did.
    ///
    /// `Err` is the resume field failing to parse — the only local validation
    /// this form performs, and it is not a duplicate of a helm rule: the helm
    /// receives argv, so a quoting mistake has to be caught on this
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
///
/// `pub(crate)` because `list::create_form` reuses this exact rule for a
/// clone's working directory, invocation and title (item2-review2.md's F5):
/// those three fields are peer-relayed text going into an editable control
/// for the same reason a profile's name and invocation are, and a second,
/// hand-copied version of this decision is exactly how the two would drift
/// apart under a future edit.
pub(crate) fn submitted_field(text: &str, edited: bool, seed: Option<&str>) -> String {
    match seed {
        Some(seed) if !edited => seed.to_string(),
        _ => text.to_string(),
    }
}

/// Manage the helm's profile catalog inside the sidebar popup.
///
/// The popup takes no host because the catalog does not. It shares the same
/// always-active [`CatalogSurface`] as the create picker, so a definition
/// accepted here is the definition every host can use immediately.
///
/// ## What it refuses to decide
///
/// Everything about validity. Names, sizes, the catalog bound and the resume
/// template's placeholder rule are the helm's, and its refusals are rendered
/// as it wrote them through `peer::PeerLine`.
///
/// `ops` is the page's live-operation token: every mutation claims it
/// synchronously at handler entry, which excludes profile edits against each
/// other and against every other page mutation. That keeps one accepted
/// change and its catalog reconciliation from racing a second action through
/// another surface. See the `ops` module for why the `disabled` attributes are
/// cosmetic and the claim is the guard.
#[component]
pub(crate) fn ProfilesPopup(
    surface: CatalogSurface,
    mut ops: OpLock,
    focus_coordinator: FocusCoordinator,
) -> Element {
    // Held in a signal rather than as a captured `String` so every handler
    // below stays `Copy`; the handle is reused by both row editors and the
    // popup's new-profile form.
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
    // be told about: a profile disappearing from under an open editor or
    // delete confirmation because another client changed it first. Distinct
    // from `form_error`, which belongs to a form that is still open and still
    // holding a draft.
    let mut notice = use_signal(|| None::<String>);
    let mut focus_request = use_signal(|| None::<FocusRequest>);

    // Opening is itself a focus transition. Going through the same consumable
    // request as every later transition prevents a mount-only special case
    // from outliving a close and applying to a newer popup opening.
    use_effect(move || {
        request_focus(
            focus_request,
            focus_coordinator,
            FocusDestination::NewProfile,
            Replace::Opening,
        );
    });

    // This is the sole owner of browser focus placement. New requests and
    // popup lifecycle changes advance the shared generation, so an older task
    // can finish only by observing that it no longer owns the request.
    use_effect(move || {
        let Some(request) = focus_request.read().clone() else {
            return;
        };
        spawn(async move {
            let mut attempt = 0_u64;
            let last_attempt = loop {
                if focus_coordinator.generation_now() != request.generation {
                    return;
                }
                if request.may_replace == Replace::Completion
                    && focus_coordinator.outside_obligation_pending()
                {
                    break FocusAttempt::Refused;
                }
                attempt += 1;
                let mut outcome = finish_before(
                    request.rust_deadline,
                    observe_focus_destination(&request, attempt),
                )
                .await
                .unwrap_or(FocusAttempt::Unknown);
                if focus_coordinator.generation_now() != request.generation {
                    return;
                }
                if outcome == FocusAttempt::Ready {
                    if request.may_replace == Replace::Completion
                        && focus_coordinator.outside_obligation_pending()
                    {
                        outcome = FocusAttempt::Refused;
                    } else {
                        outcome = finish_before(
                            request.rust_deadline,
                            commit_focus_destination(&request, attempt),
                        )
                        .await
                        .unwrap_or(FocusAttempt::Unknown);
                    }
                    if focus_coordinator.generation_now() != request.generation {
                        return;
                    }
                }
                if matches!(outcome, FocusAttempt::Unknown | FocusAttempt::Expired) {
                    let _ = finish_before(
                        request.rust_deadline,
                        clear_focus_candidate(request.generation),
                    )
                    .await;
                    if focus_coordinator.generation_now() != request.generation {
                        return;
                    }
                }
                let remaining = request
                    .rust_deadline
                    .saturating_duration_since(Instant::now());
                if matches!(outcome, FocusAttempt::Focused | FocusAttempt::Refused)
                    || remaining.is_zero()
                {
                    break outcome;
                }
                let delay = remaining.as_millis().min(25) as u64;
                if delay == 0 {
                    break outcome;
                }
                sleep_ms(delay).await;
            };
            if focus_coordinator.generation_now() == request.generation {
                let settled = match last_attempt {
                    FocusAttempt::Focused => "focused",
                    FocusAttempt::Refused => "refused",
                    FocusAttempt::Unknown | FocusAttempt::Expired => "unknown",
                    FocusAttempt::Missing | FocusAttempt::Found | FocusAttempt::Ready => "missing",
                };
                document::eval(&format!(
                    "if (window.__farhelmProfilesFocusCandidate?.generation === {generation}) \
                         delete window.__farhelmProfilesFocusCandidate; \
                     if (window.__farhelmTestProfiles) \
                         window.__farhelmTestProfiles.focusSettled = '{settled}';",
                    generation = request.generation,
                ));
                focus_coordinator.unknown.set(matches!(
                    last_attempt,
                    FocusAttempt::Unknown | FocusAttempt::Expired
                ));
                focus_request.set(None);
                focus_coordinator.pending.set(false);
            }
        });
    });

    let on_submit = move |_| {
        // The claim IS the guard and it happens synchronously: the rerender
        // that disables these controls is not synchronous with the event that
        // queued this submit, so a second submit would otherwise create two
        // profiles for one intent.
        if !ops.claim() {
            return;
        }
        let Some(editing_now) = editing.peek().clone() else {
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
        let base = base.peek().clone();
        form_error.set(None);
        // A warning describes the mutation that produced it and nothing else,
        // so it is cleared where the next one starts rather than left to
        // accumulate over a surface that has since moved on.
        warning.set(None);
        notice.set(None);
        spawn(async move {
            let sent = match &editing_now {
                Editing::New => create_profile(&base, &spec).await,
                Editing::Existing(id) => update_profile(&base, id, &spec).await,
            };
            let destination = match sent {
                Ok(ProfileCommit::Confirmed(profile)) => {
                    // Folded in BEFORE the token is released, which is what
                    // makes the window empty rather than merely short: until
                    // the authoritative re-read lands, reopening this row
                    // would otherwise seed the editor from the PRE-EDIT
                    // definition, and saving that would undo an update the
                    // helm has already accepted.
                    let id = profile.id.clone();
                    let absorbed = surface.absorb_change(CatalogChange::Upsert(profile));
                    editing.set(None);
                    surface.request(Trigger::Explicit);
                    if absorbed {
                        FocusDestination::Edit(id)
                    } else {
                        // A create form is available while the mount read is
                        // pending. Until its follow-up supplies the first
                        // catalog there is no row to focus, so use the popup's
                        // stable entry point rather than leaving focus on body.
                        FocusDestination::NewProfile
                    }
                }
                Ok(ProfileCommit::Unvalidated(unread)) => {
                    // Accepted, and this build cannot say what it produced. The
                    // held catalog is therefore known to be superseded and
                    // cannot be reconciled from, so it is dropped rather than
                    // left to seed the next editor — the authoritative read
                    // asked for below is what fills the popup back in.
                    warning.set(Some(unread));
                    surface.invalidate();
                    editing.set(None);
                    surface.request(Trigger::Explicit);
                    FocusDestination::NewProfile
                }
                // An ordinary refusal — the helm no longer distinguishes a
                // stale precondition from any other conflict, so this is
                // shown as-is. The form STAYS open with what was typed still
                // in it: a refused name is usually one keystroke from an
                // accepted one, and closing it would throw the draft away
                // with the reason still on screen.
                Err(refusal) => {
                    form_error.set(Some(refusal));
                    FocusDestination::FirstFormField
                }
            };
            request_focus(
                focus_request,
                focus_coordinator,
                destination,
                Replace::Completion,
            );
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
        errors.write().remove(&id);
        warning.set(None);
        notice.set(None);
        let next_focus = {
            let read = surface.catalog.peek();
            match read.answer() {
                CatalogLookup::Known { catalog, .. } => edit_target_after_delete(catalog, &id)
                    .map(FocusDestination::Edit)
                    .unwrap_or(FocusDestination::NewProfile),
                CatalogLookup::Pending | CatalogLookup::Failed(_) => FocusDestination::NewProfile,
            }
        };
        let base = base.peek().clone();
        spawn(async move {
            let sent = delete_profile(&base, &id).await;
            let destination = match sent {
                Ok(()) => {
                    // Same pre-unlock reconciliation as an edit's: the row is
                    // gone from this client's view of the catalog before
                    // anything else can act on it.
                    surface.absorb_change(CatalogChange::Remove(id));
                    surface.request(Trigger::Explicit);
                    next_focus
                }
                // An ordinary refusal, recorded per row (see the `errors` map
                // above) exactly like any other operation this popup can
                // fail.
                Err(refusal) => {
                    let destination = FocusDestination::Edit(id.clone());
                    errors.write().insert(id, refusal);
                    destination
                }
            };
            confirming.set(None);
            request_focus(
                focus_request,
                focus_coordinator,
                destination,
                Replace::Completion,
            );
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
        let CatalogLookup::Known { catalog, .. } = read.answer() else {
            return;
        };
        let holds = |id: &str| catalog.profiles.iter().any(|profile| profile.id == id);
        // A per-row refusal belongs to a row. Once the row is gone there is
        // nothing left to render it under, so keeping the entry would grow
        // this map for as long as the popup stays open — on a surface whose
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
            request_focus(
                focus_request,
                focus_coordinator,
                FocusDestination::NewProfile,
                Replace::Completion,
            );
            notice.set(Some(
                "the profile you were editing is no longer in this helm, so the editor was \
                 closed — nothing was saved"
                    .to_string(),
            ));
        }
        let open_prompt = confirming.peek().clone();
        if let Some(id) = open_prompt
            && !holds(&id)
        {
            confirming.set(None);
            request_focus(
                focus_request,
                focus_coordinator,
                FocusDestination::NewProfile,
                Replace::Completion,
            );
            notice.set(Some(
                "the profile you were about to delete is already gone from this helm".to_string(),
            ));
        }
    });

    let read = surface.catalog.read();
    let lookup = read.answer();
    // Cosmetic only — every handler above claims the token for itself.
    let busy = ops.busy();
    rsx! {
        section {
            class: "profiles-popup-content",
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
                        request_focus(
                            focus_request,
                            focus_coordinator,
                            FocusDestination::FirstFormField,
                            Replace::Internal,
                        );
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
            // A profile the user was editing or about to delete disappearing
            // out from under them, because another client changed the catalog
            // first. Rendered through the peer discipline like every other
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
                CatalogLookup::Failed(error) => rsx! {
                    PeerLine {
                        class: "status error profiles-error",
                        parts: vec![
                            DetailPart::text("profiles could not be read: "),
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
                            "this helm has no profiles; sessions are created from a typed command \
                             until one is defined"
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
                                draft,
                                on_edit_start: move |profile: Profile| {
                                    if ops.busy_now() {
                                        return;
                                    }
                                    confirming.set(None);
                                    form_error.set(None);
                                    notice.set(None);
                                    draft.set(ProfileDraft::of(&profile));
                                    editing.set(Some(Editing::Existing(profile.id)));
                                    request_focus(
                                        focus_request,
                                        focus_coordinator,
                                        FocusDestination::FirstFormField,
                                        Replace::Internal,
                                    );
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
                                    let destination = match editing.peek().clone() {
                                        Some(Editing::Existing(id)) => FocusDestination::Edit(id),
                                        Some(Editing::New) | None => FocusDestination::NewProfile,
                                    };
                                    editing.set(None);
                                    form_error.set(None);
                                    request_focus(
                                        focus_request,
                                        focus_coordinator,
                                        destination,
                                        Replace::Internal,
                                    );
                                },
                                on_delete_start: move |id: String| {
                                    if ops.busy_now() {
                                        return;
                                    }
                                    editing.set(None);
                                    confirming.set(Some(id.clone()));
                                    request_focus(
                                        focus_request,
                                        focus_coordinator,
                                        FocusDestination::DeleteCancel(id),
                                        Replace::Internal,
                                    );
                                },
                                on_delete_confirm,
                                on_delete_cancel: move |id: String| {
                                    // Same live-token guard: a cancel racing
                                    // the confirm it follows must not close a
                                    // prompt whose delete is already out.
                                    if ops.busy_now() {
                                        return;
                                    }
                                    confirming.set(None);
                                    request_focus(
                                        focus_request,
                                        focus_coordinator,
                                        FocusDestination::Edit(id),
                                        Replace::Internal,
                                    );
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
                        request_focus(
                            focus_request,
                            focus_coordinator,
                            FocusDestination::NewProfile,
                            Replace::Internal,
                        );
                    },
                }
            }
        }
    }
}

/// One profile: its definition, whether it is the helm's remembered default,
/// and whichever of edit / delete / the open editor belongs there.
///
/// Every user-supplied value renders through the peer discipline
/// (`peer::display_peer` plus an isolated run), because profiles are
/// user-supplied text: a name able to reorder the row around it could make a
/// delete button appear to belong to a different profile than it does.
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
    /// the fields it is about rather than at the bottom of the popup. Only
    /// one form is ever open, so the rows that are not editing ignore it.
    form_error: Option<String>,
    draft: Signal<ProfileDraft>,
    on_edit_start: EventHandler<Profile>,
    on_submit: EventHandler<()>,
    on_cancel: EventHandler<()>,
    on_delete_start: EventHandler<String>,
    on_delete_confirm: EventHandler<String>,
    on_delete_cancel: EventHandler<String>,
) -> Element {
    let id = profile.id.clone();
    let shown_name = display_peer(&profile.name);
    let shown_invocation = display_peer(&profile.invocation);
    let shown_resume = display_peer(&resume_text(profile.resume_template.as_deref()));
    let edit_target = profile.clone();

    rsx! {
        div { class: "profile-row", "data-profile-id": "{profile.id}",
            div { class: "profile-row-main",
                span { class: "profile-name peer-value", dir: "ltr", "{shown_name}" }
                span { class: "profile-kind", "{profile.agent_kind}" }
                // The remembered default is marked rather than sorted to the
                // top: the catalog's order is the helm's and stays
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
                    span { class: "confirm-title", "every host" }
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
                        onclick: {
                            let id = id.clone();
                            move |_| on_delete_cancel.call(id.clone())
                        },
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
/// The draft belongs to the popup (see [`ProfileDraft`]); this component
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
            // The helm's own words — the name rule, the size cap, the
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

    /// Requests waiting for the page-owned driver collapse to the strongest
    /// authority, so background traffic cannot make a person's refresh stand
    /// down under build skew before the reader sees it.
    #[test]
    fn pending_catalog_requests_keep_the_strongest_trigger() {
        assert_eq!(
            strongest_catalog_trigger(Some(Trigger::Scheduled), Trigger::Notice),
            Trigger::Notice
        );
        assert_eq!(
            strongest_catalog_trigger(Some(Trigger::Notice), Trigger::Explicit),
            Trigger::Explicit
        );
        assert_eq!(
            strongest_catalog_trigger(Some(Trigger::Explicit), Trigger::Scheduled),
            Trigger::Explicit
        );
    }

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
        }
    }

    /// A failed refresh keeps the shared catalog on screen and says so; it
    /// does not blank either consumer.
    ///
    /// The same choice the hosts panel makes about the registry, for the same
    /// reason: rows the user can still act on, marked as possibly out of
    /// date, beat an empty box — and a popup that emptied itself whenever one
    /// read dropped would look like the catalog had lost its profiles.
    #[test]
    fn a_failed_refresh_keeps_the_catalog_and_reports_itself() {
        let mut read = CatalogRead::default();
        read.record(Ok(catalog(vec![profile("p-1", "Codex")], Some("p-1"))));
        read.record(Err("the helm did not answer".to_string()));

        match read.answer() {
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
        read.record(Ok(catalog(vec![profile("p-1", "Codex")], Some("p-1"))));
        assert!(matches!(
            read.answer(),
            CatalogLookup::Known {
                refresh_error: None,
                ..
            }
        ));
    }

    /// An equal confirming read is a no-op when no refresh error needs clearing.
    ///
    /// This distinction avoids a needless signal write and reconciliation;
    /// the separate error assertion preserves the visible stale-state line's
    /// contract even when the catalog contents themselves compare equal.
    #[test]
    fn an_identical_catalog_reply_needs_no_rendered_update() {
        let held = catalog(vec![profile("p-1", "Codex")], Some("p-1"));
        let mut read = CatalogRead::default();
        read.record(Ok(held.clone()));

        assert!(!read.differs_from(&Ok(held.clone())));
        assert!(read.differs_from(&Err("the helm did not answer".to_string())));

        read.record(Err("the helm did not answer".to_string()));
        assert!(
            read.differs_from(&Ok(held)),
            "an equal successful catalog must still clear a held refresh error"
        );
    }

    /// Catalog order participates in equality because it is observable UI
    /// state and an authoritative read must be able to replace the temporary
    /// append order produced by local create absorption.
    #[test]
    fn reordered_profiles_are_a_catalog_change() {
        let first = profile("p-1", "First");
        let second = profile("p-2", "Second");
        let mut read = CatalogRead::default();
        read.record(Ok(catalog(
            vec![first.clone(), second.clone()],
            Some("p-1"),
        )));

        assert!(read.differs_from(&Ok(catalog(vec![second, first], Some("p-1"),))));
    }

    /// A mutation this client performed is folded into the held catalog at
    /// once, so both consumers stop serving the superseded definition.
    ///
    /// The window this closes is small and the failure inside it is durable: a
    /// save answers with the new definition, the operation token is released,
    /// and until the authoritative re-read lands the popup would otherwise
    /// still seed an editor from the definition that was just replaced.
    /// Saving THAT would undo an update the helm accepted, with nothing
    /// on screen to suggest anything went wrong.
    #[test]
    fn a_committed_mutation_is_folded_in_before_the_authoritative_read() {
        let mut read = CatalogRead::default();
        read.record(Ok(catalog(vec![profile("p-1", "Before")], None)));

        assert!(read.absorb(CatalogChange::Upsert(profile("p-1", "After"))));
        assert!(read.absorb(CatalogChange::Upsert(profile("p-2", "Fresh"))));
        match read.answer() {
            CatalogLookup::Known { catalog, .. } => {
                assert_eq!(
                    catalog.profiles[0].name, "After",
                    "an edit replaces in place"
                );
                assert_eq!(catalog.profiles[1].id, "p-2", "a create appears at once");
            }
            other => panic!("the catalog must still be readable, got {other:?}"),
        }

        assert!(read.absorb(CatalogChange::Remove("p-1".to_string())));
        match read.answer() {
            CatalogLookup::Known { catalog, .. } => {
                assert_eq!(catalog.profiles.len(), 1);
                assert_eq!(catalog.profiles[0].id, "p-2");
            }
            other => panic!("the catalog must still be readable, got {other:?}"),
        }
    }

    /// A mutation cannot synthesize a complete catalog before the first read.
    ///
    /// Reporting that absence lets a successful create choose a stable focus
    /// fallback instead of waiting on a row whose surrounding catalog has not
    /// arrived yet.
    #[test]
    fn absorbing_without_a_catalog_reports_no_focusable_row() {
        let mut read = CatalogRead::default();

        assert!(!read.absorb(CatalogChange::Upsert(profile("p-1", "Codex"))));
        assert_eq!(read.answer(), CatalogLookup::Pending);
    }

    /// An unreadable successful mutation invalidates the shared answer until
    /// the authoritative read lands.
    ///
    /// Keeping the pre-mutation row would let either consumer act on a
    /// definition the client already knows may have changed.
    #[test]
    fn invalidating_a_catalog_makes_it_pending_without_reusing_old_rows() {
        let mut read = CatalogRead::default();
        read.record(Ok(catalog(vec![profile("p-1", "Before")], None)));
        read.forget();
        assert_eq!(read.answer(), CatalogLookup::Pending);
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
    /// permanently, until the user answers or closes the dialog.
    ///
    /// Another client creating a session moves the helm's remembered id. A
    /// dialog that still consulted the default after the notification would
    /// change its selection under whoever was filling it in; the latch makes
    /// the default a decision this dialog made rather than a value it follows.
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
    /// A real race rather than a hypothetical: the popup and picker share one
    /// feed-driven catalog, so a delete made in the popup — or in another
    /// browser — reaches an open dialog within one notification. Two failures
    /// are ruled out at once: a
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
    /// not yet learned whether its helm remembers a profile cannot honestly
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
            "before the catalog answers, this dialog does not know whether the helm remembers a \
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
        let spec = ProfileDraft::of(&stored)
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
        let draft = ProfileDraft::of(&stored);
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

    /// A row names the profile a session was created FROM, as snapshotted.
    ///
    /// Both halves are SPEC.md's snapshot rule seen from the list: the name
    /// never changes under an existing session (so a rename cannot rewrite
    /// history). A rename is qualified because this helm still has the row
    /// under a different name; a deleted row is plain because an immutable
    /// historical label is not a warning. An unknown existence stays
    /// qualified rather than being rounded to a state this build understands.
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
            "profile: Claude Code"
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

    /// Deleting a row focuses the following row in catalog order, while the
    /// last row falls back to the stable new-profile control. This keeps a
    /// keyboard user from being stranded when the focused row disappears.
    #[test]
    fn delete_focus_uses_the_following_row_or_the_new_profile_fallback() {
        let catalog = catalog(
            vec![
                profile("p-1", "first"),
                profile("p-2", "second"),
                profile("p-3", "third"),
            ],
            None,
        );
        assert_eq!(
            edit_target_after_delete(&catalog, "p-2"),
            Some("p-3".to_string())
        );
        assert_eq!(edit_target_after_delete(&catalog, "p-3"), None);
        assert_eq!(edit_target_after_delete(&catalog, "missing"), None);
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
