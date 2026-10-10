//! Device attention follows the sidebar's accepted view and fleet approval data.
//!
//! The authenticated mount owns history: reconnecting the feed preserves it,
//! while a fresh mount of the authenticated tree (a browser sign-in) starts
//! from a silent baseline. A desktop re-sign-in runs under the live tree and
//! keeps history, so changes made meanwhile can still sound. A different sidebar
//! filter reseeds status history without forgetting seen approval requests.
//! The ordinary surface reader serializes approval reads and retries failures.
//! JavaScript owns device switches, window activity and best-effort Web Audio.

use std::collections::{BTreeMap, HashSet};

use dioxus::prelude::*;
use serde::Serialize;

use crate::api::{SessionListing, fetch_approvals};
use crate::feed::{fallback_polls_now, fallback_sleep};
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::{ApiBase, SessionStatus};

/// Remember observed statuses and every approval id for this authenticated mount.
/// Approval removal does not erase its identity: an already-seen request must
/// stay quiet even if a later listing presents it again.
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct History {
    statuses: BTreeMap<String, String>,
    approval_ids: HashSet<String>,
    /// The accepted view these statuses describe; sorting retains its identity.
    #[serde(skip)]
    scope: Option<u64>,
}

/// Preserve the first accepted statuses of a view even while approvals lag.
///
/// A changed filter is silent at the moment its rows arrive. Later transitions
/// within that view must still ring if an approval read spans both replies.
/// The generation also distinguishes returning to A after B was shown.
#[derive(Clone, Default)]
pub(crate) struct StatusScope {
    generation: u64,
    baseline: BTreeMap<String, String>,
}

impl StatusScope {
    /// Called only when the sidebar accepts a successful reply to a new filter.
    pub(crate) fn advance(&mut self, listing: &SessionListing) {
        self.generation += 1;
        self.baseline = status_words(listing);
    }
}

/// Exit details and other row data cannot raise attention; only status edges do.
fn status_words(listing: &SessionListing) -> BTreeMap<String, String> {
    listing
        .sessions
        .iter()
        .map(|session| {
            let word = match session.status {
                SessionStatus::Running => "running",
                SessionStatus::Waiting => "waiting",
                SessionStatus::Idle => "idle",
                _ => "other",
            };
            (session.id.clone(), word.to_string())
        })
        .collect()
}

