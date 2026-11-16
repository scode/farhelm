//! Setting up an SSH host: the confirmation that shows the helm's plan, and the submission that starts the run, for a
//! new host from Add host and for a registered host whose setup failed, from its row's menu ([`rerun`]).
//!
//! SPEC.md, "Topology" (remote provisioning: "the panel shows a concrete installation plan ... and proceeds only on
//! confirmation"). Ported from the old app's `provisioning.rs` (`SetupPlanConfirmation`, `PlanStepList`) and the add
//! form's confirm and submit path in `hosts.rs` (`AddHostForm`'s `confirm`, `on_add_submit`), with the look of the
//! `.provisioning-plan*` rules in `app.css`, all at reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **Three ways on from a plan.** A probe's plan goes on in one of three ways ([`crate::hosts::add`]'s `settled`
//!   picks). With "Set up new hosts without asking" on ([`asks_first`]), it is submitted at once and Add host closes,
//!   as the old app's remembered answer did. Otherwise, if the user has not moved on since Add, Add host closes and the
//!   confirmation opens in its place (the old dialog swapped to it). If they have moved on, nothing opens (SPEC.md,
//!   "Keyboard focus": a result after the user moved on opens no dialog), and Add host, still open, says that Farhelm
//!   is not running there and that Add shows what setting it up changes ([`MOVED_ON`]).
//! - **The confirmation** is a question of the prompt ([`confirm`]), frozen on the [`Offer`] the probe returned:
//!   "Set up host · <destination>", the helm's numbered plan as its body (each step's name, path chips and explanation
//!   as host text, built once from the frozen steps), Copy plan, which copies the helm's own plan text, and the footer
//!   note naming the gear as the way to undo the permanent answer. Three answers in the old order: Cancel, "Set up,
//!   don't ask again" and Set up, where focus starts. "Set up, don't ask again" also turns the preference on, under the
//!   preference row's key, beside its submission.
//! - **The submission** ([`submit`]) is `POST /api/hosts/provision` with the plan's id, as the operation store's work
//!   under the same host-add key the probe used. The helm answers 202 once it has registered the host and started the
//!   run, and the run's progress and outcome arrive in the pushed host progress, which the Hosts panel shows; the
//!   question has closed by then, and the submission goes on without any view.
//! - **A refusal** (a plan already used, a host added meanwhile) reopens Add host with the destination and alias as
//!   sent and the helm's reason, but not the consumed plan, if the user has not moved on since the action that sent it
//!   ([`Offer::mark`]: the answer they pressed, or Add itself when the plan went out without asking). If they have,
//!   nothing opens, and the refusal shows at the top of the Hosts panel instead ([`panel_lines`]): no row exists for a
//!   host the helm did not add. It shows there until Add host opens again or the next setup starts.
//! - **A refused permanent answer.** The setup goes ahead whether or not its preference save succeeds, and the dialog
//!   that asked has closed, so a refusal of that save shows on the same panel line, "Set up new hosts without asking:"
//!   and the helm's words, with the same lifetime; it also goes once the preference is on, or after a reset. A save the
//!   operation store refuses because another preference save is in flight shows there too, in the words Settings uses
//!   for such a press ([`Refused::WORDS`]).
//! - **The rerun of a failed setup** ([`rerun`], the host menu's Re-run) probes the host's registered destination again,
//!   with the install paths it recorded, under the host's own key, as the old app did ("failed ADD rerun probes the
//!   registered destination"). A supervisor that answers now is registered as it is and the row follows the pushed
//!   state; a plan opens the same confirmation, titled with the host's name and drawn with its mark, or goes out
//!   without asking under the same preference. The probe's refusal, why setup is manual, and a plan that arrived after
//!   the user moved on ([`RERUN_MOVED_ON`]) show under the row, the operation store's record under the host's key, as
//!   does a refused submission: the host already has a row, so nothing reopens. The refusal and the manual setup, and a
//!   refused submission, also warn the host's rail avatar while Hosts is closed ([`super::status::failed`]).
//! - **During a launch.** A submission and a rerun refuse to start while a session launch is in flight
//!   ([`super::launch_in_flight`]), as Add does. A new host's Add host reopens saying so, as for a helm refusal, while
//!   the user has not moved on, and otherwise the Hosts panel's line says "Add host: not started, …", as it prefixes a
//!   helm refusal; a registered host's refusal is on that line naming the host, "Set up <host>: not started, …" or
//!   "Re-run <host>: not started, …", as Remove's line does for a Remove not started. "Set up, don't ask again" then
//!   leaves the preference off: it is saved only when no launch holds the setup back. The old app held all of this back
//!   behind its one page-wide token for launches and host registry changes; here each side refuses its own action
//!   (decided by the migration's orchestrator on 2026-11-14).

