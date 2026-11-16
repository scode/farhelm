//! Removing Farhelm from a remote host: the host menu's Uninstall, the plan the user confirms, the run, and the one-line
//! notice once the host is gone.
//!
//! SPEC.md, "Topology" (the uninstall paragraph) and SPEC_impl.md, "Provisioning" (UNINSTALL). Ported from the old
//! app's `UninstallConfirmation`, `UninstallNotice` and the uninstall arms of `prepare` and the confirmed submission
//! (`provisioning.rs`), and the Hosts panel's notice (`hosts.rs`: `uninstall_notices`, `uninstall_notice_parts`), with
//! the look of the `.host-uninstall-dialog`, `.provisioning-plan-kept` and `.uninstall-notice` rules in `app.css`, all at
//! reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **Plan first, nothing changes until confirmed.** The menu's Uninstall ([`start`]) sends `POST
//!   /api/hosts/{id}/uninstall` without a body, the operation store's work under the host's key
//!   ([`Work::UninstallHost`]), and the folded row reads "Uninstalling…" while it is out ([`super::progress`]). The
//!   helm answers within its host-wait bound with a one-use plan, or refuses.
//! - **The confirmation** ([`confirm`]) is a question of the prompt frozen on the plan ([`Plan`]): "Uninstall Farhelm ·
//!   <name>", the host's phase and build, the helm's numbered plan as its body with what stays (the data directory) in
//!   its own amber note ([`super::setup::PlanBody`], the body the setup confirmation draws), and Cancel and Uninstall in
//!   the danger tier, where focus starts (SPEC.md, "Opening and closing": host uninstall starts on the action asked
//!   for). It opens only through the mark taken at the press: a plan that arrives after the user moved on opens nothing
//!   and says so under the row ([`MOVED_ON`]). It closes by itself if the host goes meanwhile.
//! - **The run.** Uninstall sends the same route naming the plan, which the helm accepts (202) once the run has started;
//!   its progress is the pushed host progress, which the row shows as it shows a setup's: the running row opens once,
//!   the user may fold it, and a failure opens it on the failed step. Choosing Uninstall again continues a failed run
//!   ([`super::offers`] keeps it available after a failure).
//! - **Refusals.** A refused plan (sessions or tabs still on the host, not connected, a `farhelm helm setup` unit, a
//!   supervisor running from elsewhere, a symlinked lib directory, or any other reason the helm gives) opens the host
//!   refusal dialog ([`super::refusal`], "Uninstall Farhelm · <name>") through the press's mark, with Remove when the
//!   helm marks the refusal "remove instead"; a refused confirmation opens it through the mark of the Uninstall answer.
//!   Either one that opened no dialog (the user moved on) stays under the row, the store's record under the host's key;
//!   one the dialog showed is dropped from that record, as Update's is. The helm's words are the reason; nothing here
//!   decides which refusals offer Remove.
//! - **The removal notice** ([`notice`]). The helm keeps no result for a host it forgot, so the window that confirmed
//!   builds its notice from the plan it holds: confirming owes the host a notice ("Removed from <name> · data kept",
//!   hovering naming the kept data directory, Copy copying that path, and × dismissing it), which shows at the top of
//!   the Hosts panel once the registry stops listing the host. A refused confirmation or a failed run of that uninstall
//!   withdraws what was owed, so a host that later leaves the list some other way never shows it ([`follow`]), and a
//!   refusal that arrives after an agent or the command line removed the host takes back the notice its leaving showed.
//! - **During a launch.** Like every host registry change here, Uninstall and its confirmation refuse to start while a
//!   session launch is in flight ([`super::launch_in_flight`]), saying "Uninstall <name>: not started, …" on the Hosts
//!   panel's line.
//!
//! What a notice owes and shows lives in an app global tagged with the operation store's entity id, as the panel's
//! other lines do, so a reset forgets it, as the old app's reload did. A global rather than a field of the panel because
//! the completion of the confirming request has no handle on the panel.

