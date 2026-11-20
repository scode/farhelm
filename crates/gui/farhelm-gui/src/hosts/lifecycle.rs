//! A sandbox's or container's Suspend and Resume from the Hosts panel: the row's inline Resume and the host menu's
//! Suspend or Resume.
//!
//! SPEC.md, "Sandbox sessions" and "Container sessions" (Suspend freezes the machine's processes and Resume continues
//! them; a stopped container starts again), and "Topology" for the activity check before a host's Suspend. Ported from
//! the old app's `on_lifecycle`, `lifecycle` and `run_lifecycle`, its Suspend question (`answer_suspend`,
//! `cancel_suspend`) and the row's inline `host-resume` button (`hosts.rs`), at reference commit `24bc0580`. Here's the
//! TLDR of its rules:
//!
//! - **Where they show.** On a sandbox or container in every phase but a deletion, under way or failed
//!   ([`super::offers::lifecycle_of`]): Resume where it is suspended, stopped with its place available, or between
//!   states, inline between the row's name and its phase and in the host menu after Retry; Suspend otherwise, in the
//!   menu only, an unreachable machine included. Both are unavailable while the host is busy, and Resume while its
//!   change runs, as the old button and item were. While a Resume's request is out, and while the change it starts
//!   runs, the unavailable Resume stays on the name's line and "Resuming…" takes a line of its own under it, as the
//!   old row's grid laid out `.host-resume` and its lifecycle progress; the press hands focus to the row's menu
//!   toggle, as the inline Update does, since an unavailable button holds none.
//! - **Resume never asks.** It continues processes the user froze, as the old Resume did.
//! - **Suspend asks unless the activity is known.** It reads the host's sessions first through the sessions' own
//!   activity read (`crate::session_list::environment::read_activity`, whose contract this keeps), and goes at once
//!   only when every session there is known to be idle or waiting. Otherwise it asks "Suspend <name>?" on the prompt,
//!   saying an agent there is working or could not be checked, with Cancel, where focus starts, and Suspend, as the old
//!   question did. Nothing is sent before Suspend is pressed; Cancel and Escape send nothing.
//! - **The question follows its host.** It closes when the registry drops the host, as the other host questions do,
//!   and neither opens nor sends once the host no longer offers Suspend (another viewer suspended it, its change runs,
//!   or its deletion started while the read or the question was out): the row shows the new phase. A read that answers
//!   after the user moved on opens nothing over what they turned to, and the row says so ([`MOVED_ON`]), as
//!   Uninstall's late plan does.
//! - **One request, on the host's key.** `POST /api/hosts/{id}/suspend` or `/resume`
//!   (`crate::session_list::environment::suspend_command` and `resume_command`, the commands a session's Suspend and
//!   Resume send) as the operation store's `Work::Suspend` or `Work::Resume` on the host's key, as the old panel held
//!   its own per-host request, and the row shows "Suspending…" or "Resuming…" meanwhile (`super::progress`). A
//!   session's Suspend and Resume run on that session's key, as the old session controls held theirs, so neither
//!   surface waits for the other's request, and the helm's lifecycle lock answers a press on one while the other's is
//!   out as busy. Nothing pushes the host's changing phase while this window's own request is out: the UI channel
//!   pushes nothing to a client while one of its commands is in flight, so the phase arrives with the answer. It is
//!   routine work, not live work (`super::status::live`), so the attention list and the rail show nothing for it, as
//!   the old ones did not.
//! - **The bound is routine.** The helm answers within its host reply bound, and a Tensorlake Resume routinely takes
//!   longer: its 504 that the change carries on is no failure, so its record is dropped and the row goes on showing the
//!   pushed phase, "Suspending…" or "Resuming…", until it settles.
//! - **A refusal** opens the host refusal dialog ([`super::refusal`], "Suspend host · <name>" or "Resume host ·
//!   <name>") through the mark of the press, as the old app showed it, and is then dropped from the store's record
//!   under the host's key; one that opened no dialog (the user moved on) stays under the row, which opens for it.
//!
//! NOTE: Waiting on the launcher's environment work, as the session's Suspend and Resume are: refusing them while this
//! window's own launch targets or is waking the same host (SPEC.md, "Creation"; the old `LAUNCH_ON_THIS_HOST`), since
//! the launcher exposes no predicate for its pending target yet. A launch elsewhere never holds them, and the helm's
//! lifecycle lock answers a collision on the same host as busy, which the refusal dialog shows.