#[cfg(test)]
pub(crate) mod tests;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::provisioning::PlanSteps;
use farhelm_proto::ui_channel::{HostView, PREFERENCES_KEY, Preferences};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, EntityId, FocusHandle, FontWeight,
    Global, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::json;

use super::add::{self, Reply};
use super::identity::{display_name, host_mark};
use crate::assets;
use crate::components::{HoverHelp, Tier, copy_button, with_hover_help};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::operations::{Command, NotStarted, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::settings::Refused;
use crate::theme::{self, metrics};

/// The old setup confirmation's width, the Add host dialog's.
const WIDTH: f32 = 560.;

/// What the Add host dialog says when a plan arrives after the user moved on, so the confirmation does not open.
pub(crate) const MOVED_ON: &str =
    "Farhelm is not running on this host. Press Add again to see what setting it up changes.";

/// What a host's row says when the plan its rerun asked for arrives after the user moved on: [`MOVED_ON`] for the menu.
pub(crate) const RERUN_MOVED_ON: &str =
    "Farhelm is not running on this host. Choose Re-run again to see what setting it up changes.";

/// The footer note of the host confirmations with a permanent answer (this one and the removal confirmation), the old
/// dialogs' words. SPEC.md ("Session list", on Settings' Hosts section): "each host dialog's permanent answer points to
/// the gear as the place to undo it".
pub(super) const UNDO_NOTE: &str =
    "Undo \"don't ask again\" with the gear at the foot of the navigation rail";

/// `.provisioning-plan`'s line height: 1.45 at its 12 pixels.
const PLAN_LINE: f32 = 17.4;

/// `.provisioning-plan-number`'s circle, and the rail column it sits in (`grid-template-columns: 24px ...`).
const NUMBER: f32 = 22.;
const RAIL: f32 = 24.;

/// The text size of the numbers, path chips and explanations.
const SMALL_TEXT: f32 = 11.;

/// A plan the helm offered to set up a host, as the confirmation shows it and the submission acts on it: the frozen
/// value of the question.
#[derive(Clone)]
pub(crate) struct Offer {
    /// The key the probe ran under, which the submission continues under: a new host's host-add key, or a registered
    /// host's own key for a rerun.
    pub(crate) key: OpKey,
    /// The helm's one-use id for the plan it kept; the submission names the plan by it and nothing else.
    pub(crate) probe_id: String,
    /// The helm's plan as text, which Copy plan copies exactly.
    pub(crate) confirmation: String,
    /// The plan's steps, which the body shows.
    pub(crate) steps: PlanSteps,
    /// The host the plan is for, which decides the title, the tile and what a refusal does.
    pub(crate) target: Target,
    /// The mark of the user's action behind the offer: Add's (or Re-run's) while the confirmation is to open (it opens
    /// only through it), then the answer's, taken when Set up or the permanent answer is pressed; Add's still when the
    /// plan goes out without asking. A refusal reopens Add host only through it, so not once the user has moved on
    /// since. The operation store's own mark is no substitute: on the path without asking, it is taken when the probe's
    /// reply arrives, which says nothing about what the user did while the probe was out.
    pub(crate) mark: Mark,
}

/// Which host a plan sets up.
#[derive(Clone)]
pub(crate) enum Target {
    /// A host Add host is adding.
    New {
        /// The destination as the probe sent it: the title names it, and a refusal puts it back in Add host.
        destination: String,
        /// The alias as typed in Add host (blank for none); a refusal puts it back there. The probe already sent it as
        /// the new entry's alias, and the helm's kept plan carries it, so the submission does not send it again.
        alias: String,
        /// The control that opened Add host, which gets focus back.
        opener: FocusHandle,
        /// The add began as an agent's request: a refusal reopens Add host still marked so ([`add::Seed::from_agent`]),
        /// so its next plan still asks.
        from_agent: bool,
    },
    /// A registered host whose failed setup the user runs again from its row: the title names it as its row does, and
    /// the tile is its own mark. A refusal shows under its row ([`rerun`]) and, while Hosts is closed, warns its rail
    /// avatar ([`super::status::failed`]), which is what `host`, its registry id, is for.
    Registered {
        host: i64,
        name: HostText,
        icon: HostIcon,
        color: HostColor,
    },
}

/// Whether a probe that needs setup asks first: the shared "Set up new hosts without asking" preference is off or the
/// helm stored no choice.
pub(crate) fn asks_first(cx: &App) -> bool {
    let skips = session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| preferences.skip_host_setup_confirmation)
            == Some(true)
    });
    !skips
}

