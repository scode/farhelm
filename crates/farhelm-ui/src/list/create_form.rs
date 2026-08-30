//! The inline form for creating one session on a selected host.
//!
//! Intent binding and host reconciliation live beside the form because they
//! define when an idempotency key still describes the submitted create.

use dioxus::prelude::*;

use crate::api::{self, CreateAgent, create_session, mint_intent_key};
use crate::ops::OpLock;
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::profiles::{
    AgentChoice, CatalogLookup, CatalogSurface, HostTarget, UNRESOLVED_VALUE, resolve_agent,
    seeded_choice, submitted_field,
};
use crate::reader::Trigger;
use crate::{ApiBase, HostId, ProfileExistence, Session};

use super::shared::{
    HostOption, OpenHost, effective_create_host, enrich_created_session, matching_host_option,
};

/// How many times one submit will mint a key before giving up.
///
/// The retry exists for a queued keystroke landing during the mint, which
/// resolves on the second attempt; anything beyond that is a form whose
/// values keep changing faster than a UUID can be generated, which is not a
/// create anybody is waiting on. Bounded rather than a bare loop because
/// spinning is a worse answer than saying so.
const MINT_ATTEMPTS: usize = 3;

/// What one create would actually LAUNCH.
///
/// The two creation modes are mutually exclusive on the wire (PLAN_M6_75.md
/// item 3) and they are mutually exclusive here for a second reason: they are
/// part of the intent an idempotency key stands for. Keeping the typed
/// command inside the `Command` arm rather than beside a nullable profile is
/// what makes "a profile-backed create does not care what is in the command
/// box" structural — a form field the user cannot reach in that mode can no
/// longer change what the key is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LaunchIntent {
    /// The invocation as typed into the form.
    Command(String),
    /// A profile from the target host's catalog, by id.
    Profile(String),
}

/// Everything one intended create IS — the exact thing an idempotency key
/// stands for.
///
/// The helm treats a key as "this create, retried"; this type is what makes
/// that claim true on the client's side. Two parts, and both were learned
/// the hard way:
///
/// - **The host, as an INCARNATION rather than an id.** A `HostId` is a
///   registry row, and the row outlives every edit made to it: retargeting
///   points it at another address, adopting binds it to another install.
///   Keyed on the id alone, a retry after an ambiguous failure carries the
///   first attempt's key to a machine that has never seen it — where it is
///   not idempotent at all, and the "retry" is a second real agent. See
///   `hosts::host_incarnation`.
/// - **The form's values, snapshotted.** They already start a new intent
///   when edited (each field's `oninput` is what clears the key), but that
///   rule has a gap the size of one await: minting is asynchronous and the
///   `disabled` attributes that make the form inert land one render after the
///   submit, so a keystroke queued at submit time can change a field while the
///   key is being made. The binding is re-read after minting and compared
///   against this, which turns that gap into another mint rather than a key
///   that describes something the user did not submit — the attributes are
///   honesty about a create being in flight, not the guard.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IntentBinding {
    host: HostId,
    /// The target host's incarnation at submit time — see the type docs.
    incarnation: String,
    cwd: String,
    /// What this create launches — see [`LaunchIntent`]. Switching between
    /// the two modes is a different intended create, and so is switching
    /// profiles, which is why the mode lives inside the binding rather than
    /// beside it.
    agent: LaunchIntent,
    title: String,
}

impl IntentBinding {
    /// The binding for a submit, or `None` when there is no host to create
    /// on — the one case a submit is refused locally rather than sent.
    fn of(
        selected: Option<HostId>,
        hosts: &[HostOption],
        cwd: String,
        agent: LaunchIntent,
        title: String,
    ) -> Option<IntentBinding> {
        let host = hosts.iter().find(|host| Some(host.id) == selected)?;
        Some(IntentBinding {
            host: host.id,
            incarnation: host.incarnation.clone(),
            cwd,
            agent,
            title,
        })
    }
}

/// Whether the catalog surface's target still describes the selected row —
/// the SAME registry row AND the same install fingerprint. The submit
/// handler refuses to send when this is false.
///
/// Comparing the row id alone is not enough, and the gap is the documented
/// one-render lag: after a retarget or an adoption the `hosts` snapshot can
/// already describe the successor install while the catalog surface still
/// holds the predecessor's catalog under the same row id. A profile id
/// resolved against that catalog would then be launched on the successor,
/// where the same id names a different profile (every fresh supervisor seeds
/// the same starters) — and because the connection token would also be read
/// from the successor's row, the helm's own create precondition would pass.
/// The full-target comparison is what closes that window client-side.
///
/// Both sides absent is a match on purpose: the hostless refusal downstream
/// owns that case and says something more useful than "the target changed".
/// A selected row missing from the snapshot is a mismatch — whatever catalog
/// is held, it cannot be shown to describe a row that no longer exists.
fn catalog_matches_selection(
    selected: Option<HostId>,
    hosts: &[HostOption],
    target: Option<&HostTarget>,
) -> bool {
    match (selected, target) {
        (None, None) => true,
        (Some(id), Some(target)) => hosts.iter().find(|host| host.id == id).is_some_and(|host| {
            HostTarget::new(host.id, host.incarnation.clone()).same_install(target)
        }),
        _ => false,
    }
}

/// The connection token a create names as its `expected_incarnation`, or
/// `None` when there is nothing to assert: the row is gone from the
/// snapshot, or it has never connected (`connection == 0`, the sentinel
/// `Host::incarnation` documents).
///
/// The sentinel must map to NO claim rather than a claim of zero. A host
/// making its FIRST connection between the form's snapshot and the helm's
/// routing has a real, nonzero token by then — a create carrying `0` would
/// be refused as stale even though this client never observed any
/// connection and had nothing to preserve.
fn connection_claim(hosts: &[HostOption], host: HostId) -> Option<u64> {
    hosts
        .iter()
        .find(|option| option.id == host)
        .and_then(|option| (option.connection != 0).then_some(option.connection))
}

/// Which of the two creation modes a "clone" click's snapshot TRUSTS —
/// deliberately not carrying its own payload; see [`CreatePrefill::invocation`]
/// for where that lives and why.
///
/// The choice between the two variants is [`prefill_from`]'s to make: a
/// clone trusts the row's profile id ONLY when its own `source_profile` says
/// `Present` — the catalog, as of the row's own snapshot, still holds that
/// id under the SAME name. Every other answer — no profile at all, a name
/// the catalog has since changed, an id it no longer holds at all, or an
/// existence word this build does not recognize — falls back to the raw
/// invocation instead.
///
/// This is DELIBERATELY STRICTER than `profiles::resolve_agent`'s own rule
/// for an ordinary create, not a restatement of it: `resolve_agent` accepts
/// a previously chosen profile id whenever the catalog still holds it AT
/// ALL, name changes included, because that choice was made by a human
/// looking at today's picker a moment ago. A clone's row can be arbitrarily
/// old, so "the id still exists" is not enough evidence that cloning it
/// again is what a look at today's catalog would still choose — a rename is
/// exactly the kind of change SPEC.md's own snapshot rule says a session
/// must not be silently re-bound across, and a clone re-selecting the same
/// id under new user-visible clothing would be doing precisely that.
///
/// Trusting the id at all, even under `Present`, is still a SNAPSHOT
/// decision, not a live one: once a profile-backed clone is applied and the
/// user submits, the request names the id and nothing else, and the
/// supervisor resolves it against whatever definition the catalog holds AT
/// SUBMIT TIME — an edit landing between the clone click and the submit
/// changes what the cloned session launches, exactly as it would for any
/// other profile-backed create, and a deletion in that window is refused by
/// `resolve_agent` the same way a live picker's own vanished choice is.
///
/// Not [`LaunchIntent`] reused, even though the two-mode split is
/// identical: `LaunchIntent::Command(String)` pairs its variant with the
/// launch string, but a clone's raw invocation has to reach the form's
/// command field in EITHER mode (see `CreatePrefill::invocation`) — parking
/// it inside a `Command` payload here would duplicate that string when the
/// mode already is command, and leave a `Profile`-backed clone with nowhere
/// on `LaunchIntent` to carry it at all. A bare marker beside one shared
/// field says the same thing without either problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PrefillAgent {
    /// Launch the row's own profile again, by id, on the row's own host.
    Profile { id: String },
    /// No profile this clone may trust — launch the raw invocation
    /// (`CreatePrefill::invocation`) verbatim instead.
    Command,
}

/// What one "clone" click seeds a fresh create form with: everything about
/// the clicked row that a NEW session can reuse.
///
/// Built once, by [`prefill_from`], from the row's `Session` at the moment
/// of the click — a snapshot, not a live binding, which is what SPEC.md's
/// profile-snapshot rule already requires of `Session::source_profile`
/// itself: a profile edited or deleted after the click must not reach back
/// into an open, already-prefilled form.
///
/// `title` and `cwd` travel verbatim, duplicate title included. SPEC.md's
/// creation rule has nothing to say about uniqueness, an identical title is
/// one rename away from being fixed, and inventing a "(copy)" suffix would
/// be this UI's own opinion about a field the user never asked it to guess
/// at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CreatePrefill {
    /// Bumped by every clone click, including a second clone of the SAME
    /// row. `CreateSessionForm`'s reseed effect compares this against
    /// `prefill_applied` rather than mere presence, which is what makes
    /// cloning one row twice in a row reseed the form the second time too,
    /// instead of that click being a silent no-op because a prefill was
    /// already on screen.
    pub(super) generation: u64,
    /// The row's host, so the form's selector follows the CLONED session
    /// rather than whatever the dialog last had chosen. `None` only for a
    /// row from a helm old enough to omit `Session::host` entirely, in
    /// which case the form leaves the selector alone and falls back to its
    /// ordinary default precedence (`effective_create_host`) — the same
    /// degradation every other host-carrying field on `Session` already
    /// accepts from such a peer.
    pub(super) host: Option<HostId>,
    /// The install identity the row reported alongside `host`, straight off
    /// `Session::host_identity` — see that field's own doc for the double-
    /// `Option` contract. This is what lets the reseed effect tell a row
    /// whose host still fronts the SAME install apart from one that has
    /// since been retargeted or adopted onto a successor: accepting the
    /// latter's host id at face value would send this clone's command or
    /// profile id to a machine the row no longer actually names (a raw
    /// command executes there; a profile id can collide with a different,
    /// deterministically-numbered starter profile there) — exactly the
    /// #156-style residual `shared::matching_host_option` already closes
    /// for the ordinary create default, reused here for the same question.
    pub(super) host_identity: Option<Option<String>>,
    pub(super) cwd: String,
    pub(super) title: String,
    /// The row's raw launch command, ALWAYS carried regardless of which
    /// mode [`agent`](Self::agent) trusts.
    ///
    /// Needed even for a profile-backed clone: the form's command field is
    /// merely disabled while a profile is selected, not emptied, so
    /// leaving it holding whatever the mounted form last typed would let a
    /// user who switches this clone to "custom command" submit a stale,
    /// unrelated string instead of the row's own command.
    pub(super) invocation: String,
    pub(super) agent: PrefillAgent,
}

