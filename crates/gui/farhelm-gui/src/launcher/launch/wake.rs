//! Manage a hidden launch's host wake in the launcher slot.
//!
//! The private Resume key belongs to one held draft. A delayed reply can clean up that key after the draft goes
//! away, and only the matching draft can continue to a create.
//!
//! Each wake also tags its Resume with a request tag of its own ([`request_tag`]). The helm bounds its reply at 20
//! seconds, and a Resume still running then only answers that it carries on; the helm publishes the tagged Resume's
//! terminal outcome on the host's entry in the hosts list instead (`HostView::wake_outcome`). [`follow`] reads that
//! entry so a late failure of this wake's own Resume brings the draft back at once with the helm's reason, rather
//! than after the shared minute with a generic one. The pushed host state stays the evidence for everything else.

use super::wake_state::{Advance, Phase, ResumeWait};
use super::{Held, LaunchSlot, Slot, Waiting, hold, reveal_prepared};
use crate::environment_ui::{ProviderUi, WakeWords};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::hosts::offers::HostKind;
use crate::launcher::draft::{Install, LaunchDraft, Mode};
use crate::launcher::pill::WakePill;
use crate::launcher::view::{Launcher, SourceConsent};
use crate::layers::{self, PillId};
use crate::operations::{Command, HelmMarker, OpKey, Outcome, Settled, Work};
use crate::session;
use crate::session_list::environment::CARRIES_ON;
use crate::ticker;
use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::{HostStateView, HostView, WakeResult};
use gpui::{AnyWindowHandle, App, AppContext as _, Context, Entity, Window};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

/// A user-authorized preparation and its claim on the existing host. The claim lasts through the folder-default or
/// preview wait after connection so another action cannot freeze the host before the create is sent.
pub(super) struct Wake {
    pub(super) install: Install,
    pub(super) source: Option<String>,
    key: OpKey,
    /// The request tag every Resume this wake sends carries, a busy retry's included. It names the wake rather than
    /// one request because only one of its Resumes can end in an outcome the helm records: every earlier one was a
    /// busy refusal, which the helm never records.
    request: String,
    wait: ResumeWait,
    window: AnyWindowHandle,
    mark: Mark,
    consent: SourceConsent,
    /// A YOLO answer already checked against `install`. Readiness keeps checking that same installation.
    confirm_yolo: bool,
    /// The provider's words for this wake, chosen once from whether the host was Stopped at the press, so a stopped
    /// container's wake says Starting from its pill to its last refusal.
    words: WakeWords,
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
    let request = request_tag(cx);
    if !send(host.id, &key, &request, cx) {
        return false;
    }
    let stopped = matches!(host.state, HostStateView::Stopped { .. });
    let words = ProviderUi(provider).wake_words(stopped);
    let name = display_name(host);
    let pill = cx.new(|_| WakePill::new(name, words.status));
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
                request,
                window: window.window_handle(),
                mark,
                consent,
                confirm_yolo,
                words,
                alarm: None,
                pill: shown,
                ending: false,
            }),
        }),
        cx,
    );
    true
}

/// A request tag for one wake's Resume, unique among the tags any launcher sends to one helm while it runs.
///
/// It needs no randomness. The helm keeps only the latest tagged outcome per host, and a launcher compares that outcome
/// only with its own wake's tag, so a tag has to differ from every other tag that could be the latest on the same host
/// while this wake waits, and nothing more: nobody guesses tags, and the helm grants nothing for knowing one. The
/// process-wide counter separates this app's wakes from one another (every window shares it), and the process id with
/// the time the first tag was minted separates this app from another desktop app, or a restart of this one, talking to
/// the same helm. `farhelm-gui` has no random-number dependency, and this does not justify one.
///
/// The time is [`ticker::wall_now`], the crate's one wall clock, so in tests it is GPUI's simulated clock and the same
/// in every run; the counter alone keeps a test's tags apart.
fn request_tag(cx: &App) -> String {
    static STARTED: OnceLock<u128> = OnceLock::new();
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let started = *STARTED.get_or_init(|| {
        ticker::wall_now(cx)
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    });
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("launcher-{}-{started}-{n}", std::process::id())
}