#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::provisioning::{PlanSteps, ProvisioningOperation, ProvisioningView, RunStatus};
use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, EntityId, Global,
    InteractiveElement as _, IntoElement, ParentElement as _, Role, Styled as _, Subscription,
    Window, div, px,
};
use serde::Deserialize;
use serde_json::json;

use super::identity::{display_name, host_mark};
use super::listed;
use super::refusal;
use super::setup::PlanBody;
use crate::assets::Icon;
use crate::components::{HoverHelp, IconButton, StatusMark, Tier, copy_button};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{LayerId, Subject};
use crate::operations::{self, Command, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::theme::{self, metrics};

/// The old uninstall dialog's width (`.host-uninstall-dialog`), the Add host dialog's.
const WIDTH: f32 = 560.;

/// How a refusal without words names the uninstall's requests, with the host's id hidden.
const SHOWN: &str = "POST /api/hosts/[host]/uninstall";

/// The action the refusal dialog's title names, the old dialog's "Uninstall Farhelm".
pub(crate) const ACTION: &str = "Uninstall Farhelm";

/// The menu item's hover help.
pub(super) const UNINSTALL_HELP: &str =
    "Uninstall: remove Farhelm from this host; its data stays on the host";

/// What a host's row says when the plan its Uninstall asked for arrived after the user moved on, so no confirmation
/// opened (new words, after setup's rerun's for the same case).
pub(crate) const MOVED_ON: &str = "Farhelm planned this uninstall after you moved on. Choose Uninstall again to see \
                                   what it removes.";

// ---- The requests ----------------------------------------------------------------------------------------------------

/// The uninstall route of host `host`.
fn path(host: i64) -> Result<ApiPath, PathError> {
    ApiPath::api(&["hosts", &HostView::key(host), "uninstall"])
}

/// `POST /api/hosts/{id}/uninstall` without a body: the helm checks the host and answers with a one-use plan for
/// removing Farhelm from it, or refuses.
fn plan_command(host: i64) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::UninstallHost,
        Method::Post,
        path(host)?,
        None,
        SHOWN,
    ))
}

/// The same route naming the plan `probe_id`: the helm consumes that plan, once, checks the host again and starts the
/// run.
fn confirm_command(host: i64, probe_id: &str) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::UninstallHost,
        Method::Post,
        path(host)?,
        Some(json!({ "probe_id": probe_id })),
        SHOWN,
    ))
}

/// The part of the helm's plan reply the confirmation shows and the notice needs.
#[derive(Deserialize)]
struct Reply {
    probe_id: String,
    /// The helm's plan as text, which Copy plan copies exactly.
    confirmation: String,
    steps: PlanSteps,
    plan: KeptPath,
}

/// The one field of the helm's frozen plan the GUI reads: the data directory the uninstall keeps, as the host spells
/// it, which the notice's Copy copies. The old app read it from the same place (`planned.plan.state_dir`); the plan's
/// `kept` note says it in a sentence, which Copy cannot use.
#[derive(Deserialize)]
struct KeptPath {
    state_dir: String,
}

/// An uninstall the helm planned, frozen when the confirmation opens: what the question shows and its Uninstall acts on.
#[derive(Clone)]
struct Plan {
    /// The host as the registry listed it when the plan arrived: the title, the tile and a refusal dialog name it from
    /// here, so the dialog still says which host it was about after the registry stops listing it.
    view: HostView,
    /// The helm's one-use id for the plan; the confirmation names the plan by it and nothing else.
    probe_id: String,
    confirmation: String,
    steps: PlanSteps,
    /// The kept data directory, raw, for the notice's Copy.
    kept: String,
}

/// Say on the Hosts panel's line that the uninstall of `view` was not started because a session launch is in flight.
fn refuse_during_launch(view: &HostView, cx: &mut App) {
    let line = HostSentence::new("hosts-uninstall-not-started")
        .text("Uninstall ")
        .host(&display_name(view))
        .text(format!(": {}", super::LAUNCH_IN_FLIGHT));
    super::setup::not_started(line, cx);
}