/// Build the prefill a clone click seeds the create form with, from the
/// clicked row's own `Session`.
///
/// Kept as a pure mapping — apart from `CreateSessionForm` and from whatever
/// mints `generation` (`list::view::ListView`, once per clone click) — so
/// the Present/Renamed/Deleted/Unrecognized/None decision (see
/// [`PrefillAgent`]) is checkable without mounting a component.
pub(super) fn prefill_from(session: &Session, generation: u64) -> CreatePrefill {
    let agent = match &session.source_profile {
        Some(source) if source.existence == ProfileExistence::Present => PrefillAgent::Profile {
            id: source.id.clone(),
        },
        _ => PrefillAgent::Command,
    };
    CreatePrefill {
        generation,
        host: session.host,
        host_identity: session.host_identity.clone(),
        cwd: session.cwd.clone(),
        title: session.title.clone(),
        invocation: session.invocation.clone(),
        agent,
    }
}

// ---------------------------------------------------------------------
// A clone's own host+agent binding, resolved independently of its text
// fields (item2-review2.md's F1/F2/F3/F4)
// ---------------------------------------------------------------------

/// What has become of the CURRENT clone generation's own host+agent
/// binding — tracked separately from `prefill_applied` (the text fields)
/// because, unlike them, this decision cannot always be made in the same
/// render pass a clone arrives in, and it can be TAKEN BACK later by
/// events the text fields do not care about at all.
///
/// Four states rather than a bool, because three different questions later
/// code needs answered would otherwise collapse into one flag that cannot
/// distinguish them: "is there still something to try automatically",
/// "does `chosen_host` currently hold THIS generation's own pick, subject
/// to being withdrawn", and "has this decision already been taken away
/// from automatic handling, whether because it failed or because a human
/// took over".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloneHostState {
    /// Still trying: either the registry has not answered at all yet
    /// (item2-review2.md F1 — a clone opened before the first `hosts` read
    /// lands must not give up permanently, since `prefill_applied` is
    /// already latched by the time the registry answers and will never
    /// send this generation through the ordinary reseed branch again), or
    /// this pass is the first chance to check a registry that already had.
    Waiting,
    /// `chosen_host` (and whatever agent choice is applied or queued in
    /// `pending_choice`) currently hold THIS generation's own pick.
    /// Re-checked every pass: the moment `matching_host_option` stops
    /// confirming the source installation — a retarget or an adopt lands
    /// while the form stays open — the binding is withdrawn back to
    /// `Unconfirmable` (item2-review2.md F3), rather than silently
    /// continuing to name a machine the clone was never actually taken
    /// from.
    Bound,
    /// Nothing left for the automatic resolver to try: a hostless
    /// (legacy) row, whose install can never be confirmed at all
    /// (item2-review2.md F4); a hostful row whose identity check failed
    /// once the registry had a chance to answer; or a `Bound` binding that
    /// was just withdrawn. `chosen_host` and the agent picker are left to
    /// their ordinary, non-clone rules from here — for the REST of this
    /// generation's lifetime, since only a fresh clone (a new generation)
    /// re-seeds `Waiting`.
    Unconfirmable,
    /// An explicit host interaction (the selector's own `onchange`) has
    /// taken this generation's host decision away from automatic handling
    /// entirely (item2-review2.md F6's spirit, applied to the binding
    /// itself rather than only to `pending_choice`): the user picked a
    /// host with their own hand, so a later retarget of the CLONE's row
    /// must not silently pull the rug out from under a choice the clone
    /// had nothing to do with anymore. Ordinary (non-clone) retarget
    /// handling — clearing the agent, rotating the intent key — still
    /// applies; only this file's STRONGER clone-specific withdrawal does
    /// not.
    UserTookOver,
}

/// What the resolver has decided to DO this pass, given a [`CloneHostState`]
/// and the registry as it currently stands.
///
/// Kept apart from `CloneHostState` itself (rather than folding the action
/// into the `Bound` variant, say) because the STATE is what persists across
/// passes and the ACTION is a one-shot instruction for THIS pass only —
/// conflating them would leave it ambiguous whether a stored action should
/// be replayed on the next unrelated render.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CloneHostAction {
    /// Nothing to do this pass — the caller's signals are untouched.
    Hold,
    /// Apply the clone's own choice onto `target`, the same one-render-late
    /// queue a fresh cross-host clone always uses (`pending_choice`): the
    /// caller sets `chosen_host` to `target.host` immediately and stashes
    /// `choice` for the catalog-target effect to consume once it catches
    /// up, whether that catching-up takes zero extra passes (same host
    /// already effective) or one (a cross-host jump).
    Bind {
        target: HostTarget,
        choice: AgentChoice,
    },
    /// Undo a binding this generation had previously made: `chosen_host`,
    /// whatever agent choice is applied, and `pending_choice` may all
    /// currently describe it, and the caller must clear all three.
    Withdraw,
}

/// Resolve one pass of a clone's own host+agent binding — a pure decision,
/// so item2-review2.md's F1 (retry once the registry loads), F3 (withdraw
/// the instant a bound installation stops matching) and F4 (a hostless
/// clone never resolves an agent) are each one deterministic case here,
/// checkable without mounting a component or an effect.
///
/// `chosen_host_is_bound_host` is only consulted in the `Bound` state: it
/// is how the caller reports that `chosen_host` has moved away from this
/// generation's own pick since the last pass (an explicit re-selection is
/// what usually causes that, and the host `<select>`'s own `onchange`
/// already transitions to `UserTookOver` directly for that ordinary case —
/// this is the belt to that handler's braces, covering any other path that
/// might move `chosen_host` without going through it).
fn resolve_clone_host(
    state: CloneHostState,
    prefill_host: Option<HostId>,
    prefill_identity: &Option<Option<String>>,
    prefill_agent: &PrefillAgent,
    hosts_loaded: bool,
    hosts: &[HostOption],
    chosen_host_is_bound_host: bool,
) -> (CloneHostState, CloneHostAction) {
    match state {
        CloneHostState::Waiting => {
            let Some(host) = prefill_host else {
                // A hostless clone starts life straight in `Unconfirmable`
                // (see the caller), so reaching `Waiting` with no host here
                // is unreachable in practice — handled rather than
                // `unreachable!()`, since this function's whole point is to
                // be checkable in isolation from that caller invariant.
                return (CloneHostState::Unconfirmable, CloneHostAction::Hold);
            };
            if !hosts_loaded {
                // F1: keep waiting rather than giving up — the caller's
                // `prefill_applied` latch means this is the ONLY chance
                // left to apply this generation's host once the registry
                // does answer.
                return (CloneHostState::Waiting, CloneHostAction::Hold);
            }
            let open = OpenHost {
                id: host,
                identity: prefill_identity.clone(),
            };
            match matching_host_option(&open, hosts) {
                Some(option) => {
                    let target = HostTarget::new(option.id, option.incarnation.clone());
                    let choice = match prefill_agent {
                        PrefillAgent::Command => AgentChoice::Command,
                        PrefillAgent::Profile { id } => AgentChoice::Profile(id.clone()),
                    };
                    (
                        CloneHostState::Bound,
                        CloneHostAction::Bind { target, choice },
                    )
                }
                // The registry has answered and this row's install cannot
                // be confirmed — permanently, for the rest of this
                // generation's lifetime; only a fresh clone tries again.
                None => (CloneHostState::Unconfirmable, CloneHostAction::Hold),
            }
        }
        CloneHostState::Bound => {
            if !chosen_host_is_bound_host {
                // The selector has moved on without going through
                // `Withdraw` — an explicit re-pick that reached
                // `chosen_host` some other way than the handler that
                // already transitions this directly. Nothing further to
                // automatically manage either way.
                return (CloneHostState::UserTookOver, CloneHostAction::Hold);
            }
            let Some(host) = prefill_host else {
                // Structurally unreachable: `Bound` is only ever entered
                // from a hostful `Waiting` resolution above.
                return (CloneHostState::Unconfirmable, CloneHostAction::Withdraw);
            };
            let open = OpenHost {
                id: host,
                identity: prefill_identity.clone(),
            };
            if matching_host_option(&open, hosts).is_some() {
                (CloneHostState::Bound, CloneHostAction::Hold)
            } else {
                // F3: the row's install no longer matches what it was
                // cloned from — withdraw rather than let the selector keep
                // silently naming a machine the clone was never actually
                // taken from.
                (CloneHostState::Unconfirmable, CloneHostAction::Withdraw)
            }
        }
        CloneHostState::Unconfirmable | CloneHostState::UserTookOver => {
            (state, CloneHostAction::Hold)
        }
    }
}

/// Seed one of the create form's peer-relayed text fields (working
/// directory, invocation, title) from a clone's raw value: shown escaped
/// (`peer::display_peer`), with the exact raw bytes and a cleared edited
/// flag recorded alongside so an untouched submit reads back the ORIGINAL
/// bytes rather than the escaped spelling (item2-review2.md F5;
/// `profiles::submitted_field` is the read-back half, and
/// `profiles::ProfileDraft::of` is this exact model's other caller).
fn reseed_cloned_field(
    display: &mut Signal<String>,
    raw_seed: &mut Signal<Option<String>>,
    edited: &mut Signal<bool>,
    raw: &str,
) {
    display.set(display_peer(raw));
    raw_seed.set(Some(raw.to_string()));
    edited.set(false);
}