use farhelm_proto::ui_channel::{HostStateView, HostView};
use gpui::{AnyElement, AnyWindowHandle, App, FocusHandle, IntoElement as _, SharedString, Window};

use super::identity::display_name;
use super::offers::{self, Lifecycle};
use super::{listed, refusal};
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::environment_ui::ProviderUi;
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::layers::Subject;
use crate::operations::{OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::session_list::environment::{self, CARRIES_ON};

/// What the Suspend question says under its title, after "An agent on <name>": why it asks, the old question's words.
const SUSPEND_ASKS: &str = " is working, or its status could not be checked. Suspend keeps it as it is and it \
                            continues when you resume the host.";

/// What a host's row says when its Suspend's activity read answered after the user moved on, so no question opened
/// (the module docs), in the words of Uninstall's late plan (`super::uninstall::MOVED_ON`).
pub(super) const MOVED_ON: &str = "Farhelm checked this host's sessions after you moved on. Choose Suspend again to \
                                   see the question.";

/// The refusal dialog's action for `verb`, the old app's.
fn action(verb: Lifecycle) -> &'static str {
    match verb {
        Lifecycle::Suspend => "Suspend host",
        Lifecycle::Resume => "Resume host",
    }
}

/// The hover help of `offered` on `host`, in its provider's words: Resume's says whether processes continue or the
/// machine starts again, Suspend's what freezing keeps and costs (`crate::environment_ui::ProviderUi`). `None` for a
/// host with no provider.
pub(super) fn help(host: &HostView, offered: Lifecycle) -> Option<&'static str> {
    let ui = ProviderUi(super::offers::HostKind::of(host).provider()?);
    Some(match offered {
        Lifecycle::Suspend => ui.suspend_help(),
        Lifecycle::Resume => ui.resume_help(matches!(host.state, HostStateView::Stopped { .. })),
    })
}

/// Host `host`'s inline Resume (`.host-resume`), or `None` where the row draws none: `offered` is the row's lifecycle
/// offer ([`super::offers::Offers::lifecycle`]), and only a Resume is inline, as in the old row. Unavailable while the
/// host is busy or its change runs. Pressing it moves focus to `toggle`, the row's menu toggle (the module docs).
pub(super) fn button(
    host: &HostView,
    offered: Option<(Lifecycle, bool)>,
    toggle: Option<FocusHandle>,
) -> Option<AnyElement> {
    let (Lifecycle::Resume, enabled) = offered? else {
        return None;
    };
    let help = help(host, Lifecycle::Resume)?;
    let id = host.id;
    let name = HostSentence::new("host-resume-name")
        .text("Resume ")
        .host(&display_name(host))
        .accessible_name();
    let button = Button::new(
        SharedString::from(format!("host-resume-{id}")),
        "Resume",
        HoverHelp::tooltip(help).ring_radius(crate::theme::metrics::RADIUS_SM - gpui::px(1.)),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .accessible_name(name)
    .disabled(!enabled)
    .on_click(move |_, window, cx| {
        let mark = Mark::take(cx);
        resume(id, mark, window, cx);
        if let Some(toggle) = &toggle {
            focus::focus_control(toggle, &mark, window, cx);
        }
    });
    Some(button.into_any_element())
}

/// Resume `host`, pressed with `mark` taken in its handler (the module docs): at once, with no question.
pub(super) fn resume(host: i64, mark: Mark, window: &mut Window, cx: &mut App) {
    send(host, Lifecycle::Resume, mark, window.window_handle(), cx);
}

/// Suspend `host`, pressed with `mark` taken in its handler (the module docs): read its activity, then send at once or
/// ask first. Nothing happens while the host no longer offers Suspend, when the window is inert or the registry no
/// longer lists the host.
pub(super) fn suspend(host: i64, mark: Mark, window: &mut Window, cx: &mut App) {
    if offering_suspend(host, cx).is_none() {
        return;
    }
    let handle = window.window_handle();
    environment::read_activity(host, cx, move |activity_asks, cx| {
        if !activity_asks {
            send(host, Lifecycle::Suspend, mark, handle, cx);
            return;
        }
        let asked = handle
            .update(cx, |_, window, cx| ask(host, &mark, window, cx))
            .unwrap_or(Asked::Not);
        if asked == Asked::MovedOn {
            moved_on(host, cx);
        }
    });
}

/// `host` as the registry lists it, while it still offers Suspend ([`super::offers::lifecycle_of`]): what the press,
/// the question and its answer check again, since the phase can change while the read or the question is out.
fn offering_suspend(host: i64, cx: &App) -> Option<HostView> {
    listed(host, cx).filter(|view| offers::lifecycle_of(view) == Some(Lifecycle::Suspend))
}

/// What [`ask`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    /// The question is open.
    Open,
    /// The user moved on since the press, so nothing opened over what they turned to.
    MovedOn,
    /// Nothing to ask: the host is gone or no longer offers Suspend, or the window is inert.
    Not,
}