/// Ask the setup confirmation for `offer` over everything else in `window`, unless the user moved on since its mark.
pub(crate) fn confirm(offer: Offer, window: &mut Window, cx: &mut App) {
    let mark = offer.mark;
    let (name, (icon, color)) = match &offer.target {
        Target::New { destination, .. } => (
            HostText::from_host(destination.as_str()),
            (HostIcon::Cloud, HostColor::Default),
        ),
        Target::Registered {
            name, icon, color, ..
        } => (name.clone(), (*icon, *color)),
    };
    let title = HostSentence::new("setup-title")
        .text("Set up host · ")
        .host(&name);
    let set_up = Answer {
        label: "Set up".into(),
        help: HoverHelp::tooltip("Set up: set up this host as the plan above describes"),
        tier: Tier::Primary,
    };
    let question = Question::new(title, set_up, offer)
        .summary("Farhelm is not running there yet")
        .cancel_answer(Answer {
            label: "Cancel".into(),
            help: HoverHelp::tooltip("Cancel: change nothing on this host"),
            tier: Tier::Neutral,
        })
        .alternative(
            Answer {
                label: "Set up, don't ask again".into(),
                help: HoverHelp::tooltip(
                    "Set up this host, and set up new hosts from now on without asking",
                ),
                tier: Tier::PrimaryOutline,
            },
            |offer, window, cx| {
                let offer = Offer {
                    mark: Mark::take(cx),
                    ..offer
                };
                // A setup refused before it is sent (a launch in flight) leaves the preference off, so the question
                // is asked again, as the old app, which held both back, did. Checked first, so that the preference
                // save, when it goes, goes ahead of the setup as it always has.
                if !super::launch_in_flight(cx) {
                    stop_asking(cx);
                }
                submit(offer, window.window_handle(), cx);
            },
        )
        .body(|offer: &Offer, _, cx| {
            let body = PlanBody::new(&offer.confirmation, &offer.steps);
            cx.new(|_| body)
        })
        .tile(move |_, cx| host_mark(icon, color, px(24.), theme::palette(cx)))
        .note(UNDO_NOTE)
        .width(px(WIDTH))
        // SPEC.md, "Opening and closing": the dialogs that share the host and Settings installer keep focus on the
        // control in use when their empty space is clicked.
        .keeps_focus_on_empty_click();
    prompt::ask(
        question,
        None,
        &mark,
        |offer, window, cx| {
            let offer = Offer {
                mark: Mark::take(cx),
                ..offer
            };
            submit(offer, window.window_handle(), cx);
        },
        window,
        cx,
    );
}

/// `POST /api/hosts/provision` naming the plan by `probe_id`: the helm consumes the plan it kept under that id, once,
/// and refuses any other.
pub(crate) fn provision_command(probe_id: &str) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Setup,
        Method::Post,
        ApiPath::api(&["hosts", "provision"])?,
        Some(json!({ "probe_id": probe_id })),
        "POST /api/hosts/provision",
    ))
}