/// Inline create form (PLAN_M2.md step 8's "not a modal library" design
/// choice): working directory and agent command are required, title is
/// optional. Lives entirely inside `ListView` — there is no route or
/// signal for it beyond the `show_create` toggle that mounts/unmounts it.
///
/// `submitting` is owned by the CALLER (`ListView`), not this component:
/// `ListView`'s own "new session" toggle button needs to see it too, so it
/// can refuse to unmount this form while a create is still in flight —
/// dropping this component mid-`spawn` would strand the POST's eventual
/// response with nothing left to act on it. Lifting the flag up is
/// simpler than trying to keep a detached task meaningful after the fact.
///
/// `on_created` fires only on a successful POST, with the newly created
/// `Session` from the response body; `ListView` uses that to close the
/// form and select the new session in the adjacent pane, whose terminal
/// mounts immediately (SPEC.md: "creation launches the agent; you type
/// your first prompt into its terminal") — the sidebar itself stays
/// mounted throughout. On failure the form stays mounted with its values
/// untouched and the error text rendered next to it — the fields are
/// plain `use_signal<String>`s rather than being reset or lifted into
/// `ListView`, so "form contents preserved" falls out of simply not
/// clearing them rather than needing a restore step. On success the
/// fields are left as-is too: `on_created` closes this form (unmounting
/// it and its field signals) in the same frame, so there is no one left
/// to observe a reset — only the failure path needs to leave the control
/// usable again.
///
/// ## The intent key (PLAN_M3.md item 6), and what it is bound to
///
/// One key per INTENDED create, reused across every retry of it. The
/// lifecycle is deliberately tied to the form's values rather than to its
/// mount: minted at first submit, kept across a failed submit (the retry
/// case the key exists for), and dropped the moment any field changes
/// (which makes the next submit a different intent). Both edges matter —
/// keeping it across an edit would send a request the server refuses as a
/// key reuse once the first attempt has a durable outcome, and dropping it
/// on failure would make a retry able to create a second session for the
/// same intent, which is the exact gap this closes.
///
/// The key is bound to its TARGET HOST as well as to the fields, and that
/// binding closes a hole the field-only version left open. An intent is "run
/// this command in this directory ON THIS MACHINE" — the same key against a
/// different host is a different intended create, and the helm scopes keys
/// per host, so replaying one at a second host is not idempotent there at
/// all. The dangerous shape is not the user changing the selector (that
/// clears the key inline, like any other edit) but the target changing
/// UNDER them: a host removed from the registry moves the effective default,
/// and a retry after an ambiguous failure would then carry the first
/// attempt's key to a machine that has never seen it — a second real agent,
/// which is precisely the outcome the key exists to prevent. So the key is
/// stored WITH the host it was minted for and re-minted whenever the
/// effective target no longer matches.
///
/// What makes that lifecycle a rule rather than a race is the SUBMIT PATH's
/// own discipline, not the disabled attributes: the agent, the host and the
/// text fields are resolved synchronously when the button is pressed and
/// frozen across the minting await, and the binding is re-read afterwards so
/// a keystroke that landed during it produces another mint rather than a key
/// describing values nobody submitted. The inputs are disabled too, and that
/// is worth having — an inert form is honest about a create being in flight —
/// but it is cosmetic in the way every `disabled` on this page is: the
/// attribute lands one render after the event that set it, so anything queued
/// in that gap still reaches the handler.
///
/// A create from this form ALWAYS carries a key. If the key cannot be
/// generated the create is refused locally, with the failure shown like any
/// other: falling back to an unkeyed create would silently drop the one
/// protection this whole feature exists to provide, at exactly the moment
/// something is already wrong with the environment, and a user who retries
/// after a dropped reply would get a duplicate agent with nothing to
/// indicate why.
///
/// The server's key-reuse and already-deleted refusals need no handling of
/// their own here: they arrive as ordinary create failures (a 409 with the
/// supervisor's own message) and render in the same `.create-session-error`
/// line as every other one, which is what SPEC.md's "concrete, actionable
/// errors" asks for — the message names the key and what happened to it.
///
/// ## The host selector (PLAN_M6.md item 6)
///
/// `hosts` carries EVERY registered host, whatever phase it is in (see
/// `ListView` for why filtering to connected ones would quietly rewrite
/// SPEC.md's default), with non-connected ones labelled by their phase. The
/// initial selection follows SPEC.md's default through
/// `default_create_host`. The selection is unset-until-touched rather than
/// seeded into a signal at mount: the poll underneath can change which hosts
/// exist, and a seeded value would pin the dialog to a host that has since
/// been removed with nothing to say about it.
///
/// When a chosen host DOES disappear, the reconciliation is visible rather
/// than silent: the selector moves to the default, a line says so, and the
/// intent key is re-minted for the new target. The failure that rules out is
/// the quiet one — a selector still displaying host A while the body carries
/// host B.
///
/// A submit before the first hosts read lands is refused locally with a
/// reason, rather than sent without a host. The helm would happily default a
/// hostless create to its local row, which is usually right and is not
/// something this form may decide by omission: the user is looking at a
/// selector that has not filled in yet.
///
/// A refused create — an unreachable host, a nonexistent directory —
/// surfaces the helm's words in the same error line and leaves the form
/// exactly as filled, host selection included.
///
/// ## The agent picker (PLAN_M6_75.md item 8)
///
/// The dialog offers the TARGET host's profiles and defaults to the one a
/// session was last created from there, asking rather than guessing when that
/// profile is gone — `profiles::resolve_agent` owns both halves of that rule
/// and is where the reasoning lives. Two consequences show up here:
///
/// - Changing the host CLEARS the profile choice. A profile id is minted per
///   supervisor and every fresh supervisor seeds the same starters, so an id
///   carried across would not merely go stale — it would resolve over there,
///   against a profile nobody chose.
/// - The command field is disabled while a profile is selected, because the
///   two creation modes are mutually exclusive on the wire and a body naming
///   both is refused. Disabling it is also what keeps the intent binding
///   honest: a field the user cannot reach cannot change what the key stands
///   for.
///
/// The raw command path stays on the dialog rather than being replaced. It is
/// what runs anything no profile describes, and the e2e harness's own creates
/// go through it — a dialog that only offered profiles would make an ad-hoc
/// command a trip to `curl`.
///
/// ## Clone prefill
///
/// A "clone" click (`ListView`'s `on_clone`) opens this same form with
/// `prefill` set instead of building a second, immediate-create path — see
/// `CreatePrefill`'s own doc for what it carries and why. The one wrinkle it
/// adds to the agent picker: a prefilled profile choice must WIN over the
/// target host's remembered default on the render right after the clone,
/// which is the opposite of the ordinary "nothing chosen yet, seed from the
/// remembered default" rule two paragraphs up. The reseed effect (below)
/// gets this for free rather than as a special case, because it sets
/// `chosen_profile` before the remembered-default seeding runs, and that
/// seeding already backs off the moment a choice already exists.
///
/// The host is not accepted at face value. A `HostId` is a registry row
/// that survives a retarget or an adopt while the machine behind it
/// changes, so the reseed effect runs the cloned row's `host` and
/// `host_identity` through `shared::matching_host_option` — the exact
/// install comparison `default_create_host` already applies to SPEC.md's
/// ordinary creation default — before selecting it or trusting the choice
/// that came with it (`resolve_clone_host`, and `CloneHostState`'s own doc
/// for the states that comparison moves between). A row whose install
/// cannot be confirmed — hostless entirely, or mismatched once the
/// registry has had a chance to answer — is left unselected: the selector
/// falls through to its ordinary default, no agent is carried over with
/// it, and `clone_agent_note` (below) is what tells the user why, reusing
/// the same host-note slot `choice_vanished` already renders through.
/// Unconfirmed is not the same as unchecked, though: a clone opened before
/// the FIRST `hosts` read lands is retried once the registry answers
/// rather than given up on (item2-review2.md F1) — necessary because the
/// text-field reseed above is a one-shot latch and will not give the host
/// a second chance on its own — and a clone whose binding DID succeed is
/// re-checked on every later pass too, so a retarget or an adopt landing
/// while the form stays open withdraws it back to unselected rather than
/// silently continuing to name a machine the clone was never actually
/// taken from (F3). An explicit host pick takes the decision away from
/// this reconciliation entirely, permanently, for the rest of the
/// generation (`CloneHostState::UserTookOver`). A CROSS-host clone that
/// DOES pass the install check still cannot be applied immediately —
/// `pending_choice` is the queue for the one render this form's own
/// catalog target needs to catch up with a host `chosen_host` just moved
/// to (see that field's own doc for the exact handoff and the fingerprint
/// it is bound to) — and any explicit host or agent interaction cancels a
/// still-queued choice outright, so a user who answers before the handoff
/// catches up is never overwritten by it arriving late (F6).
#[component]
pub(super) fn CreateSessionForm(
    hosts: Vec<HostOption>,
    /// See `ListView`'s parameter of the same name: the selected session's
    /// host and its reported install identity, SPEC.md's first
    /// create-default clause.
    open_host: Option<OpenHost>,
    /// Whether the hosts read has EVER succeeded. Distinguishes "there are
    /// no hosts" (impossible for a live helm, which always has its local
    /// row) from "nothing has come back yet", which is what a submit has to
    /// be refused for.
    hosts_loaded: bool,
    /// The user's explicit host choice, if they have made one. `None` means
    /// "no choice yet", not "no host" — the effective target is
    /// [`effective_create_host`]'s answer, recomputed per render against the
    /// hosts that exist right now.
    ///
    /// `ListView`'s signal rather than this form's, because the profile
    /// catalog is read for whatever this names and the reads on that page go
    /// through readers it owns (see `ListView`).
    mut chosen_host: Signal<Option<HostId>>,
    /// The target host's profile catalog, read by `ListView` and rendered
    /// here. Pointed at [`effective_create_host`]'s answer, so the picker and
    /// the create can never describe different machines.
    catalog: CatalogSurface,
    /// The page's live-operation token. Claimed at submit, released when the
    /// request completes — the exclusion against every host mutation, and
    /// against a second submit of this form (see `ops`).
    mut ops: OpLock,
    /// A "clone" click's seed, or `None` for the ordinary blank-form open.
    /// `ListView` owns the signal this reads and bumps `generation` on
    /// every clone (see `CreatePrefill`); this component's own reseed
    /// effect (below `chosen_profile`'s declaration) is what turns a new
    /// generation into field values.
    prefill: Option<CreatePrefill>,
    on_created: EventHandler<Session>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    // Prefilled rather than empty-with-a-placeholder, deliberately: what
    // gets sent is always exactly what the field shows, and the common
    // "just give me a session in my home directory" create needs no typing
    // at all. `~` resolves against the TARGET host's home — the supervisor
    // expands it at create time (SPEC.md's working-directory rule), which
    // is what makes a host-independent default possible here at all: this
    // form cannot know a remote host's home path.
    let mut cwd = use_signal(|| "~".to_string());
    let mut invocation = use_signal(String::new);
    let mut title = use_signal(String::new);
    // What each of the three text fields above was SEEDED from, raw, and
    // whether the user has typed in it since — `profiles::ProfileDraft`'s
    // escaped-display / raw-seed / edited-flag model, reused rather than
    // re-invented (item2-review2.md F5): a clone's directory, invocation and
    // title are peer-relayed text going into an editable control for
    // exactly the reason a profile's name and invocation are, and an
    // untouched field must submit the ORIGINAL bytes rather than the
    // escaped spelling `cwd`/`invocation`/`title` display while the clone is
    // on screen (see the reseed effect below for where the escaping is
    // applied, and `profiles::submitted_field` for the read-back half these
    // are fed into at submit time). `None` seeds mean "never clone-seeded",
    // which is the ordinary blank-create case: there the field's own text
    // already IS the value to submit, since nothing relayed it from a peer.
    let mut cwd_raw_seed = use_signal(|| None::<String>);
    let mut cwd_edited = use_signal(|| false);
    let mut invocation_raw_seed = use_signal(|| None::<String>);
    let mut invocation_edited = use_signal(|| false);
    let mut title_raw_seed = use_signal(|| None::<String>);
    let mut title_edited = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    // The agent this dialog is going to use, once anything has decided it —
    // the user picking, or the host's remembered default being CONSUMED (see
    // the effect below). `None` means nobody has decided yet.
    let mut chosen_profile = use_signal(|| None::<AgentChoice>);
    // Whether an explicit choice has been overtaken by reality. Derived per
    // render rather than written back into `chosen_host`, so it cannot
    // outlive the condition that produced it — and so a host that comes back
    // (a re-added destination) silently reinstates the user's choice.
    let choice_vanished =
        chosen_host().is_some_and(|chosen| !hosts.iter().any(|host| host.id == chosen));
    // What has become of the CURRENT clone generation's own host+agent
    // binding — see `CloneHostState`'s own doc for the four states and why
    // a bool cannot stand in for them. Reseeded to `Waiting` (or, for a
    // hostless clone, straight to `Unconfirmable`) every time a new
    // generation arrives, and otherwise updated only by the reseed effect
    // below (`resolve_clone_host`) and by an explicit host pick.
    let mut clone_host_state = use_signal(|| CloneHostState::Unconfirmable);
    // Why this clone's own host+agent portion is not (or is no longer) in
    // play, when there is something worth telling the user about it — read
    // straight off `clone_host_state` every render, never cached, so a
    // mismatch that resolves later (the registry catching up, an identity-
    // mismatch phase clearing) stops being reported the instant it stops
    // being true. `None` covers three unremarkable cases at once: no clone
    // is open, this generation is still `Waiting` on the registry (F1 says
    // nothing yet rather than guessing), and `Bound`/`UserTookOver`, where
    // there is nothing to explain.
    let clone_agent_note = prefill
        .as_ref()
        .and_then(|prefill| match *clone_host_state.read() {
            CloneHostState::Unconfirmable if prefill.host.is_none() => Some(
                "the session you cloned predates host tracking, so its agent could not be \
                 confirmed on any host and was not carried over here — check the agent below",
            ),
            CloneHostState::Unconfirmable => Some(
                "the session you cloned reports a different installation now, so its host and \
                 agent were not carried over here — check the host and agent below",
            ),
            CloneHostState::Waiting | CloneHostState::Bound | CloneHostState::UserTookOver => None,
        });
    let selected = effective_create_host(&hosts, chosen_host(), open_host.as_ref());
    // This form's current intended create, if one has been submitted yet
    // (PLAN_M3.md item 6), together with the BINDING it was minted for.
    // Minted at first submit, reused by every later submit of the same
    // intent, and superseded the moment any part of that binding changes.
    let mut intent_key = use_signal(|| None::<(String, IntentBinding)>);
    let busy = ops.busy();

    // The agent choice is bound to an INSTALL, and this is what binds it.
    //
    // Two rules, and both are about not letting a decision outlive the thing
    // it was made about:
    //
    // - A target change — the user picking another host, a chosen host
    //   leaving the registry and the default taking over, or the SAME row
    //   being retargeted or adopted — discards the choice and the intent
    //   key. A profile id means nothing on another supervisor, and because
    //   every fresh supervisor seeds the same starters, carrying one across
    //   does not fail loudly: it resolves, to a profile nobody picked. A
    //   mere RECONNECTION is deliberately not a target change (`HostTarget`
    //   carries no connection token): a reconnect is precisely when a reply
    //   gets lost, and rotating the key there would turn the user's retry
    //   into a second intended create.
    // - The remembered default is CONSUMED ONCE per target, and `seeded_for`
    //   is what records that it has been. Tracking consumption by "a choice
    //   exists" is not enough and the gap is reachable: a first catalog with
    //   NO remembered default, or one whose default was already deleted,
    //   leaves no choice behind — so a later refresh would seed from whatever
    //   the helm remembers BY THEN, and another client's create would move the
    //   selection under an open dialog. Latching the first answer, whatever it
    //   was, is what makes the default a decision this dialog made rather than
    //   a value it follows.
    let mut bound_target = use_signal(|| None::<HostTarget>);
    let mut seeded_for = use_signal(|| None::<HostTarget>);
    // Which prefill GENERATION (`CreatePrefill`) this form has already
    // applied. Compared by generation rather than by mere presence in the
    // effect below, because `prefill` stays populated at its latest
    // generation for as long as this form is open, so presence alone would
    // reseed on every unrelated rerun of that effect (a host reconnect, a
    // catalog refresh) for as long as a clone is on screen.
    let mut prefill_applied = use_signal(|| None::<u64>);
    // A prefilled agent CHOICE whose target host has not yet been PROVEN
    // to be the effective one, held here until it has — or dropped, when
    // the clone's own host never earns that proof at all.
    //
    // Two different reasons queue a choice here rather than applying it on
    // the spot, and both matter for what "proven" has to mean:
    //
    // - Cross-host: the reseed effect below writes `chosen_host` the
    //   instant a clone names a different host, but the catalog surface
    //   that decides `catalog.watch_target()` is `ListView`'s
    //   (`create_target`/`create_catalog`), derived from `chosen_host`
    //   through an effect on THAT component — so it only catches up on a
    //   LATER render pass, not this one.
    // - Same host, but not yet TRUSTED: `shared::matching_host_option`
    //   (the same install comparison `default_create_host` runs for an
    //   ordinary create) needs `hosts` to have loaded before it can tell a
    //   row whose host still fronts the same install apart from one
    //   retargeted or adopted onto a successor — see `CreatePrefill::
    //   host_identity`'s own doc for why trusting it early would be able
    //   to run a stale command, or resolve a profile id, on the wrong
    //   machine. Until `hosts_loaded`, nothing here is proven either way.
    //
    // Consumed — applied, then cleared — the moment a target update's host
    // matches AND that install check passes; until then `resolve_agent`
    // simply sees no chosen profile and blocks, the same as it would for
    // any other host switch still in flight. Bound to the target's full
    // `HostTarget` (fingerprint included, via `HostTarget::same_install`)
    // rather than to a bare host id, so a retarget of the SAME row landing
    // during this wait cannot be mistaken for the install this choice was
    // actually queued for.
    let mut pending_choice = use_signal(|| None::<(HostTarget, AgentChoice)>);
    // Cloned rather than borrowed into the effect below: `hosts` is this
    // component's own prop (not a `Signal`, so it cannot be `Copy`-captured
    // the way the surrounding signals are), and the render body further
    // down needs its own, unmoved copy for the selector and the agent
    // picker.
    let hosts_for_reseed = hosts.clone();
    use_effect(use_reactive(
        (&prefill.as_ref().map(|prefill| prefill.generation),),
        move |_| {
            let hosts = &hosts_for_reseed;
            let target = catalog.watch_target();
            let read = catalog.catalog.read();
            let previous = bound_target.peek().clone();

            // Applied BEFORE the host+agent resolution below, and in the
            // SAME effect invocation rather than a separate one: a clone
            // aimed at a different host is itself what moves
            // `chosen_host`, and these fields must seed exactly once
            // whether or not the host also moves as a result.
            if let Some(prefill) = prefill
                .as_ref()
                .filter(|prefill| Some(prefill.generation) != *prefill_applied.peek())
            {
                prefill_applied.set(Some(prefill.generation));
                reseed_cloned_field(&mut cwd, &mut cwd_raw_seed, &mut cwd_edited, &prefill.cwd);
                reseed_cloned_field(
                    &mut title,
                    &mut title_raw_seed,
                    &mut title_edited,
                    &prefill.title,
                );
                // Every clone generation establishes the WHOLE form state,
                // the dormant command field included — see `CreatePrefill::
                // invocation`'s own doc for why a profile-backed clone
                // still needs this written.
                reseed_cloned_field(
                    &mut invocation,
                    &mut invocation_raw_seed,
                    &mut invocation_edited,
                    &prefill.invocation,
                );
                // A prefill is as fresh an intent as any manual edit —
                // see the field `oninput` handlers below for both edges
                // of the "the key describes what was submitted" rule
                // this keeps.
                intent_key.set(None);
                // A refusal from before this clone described whatever
                // the form held then, which this prefill has just
                // replaced wholesale.
                error.set(None);

                // item2-review2.md F2: every new generation starts its OWN
                // host+agent binding from a clean slate, cleared BEFORE any
                // attempt to resolve it — otherwise a clone whose own host
                // cannot be confirmed (rejected below, or hostless) could
                // silently inherit whatever a PREVIOUS generation (or an
                // earlier manual pick) had left in these three signals.
                chosen_host.set(None);
                chosen_profile.set(None);
                pending_choice.set(None);
                clone_host_state.set(match prefill.host {
                    None => CloneHostState::Unconfirmable, // F4: nothing to resolve safely
                    Some(_) => CloneHostState::Waiting,
                });
            }

            // Resolve (or re-resolve) THIS generation's own host+agent
            // binding. Deliberately NOT gated on the generation transition
            // above — it runs on every pass this effect fires, reading
            // whatever `clone_host_state` currently holds, which is what
            // makes both F1's retry (once the registry answers a clone that
            // opened before it did) and F3's withdrawal (the instant a
            // `Bound` installation stops matching) possible: `prefill_applied`
            // above is a one-shot latch and will never route this
            // generation through the reseed branch a second time, so the
            // binding needs a path that keeps trying independently of it.
            if let Some(prefill) = prefill.as_ref() {
                let (next_state, action) = resolve_clone_host(
                    *clone_host_state.peek(),
                    prefill.host,
                    &prefill.host_identity,
                    &prefill.agent,
                    hosts_loaded,
                    hosts,
                    prefill.host.is_some() && chosen_host.peek().as_ref() == prefill.host.as_ref(),
                );
                clone_host_state.set(next_state);
                match action {
                    CloneHostAction::Hold => {}
                    CloneHostAction::Bind { target, choice } => {
                        chosen_host.set(Some(target.host));
                        // Cleared HERE, not left to whatever this form
                        // held before the clone: a queued choice must not
                        // leave a PREVIOUS selection visible to
                        // `resolve_agent` while this one waits for the
                        // catalog to catch up, since that previous choice
                        // names a different install's profile — see
                        // `pending_choice`'s own doc for why the guard
                        // that actually blocks submission during the wait
                        // is the host/catalog mismatch check, not this
                        // clear, which exists so a blocked dialog reads as
                        // "nothing chosen yet" rather than "still showing
                        // the previous pick" while aimed at a new host.
                        chosen_profile.set(None);
                        pending_choice.set(Some((target, choice)));
                    }
                    CloneHostAction::Withdraw => {
                        chosen_host.set(None);
                        chosen_profile.set(None);
                        pending_choice.set(None);
                    }
                }
            }
            // A `resolve_clone_host` outcome of `Unconfirmable` (a hostless
            // row, or one whose install could not be confirmed) leaves the
            // selector unselected: `clone_agent_note` (rendered above) is
            // what tells the user why, and the selector falls through to
            // its ordinary default (`effective_create_host`) instead of a
            // host this clone can no longer vouch for. The profile choice
            // stays dropped with it — a profile id is only as good as the
            // install it was minted on, and applying it against whatever
            // host ends up being the default would risk resolving a
            // deterministically-numbered starter profile that belongs to a
            // different machine.

            // A queued choice is applied the instant its OWN install
            // becomes the effective target, whether or not the target
            // moved on THIS pass — a fresh clone whose host is already
            // current (the ordinary "clone again without closing the
            // form" case) needs this to run in the SAME pass that queued
            // it, since the target-change branch below only fires when
            // `previous != target`. Bound once and consumed here, rather
            // than peeked twice, so there is exactly one place that reads
            // it and exactly one that decides whether it matched.
            let pending_for_target = pending_choice.peek().clone().filter(|(pending, _)| {
                target.as_ref().is_some_and(|now| pending.same_install(now))
            });
            let caught_up = pending_for_target.is_some();
            if let Some((_, choice)) = pending_for_target {
                chosen_profile.set(Some(choice));
                pending_choice.set(None);
            }

            if previous != target {
                bound_target.set(target.clone());
                // The catalog must be re-seeded whenever the TARGET moved —
                // an install change (another row picked, or the same row
                // retargeted or adopted), never a mere reconnection, which
                // does not change `HostTarget` at all. What was held so far
                // describes the previous install, and treating it as an
                // answer about this one would offer profile ids that
                // resolve to something else over here.
                seeded_for.set(None);
                // The CHOICE and the KEY, however, rotate only when the
                // INSTALL changes. A reconnection to the same machine is
                // precisely when a reply gets lost, and that is the case
                // the idempotency key exists for — rotating it there would
                // turn the user's retry into a second intended create and
                // hand them two sessions for one press. A retarget or an
                // adoption is the opposite and must rotate: the id now
                // means another machine, where the key dedups against
                // nothing and the profile id resolves to something else.
                let same_install = match (&previous, &target) {
                    (Some(before), Some(now)) => before.same_install(now),
                    // Opening the dialog (None -> Some) and closing it are
                    // not reconnections; a fresh dialog starts fresh.
                    _ => false,
                };
                // `caught_up`, computed above, is what keeps this branch
                // from undoing the choice a fresh clone just applied (or
                // just consumed from `pending_choice`) on the very same
                // pass this target change is itself part of — see
                // `pending_choice`'s own doc for the concrete case
                // (F2-shaped: a clone into a freshly mounted form, whose
                // source host is already the child's effective target)
                // this guards against.
                if !caught_up && !same_install {
                    chosen_profile.set(None);
                    intent_key.set(None);
                    // A refusal belongs to the host it came from. Left
                    // standing, host A's "directory does not exist" would
                    // sit under a form now aimed at host B, where it may
                    // not even be true.
                    error.set(None);
                }
            }
            // Consumption is only meaningful once there IS a target: on the
            // first render both are `None`, and reading that equality as
            // "already seeded" would make an unread catalog
            // indistinguishable from a deleted remembered profile — the
            // dialog would open claiming something was gone before it had
            // asked anything.
            if target.is_some() && *seeded_for.peek() == target {
                return;
            }
            // Only a catalog that answers the CURRENT question may seed a
            // choice — `lookup` is what refuses one belonging to a previous
            // activation or another install.
            let CatalogLookup::Known { catalog: held, .. } = catalog.lookup(&read) else {
                return;
            };
            seeded_for.set(target);
            // The user (or a clone prefill, applied above) may have
            // answered while the read was in flight; an answer outranks a
            // default.
            if chosen_profile.peek().is_some() {
                return;
            }
            // Three outcomes, decided in one place (`profiles::seeded_choice`):
            // the remembered profile, the command path where nothing was ever
            // remembered, or NO choice where the remembered one is gone — which is
            // what leaves the dialog blocked and asking, told apart from "not read
            // yet" by the latch this effect just set.
            if let Some(choice) = seeded_choice(held) {
                chosen_profile.set(Some(choice));
            }
        },
    ));

    // What the picker may offer: this surface's catalog, and nothing at all
    // until one has been read for the question it is asking right now
    // (`CatalogSurface::lookup` refuses anything else, which is what stops
    // one install's profiles being offered for a create aimed at another).
    let catalog_read = catalog.catalog.read();
    let held = catalog.lookup(&catalog_read);
    let offered = match &held {
        CatalogLookup::Known { catalog, .. } => Some(*catalog),
        _ => None,
    };
    // Seeded only once a concrete target has been answered — see the effect
    // above for why `None == None` must not read as consumption.
    let seeded = bound_target.read().is_some() && *seeded_for.read() == *bound_target.read();
    let agent = resolve_agent(chosen_profile.read().as_ref(), offered, seeded);
    let by_profile = matches!(agent.choice, Some(AgentChoice::Profile(_)));
    // Owned, because the picker's options compare against it inside a loop
    // that also borrows the catalog guard this selection was derived from.
    // The placeholder's value stands in for "nothing is selected", which is a
    // state this dialog can genuinely be in — see `profiles::resolve_agent`.
    let chosen_agent = agent
        .choice
        .as_ref()
        .map(|choice| choice.value().to_string())
        .unwrap_or_else(|| UNRESOLVED_VALUE.to_string());

    // What a submit would launch, resolved SYNCHRONOUSLY inside the handler
    // from the live signals and the catalog as it stands at that instant —
    // never from a value the last render happened to compute.
    //
    // The distinction is one JavaScript turn wide and it decides what runs: a
    // change to the picker followed by a submit in the same turn reaches the
    // handler before any re-render, so a captured render-time value would send
    // the PREVIOUS selection under a freshly minted key — a key that faithfully
    // describes an intent nobody had. What is frozen is this resolution's
    // result, held across the minting await (see the submit path).
    let resolve_now = move || {
        let read = catalog.catalog.peek();
        let offered = match catalog.lookup(&read) {
            CatalogLookup::Known { catalog, .. } => Some(catalog),
            _ => None,
        };
        let seeded = bound_target.peek().is_some() && *seeded_for.peek() == *bound_target.peek();
        resolve_agent(chosen_profile.peek().as_ref(), offered, seeded).choice
    };

    rsx! {
        form {
            class: "create-session-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // The claim is the guard, and it is synchronous: it covers a
                // second submit of THIS form (a double-click, a stray repeat
                // event) and every host mutation at once, with no render in
                // between for a stale boolean to be read from.
                //
                // The OTHER half of double-submission — a retry after an
                // ambiguous transport failure (request sent, response lost)
                // reaching the supervisor a second time — is what
                // `intent_key` closes, and it cannot be closed here: only
                // the server knows whether the lost reply belonged to a
                // session that actually exists. This handler's job is merely
                // to send the SAME key for every retry of one intent.
                if !ops.claim() {
                    return;
                }
                // No agent, no create. "Nothing is selected" is a real state
                // rather than a gap to be filled — a profile that was chosen
                // or remembered and has since been deleted leaves the dialog
                // waiting for an answer, and the command field it would
                // otherwise fall back to still holds whatever was typed into
                // it earlier. Launching that would run something nobody
                // picked while the note beside it said nothing was selected.
                let Some(choice) = resolve_now() else {
                    error.set(Some(
                        "no agent is selected for this create — choose a profile, or choose \
                         \"custom command\" to run the command below"
                            .to_string(),
                    ));
                    ops.release();
                    return;
                };
                // Frozen HERE, from what was just resolved, and not touched
                // again: the minting await below can span a deletion or
                // another client's remembered-default write, and re-resolving
                // across it would let the request's MODE differ from the one
                // the button was pressed on. A profile that goes away in that
                // window is refused by the supervisor, by name.
                let launch = match choice {
                    // The RAW bytes while untouched, not the escaped display
                    // the field shows — `profiles::submitted_field` is the
                    // same read-back rule the profile editor uses for its
                    // own peer-relayed fields (item2-review2.md F5).
                    AgentChoice::Command => LaunchIntent::Command(submitted_field(
                        &invocation.peek(),
                        *invocation_edited.peek(),
                        invocation_raw_seed.peek().as_deref(),
                    )),
                    AgentChoice::Profile(id) => LaunchIntent::Profile(id),
                };
                // The HOST is derived here too, from the live signal — never
                // from what the last render computed. The same one-turn window
                // the agent has: changing the selector and pressing create in
                // one turn reaches this handler before any re-render, and a
                // captured host would send the create to the PREVIOUS machine
                // while the selector on screen names another.
                let target_now = catalog.target();
                let selected_now =
                    effective_create_host(&hosts, chosen_host.peek().to_owned(), open_host.as_ref());
                // And the catalog the agent was just resolved against has to
                // be the catalog OF that host — the same row AND the same
                // install fingerprint, not merely the same id. When they
                // disagree the target effect has not caught up yet — a window
                // of one render, which a retarget or an adoption can also
                // open on an UNCHANGED selection — and a profile id resolved
                // against the old install's catalog would be sent to the new
                // one, where it means something else or nothing.
                if !catalog_matches_selection(selected_now, &hosts, target_now.as_ref()) {
                    error.set(Some(
                        "the target host changed while this create was being submitted, so                          nothing was sent — check the agent and press create again"
                            .to_string(),
                    ));
                    ops.release();
                    return;
                }
                // No host, no create. The helm would default a hostless body
                // to its local row — usually the right answer, and not one
                // this form may reach by omission while its own selector is
                // still blank. Saying so beats creating on a machine the
                // user was never shown.
                let Some(binding) = IntentBinding::of(
                    selected_now,
                    &hosts,
                    submitted_field(&cwd(), cwd_edited(), cwd_raw_seed.peek().as_deref()),
                    launch,
                    submitted_field(&title(), title_edited(), title_raw_seed.peek().as_deref()),
                ) else {
                    error.set(Some(
                        if hosts_loaded {
                            "this helm reported no hosts at all, so there is nothing to create on"
                                .to_string()
                        } else {
                            "the host list has not loaded yet, so this create was not sent — it \
                             would have gone to whichever host the helm picked rather than one \
                             you chose"
                                .to_string()
                        },
                    ));
                    ops.release();
                    return;
                };
                let base = base.clone();
                // The target row's install identity as this form knew it at
                // submit — resolved here, outside the spawn, because
                // `binding.host` is frozen (the mint loop re-reads text
                // fields only) and the reply below carries no host facts to
                // resolve it from. It backfills the created `Session` so
                // the selection this create becomes carries the same
                // install claim a listing row would have carried.
                let created_host_identity = hosts
                    .iter()
                    .find(|host| host.id == binding.host)
                    .map(|host| host.identity.clone());
                // The connection this create is prepared against, read from
                // the same hosts snapshot the target match above just
                // vouched for: the helm refuses the create if the host has
                // been retargeted or adopted onto another install by the
                // time it routes, which is what keeps a profile id chosen
                // from THIS catalog from resolving on a successor's starter
                // of the same id. `None` — a vanished row, or one that has
                // never connected — means no claim (see `connection_claim`).
                let expected_incarnation = connection_claim(&hosts, binding.host);
                error.set(None);
                spawn(async move {
                    // Mint until the key and the binding agree.
                    //
                    // Minting is an `await` (the wasm renderer asks the
                    // browser for a UUID), and the `disabled` attributes that
                    // make this form inert land one render AFTER the submit —
                    // so a keystroke already queued when it fired can still
                    // change a field while the key is being made. The
                    // re-read below is what closes that, not the attribute. Publishing that key
                    // would bind it to values the user has since edited,
                    // which is the same wrong-intent failure a changed host
                    // causes, arriving through a narrower window. Re-reading
                    // the binding after the await and minting again on a
                    // mismatch is what closes it.
                    //
                    // Bounded rather than a `loop`: a form whose values keep
                    // changing every time a key is minted is not a create
                    // anybody is waiting on, and spinning would be worse
                    // than saying so.
                    let mut binding = binding;
                    let mut attempts = 0;
                    let (key, bound) = loop {
                        let held = intent_key.peek().clone();
                        if let Some((key, held_binding)) = held
                            && held_binding == binding
                        {
                            break (key, held_binding);
                        }
                        if attempts >= MINT_ATTEMPTS {
                            error.set(Some(
                                "the form kept changing while an idempotency key was being \
                                 generated, so this create was not sent; try again"
                                    .to_string(),
                            ));
                            ops.release();
                            return;
                        }
                        attempts += 1;
                        match mint_intent_key().await {
                            Ok(key) => intent_key.set(Some((key, binding.clone()))),
                            Err(reason) => {
                                // No key, no create: see this component's
                                // docs on why an unkeyed create is not an
                                // acceptable degradation. The message says
                                // what failed rather than blaming the
                                // request, since nothing the user typed
                                // caused it.
                                error.set(Some(format!(
                                    "could not generate an idempotency key for this create, so \
                                     it was not sent (a retry could otherwise create a second \
                                     session): {reason}"
                                )));
                                ops.release();
                                return;
                            }
                        }
                        // What the form's TEXT says now. Identical on the
                        // ordinary path; different exactly when a queued edit
                        // landed during the mint.
                        //
                        // The agent is deliberately NOT re-read here. It was
                        // frozen when the button was pressed, and re-resolving
                        // it would let a deletion or another client's
                        // remembered-default write — either of which can land
                        // during this await — change which creation MODE the
                        // request carries. A key that names one intent and a
                        // body that carries another is the exact failure the
                        // key exists to prevent, so the press wins and a
                        // profile that has since gone is refused by the
                        // supervisor, by name.
                        binding = IntentBinding {
                            cwd: submitted_field(
                                &cwd.peek(),
                                *cwd_edited.peek(),
                                cwd_raw_seed.peek().as_deref(),
                            ),
                            title: submitted_field(
                                &title.peek(),
                                *title_edited.peek(),
                                title_raw_seed.peek().as_deref(),
                            ),
                            ..binding
                        };
                    };
                    // Key, fields, host AND creation mode all travel from ONE
                    // value, so there is no arrangement of edits or reads in
                    // which the body describes a different intent than the
                    // key claims — including the mode itself, which the
                    // supervisor folds into its own idempotency fingerprint
                    // precisely so a retried create cannot flip it.
                    let agent = match &bound.agent {
                        LaunchIntent::Command(invocation) => CreateAgent::Command(invocation),
                        LaunchIntent::Profile(id) => CreateAgent::Profile(id),
                    };
                    match create_session(
                        &base,
                        &bound.cwd,
                        agent,
                        &bound.title,
                        &key,
                        Some(bound.host),
                        expected_incarnation,
                    )
                    .await
                    {
                        Ok(session) => {
                            // Released BEFORE navigating: `on_created`
                            // unmounts this component, and a token released
                            // afterwards would be released by a task nobody
                            // is left to run.
                            ops.release();
                            on_created.call(enrich_created_session(
                                session,
                                bound.host,
                                created_host_identity,
                            ));
                        }
                        Err(e) => {
                            // Gated on the target this request was DISPATCHED
                            // for still being the one on screen: a refusal
                            // naming host A must not land under a form that
                            // has since been re-pointed at host B, where it
                            // would describe a machine the user is not looking
                            // at and may not even be true.
                            if catalog.target() == target_now {
                                let (stale, prose) = api::precondition_of(&e);
                                if stale {
                                    // The world moved between preparing this
                                    // create and routing it — the id now
                                    // reaches another install, where the
                                    // profile id would have resolved to
                                    // something else. The key goes with it (a
                                    // retry must be a NEW intent, not a replay
                                    // aimed at a machine that never saw the
                                    // first) and the catalog is re-read, which
                                    // is what supersedes this message.
                                    intent_key.set(None);
                                    chosen_profile.set(None);
                                    catalog.request(Trigger::Explicit);
                                }
                                // The key otherwise deliberately SURVIVES a
                                // failure: a failure whose cause was an
                                // ambiguous transport error may have created
                                // a session the user cannot see, and
                                // resubmitting unchanged must reach that same
                                // session rather than launch a second agent.
                                // A user who instead fixes the form gets a new
                                // key, because the binding no longer matches.
                                error.set(Some(prose));
                            }
                            ops.release();
                        }
                    }
                });
            },
            // Working directory and agent command are literal text that
            // gets EXECUTED, never prose — OS-level text mangling has no
            // way to tell the difference and "corrects" them anyway
            // (observed directly: WKWebView's autocorrect silently
            // substituting "claude" with "Claude" in place, with no
            // visible suggestion popup to catch and reject). A
            // capitalized command or a suggestion-popup keystroke
            // swallowed mid-path corrupts what actually runs. Title IS
            // ordinary prose, but the same opt-out applies to it too, for
            // a narrower reason: whatever the user types is what should
            // come back out verbatim (SPEC.md's "auto-generated when
            // omitted" is the only substitution this field ever gets, and
            // it happens server-side, deliberately, not as a silent
            // client-side "helpful" rewrite) — so every input here opts
            // out of every form of text mangling a browser might apply on
            // its own, for whichever of these two reasons applies to it.
            // First, because it decides what everything below it means: a
            // working directory and an agent command are only meaningful
            // relative to the machine they will run on.
            label {
                "host"
                select {
                    class: "create-session-host",
                    // Disabled for the whole round trip, exactly like the
                    // text fields: the key is bound to the target, and a
                    // selection changing between minting and sending would
                    // publish a key that belongs to a different machine.
                    disabled: busy,
                    // Empty only before the first hosts read lands — a live
                    // helm always has its local row. The submit handler
                    // refuses in that window rather than sending a hostless
                    // create.
                    value: selected.map(|id| id.to_string()).unwrap_or_default(),
                    onchange: move |evt| {
                        chosen_host.set(evt.value().parse::<HostId>().ok());
                        // A profile id belongs to ONE supervisor, so a choice
                        // cannot follow the user to another host: carrying it
                        // over would either name nothing there or — because
                        // every fresh supervisor seeds the same starters —
                        // resolve to a profile they never chose. Cleared
                        // rather than remembered, so the new host's own
                        // remembered default takes over.
                        chosen_profile.set(None);
                        // item2-review2.md F6: an explicit host pick must not
                        // be silently overwritten by a cross-host clone
                        // handoff that is still in flight — the choice made
                        // just now is newer than anything queued before it.
                        pending_choice.set(None);
                        // And it takes this generation's clone-derived
                        // binding off automatic handling for good
                        // (`CloneHostState::UserTookOver`): the user is now
                        // driving host selection by hand, so a later
                        // retarget of the CLONE's own row must not pull the
                        // rug out from under a choice the clone had nothing
                        // to do with anymore.
                        clone_host_state.set(CloneHostState::UserTookOver);
                        // A different host is a different intended create,
                        // exactly as a different directory is — so the key
                        // the last submit used stops applying (see this
                        // component's docs for both edges of that rule).
                        intent_key.set(None);
                    },
                    for host in hosts.iter() {
                        option {
                            key: "{host.id}",
                            value: "{host.id}",
                            // Marked on the OPTION as well as through the
                            // select's `value` above, and that redundancy is
                            // load-bearing rather than belt-and-braces — see
                            // the agent picker below, where the same
                            // arrangement is what makes a preselection appear
                            // at all.
                            selected: selected == Some(host.id),
                            "{host.label()}"
                        }
                    }
                }
            }
            // The reconciliation, said out loud. A chosen host leaving the
            // registry moves the effective target, and the one thing that
            // must not happen is that move being invisible — a selector
            // showing host A while the body carries host B is a create on a
            // machine nobody picked. The key is re-minted for the new target
            // by the submit path's own binding check.
            if choice_vanished {
                div { class: "create-session-host-note",
                    "the host you picked is no longer registered, so this create would go to the \
                     one selected now"
                }
            }
            // The clone-specific reconciliation, in the same voice and the
            // same slot: the row this form was cloned from could not be
            // confirmed as the install it was cloned from (mismatched, or
            // predating host tracking entirely — see `clone_agent_note`'s
            // own doc), so its host and agent were not carried over, and
            // the selector below shows its ordinary default instead. Says
            // nothing while a hostful clone is still `Waiting` on the
            // registry (F1) — that is not yet a fact worth reporting.
            if let Some(note) = clone_agent_note {
                div { class: "create-session-host-note", "{note}" }
            }
            // The agent, offered from the TARGET host's catalog and defaulting
            // to what a session was last created from there (SPEC.md's
            // creation rule; `profiles::resolve_agent`). The empty option is
            // the raw command path below rather than "no agent" — a create
            // always launches something, and this select is which of the two
            // mutually exclusive modes it uses.
            label {
                "agent"
                select {
                    class: "create-session-profile",
                    // Inert for the whole round trip, exactly like the host
                    // selector and for the same reason: the idempotency key
                    // is bound to what is launched, so a selection that moved
                    // between minting and sending would publish a key
                    // belonging to a different create.
                    disabled: busy,
                    value: "{chosen_agent}",
                    onchange: move |evt| {
                        chosen_profile.set(AgentChoice::from_value(&evt.value()));
                        // item2-review2.md F6: an explicit agent pick is
                        // newer than any cross-host clone handoff still
                        // queued behind it, and must win outright rather
                        // than being overwritten once that handoff catches
                        // up on a later render.
                        pending_choice.set(None);
                        // A different agent is a different intended create,
                        // exactly as a different directory is.
                        intent_key.set(None);
                    },
                    // The placeholder exists only while nothing is selected,
                    // and it is what a blocked dialog SHOWS: a `size=1` select
                    // always has one option selected, so "no answer yet" needs
                    // an option of its own rather than borrowing the command
                    // path's — borrowing it is exactly how a vanished profile
                    // used to turn into a silent command launch.
                    if agent.choice.is_none() {
                        option {
                            value: UNRESOLVED_VALUE,
                            selected: true,
                            "— choose an agent —"
                        }
                    }
                    // Which option is CHOSEN is stated on the options
                    // themselves, not only through the select's `value`
                    // above, and that is a correctness fix rather than
                    // belt-and-braces. A select's `value` is applied as a DOM
                    // PROPERTY, which a browser silently ignores when no
                    // option matches it yet — and this picker's options
                    // arrive later than its value by construction, since the
                    // catalog is read after the dialog opens. The property
                    // set is then never retried (the framework only re-emits
                    // an attribute whose value CHANGED), so the picker would
                    // sit on "custom command" forever while this component
                    // believed a profile was selected: the invisible
                    // mismatch — a control showing one thing while the submit
                    // sends another — that the host selector's own note calls
                    // the failure worth preventing. An option's `selected` is
                    // applied when the option itself is created, so it cannot
                    // race its own list.
                    option {
                        value: "",
                        // Selected only when the command path is what a
                        // submit would actually use — never merely because
                        // nothing else is, which is what the placeholder
                        // above is for.
                        selected: agent.choice == Some(AgentChoice::Command),
                        "custom command (below)"
                    }
                    for profile in offered.map(|catalog| catalog.profiles.as_slice()).unwrap_or_default() {
                        option {
                            key: "{profile.id}",
                            value: "{profile.id}",
                            selected: chosen_agent == profile.id,
                            // Escaped like every other rendering of
                            // peer-supplied text: an option label is exactly
                            // where a directional override could make one
                            // profile read as another, and what is chosen
                            // here decides what runs.
                            "{display_peer(&profile.name)}"
                        }
                    }
                }
            }
            // SPEC.md's ask-don't-guess fallback, said out loud. It appears
            // only when a profile that WAS available is not anymore — a first
            // create on a host has nothing to explain — and the thing it
            // rules out is the silent substitution: another profile quietly
            // preselected under the label of a remembered preference.
            if let Some(note) = agent.note {
                div { class: "create-session-profile-note", "{note.text()}" }
            }
            // A catalog that could not be READ is a third state, and it must
            // not look like a host with no profiles: the usual cause is a
            // host that is not connected, and the picker offering only the
            // command path with nothing said would leave a user wondering
            // where their profiles went — and then hitting the same refusal
            // from the create itself. The helm's sentence names the phase,
            // so it is printed as written.
            if let CatalogLookup::Failed(error) = &held {
                PeerLine {
                    class: "create-session-profile-error".to_string(),
                    parts: vec![
                        DetailPart::text("this host's profiles could not be read: "),
                        DetailPart::peer(*error),
                    ],
                }
            }
            label {
                "working directory"
                input {
                    r#type: "text",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    // A clone can seed this from a peer-supplied path (see
                    // `CreatePrefill`), shown ESCAPED while untouched
                    // (`peer::display_peer`, applied by `reseed_cloned_field`)
                    // so a directional override or an invisible character in
                    // it cannot make the field say something different from
                    // the bytes a submit would actually send — the same
                    // escaped-display / raw-seed / edited-flag model
                    // `profiles::ProfileDraft` uses (item2-review2.md F5).
                    // `dir: "ltr"` is the SEPARATE per-value isolation every
                    // other rendering of relayed text gets (`crate::peer`),
                    // so a directional override also cannot visually reorder
                    // this field against the labels and buttons around it.
                    dir: "ltr",
                    value: "{cwd}",
                    disabled: busy,
                    oninput: move |evt| {
                        cwd.set(evt.value());
                        // The user is now typing their OWN text, not
                        // reviewing a clone's — a submit from here on sends
                        // exactly what this field shows, not the raw seed
                        // (`profiles::submitted_field`).
                        cwd_edited.set(true);
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            // Present in both modes and INERT in one: a profile already says
            // what to run, the wire refuses a create naming both, and a field
            // that stayed live would invite a user to type a command that is
            // not what launches. `required` follows the mode for the same
            // reason — an empty command is exactly right when a profile
            // supplies it.
            label {
                if by_profile {
                    "agent command (unused: the selected profile supplies it)"
                } else {
                    "agent command"
                }
                input {
                    r#type: "text",
                    required: !by_profile,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    // See the working-directory field's own comment just
                    // above: a clone can seed this from a peer-supplied
                    // command line, and this is the same escaped-display /
                    // raw-seed model plus the same per-value isolation.
                    dir: "ltr",
                    value: "{invocation}",
                    disabled: busy || by_profile,
                    oninput: move |evt| {
                        invocation.set(evt.value());
                        invocation_edited.set(true);
                        // Typing a command IS choosing the command path, and
                        // recording it here is what keeps a late arrival from
                        // taking the choice away: without it, a catalog
                        // landing a moment later could seed a remembered
                        // profile, disable this very field, and leave what was
                        // just typed on screen but unused. The user said what
                        // they want by typing it.
                        chosen_profile.set(Some(AgentChoice::Command));
                        // item2-review2.md F6: this is an explicit agent
                        // choice too, and must not be overwritten by a
                        // cross-host clone handoff still queued behind it.
                        pending_choice.set(None);
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            label {
                "title (optional)"
                input {
                    r#type: "text",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    // See the working-directory field's own comment above:
                    // a clone can seed this from a peer-supplied title, with
                    // the same escaped-display / raw-seed model.
                    dir: "ltr",
                    value: "{title}",
                    disabled: busy,
                    oninput: move |evt| {
                        title.set(evt.value());
                        title_edited.set(true);
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            button {
                r#type: "submit",
                class: "btn btn-primary create-session-submit",
                // `blocked` as well as this form's own flag: a create must
                // not overlap a host mutation (see `ListView`'s operation
                // gate), and a control that is inert for that window says so
                // rather than silently dropping the click.
                //
                // Inert with no agent selected for a different reason: there
                // is nothing to launch, and the handler refuses in words
                // anyway (a `disabled` attribute is one render behind, so it
                // is the visible half of that rule rather than the guard).
                disabled: busy || agent.choice.is_none(),
                "create"
            }
            if let Some(err) = error.read().clone() {
                // The helm's own words, which for a create refused by a
                // host's state quote that host and its identities — so the
                // same escaping and isolation every other peer string gets
                // (see `peer::PeerLine`).
                PeerLine {
                    class: "create-session-error".to_string(),
                    parts: vec![DetailPart::Peer(err)],
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::row::row_specimen;
    use super::super::shared::tests::{open, option};
    use super::*;
    use crate::SourceProfile;

    /// An intent is a command, in a directory, on one INCARNATION of a host
    /// — so a binding must differ whenever any of those does, and the
    /// incarnation is the part an id alone cannot express.
    ///
    /// The failure this pins is the expensive one: a retarget or an adopt
    /// leaves the id untouched, so a key bound to the id alone survives into
    /// a retry aimed at a machine that has never seen it, where it dedups
    /// nothing and launches a second real agent.
    ///
    /// The CREATION MODE joins that list at M6.75, and it is the sharpest
    /// case of the same rule: the same command line run from a profile and
    /// typed by hand are two different intended creates, and the supervisor
    /// folds the mode into its own idempotency fingerprint precisely so a
    /// retry cannot flip between them.
    #[test]
    fn an_intent_binding_changes_with_the_host_incarnation_and_with_the_fields() {
        let hosts = vec![option(1, "this machine", true)];
        let command = || LaunchIntent::Command("agent".to_string());
        let base = IntentBinding::of(
            Some(1),
            &hosts,
            "/tmp".to_string(),
            command(),
            "title".to_string(),
        )
        .expect("the selected host is in the list");

        // Same id, different incarnation: a retarget or an adopt.
        let moved = vec![HostOption {
            incarnation: "incarnation-after-the-retarget".to_string(),
            ..hosts[0].clone()
        }];
        let after = IntentBinding::of(
            Some(1),
            &moved,
            "/tmp".to_string(),
            command(),
            "title".to_string(),
        )
        .expect("still selectable");
        assert_ne!(
            base, after,
            "the row is the same row; the machine behind it is not"
        );

        // Every form field is part of the intent too — this is what the
        // post-mint re-read compares against.
        for edited in [
            IntentBinding {
                cwd: "/other".to_string(),
                ..base.clone()
            },
            IntentBinding {
                agent: LaunchIntent::Command("other-agent".to_string()),
                ..base.clone()
            },
            // A profile-backed create of the "same" thing is a DIFFERENT
            // intent: what runs is the profile's definition, which nothing on
            // this side can compare against a typed command.
            IntentBinding {
                agent: LaunchIntent::Profile("p-1".to_string()),
                ..base.clone()
            },
            IntentBinding {
                title: "other title".to_string(),
                ..base.clone()
            },
        ] {
            assert_ne!(base, edited);
        }

        // And an unchanged submit is the SAME intent, which is the whole
        // point of the key surviving a failure.
        assert_eq!(
            base,
            IntentBinding::of(
                Some(1),
                &hosts,
                "/tmp".to_string(),
                command(),
                "title".to_string()
            )
            .expect("still selectable")
        );
    }

    /// A submit with no host selected has no binding at all — the one case
    /// the form refuses locally instead of sending, because a hostless body
    /// would be silently defaulted by the helm to a machine the user was
    /// never shown.
    /// Submission refuses when the catalog target and the selected row
    /// disagree about the INSTALL, not merely about the row id.
    ///
    /// This is the rule-level pin of the one-render-lag regression: after a
    /// retarget or an adoption the hosts snapshot describes the successor
    /// while the catalog surface still holds the predecessor's catalog under
    /// the same row id. The component-level version (staging the submit
    /// handler mid-lag) is not stageable in this harness — the handler lives
    /// inside the dioxus closure — so the comparison is extracted and pinned
    /// here instead.
    #[test]
    fn submission_requires_the_catalog_to_match_the_selected_install() {
        let hosts = vec![option(1, "this machine", true)];
        let current = HostTarget::new(1, "incarnation-1".to_string());
        assert!(catalog_matches_selection(Some(1), &hosts, Some(&current)));
        assert!(
            !catalog_matches_selection(
                Some(1),
                &hosts,
                Some(&HostTarget::new(
                    1,
                    "incarnation-before-retarget".to_string()
                )),
            ),
            "the same row id under a moved install fingerprint is the lag window, not a match"
        );
        assert!(
            !catalog_matches_selection(Some(2), &hosts, Some(&current)),
            "a selected row absent from the snapshot cannot vouch for any catalog"
        );
        assert!(
            catalog_matches_selection(None, &hosts, None),
            "no selection and no target fall through to the hostless refusal downstream"
        );
        assert!(!catalog_matches_selection(Some(1), &hosts, None));
        assert!(!catalog_matches_selection(None, &hosts, Some(&current)));
    }

    /// A never-connected host yields NO connection claim, and a connected
    /// one yields exactly its token.
    ///
    /// `Host::incarnation == 0` is the never-connected sentinel, and sending
    /// it as `expected_incarnation: 0` would turn "I observed nothing" into
    /// a precondition the host's very first connection then fails — a create
    /// racing that first connect would be refused as stale with nothing
    /// actually wrong.
    #[test]
    fn a_never_connected_host_makes_no_connection_claim() {
        let mut connected = option(1, "connected", true);
        connected.connection = 7;
        let mut fresh = option(2, "never connected", false);
        fresh.connection = 0;
        let hosts = vec![connected, fresh];
        assert_eq!(connection_claim(&hosts, 1), Some(7));
        assert_eq!(
            connection_claim(&hosts, 2),
            None,
            "the sentinel is the absence of a claim, not a claim of zero"
        );
        assert_eq!(connection_claim(&hosts, 3), None);
    }

    #[test]
    fn no_selected_host_yields_no_binding() {
        let hosts = vec![option(1, "this machine", true)];
        let nothing = || LaunchIntent::Command(String::new());
        assert!(IntentBinding::of(None, &hosts, String::new(), nothing(), String::new()).is_none());
        assert!(
            IntentBinding::of(Some(99), &hosts, String::new(), nothing(), String::new()).is_none(),
            "a selection the option list no longer contains is not a target either"
        );
    }

    /// The effective create target is the user's choice while it exists and
    /// the local row otherwise — one answer, used by both the dialog that
    /// renders it and the reader that follows it.
    ///
    /// The middle case is why this is a function rather than two expressions:
    /// a chosen host leaving the registry moves the target, and if the picker
    /// and the catalog reader disagreed about when, the dialog would offer
    /// one host's profiles for a create aimed at another — an id that names
    /// nothing over there, or worse, a starter profile that resolves.
    #[test]
    fn the_effective_create_target_follows_a_choice_until_it_is_gone() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
        ];
        assert_eq!(effective_create_host(&hosts, Some(2), None), Some(2));
        assert_eq!(
            effective_create_host(&hosts, None, None),
            Some(1),
            "with no choice made, the target is SPEC.md's default"
        );
        assert_eq!(
            effective_create_host(&hosts, Some(99), None),
            Some(1),
            "a choice the registry no longer holds falls back to the default rather than staying \
             on a host nothing can reach"
        );
        assert_eq!(effective_create_host(&[], Some(2), None), None);
    }

    /// Full three-candidate precedence: explicit choice over the open
    /// session's host over the local row — and a VANISHED choice falls back
    /// to the open session's host, not past it to local.
    ///
    /// The earlier tests each exercise one clause with the others absent,
    /// which a reversed precedence or a fallback that skips the middle
    /// clause would pass; this is the arrangement where every wrong order
    /// gives a different answer. The middle assertion is the subtle one:
    /// SPEC.md's first clause is "the host of the currently open session",
    /// so a dead explicit choice lands there, and skipping to the local
    /// row would silently move the create off the machine whose session
    /// the user is looking at.
    #[test]
    fn precedence_holds_with_all_three_candidates_present() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
            option(3, "user@other", false),
        ];
        assert_eq!(
            effective_create_host(&hosts, Some(3), Some(&open(2))),
            Some(3),
            "a valid explicit choice beats the open session's host"
        );
        assert_eq!(
            effective_create_host(&hosts, Some(99), Some(&open(2))),
            Some(2),
            "a vanished choice falls back to the open session's host, not to local"
        );
        assert_eq!(
            effective_create_host(&hosts, Some(99), Some(&open(98))),
            Some(1),
            "and only when BOTH are gone does the local row answer"
        );
    }

    /// A profile snapshot builds a source in whatever existence the test
    /// wants to pretend the catalog currently reports.
    fn source(existence: ProfileExistence) -> SourceProfile {
        SourceProfile {
            id: "profile-1".to_string(),
            name: "shipped profile".to_string(),
            existence,
        }
    }

    /// A clone trusts the row's own profile choice ONLY while the catalog
    /// still recognizes it under its snapshotted id AND name — every other
    /// existence answer, and no profile at all, falls back to the raw
    /// invocation the row actually ran.
    ///
    /// This is the one decision `PrefillAgent` exists to encode, and it is
    /// STRICTER than `profiles::resolve_agent`'s own rule for an ordinary
    /// create (see `PrefillAgent`'s own doc for why a `Renamed` id is not
    /// trusted here even though `resolve_agent` would accept it): a clone
    /// can be arbitrarily old, so an id merely still existing is not enough
    /// evidence that recreating it is what the current catalog would still
    /// offer under that name.
    #[test]
    fn prefill_from_clones_the_profile_only_when_it_is_present() {
        let present = Session {
            source_profile: Some(source(ProfileExistence::Present)),
            ..row_specimen("s1")
        };
        assert_eq!(
            prefill_from(&present, 1).agent,
            PrefillAgent::Profile {
                id: "profile-1".to_string()
            }
        );

        for existence in [
            ProfileExistence::Renamed,
            ProfileExistence::Deleted,
            ProfileExistence::Unrecognized,
        ] {
            let session = Session {
                source_profile: Some(source(existence)),
                ..row_specimen("s1")
            };
            assert_eq!(
                prefill_from(&session, 1).agent,
                PrefillAgent::Command,
                "{existence:?} does not name a definition this clone may trust"
            );
        }

        let no_profile = Session {
            source_profile: None,
            ..row_specimen("s1")
        };
        assert_eq!(
            prefill_from(&no_profile, 1).agent,
            PrefillAgent::Command,
            "a raw-invocation session clones its invocation, not a profile it never had"
        );
    }

    /// The raw invocation is carried on every prefill, REGARDLESS of which
    /// mode `agent` trusts — including a profile-backed clone, which has no
    /// use for it until the user switches the mounted form to "custom
    /// command" (see `CreatePrefill::invocation`'s own doc for why leaving
    /// it unset there would let a stale, unrelated command surface then).
    #[test]
    fn prefill_from_carries_the_raw_invocation_even_for_a_profile_backed_clone() {
        let session = Session {
            invocation: "claude --resume abc".to_string(),
            source_profile: Some(source(ProfileExistence::Present)),
            ..row_specimen("s1")
        };
        assert_eq!(prefill_from(&session, 1).invocation, "claude --resume abc");
    }

    /// Everything else on a prefill travels off the row unmodified — no
    /// suffix on the title, no rewriting of the directory, the row's own
    /// host and install identity together — and the generation is exactly
    /// what the caller passed in (`ListView` is the one that decides what
    /// counts as a new clone).
    #[test]
    fn prefill_from_carries_title_cwd_host_and_identity_verbatim() {
        let session = Session {
            cwd: "/work/api".to_string(),
            title: "my session".to_string(),
            host: Some(7),
            host_identity: Some(Some("install-7".to_string())),
            ..row_specimen("s1")
        };
        let prefill = prefill_from(&session, 3);
        assert_eq!(prefill.generation, 3);
        assert_eq!(prefill.host, Some(7));
        assert_eq!(prefill.host_identity, Some(Some("install-7".to_string())));
        assert_eq!(prefill.cwd, "/work/api");
        assert_eq!(prefill.title, "my session");
    }

    // -------------------------------------------------------------
    // `resolve_clone_host` (item2-review2.md F1/F3/F4): the pure decision
    // behind a clone's own host+agent binding, checkable without mounting
    // a component or an effect.
    // -------------------------------------------------------------

    /// F1: a clone opened before the host registry has answered must keep
    /// retrying rather than giving up — `prefill_applied` (the text-field
    /// latch in the reseed effect) means the ordinary reseed branch never
    /// revisits this generation, so this function is the only thing left
    /// that can still apply its host once the registry does load.
    #[test]
    fn resolve_clone_host_keeps_waiting_until_the_registry_loads_then_binds_a_matching_row() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));

        // Before the registry has answered: hold, stay `Waiting`.
        let (state, action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &identity,
            &PrefillAgent::Command,
            false, // hosts_loaded
            &hosts,
            false,
        );
        assert_eq!(state, CloneHostState::Waiting);
        assert_eq!(action, CloneHostAction::Hold);

        // Once it has, and the row's install still matches: bind.
        let (state, action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &identity,
            &PrefillAgent::Profile {
                id: "p-1".to_string(),
            },
            true,
            &hosts,
            false,
        );
        assert_eq!(state, CloneHostState::Bound);
        assert_eq!(
            action,
            CloneHostAction::Bind {
                target: HostTarget::new(1, "incarnation-1".to_string()),
                choice: AgentChoice::Profile("p-1".to_string()),
            }
        );
    }

    /// F1's other half: once the registry HAS answered and the row's
    /// install cannot be confirmed, the clone gives up permanently for this
    /// generation rather than sitting in `Waiting` forever.
    #[test]
    fn resolve_clone_host_gives_up_once_the_registry_answers_without_a_match() {
        let hosts = vec![option(1, "remote", false)];
        let stale_identity = Some(Some("install-superseded".to_string()));
        let (state, action) = resolve_clone_host(
            CloneHostState::Waiting,
            Some(1),
            &stale_identity,
            &PrefillAgent::Command,
            true,
            &hosts,
            false,
        );
        assert_eq!(state, CloneHostState::Unconfirmable);
        assert_eq!(action, CloneHostAction::Hold);
    }

    /// F4: a hostless clone never resolves an agent, however this function
    /// is reached — the caller starts such a clone straight in
    /// `Unconfirmable` (see the reseed effect's generation-transition
    /// branch), and this pins the same guarantee at the function level too:
    /// `Waiting` with no host to check never produces a `Bind`.
    #[test]
    fn resolve_clone_host_never_binds_a_hostless_clone() {
        let hosts = vec![option(1, "remote", false)];
        let (state, action) = resolve_clone_host(
            CloneHostState::Waiting,
            None,
            &None,
            &PrefillAgent::Profile {
                id: "p-1".to_string(),
            },
            true,
            &hosts,
            false,
        );
        assert_eq!(state, CloneHostState::Unconfirmable);
        assert_eq!(action, CloneHostAction::Hold);
    }

    /// F3: a `Bound` clone is re-checked on every pass, and withdraws the
    /// instant the row's install stops matching — a retarget or an adopt
    /// landing while the form stays open must not leave the selector
    /// silently naming a machine the clone was never actually taken from.
    #[test]
    fn resolve_clone_host_withdraws_a_bound_clone_once_its_installation_changes() {
        let identity = Some(Some("install-1".to_string()));
        let retargeted = vec![HostOption {
            identity: Some("install-after-the-retarget".to_string()),
            ..option(1, "remote", false)
        }];
        let (state, action) = resolve_clone_host(
            CloneHostState::Bound,
            Some(1),
            &identity,
            &PrefillAgent::Command,
            true,
            &retargeted,
            true, // chosen_host is still this generation's own pick
        );
        assert_eq!(state, CloneHostState::Unconfirmable);
        assert_eq!(action, CloneHostAction::Withdraw);
    }

    /// The negative case beside it: while the installation still matches, a
    /// `Bound` clone holds — nothing is touched just because the effect
    /// happened to fire again (a feed notice, an unrelated host's refresh).
    #[test]
    fn resolve_clone_host_stays_bound_while_its_installation_still_matches() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));
        let (state, action) = resolve_clone_host(
            CloneHostState::Bound,
            Some(1),
            &identity,
            &PrefillAgent::Command,
            true,
            &hosts,
            true,
        );
        assert_eq!(state, CloneHostState::Bound);
        assert_eq!(action, CloneHostAction::Hold);
    }

    /// A `Bound` clone whose selector has moved away from its own pick (the
    /// host `<select>`'s own `onchange` already transitions directly to
    /// `UserTookOver`; this is the same outcome reached from this
    /// function's own side, covering any other path that might move
    /// `chosen_host`) yields without forcing anything — no `Withdraw`,
    /// since there is nothing left of the clone's OWN pick in play to undo.
    #[test]
    fn resolve_clone_host_yields_once_the_selector_moves_away_from_its_own_pick() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));
        let (state, action) = resolve_clone_host(
            CloneHostState::Bound,
            Some(1),
            &identity,
            &PrefillAgent::Command,
            true,
            &hosts,
            false, // chosen_host no longer names this generation's host
        );
        assert_eq!(state, CloneHostState::UserTookOver);
        assert_eq!(action, CloneHostAction::Hold);
    }

    /// Terminal states stay terminal: once a generation's binding has
    /// failed or been taken over, nothing about a LATER pass — even one
    /// where the row's install would once again match — reopens it. Only a
    /// fresh clone (a new generation, a fresh `Waiting`) tries again.
    #[test]
    fn resolve_clone_host_never_reopens_a_terminal_state() {
        let hosts = vec![option(1, "remote", false)];
        let identity = Some(Some("install-1".to_string()));
        for terminal in [CloneHostState::Unconfirmable, CloneHostState::UserTookOver] {
            let (state, action) = resolve_clone_host(
                terminal,
                Some(1),
                &identity,
                &PrefillAgent::Command,
                true,
                &hosts,
                true,
            );
            assert_eq!(state, terminal);
            assert_eq!(action, CloneHostAction::Hold);
        }
    }

    /// item2-review2.md F5's untouched-vs-edited submission rule, exercised
    /// through THIS file's own reuse of it (a clone's directory, invocation
    /// and title) rather than only through the profile editor's copy — the
    /// two must agree because they share one function
    /// (`profiles::submitted_field`), but only this test pins that this
    /// file's own field handling is wired to it correctly, with a value a
    /// clone could plausibly carry: a right-to-left override inside an
    /// otherwise ordinary invocation.
    #[test]
    fn a_cloned_fields_untouched_submission_sends_the_original_bytes_not_the_escaped_display() {
        let raw = "claude --resume \u{202E}reversed-arg";
        let display = display_peer(raw);
        assert_ne!(
            display, raw,
            "the escaped display must differ from the raw bytes for this test to mean anything"
        );

        // What `reseed_cloned_field` seeds the field with, and what an
        // UNTOUCHED submit sends back — the original bytes, not the
        // escaped spelling the input box is showing.
        assert_eq!(
            submitted_field(&display, false, Some(raw)),
            raw,
            "untouched: the clone's own bytes travel, exactly as the row ran them"
        );

        // The user retypes EXACTLY the escaped spelling (cleaning up a
        // hostile command is precisely this) — equality against the seed
        // cannot tell that apart from "never touched", so only the edited
        // flag can.
        assert_eq!(
            submitted_field(&display, true, Some(raw)),
            display,
            "edited: the user's own literal text travels, even when it happens to equal the \
             escaped rendering of what was there"
        );
    }
}