// ---- Planning --------------------------------------------------------------------------------------------------------

/// The host menu's Uninstall for `host`, pressed with `mark` taken in its handler: ask the helm for a plan, and go on
/// from its answer as the module docs say. While a session launch is in flight it sends nothing and the Hosts panel's
/// line says why; nothing happens when the window is inert or the registry no longer lists the host. A press that meets
/// other work on the host's key sends and records nothing: the menu offers Uninstall only on an idle host, so only a
/// press racing a frame gets there.
pub(super) fn start(host: i64, mark: Mark, window: &mut Window, cx: &mut App) {
    let Some(view) = listed(host, cx) else {
        return;
    };
    if super::launch_in_flight(cx) {
        refuse_during_launch(&view, cx);
        return;
    }
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let command = match plan_command(host) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host uninstall plan not built");
            return;
        }
    };
    let key = OpKey::host(&HostView::key(host));
    let planned = key.clone();
    let handle = window.window_handle();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            match &settled.outcome {
                // The host as listed when the plan arrives, which is what the confirmation freezes: up to the helm's
                // host-wait bound can have passed since the press.
                Outcome::Done(body) => {
                    let view = listed(host, cx).unwrap_or(view);
                    offer(view, &planned, body.clone(), mark, handle, cx)
                }
                Outcome::Refused(refusal) => {
                    let view = listed(host, cx).unwrap_or(view);
                    if refused(&view, refusal, mark, handle, cx) {
                        forget(&planned, cx);
                    }
                }
                // The plan request is answered 200; the helm never starts an uninstall without a confirmed plan.
                Outcome::Accepted(_) => {}
            }
        })
    });
    match started {
        // A line about an earlier press not started belongs to that press; one has started since.
        Ok(()) => super::setup::forget_not_started(cx),
        Err(not_started) => tracing::debug!(?not_started, "host uninstall plan not sent"),
    }
}

/// The plan request for `view` was answered with `body`: open the confirmation through `mark`, or say under the row why
/// it did not open (the reply could not be read, or the user moved on since the press). A host the registry no longer
/// lists gets neither: it has no row, and there is nothing left to uninstall from this window.
fn offer(
    view: HostView,
    key: &OpKey,
    body: Option<serde_json::Value>,
    mark: Mark,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    let reply = match operations::decode::<Reply>(Ok(body), SHOWN) {
        Ok(reply) => reply,
        Err(problem) => return note(key, problem, cx),
    };
    let host = view.id;
    let plan = Plan {
        view,
        probe_id: reply.probe_id,
        confirmation: reply.confirmation,
        steps: reply.steps,
        kept: reply.plan.state_dir,
    };
    let opened = window
        .update(cx, |_, window, cx| confirm(plan, &mark, window, cx))
        .ok()
        .flatten();
    if opened.is_none() && listed(host, cx).is_some() {
        note(key, HostText::from_host(MOVED_ON), cx);
    }
}

/// Record `message` under `key`, where the row shows it: from the plan request's completion, so no later work on the
/// key is running yet.
fn note(key: &OpKey, message: HostText, cx: &mut App) {
    if let Some(operations) = session::current_operations(cx) {
        operations.update(cx, |operations, cx| {
            operations.note(key, Work::UninstallHost, message, cx)
        });
    }
}

/// Drop the store's record of a refusal under `key`, which the refusal dialog showed, so the row neither shows it again
/// nor opens for it.
fn forget(key: &OpKey, cx: &mut App) {
    if let Some(operations) = session::current_operations(cx) {
        operations.update(cx, |operations, cx| operations.forget(key, cx));
    }
}