/// Submit `offer` under its key, as the operation store's work, so it goes on whatever view is open. An accepted
/// submission needs nothing more here: the run shows in the pushed host progress. A new host's refusal reopens Add host
/// through [`Offer::mark`], or shows on the Hosts panel's line if the user moved on since; a registered host's shows
/// under its row, where the operation store keeps it (the module docs). Sends nothing while a session launch is in
/// flight ([`refuse_during_launch`]).
pub(crate) fn submit(offer: Offer, window: AnyWindowHandle, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    if super::launch_in_flight(cx) {
        refuse_during_launch(offer, window, cx);
        return;
    }
    let command = match provision_command(&offer.probe_id) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host setup not built");
            return;
        }
    };
    let key = offer.key.clone();
    let new = matches!(offer.target, Target::New { .. });
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            // A registered host's refusal is the store's record under the host's key, which its row shows; it also
            // warns the host's rail avatar while Hosts is closed, as any failed setup does.
            // A new host's falls back to the panel's line, which reads the same record under the add's key, which
            // nothing else starts work on.
            if let Outcome::Refused(refusal) = &settled.outcome
                && !reopen_new(&offer, add::error_line().host(&refusal.words), window, cx)
            {
                match &offer.target {
                    Target::New { .. } => show(cx, |lines| lines.setup = Some(offer.key.clone())),
                    Target::Registered { host, .. } => {
                        super::status::failed(*host, super::status::Action::Setup, cx);
                    }
                }
            }
        })
    });
    match started {
        // A refusal still shown from an earlier add belongs to that add; a new one replaces it, and a rerun leaves it.
        // A setup that was not started is over once one is.
        Ok(()) => {
            show(cx, |lines| {
                lines.not_started = None;
                if new {
                    lines.setup = None;
                }
            });
        }
        Err(not_started) => tracing::debug!(?not_started, "host setup not sent"),
    }
}

/// A new host's refused submission, `problem` its line in Add host: reopen Add host with the destination and alias as
/// sent and `problem` in its error line, if the user has not moved on since [`Offer::mark`]. Whether it reopened; never
/// for a registered host, whose refusals show elsewhere, nor once the user moved on or the window went inert, when the
/// caller shows the refusal on the Hosts panel's line instead.
fn reopen_new(offer: &Offer, problem: HostSentence, window: AnyWindowHandle, cx: &mut App) -> bool {
    let Target::New {
        destination,
        alias,
        opener,
        from_agent,
    } = &offer.target
    else {
        return false;
    };
    !focus::has_moved_on(&offer.mark, cx)
        && window
            .update(cx, |_, window, cx| {
                let seed = add::Seed {
                    destination: destination.clone(),
                    alias: alias.clone(),
                    problem,
                    from_agent: *from_agent,
                };
                add::reopen(opener, &offer.mark, Some(seed), window, cx).is_some()
            })
            .unwrap_or(false)
}

/// Refuse `offer`'s submission because a session launch is in flight ([`super::launch_in_flight`]), sending nothing:
/// the plan stays unused, and the user can probe again once the launch is over. A new host's refusal reads "Add host:
/// not started, …", the words Add itself shows for the same refusal and the prefix the panel gives a helm refusal of a
/// new host's setup: in Add host reopened, or on the Hosts panel's line if the user moved on. A registered host's
/// refusal is on the panel's line, "Set up <host>: not started, …", naming the host as Remove's line does
/// ([`not_started`]).
///
/// Add host reopens on a later turn of the app, not now: a submission from the confirmation's answer runs while the
/// window is being updated, where updating it again would fail. Add host's own close of the dialog, on the path
/// without asking, is queued before this, so the reopened dialog is not closed with it.
fn refuse_during_launch(offer: Offer, window: AnyWindowHandle, cx: &mut App) {
    let line = match &offer.target {
        Target::New { .. } => {
            not_started_line().text(format!("Add host: {}", super::LAUNCH_IN_FLIGHT))
        }
        Target::Registered { name, .. } => not_started_line()
            .text("Set up ")
            .host(name)
            .text(format!(": {}", super::LAUNCH_IN_FLIGHT)),
    };
    cx.defer(move |cx| {
        let problem = add::error_line().text(format!("Add host: {}", super::LAUNCH_IN_FLIGHT));
        if !reopen_new(&offer, problem, window, cx) {
            not_started(line, cx);
        }
    });
}

