//! Setting up a new SSH host: the confirmation that shows the helm's plan, and the submission that starts the run.
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
//!   and the helm's words, with the same lifetime; it also goes once the preference is on, or after a reset.

#[cfg(test)]
pub(crate) mod tests;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::provisioning::PlanSteps;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, EntityId, FocusHandle, FontWeight,
    Global, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::json;

use super::add;
use super::identity::host_mark;
use crate::assets;
use crate::components::{HoverHelp, Tier, copy_button, with_hover_help};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::theme::{self, metrics};

/// The old setup confirmation's width, the Add host dialog's.
const WIDTH: f32 = 560.;

/// What the Add host dialog says when a plan arrives after the user moved on, so the confirmation does not open.
pub(crate) const MOVED_ON: &str =
    "Farhelm is not running on this host. Press Add again to see what setting it up changes.";

/// `.provisioning-plan`'s line height: 1.45 at its 12 pixels.
const PLAN_LINE: f32 = 17.4;

/// `.provisioning-plan-number`'s circle, and the rail column it sits in (`grid-template-columns: 24px ...`).
const NUMBER: f32 = 22.;
const RAIL: f32 = 24.;

/// The text size of the numbers, path chips and explanations.
const SMALL_TEXT: f32 = 11.;

/// A plan the helm offered for a new host, as the confirmation shows it and the submission acts on it: the frozen value
/// of the question.
#[derive(Clone)]
pub(crate) struct Offer {
    /// The host-add key the probe ran under, which the submission continues under.
    pub(crate) key: OpKey,
    /// The helm's one-use id for the plan it kept; the submission names the plan by it and nothing else.
    pub(crate) probe_id: String,
    /// The helm's plan as text, which Copy plan copies exactly.
    pub(crate) confirmation: String,
    /// The plan's steps, which the body shows.
    pub(crate) steps: PlanSteps,
    /// The destination as the probe sent it: the title names it, and a refusal puts it back in Add host.
    pub(crate) destination: String,
    /// The alias as typed in Add host (blank for none); a refusal puts it back there. The probe already sent it as the
    /// new entry's alias, and the helm's kept plan carries it, so the submission does not send it again.
    pub(crate) alias: String,
    /// The control that opened Add host, which gets focus back.
    pub(crate) opener: FocusHandle,
    /// The mark of the user's action behind the offer: Add's while the confirmation is to open (it opens only through
    /// it), then the answer's, taken when Set up or the permanent answer is pressed; Add's still when the plan goes out
    /// without asking. A refusal reopens Add host only through it, so not once the user has moved on since. The
    /// operation store's own mark is no substitute: on the path without asking, it is taken when the probe's reply
    /// arrives, which says nothing about what the user did while the probe was out.
    pub(crate) mark: Mark,
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
    let title = HostSentence::new("setup-title")
        .text("Set up host · ")
        .host(&HostText::from_host(offer.destination.as_str()));
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
                stop_asking(cx);
                submit(offer, window.window_handle(), cx);
            },
        )
        .body(|offer: &Offer, _, cx| {
            let body = PlanBody::new(offer);
            cx.new(|_| body)
        })
        .tile(|_, cx| {
            host_mark(
                HostIcon::Cloud,
                HostColor::Default,
                px(24.),
                theme::palette(cx),
            )
        })
        .note("Undo \"don't ask again\" with the gear at the foot of the navigation rail")
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
            submit(offer, window.window_handle(), cx)
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

/// Submit `offer` under its host-add key, as the operation store's work, so it goes on whatever view is open. An
/// accepted submission needs nothing more here: the run shows in the pushed host progress. A refusal reopens Add host
/// through [`Offer::mark`], or shows on the Hosts panel's line if the user moved on since (the module docs).
pub(crate) fn submit(offer: Offer, window: AnyWindowHandle, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let command = match provision_command(&offer.probe_id) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host setup not built");
            return;
        }
    };
    let key = offer.key.clone();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            let Outcome::Refused(reason) = &settled.outcome else {
                return;
            };
            let reopened = !focus::has_moved_on(&offer.mark, cx)
                && window
                    .update(cx, |_, window, cx| {
                        let seed = add::Seed {
                            destination: offer.destination.clone(),
                            alias: offer.alias.clone(),
                            problem: add::error_line().host(reason),
                        };
                        add::reopen(&offer.opener, &offer.mark, Some(seed), window, cx).is_some()
                    })
                    .unwrap_or(false);
            // Moved on, or Add host could not open (the window went inert): the panel's line is where it belongs. The
            // words stay the operation store's record under this add's key, which nothing else starts work on.
            if !reopened {
                show(cx, |lines| lines.setup = Some(offer.key.clone()));
            }
        })
    });
    match started {
        // A refusal still shown from an earlier add belongs to that add; this one replaces it.
        Ok(()) => show(cx, |lines| lines.setup = None),
        Err(not_started) => tracing::debug!(?not_started, "host setup not sent"),
    }
}