/// Observe only accepted sidebar statuses, while approvals remain fleet-wide.
///
/// A successful approval read pairs with the current sidebar evidence for one
/// priority decision. A filter change silently establishes its status baseline;
/// failures and superseded sidebar replies cannot manufacture transitions.
#[component]
pub(crate) fn SessionSounds(
    selected: ReadSignal<Option<String>>,
    listing: Signal<Option<Result<SessionListing, String>>>,
    listing_scope: Signal<StatusScope>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut history = use_signal(|| None::<History>);
    let read = move || {
        let base = base.clone();
        async move {
            let Ok(approvals) = fetch_approvals(&base).await else {
                return false;
            };
            // Keep only the status word across the JS boundary; exit details
            // and other session data cannot produce an attention event.
            let before = history.peek().clone();
            let visible = listing.peek();
            let evidence = visible.as_ref().and_then(|reply| reply.as_ref().ok());
            let statuses: BTreeMap<String, String> =
                evidence.map(status_words).unwrap_or_else(|| {
                    before
                        .as_ref()
                        .map(|history| history.statuses.clone())
                        .unwrap_or_default()
                });
            let scope = listing_scope.peek();
            let current_scope = evidence.map(|_| scope.generation);
            // Newly visible rows are context, not newly raised attention. Keep
            // approval identity independent so changing hosts cannot replay it.
            let mut comparison = before.clone().unwrap_or_default();
            if before.is_none() {
                // Approval baseline starts with its first successful read;
                // status baseline starts when the sidebar first accepts rows.
                // Either endpoint may finish first without silencing a later
                // status edge or replaying an existing approval.
                comparison
                    .approval_ids
                    .extend(approvals.iter().map(|request| request.id.clone()));
            }
            if current_scope
                .as_ref()
                .is_some_and(|scope| comparison.scope.as_ref() != Some(scope))
            {
                comparison.statuses = scope.baseline.clone();
            }
            drop(visible);
            drop(scope);
            let current = serde_json::json!({
                "statuses": statuses,
                "approvals": approvals.iter().map(|request| serde_json::json!({
                    "id": request.id, "session": request.session.id,
                })).collect::<Vec<_>>(),
            });
            let before_json =
                serde_json::to_string(&comparison).expect("sound history is JSON data");
            let selected_json =
                serde_json::to_string(&*selected.peek()).expect("session selection is JSON data");
            // Asset loading can trail the component's first read. Await its
            // registration within this serialized read, rather than dropping
            // the first baseline or letting later decisions overtake it.
            // Timers still run in hidden tabs; an animation-frame wait would
            // suspend attention detection while the user is elsewhere.
            let _ = document::eval(&format!(
                "while (!window.farhelmSounds) {{ await new Promise(r => setTimeout(r, 50)); }}\nwindow.farhelmSounds.observe({before_json}, {current}, {selected_json});"
            )).await;
            let mut remembered = before.unwrap_or_default();
            remembered.statuses = statuses;
            if current_scope.is_some() {
                remembered.scope = current_scope;
            }
            remembered
                .approval_ids
                .extend(approvals.into_iter().map(|request| request.id));
            // Silent playback refusal still consumes the transition. A user
            // gesture or setting change must never replay old attention.
            history.set(Some(remembered));
            true
        }
    };
    let surface = use_signal(SurfaceReader::default);
    let request = move |trigger| request_read(surface, trigger, read.clone());
    let mount = request.clone();
    use_hook(move || mount(Trigger::Explicit));
    let notice = request.clone();
    let acted_on = crate::feed::use_feed_reader(move || notice(Trigger::Notice));
    let changed_listing = request.clone();
    use_effect(move || {
        // Only the existing commit gate writes these inputs. Notice retains
        // the unattended build-skew boundary even after an explicit list read.
        let _ = listing.read();
        let _ = listing_scope.read();
        changed_listing(Trigger::Notice);
    });
    // Network completion does not establish retirement: a coalesced notice
    // can still owe a read. Feed tests observe this consumer alongside the
    // sidebar before starting a window that must contain no requests.
    crate::reader::use_test_reader_snapshot(move || {
        serde_json::json!({
            "role": "sounds",
            "acted_on": acted_on.peek().to_string(),
            "readers": [surface.peek().test_snapshot()],
            "baseline": history.peek().is_some(),
            "selected": &*selected.peek(),
        })
    });
    use_future(move || {
        let request = request.clone();
        async move {
            loop {
                fallback_sleep().await;
                if fallback_polls_now() {
                    request(Trigger::Scheduled);
                }
            }
        }
    });
    rsx! {}
}

/// Device switches keep native label and keyboard behavior like host settings.
/// Storage and the live checked properties belong to the audio asset; these
/// controls deliberately send no helm preference writes.
#[component]
pub(crate) fn SoundSettings() -> Element {
    rsx! {
        section {
            aria_label: "sounds",
            onmounted: move |_| {
                spawn(async move {
                    let _ = document::eval("while (!window.farhelmSounds) { await new Promise(r => setTimeout(r, 50)); } window.farhelmSounds.mountSettings();").await;
                });
            },
            h3 { "sounds" }
            p { class: "host-settings-help", "These choices apply only to this device. The session open in an active window stays quiet." }
            for (kind, label, default, help) in [
                ("waiting", "session waiting", true, "play a bell when a session starts waiting for your answer"),
                ("approval", "approval request", true, "play a ding-dong when an agent asks Farhelm to do something that needs your approval"),
                ("finished", "turn finished", false, "play a soft pluck when an agent goes from running to idle"),
            ] {
                label { class: "app-settings-choice",
                    "data-tooltip": help,
                    span { "{label}" }
                    input { r#type: "checkbox", class: "host-settings-switch", checked: default, "data-sound-event": kind }
                }
            }
        }
    }
}