/// Show `line` on the Hosts panel's line, saying the host menu's `action` was not started because a session launch is
/// in flight, in place of any earlier such line. It shows until that same action starts on any host
/// ([`forget_remedy_not_started`]), the other remedy is refused in its place, Add host opens or the UI session resets.
pub(super) fn remedy_not_started(
    action: super::remedies::Action,
    line: HostSentence,
    cx: &mut App,
) {
    let Some(store) = super::current_operations_id(cx) else {
        return;
    };
    show(cx, |lines| {
        lines.remedy_not_started = Some((store, action, line))
    });
}

/// The host menu's `action` started, on any host: a line saying an `action` was not started is over, and one about the
/// other remedy stays.
pub(super) fn forget_remedy_not_started(action: super::remedies::Action, cx: &mut App) {
    show(cx, |lines| {
        if lines
            .remedy_not_started
            .as_ref()
            .is_some_and(|(_, shown, _)| *shown == action)
        {
            lines.remedy_not_started = None;
        }
    });
}

/// Clear the Hosts panel's line about work not started during a launch ([`not_started`]): work of the kind it was about
/// has started since.
pub(super) fn forget_not_started(cx: &mut App) {
    show(cx, |lines| lines.not_started = None);
}

/// The Hosts panel's line for a setup or a setup's rerun not started during a launch, empty: what [`not_started`]
/// shows. Update's line has its own id (`super::update`).
fn not_started_line() -> HostSentence {
    HostSentence::new("hosts-setup-not-started")
}

/// Show `line` ([`not_started_line`] and the refusal) on the Hosts panel's line, saying a setup, rerun or Update was not
/// started because a session launch is in flight, until the next setup, rerun or Update starts, Add host opens, or the
/// UI session resets. The refusal is the app's own, of work that never started, so it is no record of the operation
/// store, whose records are the helm's answers to work it was sent. Visible to the hosts module for Update's own line
/// (`super::update`), which carries its own id.
pub(super) fn not_started(line: HostSentence, cx: &mut App) {
    let Some(store) = super::current_operations_id(cx) else {
        return;
    };
    show(cx, |lines| lines.not_started = Some((store, line)));
}

/// `POST /api/hosts/probe` for a registered SSH host's rerun of its setup: its destination with the install paths the
/// helm recorded for it (`null` for none), and no `new_entry`, so a supervisor that answers is registered onto the same
/// entry rather than refused as a second one, as the old rerun's probe did.
pub(crate) fn rerun_command(host: &HostView) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Probe,
        Method::Post,
        ApiPath::api(&["hosts", "probe"])?,
        Some(json!({
            "target": { "kind": "ssh", "destination": host.destination },
            "remote_farhelm": host.remote_farhelm,
            "remote_state_dir": host.remote_state_dir,
        })),
        "POST /api/hosts/probe",
    ))
}

/// Run `host`'s failed setup again, the host menu's Re-run, with `mark` the [`Mark`] taken in its click handler: probe
/// the host under its own key, and go on from the plan as the module docs say. Sends nothing for a host with no
/// destination (the menu offers the rerun only on an SSH host, which always has one) or while work on the host is
/// out. While a session launch is in flight ([`super::launch_in_flight`]) it sends nothing either, and says so on the
/// Hosts panel's line: "Re-run <host>: not started, …" ([`not_started`]).
pub(crate) fn rerun(host: &HostView, mark: Mark, window: &mut Window, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    if host.destination.is_none() {
        return;
    }
    if super::launch_in_flight(cx) {
        let line = not_started_line()
            .text("Re-run ")
            .host(&display_name(host))
            .text(format!(": {}", super::LAUNCH_IN_FLIGHT));
        not_started(line, cx);
        return;
    }
    let command = match rerun_command(host) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "setup rerun probe not built");
            return;
        }
    };
    let key = OpKey::host(&HostView::key(host.id));
    let target = Target::Registered {
        host: host.id,
        name: display_name(host),
        icon: host.icon,
        color: host.color,
    };
    let handle = window.window_handle();
    let probed = key.clone();
    let id = host.id;
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            let problem = match add::read_reply(&settled.outcome) {
                // The pushed hosts and host progress already show what the helm did.
                Reply::Added => return,
                // A refusal or a manual setup is a failed attempt, which warns the rail while Hosts is closed; a plan
                // that arrived after the user moved on is not, since nothing was refused and nothing ran.
                Reply::Show(problem) => {
                    super::status::failed(id, super::status::Action::Setup, cx);
                    problem.text()
                }
                Reply::Setup {
                    probe_id,
                    confirmation,
                    steps,
                } => {
                    let offer = Offer {
                        key: probed.clone(),
                        probe_id,
                        confirmation,
                        steps,
                        target,
                        mark,
                    };
                    if !asks_first(cx) {
                        submit(offer, handle, cx);
                        return;
                    }
                    if !focus::has_moved_on(&mark, cx)
                        && handle
                            .update(cx, |_, window, cx| confirm(offer, window, cx))
                            .is_ok()
                    {
                        return;
                    }
                    HostText::from_host(RERUN_MOVED_ON)
                }
            };
            // Why the rerun leads nowhere shows under the host's row, where a refusal of the probe already does.
            if let Some(operations) = session::current_operations(cx) {
                operations.update(cx, |operations, cx| {
                    operations.note(&probed, Work::Probe, problem, cx)
                });
            }
        })
    });
    match started {
        Ok(()) => show(cx, |lines| lines.not_started = None),
        Err(not_started) => tracing::debug!(?not_started, "setup rerun probe not sent"),
    }
}

