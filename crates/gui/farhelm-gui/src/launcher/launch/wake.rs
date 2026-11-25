//! Manage a hidden launch's host wake in the launcher slot.
//!
//! The private Resume key belongs to one held draft. A delayed reply can clean up that key after the draft goes
//! away, and only the matching draft can continue to a create.

use super::wake_state::{Advance, Phase, ResumeWait};
use super::{Held, LaunchSlot, Slot, Waiting, hold, reveal_prepared};
use crate::environment_ui::ProviderUi;
use crate::focus::Mark;
use crate::host_text::HostSentence;
use crate::hosts::identity::display_name;
use crate::hosts::offers::HostKind;
use crate::launcher::draft::{Install, LaunchDraft, Mode};
use crate::launcher::pill::WakePill;
use crate::launcher::view::{Launcher, SourceConsent};
use crate::layers::{self, PillId};
use crate::operations::{HelmMarker, OpKey, Outcome, Settled};
use crate::session;
use crate::session_list::environment::{CARRIES_ON, resume_command};
use crate::ticker;
use farhelm_proto::ui_channel::{HostStateView, HostView};
use gpui::{AnyWindowHandle, App, AppContext as _, Context, Entity, Window};

/// A user-authorized preparation and its claim on the existing host. The claim lasts through the folder-default or
/// preview wait after connection so another action cannot freeze the host before the create is sent.
pub(super) struct Wake {
    pub(super) install: Install,
    pub(super) source: Option<String>,
    key: OpKey,
    wait: ResumeWait,
    window: AnyWindowHandle,
    mark: Mark,
    consent: SourceConsent,
    /// A YOLO answer already checked against `install`. Readiness keeps checking that same installation.
    confirm_yolo: bool,
    words: ProviderUi,
    started_stopped: bool,
    alarm: Option<ticker::Deadline>,
    pill: Option<PillId>,
    ending: bool,
}

impl Wake {
    /// The create owns the preparation pill until the ordinary slow-create pill replaces it or the reply arrives.
    pub(super) fn take_pill(&mut self) -> Option<PillId> {
        self.pill.take()
    }

    /// The existing installation must still be connected when a delayed preparation callback submits the create.
    pub(super) fn allows_create(&self, cx: &App) -> bool {
        if self.ending || self.wait.phase != Phase::Ready {
            return false;
        }
        session::current_store(cx).is_some_and(|store| {
            store
                .read(cx)
                .collections()
                .get::<HostView>(&HostView::key(self.install.host))
                .is_some_and(|host| {
                    Install::of(host) == self.install
                        && matches!(host.state, HostStateView::Connected { .. })
                })
        })
    }
}

/// Hide a launch only after its first Resume can be sent. The pending create key is separate so a refused Resume
/// cannot show as the future session's create failure or leave a false error under Hosts.
#[expect(
    clippy::too_many_arguments,
    reason = "the wake owns the pressed launch and its consent"
)]
pub(in crate::launcher) fn start(
    launcher: Entity<Launcher>,
    draft: &LaunchDraft,
    host: &HostView,
    confirm_yolo: bool,
    consent: SourceConsent,
    mark: Mark,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(provider) = HostKind::of(host).provider() else {
        return false;
    };
    let key = OpKey::new_launch();
    if !send(host.id, &key, cx) {
        return false;
    }
    let stopped = matches!(host.state, HostStateView::Stopped { .. });
    let name = display_name(host);
    let pill = cx.new(|_| WakePill::new(name, stopped));
    let shown = layers::show_launch_pill(pill.into(), window, cx);
    let source = match &draft.mode {
        Mode::ReplaceWith { source } => Some(source.clone()),
        Mode::RestartWith(restart) => Some(restart.source.clone()),
        Mode::New | Mode::Clone => None,
    };
    hold(
        Some(Held {
            launcher,
            waiting: Waiting::Preparing,
            progress: None,
            run: None,
            wake: Some(Wake {
                install: Install::of(host),
                source,
                wait: ResumeWait::new(
                    &host.state,
                    cx.background_executor().now(),
                    draft.checkout.is_some(),
                ),
                key,
                window: window.window_handle(),
                mark,
                consent,
                confirm_yolo,
                words: ProviderUi(provider),
                started_stopped: stopped,
                alarm: None,
                pill: shown,
                ending: false,
            }),
        }),
        cx,
    );
    true
}

/// Send or retry the ordinary Resume command. Only the held wake interprets its reply.
fn send(host: i64, key: &OpKey, cx: &mut App) -> bool {
    let (Ok(command), Some(operations)) = (resume_command(host), session::current_operations(cx))
    else {
        return false;
    };
    let settled_key = key.clone();
    operations
        .update(cx, |operations, cx| {
            operations.start(key, command, cx, move |settled, cx| {
                reply(&settled_key, settled, cx)
            })
        })
        .is_ok()
}

/// Clear the launcher-private operation result even if the preparation has already failed on a pushed host change.
fn reply(key: &OpKey, settled: &Settled, cx: &mut App) {
    if let Some(operations) = session::current_operations(cx) {
        operations.update(cx, |operations, cx| operations.forget(key, cx));
    }
    let outcome = match &settled.outcome {
        Outcome::Done(_) | Outcome::Accepted(_) => Ok(false),
        Outcome::Refused(reason) if reason.has(HelmMarker::HostBusy) => Ok(true),
        Outcome::Refused(reason) if reason.status == CARRIES_ON && reason.markers.is_empty() => {
            Ok(false)
        }
        Outcome::Refused(reason) => Err(HostSentence::new("launcher-wake-refusal")
            .text("Resuming the host failed: ")
            .host(&reason.words)),
    };
    update(key, cx, |slot, cx| {
        let Some(held) = slot.held.current(cx).as_mut() else {
            return;
        };
        let Some(wake) = held.wake.as_mut() else {
            return;
        };
        match outcome {
            Ok(busy) => {
                wake.wait.replied(busy, cx.background_executor().now());
                follow(slot, cx);
            }
            Err(reason) => reject(held, reason, cx),
        }
    });
}