/// Turn on "Set up new hosts without asking" for the permanent answer: only its field of the preference row, under the
/// row's key, as Settings' switch saves it. Not retried; a refusal shows on the Hosts panel's line (the module docs).
/// While another preference save is in flight the store refuses this one, and it is dropped (recorded in the parity
/// file): only Settings saves under that key, and it cannot be open while this question is.
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
            if let Outcome::Refused(reason) = &settled.outcome {
                let store =
                    session::current_operations(cx).map(|operations| operations.entity_id());
                show(cx, |lines| {
                    lines.stop_asking = store.map(|store| (store, reason.clone()))
                });
            }
        })
    });
    match started {
        Ok(()) => show(cx, |lines| lines.stop_asking = None),
        Err(not_started) => {
            tracing::debug!(?not_started, "the permanent answer's preference not sent")
        }
    }
}

/// What the Hosts panel's line shows: a setup refused after the user moved on from it, by its host-add key (the words
/// are the operation store's record under that key, so a reset, which empties the store, takes them away), and a
/// refused permanent answer's words, copied, because Settings' saves share the preference row's key and its record
/// would show their refusals too. Each stays until Add host opens again or the next one of its kind starts. The panel
/// redraws when it changes ([`observe_panel_lines`]).
///
/// The copied words carry the entity id of the operation store they came from, and show only while that is the current
/// store: a reset drops everything that came from the helm (SPEC_impl.md, "One failure path: the reset"), and a new
/// store has a new id.
#[derive(Default)]
struct PanelLines {
    setup: Option<OpKey>,
    stop_asking: Option<(EntityId, HostText)>,
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
    });
}

/// The Hosts panel's line, as sentences under their own ids: a setup refused after the user moved on from it, and a
/// refused permanent answer, while it was refused by the current UI session's helm and the preference it tried to turn
/// on is still off (once on, the refusal no longer matters). Empty when there is neither.
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
    let current = session::current_operations(cx).map(|operations| operations.entity_id());
    let stop_asking = lines
        .stop_asking
        .as_ref()
        .filter(|(store, _)| Some(*store) == current && asks_first(cx))
        .map(|(_, reason)| {
            HostSentence::new("hosts-stop-asking-error")
                .text("Set up new hosts without asking: ")
                .host(reason)
        });
    setup.into_iter().chain(stop_asking).collect()
}

// ---- The plan --------------------------------------------------------------------------------------------------------

/// One step of the plan, as host text.
struct Step {
    name: HostText,
    /// Each path's short spelling, shown, and its full one, the chip's hover.
    paths: Vec<(HostText, HostText)>,
    explanation: Option<HostText>,
}

/// The confirmation's body: the helm's plan, copied out of the frozen offer when the question opens, so it shows that
/// plan for as long as it is open (`Question::body`).
struct PlanBody {
    lead: HostText,
    steps: Vec<Step>,
    /// The helm's plan text, which Copy plan copies exactly, trailing newline included.
    confirmation: SharedString,
}

impl PlanBody {
    fn new(offer: &Offer) -> Self {
        let text = |value: &str| HostText::from_host(value);
        PlanBody {
            lead: text(&offer.steps.lead),
            steps: offer
                .steps
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
            confirmation: offer.confirmation.clone().into(),
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
            // An uninstall's plan also carries what it keeps (`PlanSteps::kept`); a setup's never does, so it is not
            // drawn here until Uninstall shares this body.
            .children(steps)
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
            with_hover_help(chip, &id, &help, false, window, cx).into_any_element()
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