/// Show the helm's `refusal` of an uninstall of `view` in the host refusal dialog, through `mark`; whether it opened.
/// With the helm's "remove instead" marker the dialog offers Remove.
fn refused(
    view: &HostView,
    refusal: &operations::Refusal,
    mark: Mark,
    window: AnyWindowHandle,
    cx: &mut App,
) -> bool {
    window
        .update(cx, |_, window, cx| {
            refusal::open(
                refusal::Refusal::of(ACTION, view, refusal),
                &mark,
                window,
                cx,
            )
        })
        .ok()
        .flatten()
        .is_some()
}

// ---- The confirmation ------------------------------------------------------------------------------------------------

/// Ask the uninstall confirmation for `plan` (see the module docs) unless the user has moved on since `mark`, the
/// window is inert, or the host is already gone. Returns the question's layer. It has no permanent answer, unlike the
/// setup and removal confirmations, so it saves no preference and has no footer note (the old dialog had neither).
fn confirm(plan: Plan, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let key = HostView::key(plan.view.id);
    let name = display_name(&plan.view);
    let title = HostSentence::new("uninstall-title")
        .text("Uninstall Farhelm · ")
        .host(&name);
    let summary = super::remove::phase_summary("uninstall-summary", &plan.view);
    let (icon, color) = (plan.view.icon, plan.view.color);
    let uninstall = Answer::new(
        "Uninstall",
        HoverHelp::tooltip("Uninstall: remove Farhelm from this host as described above"),
        Tier::Danger,
    );
    let question = Question::new(title, uninstall, plan)
        .summary(summary)
        .cancel_answer(Answer::new(
            "Cancel",
            HoverHelp::tooltip("Cancel: leave Farhelm installed on this host"),
            Tier::Neutral,
        ))
        .body(|plan: &Plan, _, cx| {
            let body = PlanBody::new(&plan.confirmation, &plan.steps);
            cx.new(|_| body)
        })
        .tile(move |_, cx| host_mark(icon, color, px(24.), theme::palette(cx)))
        .width(px(WIDTH))
        // SPEC.md, "Opening and closing": the dialogs that share the host and Settings installer keep focus on the
        // control in use when their empty space is clicked.
        .keeps_focus_on_empty_click();
    let subject = Subject::when(store, move |store, _| super::settings::gone(store, &key));
    prompt::ask(
        question,
        Some(subject),
        mark,
        |plan, window, cx| {
            // The mark of this answer, taken after the question closed: a refusal of the confirmation opens through it.
            let mark = Mark::take(cx);
            submit(plan, mark, window.window_handle(), cx);
        },
        window,
        cx,
    )
}

/// The reference screens' `build-box` uninstall confirmation, for the screenshot golden: the plan a connected host's
/// uninstall shows, opened through `mark`.
#[cfg(test)]
pub(crate) fn confirm_for_golden(view: HostView, mark: &Mark, window: &mut Window, cx: &mut App) {
    let _ = confirm(tests::reference_plan(view), mark, window, cx);
}

/// Send `plan`'s confirmation under its host's key, with `mark` the Uninstall answer's, after owing the host its
/// removal notice. A refusal withdraws the notice and opens the refusal dialog through `mark` (or stays under the row
/// once the user has moved on); an accepted one needs nothing more here, since the run is already pushed and
/// [`follow`] shows the notice once the host is gone. Sends nothing while a session launch is in flight.
fn submit(plan: Plan, mark: Mark, window: AnyWindowHandle, cx: &mut App) {
    if super::launch_in_flight(cx) {
        refuse_during_launch(&plan.view, cx);
        return;
    }
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let host = plan.view.id;
    let command = match confirm_command(host, &plan.probe_id) {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host uninstall confirmation not built");
            return;
        }
    };
    owe(&plan, cx);
    let key = OpKey::host(&HostView::key(host));
    let confirmed = key.clone();
    let view = plan.view;
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            if let Outcome::Refused(refusal) = &settled.outcome {
                withdraw(host, cx);
                let view = listed(host, cx).unwrap_or(view);
                if refused(&view, refusal, mark, window, cx) {
                    forget(&confirmed, cx);
                }
            }
        })
    });
    match started {
        Ok(()) => super::setup::forget_not_started(cx),
        Err(not_started) => {
            withdraw(host, cx);
            tracing::debug!(?not_started, "host uninstall confirmation not sent");
        }
    }
}