/// Turn on "Set up new hosts without asking" for the permanent answer: only its field of the preference row, under the
/// row's key, as Settings' switch saves it. Not retried; a refusal shows on the Hosts panel's line (the module docs).
/// While another preference save is in flight (from Settings, the templates dialog or a tunnel consent) the store refuses
/// this one, which sends nothing and says so on the same line, so the user is not asked again later with no word why.
fn stop_asking(cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let command = match crate::settings::skip_host_setup_command() {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "the permanent answer's preference not built");
            return;
        }
    };
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::preferences(), command, cx, |settled, cx| {
            if let Outcome::Refused(refusal) = &settled.outcome {
                let store = super::current_operations_id(cx);
                show(cx, |lines| {
                    lines.stop_asking = store.map(|store| (store, refusal.words.clone()))
                });
            }
        })
    });
    match started {
        Ok(()) => show(cx, |lines| lines.stop_asking = None),
        Err(NotStarted::Busy(_)) => {
            let store = super::current_operations_id(cx);
            let words = HostText::from_host(Refused::WORDS);
            show(cx, |lines| {
                lines.stop_asking = store.map(|store| (store, words))
            });
        }
        Err(not_started) => {
            tracing::debug!(?not_started, "the permanent answer's preference not sent")
        }
    }
}

/// What the Hosts panel's line shows: a setup refused after the user moved on from it, by its host-add key (the words
/// are the operation store's record under that key, so a reset, which empties the store, takes them away), and a
/// refused permanent answer's words, copied, because Settings' saves share the preference row's key and its record
/// would show their refusals too, and a setup, rerun or Update not started during a launch. Each stays until Add host
/// opens again or the next one of its kind starts. The panel redraws when it changes ([`observe_panel_lines`]).
///
/// The copied words carry the entity id of the operation store they came from, and show only while that is the current
/// store: a reset drops everything that came from the helm (SPEC_impl.md, "One failure path: the reset"), and a new
/// store has a new id.
#[derive(Default)]
struct PanelLines {
    setup: Option<OpKey>,
    stop_asking: Option<(EntityId, HostText)>,
    /// A setup, rerun or Update the app did not start because a session launch was in flight ([`not_started`]), with the
    /// id of the store current then.
    not_started: Option<(EntityId, HostSentence)>,
    /// The latest host menu remedy (Retry or Adopt) not started because a session launch was in flight, with the
    /// store's id and the remedy it names: a slot of its own, so other host work starting leaves it standing. It ends
    /// when that remedy starts on any host, when the other remedy is refused in its place, when Add host opens, or on a
    /// reset ([`remedy_not_started`]).
    remedy_not_started: Option<(EntityId, super::remedies::Action, HostSentence)>,
}

impl Global for PanelLines {}

/// Change the panel's lines with `change`.
fn show(cx: &mut App, change: impl FnOnce(&mut PanelLines)) {
    change(cx.default_global::<PanelLines>());
}