/// The launcher's own Resume of `host`, `POST /api/hosts/{id}/resume` with `{"request": request}`, as [`Work::Resume`].
/// The body is [`farhelm_proto::ui_channel::ResumeHostBody`]'s spelling, written out because serializing the type
/// would add a failure path to a command that has none; the wake tests decode it as that type.
///
/// The shared `session_list::environment::resume_command` sends no body, and the Hosts panel and the session controls
/// keep it that way: they read the pushed host state for a Resume's result and have no wake to end early, so a tag
/// would only make the helm record an outcome nobody reads. Only the launcher tags its Resume, because only the
/// launcher holds a draft hostage to that Resume's outcome (see [`follow`]).
fn tagged_resume_command(host: i64, request: &str) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Resume,
        Method::Post,
        ApiPath::api(&["hosts", &host.to_string(), "resume"])?,
        Some(serde_json::json!({ "request": request })),
        "POST /api/hosts/[host]/resume",
    ))
}

/// Send or retry this wake's tagged Resume command. Only the held wake interprets its reply.
fn send(host: i64, key: &OpKey, request: &str, cx: &mut App) -> bool {
    let (Ok(command), Some(operations)) = (
        tagged_resume_command(host, request),
        session::current_operations(cx),
    ) else {
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
        Outcome::Refused(reason) => Err(reason.words.clone()),
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
            Err(reason) => {
                let reason = refusal(&wake.words, &reason);
                reject(held, reason, cx)
            }
        }
    });
}

/// The alert for a Resume this wake sent that the helm refused or that failed: the provider's lead, then the helm's
/// own reason, which may quote the provider and so stays host text. A refusal in the reply and a late failure the helm
/// publishes under the wake's tag read alike, since the helm gives both the same text.
fn refusal(words: &WakeWords, reason: &HostText) -> HostSentence {
    HostSentence::new("launcher-wake-refusal")
        .text(words.failed_lead)
        .host(reason)
}

/// The helm's reason when `host` lists a failed outcome of the Resume tagged `request`, else `None`.
///
/// Any other tag is ignored, whatever its result: it belongs to an earlier wake, another window's wake, or another
/// app's (the contract fixture's host carries one), and says nothing about whether this wake's own Resume failed. A
/// `Ready` outcome under this wake's tag ends nothing either: it means only that the provider finished, and the wait
/// for a fresh connection, with its minute, goes on from the pushed host state as before tags existed.
fn own_failure(host: &HostView, request: &str) -> Option<HostText> {
    let outcome = host.wake_outcome.as_ref()?;
    if outcome.request != request {
        return None;
    }
    match &outcome.outcome {
        WakeResult::Failed { reason } => Some(HostText::from_host(reason.as_str())),
        WakeResult::Ready => None,
    }
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
    // This wake's own Resume failed, perhaps after its reply had only said it carries on, before the provider took
    // its hold, or with the same diagnosis the host already showed: none of those leaves a pushed state that tells
    // this wake its Resume failed, so the helm's tagged outcome decides at once. Busy refusals are never recorded,
    // so this cannot cut short the busy retry, and a Ready wake has already seen its connection.
    let failed = host
        .filter(|host| Install::of(host) == wake.install && wake.wait.phase != Phase::Ready)
        .and_then(|host| own_failure(host, &wake.request));
    if let Some(reason) = failed {
        let reason = refusal(&wake.words, &reason);
        reject(held, reason, cx);
        return;
    }
    let now = cx.background_executor().now();
    let advance = match host {
        None => Advance::Fail(wake.words.removed),
        Some(host) if Install::of(host) != wake.install => Advance::Fail(
            "The host now points to a different installation; choose it again before launching",
        ),
        Some(host) if wake.wait.phase == Phase::Ready => {
            if matches!(host.state, HostStateView::Connected { .. }) {
                return;
            }
            Advance::Fail("The host became unavailable before the session could launch")
        }
        Some(host) => wake.wait.observe(&host.state, now, &wake.words),
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
            let request = wake.request.clone();
            let unasked = wake.words.unasked;
            let host = wake.install.host;
            cx.defer(move |cx| {
                if !is_current(&key, cx) {
                    return;
                }
                if !send(host, &key, &request, cx) {
                    update(&key, cx, |slot, cx| {
                        if let Some(held) = slot.held.current(cx).as_mut() {
                            reject(held, failure_words(unasked), cx);
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