// ---- The removal notice ----------------------------------------------------------------------------------------------

/// The one-line removal notice: which host Farhelm was removed from, by the name its row showed when the user
/// confirmed, and the data directory that stays there.
#[derive(Clone, PartialEq)]
struct Notice {
    /// The host's registry id, by which a refusal that arrives after the host left the list takes the notice back.
    host: i64,
    name: HostText,
    /// Raw, for Copy; drawn as host text.
    kept: String,
}

/// A notice this window owes a host it confirmed an uninstall of, until the host leaves the list.
#[derive(Clone, PartialEq)]
struct Owed {
    notice: Notice,
    /// The host's retained run when the notice was owed: on a continuation, the earlier failed run, whose failure says
    /// nothing about the run this notice belongs to. Only a failure of another uninstall run withdraws the notice.
    superseded: Option<String>,
}

/// The notices of one UI session: those owed by host id, and the one shown, the newest. Tagged with the entity id of
/// the operation store they were made under ([`notices`]).
#[derive(Clone, Default, PartialEq)]
struct Notices {
    store: Option<EntityId>,
    owed: HashMap<i64, Owed>,
    shown: Option<Notice>,
}

impl Global for Notices {}

/// The current UI session's notices; `None` while the window is inert or when nothing was kept under it.
fn notices(cx: &App) -> Option<&Notices> {
    let store = super::current_operations_id(cx)?;
    cx.try_global::<Notices>()
        .filter(|notices| notices.store == Some(store))
}

/// Apply `change` to the current UI session's notices, starting from none when they were made under another session,
/// and store them only when they changed, so the Hosts panel redraws only then. Nothing while the window is inert.
fn change(cx: &mut App, change: impl FnOnce(&mut Notices)) {
    let Some(store) = super::current_operations_id(cx) else {
        return;
    };
    let before = notices(cx);
    let mut after = before.cloned().unwrap_or(Notices {
        store: Some(store),
        ..Notices::default()
    });
    change(&mut after);
    if before != Some(&after) {
        cx.set_global(after);
    }
}

/// Owe `plan`'s host its removal notice, replacing any notice owed it before.
fn owe(plan: &Plan, cx: &mut App) {
    let superseded = session::current_store(cx).and_then(|store| {
        store
            .read(cx)
            .collections()
            .get::<ProvisioningView>(&HostView::key(plan.view.id))
            .and_then(|run| run.run_id.clone())
    });
    let owed = Owed {
        notice: Notice {
            host: plan.view.id,
            name: display_name(&plan.view),
            kept: plan.kept.clone(),
        },
        superseded,
    };
    change(cx, |notices| {
        notices.owed.insert(plan.view.id, owed);
    });
}

/// The notice owed `host` is withdrawn: its uninstall was refused or never sent. One already shown for it goes too: the
/// host left the list some other way (an agent or the command line removed it) while the confirmation was out, and the
/// helm then refused it, so Farhelm was not removed from it.
fn withdraw(host: i64, cx: &mut App) {
    change(cx, |notices| {
        notices.owed.remove(&host);
        if notices
            .shown
            .as_ref()
            .is_some_and(|shown| shown.host == host)
        {
            notices.shown = None;
        }
    });
}