/// Redraw the Hosts panel whenever its lines change, for as long as it keeps the subscription: the global notifies no
/// view by itself.
pub(crate) fn observe_panel_lines(cx: &mut Context<super::HostsPanel>) -> Subscription {
    cx.observe_global::<PanelLines>(|_, cx| cx.notify())
}

/// Add host opened: what the panel's line said belongs to an earlier add, as the old panel's error did, so it goes.
pub(crate) fn forget_panel_lines(cx: &mut App) {
    show(cx, |lines| {
        lines.setup = None;
        lines.stop_asking = None;
        lines.not_started = None;
        lines.remedy_not_started = None;
    });
}

/// The Hosts panel's line, as sentences under their own ids: a setup refused after the user moved on from it, a setup,
/// rerun or Update not started during a launch, a Retry or Adopt not started during one, and a refused permanent
/// answer, while it was refused by the current UI session's helm and the preference it tried to turn on is still off
/// (once on, the refusal no longer matters). Each of the last three only while it came from the current UI session's
/// store. Empty when there is none.
pub(crate) fn panel_lines(cx: &App) -> Vec<HostSentence> {
    let Some(lines) = cx.try_global::<PanelLines>() else {
        return Vec::new();
    };
    let setup = lines.setup.as_ref().and_then(|key| {
        let operations = session::current_operations(cx)?;
        let failure = operations.read(cx).failure(key)?;
        (failure.work == Work::Setup).then(|| {
            HostSentence::new("hosts-add-error")
                .text("Add host: ")
                .host(&failure.message)
        })
    });
    let current = super::current_operations_id(cx);
    let stop_asking = lines
        .stop_asking
        .as_ref()
        .filter(|(store, _)| Some(*store) == current && asks_first(cx))
        .map(|(_, reason)| {
            HostSentence::new("hosts-stop-asking-error")
                .text("Set up new hosts without asking: ")
                .host(reason)
        });
    let not_started = lines
        .not_started
        .as_ref()
        .filter(|(store, _)| Some(*store) == current)
        .map(|(_, line)| line.clone());
    let remedy_not_started = lines
        .remedy_not_started
        .as_ref()
        .filter(|(store, _, _)| Some(*store) == current)
        .map(|(_, _, line)| line.clone());
    setup
        .into_iter()
        .chain(not_started)
        .chain(remedy_not_started)
        .chain(stop_asking)
        .collect()
}

// ---- The plan -------------------------------------------------------------------------------------------------------

/// One step of the plan, as host text.
struct Step {
    name: HostText,
    /// Each path's short spelling, shown, and its full one, the chip's hover.
    paths: Vec<(HostText, HostText)>,
    explanation: Option<HostText>,
}

/// A host confirmation's body: the helm's numbered plan, copied out of the question's frozen value when it opens, so it
/// shows that plan for as long as it is open (`Question::body`). The setup confirmation and the uninstall confirmation
/// ([`super::uninstall`]) both draw it, as the old app's two dialogs shared `PlanStepList`, so the two plans cannot
/// drift apart in how they show the helm's promises.
pub(super) struct PlanBody {
    lead: HostText,
    steps: Vec<Step>,
    /// What an uninstall keeps (`PlanSteps::kept`): the host's data directory, which no step touches, in the helm's
    /// words. A setup's plan keeps nothing.
    kept: Option<HostText>,
    /// The helm's plan text, which Copy plan copies exactly, trailing newline included.
    confirmation: SharedString,
}

impl PlanBody {
    /// The body for the helm's plan `steps`, whose plan text `confirmation` is what Copy plan copies.
    pub(super) fn new(confirmation: &str, steps: &PlanSteps) -> Self {
        let text = |value: &str| HostText::from_host(value);
        PlanBody {
            lead: text(&steps.lead),
            kept: steps.kept.as_deref().map(text),
            steps: steps
                .steps
                .iter()
                .map(|step| Step {
                    name: text(&step.name),
                    paths: step
                        .paths
                        .iter()
                        .map(|path| (text(&path.short), text(&path.full)))
                        .collect(),
                    explanation: step.explanation.as_deref().map(text),
                })
                .collect(),
            confirmation: SharedString::from(confirmation.to_owned()),
        }
    }
}