/// Ask whether to suspend `host` (the module docs), over everything else in `window`, unless the user has moved on
/// since `mark` or the host no longer offers Suspend. The question closes if the registry drops the host.
fn ask(host: i64, mark: &Mark, window: &mut Window, cx: &mut App) -> Asked {
    let (Some(view), Some(store)) = (offering_suspend(host, cx), session::current_store(cx)) else {
        return Asked::Not;
    };
    let Some(provider) = super::offers::HostKind::of(&view).provider() else {
        return Asked::Not;
    };
    let ui = ProviderUi(provider);
    let name = display_name(&view);
    let title = HostSentence::new("host-suspend-title")
        .text("Suspend ")
        .host(&name)
        .text("?");
    let line = HostSentence::new("host-suspend-line")
        .text("An agent on ")
        .host(&name)
        .text(SUSPEND_ASKS);
    let question = Question::new(
        title,
        Answer::new(
            "Suspend",
            HoverHelp::tooltip(ui.host_suspend_help()),
            Tier::Primary,
        ),
        host,
    )
    .line(line)
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip(ui.suspend_cancel(None)),
        Tier::Neutral,
    ))
    .starts_on_cancel()
    .keeps_focus_on_empty_click();
    let key = HostView::key(host);
    let subject = Subject::when(store, move |store, _| super::settings::gone(store, &key));
    let opened = prompt::ask(
        question,
        Some(subject),
        mark,
        |host, window, cx| {
            // The answer's own mark: a refusal opens through it.
            let mark = Mark::take(cx);
            send(host, Lifecycle::Suspend, mark, window.window_handle(), cx);
        },
        window,
        cx,
    );
    match opened {
        Some(_) => Asked::Open,
        None => Asked::MovedOn,
    }
}

/// Say under `host`'s row that its Suspend's read answered after the user moved on ([`MOVED_ON`]), unless other work
/// holds the host's key now, which a note would outlive (`crate::operations::Operations::note`).
fn moved_on(host: i64, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let key = OpKey::host(&HostView::key(host));
    if operations.read(cx).in_flight(&key).is_some() {
        return;
    }
    operations.update(cx, |operations, cx| {
        operations.note(&key, Work::Suspend, HostText::from_host(MOVED_ON), cx)
    });
}

/// Send `verb` for `host` on its key, pressed with `mark` in `window` (the module docs): nothing when the window is
/// inert, the registry no longer lists the host, a Suspend is no longer offered, or work is already in flight on its
/// key.
fn send(host: i64, verb: Lifecycle, mark: Mark, window: AnyWindowHandle, cx: &mut App) {
    let current = match verb {
        Lifecycle::Suspend => offering_suspend(host, cx),
        Lifecycle::Resume => listed(host, cx),
    };
    if current.is_none() {
        return;
    }
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let built = match verb {
        Lifecycle::Suspend => environment::suspend_command(host),
        Lifecycle::Resume => environment::resume_command(host),
    };
    let command = match built {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "host Suspend or Resume not built");
            return;
        }
    };
    let key = OpKey::host(&HostView::key(host));
    let refused_key = key.clone();
    let action = action(verb);
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            let Outcome::Refused(refusal) = &settled.outcome else {
                return;
            };
            if refusal.status != CARRIES_ON {
                refusal::show_and_forget(host, action, refusal, &refused_key, &mark, window, cx);
            } else if let Some(operations) = session::current_operations(cx) {
                // The change carries on past the bound: the pushed phase shows it, not a failure.
                operations.update(cx, |operations, cx| operations.forget(&refused_key, cx));
            }
        })
    });
    if let Err(not_started) = started {
        tracing::debug!(?not_started, "host Suspend or Resume not sent");
    }
}
