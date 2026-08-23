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
    seeded_choice,
};
use crate::reader::Trigger;
use crate::{ApiBase, HostId, Session};

use super::shared::{HostOption, OpenHost, effective_create_host, enrich_created_session};

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
    //   being retargeted, adopted or reconnected — discards the choice and the
    //   intent key. A profile id means nothing on another supervisor, and
    //   because every fresh supervisor seeds the same starters, carrying one
    //   across does not fail loudly: it resolves, to a profile nobody picked.
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
    use_effect(move || {
        let target = catalog.watch_target();
        let read = catalog.catalog.read();
        let previous = bound_target.peek().clone();
        if previous != target {
            bound_target.set(target.clone());
            // The catalog must be re-seeded for ANY change, including a mere
            // reconnection: it was read on a connection that is gone, and the
            // helm now refuses reads that name it.
            seeded_for.set(None);
            // The CHOICE and the KEY, however, rotate only when the INSTALL
            // changes. A reconnection to the same machine is precisely when a
            // reply gets lost, and that is the case the idempotency key exists
            // for — rotating it there would turn the user's retry into a
            // second intended create and hand them two sessions for one press.
            // A retarget or an adoption is the opposite and must rotate: the
            // id now means another machine, where the key dedups against
            // nothing and the profile id resolves to something else.
            let same_install = match (&previous, &target) {
                (Some(before), Some(now)) => before.same_install(now),
                // Opening the dialog (None -> Some) and closing it are not
                // reconnections; a fresh dialog starts fresh.
                _ => false,
            };
            if !same_install {
                chosen_profile.set(None);
                intent_key.set(None);
                // A refusal belongs to the host it came from. Left standing,
                // host A's "directory does not exist" would sit under a form
                // now aimed at host B, where it may not even be true.
                error.set(None);
            }
        }
        // Consumption is only meaningful once there IS a target: on the first
        // render both are `None`, and reading that equality as "already
        // seeded" would make an unread catalog indistinguishable from a
        // deleted remembered profile — the dialog would open claiming
        // something was gone before it had asked anything.
        if target.is_some() && *seeded_for.peek() == target {
            return;
        }
        // Only a catalog that answers the CURRENT question may seed a choice
        // — `lookup` is what refuses one belonging to a previous activation
        // or another install.
        let CatalogLookup::Known { catalog: held, .. } = catalog.lookup(&read) else {
            return;
        };
        seeded_for.set(target);
        // The user may have answered while the read was in flight (picking a
        // profile, or typing a command); an answer outranks a default.
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
    });

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
                    AgentChoice::Command => LaunchIntent::Command(invocation.peek().clone()),
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
                // be the catalog OF that host. When they disagree the target
                // effect has not caught up with the selector yet — a window of
                // one render — and a profile id resolved against the old
                // host's catalog would be sent to the new one, where it means
                // something else or nothing.
                if selected_now != target_now.as_ref().map(|target| target.host) {
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
                let Some(binding) =
                    IntentBinding::of(selected_now, &hosts, cwd(), launch, title())
                else {
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
                            cwd: cwd.peek().clone(),
                            title: title.peek().clone(),
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
                    // The connection this create was prepared against. Read
                    // from the surface's target rather than from the hosts
                    // snapshot, because the target is what the picker's
                    // catalog was read on — so the profile id in the body and
                    // the connection in the precondition describe one moment.
                    let expectation = target_now
                        .as_ref()
                        .map(|target| target.expectation())
                        .unwrap_or_default();
                    match create_session(
                            &base,
                            &bound.cwd,
                            agent,
                            &bound.title,
                            &key,
                            Some(bound.host),
                            expectation,
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
                                    // something else. The binding is stale in
                                    // every part, so the key goes with it (a
                                    // retry must be a NEW intent, not a replay
                                    // aimed at a machine that never saw the
                                    // first) and the catalog is re-read, which
                                    // is what supersedes this message.
                                    intent_key.set(None);
                                    chosen_profile.set(None);
                                    catalog.request(Trigger::Explicit);
                                }
                                // The key otherwise deliberately SURVIVES a
                                // failure: this is exactly the case it exists
                                // for. A failure whose cause was an ambiguous
                                // transport error may have created a session
                                // the user cannot see, and resubmitting
                                // unchanged must reach that same session
                                // rather than launch a second agent. A user
                                // who instead fixes the form gets a new key,
                                // because the binding no longer matches.
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
                    value: "{cwd}",
                    disabled: busy,
                    oninput: move |evt| {
                        cwd.set(evt.value());
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
                    value: "{invocation}",
                    disabled: busy || by_profile,
                    oninput: move |evt| {
                        invocation.set(evt.value());
                        // Typing a command IS choosing the command path, and
                        // recording it here is what keeps a late arrival from
                        // taking the choice away: without it, a catalog
                        // landing a moment later could seed a remembered
                        // profile, disable this very field, and leave what was
                        // just typed on screen but unused. The user said what
                        // they want by typing it.
                        chosen_profile.set(Some(AgentChoice::Command));
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
                    value: "{title}",
                    disabled: busy,
                    oninput: move |evt| {
                        title.set(evt.value());
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
    use super::super::shared::tests::{open, option};
    use super::*;

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
}