impl Render for PlanBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let confirmation = self.confirmation.clone();
        // `.provisioning-plan-heading`: the lead, and Copy plan at its right.
        let heading = div()
            .flex()
            .flex_row()
            .items_start()
            .justify_between()
            .gap(px(12.))
            .mb(px(14.))
            .child(host_text::element(&self.lead).flex_1().min_w_0())
            .child(
                copy_button(
                    "setup-copy-plan",
                    "Copy plan",
                    HoverHelp::tooltip("Copy the complete host plan"),
                    confirmation,
                )
                .compact(),
            );
        let count = self.steps.len();
        let steps: Vec<AnyElement> = self
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| self.step(index, step, index + 1 == count, window, cx))
            .collect();
        div()
            .id("setup-plan")
            .flex()
            .flex_col()
            .mt(px(2.))
            .mb(px(6.))
            .py(px(7.))
            .px(px(9.))
            .bg(theme::color(palette.well))
            .border_1()
            .border_color(theme::color(palette.border_dim))
            .rounded(metrics::RADIUS_SM)
            .text_size(metrics::TEXT_SM)
            .line_height(px(PLAN_LINE))
            .text_color(theme::color(palette.fg_0))
            .child(heading)
            .children(steps)
            // `.provisioning-plan-kept`: what an uninstall keeps, on an amber wash with an amber edge, apart from the
            // steps because no step touches it. The old wash was `--warn` at 10%; the palette's nearest is the
            // warning chip's 12%.
            .children(self.kept.as_ref().map(|kept| {
                div()
                    .id("plan-kept")
                    .mt(px(14.))
                    .py(px(7.))
                    .px(px(9.))
                    .border_l_2()
                    .border_color(theme::color(palette.warn))
                    .bg(theme::color(palette.warn_chip_bg))
                    .child(HostSentence::new("plan-kept-text").host(kept))
            }))
    }
}

impl PlanBody {
    /// One numbered step (`.provisioning-plan-step`): the number on the rail that joins it to the next, then the
    /// step's name, its path chips and its explanation.
    fn step(
        &self,
        index: usize,
        step: &Step,
        last: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let palette = theme::palette(cx);
        let rail = (!last).then(|| {
            div()
                .absolute()
                .top(px(RAIL))
                .bottom_0()
                .left(px(RAIL / 2. - 1.))
                .w(px(1.))
                .bg(theme::color(palette.accent_edge))
        });
        let number = div()
            .flex_none()
            .size(px(NUMBER))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(theme::color(palette.accent_edge))
            .bg(theme::color(palette.well))
            .text_size(px(SMALL_TEXT))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme::color(palette.accent))
            .child((index + 1).to_string());
        let paths = step.paths.iter().enumerate().map(|(n, (short, full))| {
            let id = SharedString::from(format!("setup-plan-path-{index}-{n}"));
            let help = HoverHelp::sentence(
                HostSentence::new(SharedString::from(format!("{id}-help"))).host(full),
            );
            let chip = div()
                .id(id.clone())
                .max_w_full()
                .py(px(2.))
                .px(px(6.))
                .border_1()
                .border_color(theme::color(palette.border_dim))
                .rounded(metrics::RADIUS_SM)
                .bg(theme::color(palette.well))
                .text_color(theme::color(palette.fg_1))
                .text_size(px(SMALL_TEXT))
                .font_family(assets::TERMINAL_FONT)
                .child(host_text::element(short).min_w_0());
            with_hover_help(chip, &id, &help, false, None, window, cx).into_any_element()
        });
        let detail = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                host_text::element(&step.name)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0)),
            )
            .when(!step.paths.is_empty(), |detail| {
                detail.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(4.))
                        .mt(px(5.))
                        .children(paths),
                )
            })
            .children(step.explanation.as_ref().map(|explanation| {
                host_text::element(explanation)
                    .mt(px(5.))
                    .text_size(px(SMALL_TEXT))
                    .text_color(theme::color(palette.fg_dim))
            }));
        div()
            .relative()
            .flex()
            .flex_row()
            .gap(px(10.))
            .when(!last, |row| row.pb(px(14.)))
            .children(rail)
            .child(div().w(px(RAIL)).flex_none().child(number))
            .child(detail)
            .into_any_element()
    }
}