/// Follow the current store: show the notice owed a host the registry no longer lists (the newest replacing any shown),
/// and withdraw the one owed a host whose uninstall run failed. The Hosts panel calls it on every change of the store,
/// whether or not Hosts shows, so the notice waits in the panel for whenever it is opened.
pub(super) fn follow(cx: &mut App) {
    let Some(store) = session::current_store(cx) else {
        return;
    };
    let Some(owed) = notices(cx).map(|notices| notices.owed.clone()) else {
        return;
    };
    let store = store.read(cx);
    let mut gone = Vec::new();
    let mut failed = Vec::new();
    for (host, owed) in &owed {
        let key = HostView::key(*host);
        if super::settings::gone(store, &key) {
            gone.push(*host);
            continue;
        }
        let run = store.collections().get::<ProvisioningView>(&key);
        if run.is_some_and(|run| {
            run.operation == Some(ProvisioningOperation::Uninstall)
                && run.status == RunStatus::Failed
                && run.run_id != owed.superseded
        }) {
            failed.push(*host);
        }
    }
    if gone.is_empty() && failed.is_empty() {
        return;
    }
    change(cx, |notices| {
        for host in failed {
            notices.owed.remove(&host);
        }
        for host in gone {
            if let Some(owed) = notices.owed.remove(&host) {
                notices.shown = Some(owed.notice);
            }
        }
    });
}

/// Hide the notice, as its × does.
fn dismiss(cx: &mut App) {
    change(cx, |notices| notices.shown = None);
}

/// Show the removal notice for a host named `name` whose data stays in `kept`, as an uninstall this window confirmed
/// does once its host is gone: for the screenshot golden, which has no run to finish.
#[cfg(test)]
pub(crate) fn show_notice_for_golden(name: &str, kept: &str, cx: &mut App) {
    change(cx, |notices| {
        notices.shown = Some(Notice {
            host: 0,
            name: HostText::from_host(name),
            kept: kept.to_owned(),
        })
    });
}

/// Redraw the Hosts panel whenever the notices change, for as long as it keeps the subscription: the global notifies no
/// view by itself.
pub(super) fn observe_notices(cx: &mut Context<super::HostsPanel>) -> Subscription {
    cx.observe_global::<Notices>(|_, cx| cx.notify())
}

/// The removal notice the Hosts panel draws above its rows (`.uninstall-notice`), or `None` while none shows: one line,
/// "Removed from <name> · data kept", cut short to fit, whose hover and accessible name say in full where the data
/// stays ("Farhelm was removed from <name>; its data remains in <path> on that host."), then Copy, which copies that
/// path, and ×, which dismisses the notice. The host's name and the path are host text.
pub(super) fn notice(cx: &App) -> Option<AnyElement> {
    let notice = notices(cx)?.shown.clone()?;
    let palette = theme::palette(cx);
    let kept = HostText::from_host(notice.kept.as_str());
    let full = HostSentence::new("uninstall-notice-text")
        .text("Farhelm was removed from ")
        .host(&notice.name)
        .text("; its data remains in ")
        .mono(&kept)
        .text(" on that host.");
    // The visible line names the result without spending the sidebar's width on the path; the name gives way first.
    let line = div()
        .flex()
        .flex_row()
        .items_center()
        .min_w_0()
        .whitespace_nowrap()
        .child(div().flex_none().child("Removed from "))
        .child(host_text::element(&notice.name).min_w_0().truncate())
        .child(div().flex_none().child(" · data kept"));
    let summary = StatusMark::new(
        "uninstall-notice-summary",
        full.accessible_name(),
        HoverHelp::sentence(full),
        Role::Status,
        "",
    )
    .ink(palette.fg_0)
    .described(false)
    .truncate()
    .content(line);
    let copy = copy_button(
        "uninstall-notice-copy",
        "Copy",
        HoverHelp::tooltip("Copy the kept data path"),
        notice.kept,
    )
    .compact()
    .accessible_name("Copy kept data path");
    let close = IconButton::new(
        "uninstall-notice-dismiss",
        Icon::Close,
        "Dismiss uninstall notice",
        HoverHelp::tooltip("Dismiss: hide this notice"),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .square(px(20.))
    .on_click(|_, _, cx| dismiss(cx));
    Some(
        div()
            .id("uninstall-notice")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .pt(px(2.))
            .px(px(12.))
            .text_size(metrics::TEXT_SM)
            .child(div().flex_1().min_w_0().child(summary))
            .child(div().flex_none().child(copy))
            .child(div().flex_none().child(close))
            .into_any_element(),
    )
}