/// Run a callback only while the same preparation is still active.
fn update(
    key: &OpKey,
    cx: &mut App,
    callback: impl FnOnce(&mut LaunchSlot, &mut Context<LaunchSlot>),
) {
    let Some(slot) = cx.try_global::<Slot>().map(|slot| slot.0.clone()) else {
        return;
    };
    slot.update(cx, |slot, cx| {
        if slot
            .held
            .current(cx)
            .as_ref()
            .and_then(|held| held.wake.as_ref())
            .is_some_and(|wake| !wake.ending && &wake.key == key)
        {
            callback(slot, cx);
        }
    });
}

/// Reevaluate on the actual pushed state and on the deadline. Merely changing a host's description or refreshing a
/// session list cannot extend either the resend time or the connection budget.
pub(super) fn follow(slot: &mut LaunchSlot, cx: &mut Context<LaunchSlot>) {
    let Some(held) = slot.held.current(cx).as_mut() else {
        return;
    };
    let Some(wake) = held.wake.as_mut().filter(|wake| !wake.ending) else {
        return;
    };
    let store = session::current_store(cx);
    let host = store.as_ref().and_then(|store| {
        store
            .read(cx)
            .collections()
            .get::<HostView>(&HostView::key(wake.install.host))
    });
    let now = cx.background_executor().now();
    let advance = match host {
        None => Advance::Fail("The host was removed while resuming"),
        Some(host) if Install::of(host) != wake.install => Advance::Fail(
            "The host now points to a different installation; choose it again before launching",
        ),
        Some(host) if wake.wait.phase == Phase::Ready => {
            if matches!(host.state, HostStateView::Connected { .. }) {
                return;
            }
            Advance::Fail("The host became unavailable before the session could launch")
        }
        Some(host) => wake.wait.observe(
            &host.state,
            now,
            wake.words.wake_failed(wake.started_stopped),
        ),
    };
    wake.alarm = None;
    match advance {
        Advance::Wait(Some(after)) => {
            let key = wake.key.clone();
            wake.alarm = Some(ticker::after(after, cx, move |cx| update(&key, cx, follow)));
        }
        Advance::Wait(None) => {}
        Advance::Retry => {
            wake.wait.again(now);
            let key = wake.key.clone();
            let host = wake.install.host;
            cx.defer(move |cx| {
                if !is_current(&key, cx) {
                    return;
                }
                if !send(host, &key, cx) {
                    update(&key, cx, |slot, cx| {
                        if let Some(held) = slot.held.current(cx).as_mut() {
                            reject(
                                held,
                                failure_words("Farhelm could not ask the host to resume"),
                                cx,
                            );
                        }
                    });
                }
            });
        }
        Advance::Ready => {
            let launcher = held.launcher.clone();
            let (window, mark, consent) = (wake.window, wake.mark, wake.consent);
            let yolo_consent = wake.confirm_yolo.then(|| wake.install.clone());
            let key = wake.key.clone();
            cx.defer(move |cx| {
                if !is_current(&key, cx) {
                    return;
                }
                let waiting = cx.update_window(window, |_, window, cx| {
                    launcher.update(cx, |launcher, cx| {
                        launcher.continue_after_wake(yolo_consent, consent, mark, window, cx)
                    })
                });
                if !matches!(waiting, Ok(true)) {
                    reveal_prepared(launcher, window, mark, cx);
                }
            });
        }
        Advance::Fail(words) => reject(held, failure_words(words), cx),
    }
}

fn failure_words(reason: &'static str) -> HostSentence {
    HostSentence::new("launcher-wake-refusal").text(reason)
}

/// Take the source and destination holds down on a failed preparation before showing the launcher's usual refusal.
fn reject(held: &mut Held, reason: HostSentence, cx: &mut Context<LaunchSlot>) {
    let Some(wake) = held.wake.as_mut() else {
        return;
    };
    wake.ending = true;
    wake.alarm = None;
    let launcher = held.launcher.clone();
    let (window, mark) = (wake.window, wake.mark);
    cx.defer(move |cx| {
        if !super::still_preparing(&launcher, cx) {
            return;
        }
        launcher.update(cx, |launcher, cx| launcher.wake_refused(reason, cx));
        reveal_prepared(launcher, window, mark, cx);
    });
}

/// Narrow callback witnesses for requests and deadlines that belonged to a particular wake and a current UI session.
fn is_current(key: &OpKey, cx: &App) -> bool {
    super::current_held(cx)
        .and_then(|held| held.wake.as_ref())
        .is_some_and(|wake| &wake.key == key && !wake.ending)
}

/// Remove only this preparation's capsule when replacing or revealing its draft.
pub(super) fn dismiss(wake: Wake, cx: &mut App) {
    if let Some(pill) = wake.pill {
        let _ = cx.update_window(wake.window, |_, window, cx| {
            layers::hide_pill(pill, window, cx)
        });
    }
}
