//! The user's attention to one session, and the blocked mark it clears.
//!
//! A session's agent can say it is blocked (`farhelm agent notify`), which marks the session blocked
//! (`store::session_blocked`) so that the session list reads it as waiting on the user. The mark clears when the user
//! attends to that session, by switching to it in an active window or by typing or clicking in its terminal, and when
//! the session goes: it ends or leaves its host's listing (`manager`), or it is stopped, restarted, replaced or deleted
//! (`sessions`). A question the supervisor detects on screen (`SessionStatus::Waiting`) is a separate thing that none
//! of this touches: it clears only when the supervisor sees the question go (SPEC.md, Status).
//!
//! This module holds the user's half: the attention route the desktop app calls, and the hook the terminal socket
//! calls for every input frame. Both also hand the attention to the ntfy alerts ([`crate::ntfy::Alerts::note_attention`]),
//! which is the only per-session evidence the alerts may take that the user is already looking.
//!
//! It also holds the ONE EXIT for a mark: every removal, whether the user's, a lifecycle path's, the host refresh's
//! ([`clear`]) or the per-host cap's eviction ([`cleared_marks`]), ends in [`cleared_marks`], so whatever must follow a
//! removal is written once and reaches all of them. The one exception is removing a host entry, whose rows go by the
//! foreign key's cascade without passing through here; anything hung on [`cleared_marks`] that keeps per-session state
//! must drop that host's sessions when the host goes as well.

use crate::feed::FleetEvents;
use crate::sessions::{resolve_owner, resolve_session_ref};
use crate::store::{HelmStore, SessionKey};
use crate::{AppState, http_error};
use axum::extract::{Path as AxPath, State};
use axum::response::IntoResponse;
use farhelm_proto::grid::MouseEventKind;
use farhelm_proto::{SessionAttention, SessionAttentionRequest};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// `POST /api/sessions/{id}/attention`: the desktop app saying the user attended to this session
/// ([`SessionAttentionRequest`]). A switch clears the session's blocked mark; an interaction does not. Both reach the
/// ntfy alerts' awareness hook.
///
/// A helm-local write like `PUT /api/sessions/{id}/seen`, so it does not route through the session's host: a session
/// on an unreachable host can still be switched to, and its mark cleared. The existence check is the same owner lookup
/// the seen route uses. Unlike the seen route it does not refuse a retired containerized session (SPEC.md, "Retired
/// containerized sessions"): such a session's terminal cannot be opened and it cannot be stopped, so switching to it
/// is the only way left to clear a mark its agent left, short of deleting it. Bumps the fleet-events revision only
/// when a mark was actually cleared, so the frequent signal that changes nothing wakes no client.
pub(crate) async fn attention(
    State(state): State<Arc<AppState>>,
    AxPath(id): AxPath<String>,
    axum::Json(request): axum::Json<SessionAttentionRequest>,
) -> impl IntoResponse {
    let key = match resolve_session_ref(&state, &id).await {
        Ok(key) => key,
        Err(error) => return http_error(error),
    };
    if let Err(error) = resolve_owner(&state, &key).await {
        return http_error(error);
    }
    state.ntfy.note_attention(&key);
    match request.kind {
        SessionAttention::Switch => {
            if let Err(error) = try_clear_blocked(&state, &key).await {
                return http_error(error);
            }
        }
        SessionAttention::Interaction => {}
    }
    axum::Json(serde_json::json!({})).into_response()
}

/// Just enough of a terminal input message to tell what produced it: its tag, and for a mouse report its kind and
/// button. Every other field (a paste's text, say) is skipped without being copied, so judging a frame costs one scan
/// of it however large it is.
#[derive(serde::Deserialize)]
struct InputShape {
    t: InputTag,
    #[serde(default)]
    kind: Option<MouseEventKind>,
    #[serde(default)]
    button: Option<i8>,
}

/// The `t` of a `farhelm_proto::grid::GridInput` message. An unknown tag does not decode, and counts for nothing, so
/// a new `GridInput` variant must be added here too; the test `every_grid_input_variant_has_a_known_tag`, whose match
/// over `GridInput` is exhaustive, stops compiling until it is.
#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum InputTag {
    Key,
    Text,
    Paste,
    Mouse,
    Focus,
}

/// Whether one terminal input frame is evidence that the user is attending to its session.
///
/// Keys, text, paste parts and mouse button reports (a press, a release, or motion with a button held, which is a
/// drag) count: each needs the user's hand on that terminal. Three kinds of frame do not:
///
/// - A focus report: the desktop app sends one by itself whenever the window gains or loses focus, or an attachment
///   goes live, while the program has asked for them.
/// - Mouse motion with no button held (`button` below 0), which a program in any-motion mode receives whenever the
///   pointer merely crosses the terminal.
/// - A wheel report.
///
/// The last two can reach a window that is not active, and this path cannot tell: the helm sees the frame, not the
/// window it came from. The ntfy alerts' awareness rule counts only activity in an active window, so the frames that
/// count here are those a user produces only in the window they are using. Scrolling in an open session is the
/// desktop app's to report, as an `interaction` through the attention route, where it can check that the window is
/// active. A frame that does not decode counts for nothing, which is also what the supervisor makes of it.
fn counts_as_attention(frame: &[u8]) -> bool {
    let Ok(shape) = serde_json::from_slice::<InputShape>(frame) else {
        return false;
    };
    match shape.t {
        InputTag::Key | InputTag::Text | InputTag::Paste => true,
        InputTag::Focus => false,
        InputTag::Mouse => match shape.kind {
            Some(MouseEventKind::Down | MouseEventKind::Up) => true,
            Some(MouseEventKind::Move) => shape.button.is_some_and(|button| button >= 0),
            Some(MouseEventKind::Wheel) | None => false,
        },
    }
}

/// Hand one input frame the terminal socket has just forwarded to one of `key`'s terminals to the attention rules.
///
/// Never waits, so the socket's input loop keeps forwarding keystrokes at once: it judges the frame
/// ([`counts_as_attention`]), tells the awareness hook, asks the store's in-memory gate whether the session may be
/// blocked, and only then hands the clear to a task of its own ([`Clears`] keeps that to one at a time per session,
/// and spaces out retries after a failure). The ordinary frame therefore costs one scan of its bytes and two memory
/// checks.
pub(crate) fn terminal_input(state: &Arc<AppState>, key: &SessionKey, frame: &[u8]) {
    if !counts_as_attention(frame) {
        return;
    }
    state.ntfy.note_attention(key);
    if !state.store.may_be_blocked(key) || !state.attention.begin(key, Instant::now()) {
        return;
    }
    let state = Arc::clone(state);
    let key = key.clone();
    tokio::spawn(async move {
        let result = try_clear_blocked(&state, &key).await;
        if let Err(error) = &result {
            tracing::warn!(
                host = key.host,
                session_id = crate::manager::peer_text(&key.session_id).as_str(),
                %error,
                "could not clear the session's blocked mark after its terminal input; trying again on later input"
            );
        }
        state.attention.finish(&key, result.is_ok(), Instant::now());
    });
}

/// How long after a failed clear terminal input waits before trying that session again. Without it a persistent store
/// failure would cost a database call and a warning per keystroke.
const CLEAR_RETRY: Duration = Duration::from_secs(5);

/// The terminal input path's clears, per session: one in flight (`None`) or the time the last one failed. A burst of
/// keystrokes into a blocked session therefore starts one clear, and a failing store is asked again at most every
/// [`CLEAR_RETRY`]. An entry goes when its clear succeeds, and a failure older than [`CLEAR_RETRY`] is dropped at the
/// next claim of any session (it would be granted anyway), so the map holds only clears in flight and failures from
/// the last few seconds.
///
/// One known gap, accepted because it heals on the next counted input: an input that arrives while a clear is in
/// flight is not claimed, so if the agent reports again in that window, after the in-flight clear's delete, the new
/// mark stays until the next key, text, paste or click.
///
/// Times are passed in rather than read here, so a test can step past [`CLEAR_RETRY`] without arithmetic on the
/// current time.
#[derive(Default)]
pub(crate) struct Clears(Mutex<HashMap<SessionKey, Option<Instant>>>);

impl Clears {
    /// Claim the clear for `key` at `now`, unless one is in flight or the last failed less than [`CLEAR_RETRY`]
    /// before `now`.
    fn begin(&self, key: &SessionKey, now: Instant) -> bool {
        let recent = |failed: &Instant| now.saturating_duration_since(*failed) < CLEAR_RETRY;
        let mut clears = self.0.lock().expect("attention clears poisoned");
        clears.retain(|_, failed| failed.as_ref().is_none_or(recent));
        match clears.get(key) {
            Some(None) => false,
            Some(Some(failed)) if recent(failed) => false,
            _ => {
                clears.insert(key.clone(), None);
                true
            }
        }
    }

    /// Release the claim [`Self::begin`] took, remembering a failure's time.
    fn finish(&self, key: &SessionKey, succeeded: bool, now: Instant) {
        let mut clears = self.0.lock().expect("attention clears poisoned");
        if succeeded {
            clears.remove(key);
        } else {
            clears.insert(key.clone(), Some(now));
        }
    }
}

/// Clear `key`'s blocked mark for a path whose own outcome does not depend on it (a Stop, a Delete): a failure is
/// logged and the caller carries on, since the worst it leaves is a session still shown as blocked until a later clear
/// succeeds.
pub(crate) async fn clear_blocked(state: &AppState, key: &SessionKey) {
    if let Err(error) = try_clear_blocked(state, key).await {
        tracing::warn!(
            host = key.host,
            session_id = crate::manager::peer_text(&key.session_id).as_str(),
            %error,
            "could not clear the session's blocked mark; it stays shown as blocked"
        );
    }
}

/// [`clear`] with the application state's store and feed.
async fn try_clear_blocked(state: &AppState, key: &SessionKey) -> anyhow::Result<()> {
    clear(&state.store, state.manager.events(), key).await?;
    Ok(())
}

/// Clear `key`'s blocked mark, returning whether there was one, and pass a removal on to [`cleared_marks`]. Every
/// clear goes through here; the host refresh, which has no application state, calls it with its own store and feed.
pub(crate) async fn clear(
    store: &HelmStore,
    events: &FleetEvents,
    key: &SessionKey,
) -> anyhow::Result<bool> {
    let cleared = store.clear_blocked(key).await?;
    if cleared {
        cleared_marks(events, std::slice::from_ref(key));
    }
    Ok(cleared)
}

/// The one exit for marks that have just left the table, whichever path removed them ([`clear`], or the per-host cap
/// evicting the oldest when an agent reports: `HelmStore::mark_blocked`'s `evicted`). It wakes clients, since every
/// such removal changes a listed row. Anything else that must follow a mark's removal, such as withdrawing an alert
/// still pending for the session, belongs here, so it reaches every exit at once. A host entry's removal is the one
/// path that does not come here (its rows cascade away in the database); see the module docs.
pub(crate) fn cleared_marks(events: &FleetEvents, keys: &[SessionKey]) {
    if !keys.is_empty() {
        events.bump();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rest_harness::{self, FleetBuilder, HostScript};
    use axum::http::{Request, StatusCode};
    use farhelm_proto::grid::GridInput;
    use tower::ServiceExt;

    /// A connected local host listing `sess-1`, with its served id for routes.
    async fn harness() -> (rest_harness::Harness, SessionKey, String) {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: vec![rest_harness::session("sess-1", 100)],
                ..HostScript::default()
            })
            .await
            .start()
            .await;
        let host = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(host).await;
        let served = harness.local_session_uri("/api/sessions/sess-1/attention");
        (harness, SessionKey::new(host, "sess-1"), served)
    }

    /// Send one attention signal through the real router, as the desktop app's channel does.
    async fn send(
        harness: &rest_harness::Harness,
        uri: &str,
        body: serde_json::Value,
    ) -> StatusCode {
        harness
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("host", "127.0.0.1:7433")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    /// Switching to a blocked session is the user seeing it, as opening a session reads its output, while a stray
    /// click or scroll inside it is not enough to say they read what the agent is blocked on. Specified: on a blocked
    /// session an `interaction` answers 200 and leaves the mark, a `switch` answers 200 and clears it, and a kind the
    /// helm does not know is refused without touching the mark.
    #[farhelm_testtrace::test]
    async fn a_switch_clears_the_mark_and_an_interaction_does_not() {
        let (harness, key, uri) = harness().await;
        assert!(
            harness
                .store
                .mark_blocked(&key)
                .await
                .unwrap()
                .newly_blocked
        );
        let blocked = || async {
            harness
                .store
                .blocked_sessions(std::slice::from_ref(&key))
                .await
                .unwrap()
                .contains(&key)
        };

        assert_eq!(
            send(&harness, &uri, serde_json::json!({"kind": "interaction"})).await,
            StatusCode::OK
        );
        assert!(blocked().await, "an interaction leaves the mark");
        assert!(
            send(&harness, &uri, serde_json::json!({"kind": "glance"}))
                .await
                .is_client_error()
        );
        assert!(blocked().await, "a refused signal leaves the mark");
        assert_eq!(
            send(&harness, &uri, serde_json::json!({"kind": "switch"})).await,
            StatusCode::OK
        );
        assert!(!blocked().await, "a switch clears the mark");
    }

    /// The attention route is a helm-local write, so it must refuse a session no host has reported rather than
    /// answer success for nothing. Specified: a switch to an unknown session answers 404.
    #[farhelm_testtrace::test]
    async fn attention_to_an_unknown_session_is_not_found() {
        let (harness, _key, _uri) = harness().await;
        let uri = harness.local_session_uri("/api/sessions/sess-missing/attention");
        assert_eq!(
            send(&harness, &uri, serde_json::json!({"kind": "switch"})).await,
            StatusCode::NOT_FOUND
        );
    }

    /// Wait, bounded, until exactly `expected` of `keys` are blocked. Every clear that removes a mark bumps the fleet
    /// feed, so the loop re-reads on each revision rather than polling.
    async fn await_blocked(
        harness: &rest_harness::Harness,
        keys: &[SessionKey],
        expected: &[&SessionKey],
    ) {
        let expected: std::collections::HashSet<SessionKey> =
            expected.iter().map(|key| (*key).clone()).collect();
        let mut revisions = harness.manager.events().subscribe();
        let reached = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                revisions.borrow_and_update();
                if harness.store.blocked_sessions(keys).await.unwrap() == expected {
                    return;
                }
                revisions.changed().await.unwrap();
            }
        })
        .await;
        if reached.is_err() {
            panic!(
                "blocked marks never became {expected:?}; they are {:?}",
                harness.store.blocked_sessions(keys).await.unwrap()
            );
        }
    }

    /// An agent that ended, or a session its host no longer has, is not blocked on anything, and only the host's
    /// listing says so for a session that went without the helm's help. But a listing cut at the supervisor's cap
    /// omits sessions that still exist, so absence from one proves nothing. Specified: with three blocked sessions on
    /// a connected host, a truncated listing that reports `a` exited and omits `c` clears `a` alone; a second
    /// truncated listing reporting `b` exited clears `b` and still leaves `c`; and a complete listing without `c`
    /// clears it.
    #[farhelm_testtrace::test]
    async fn a_refresh_clears_the_marks_of_ended_and_vanished_sessions_only() {
        let harness = FleetBuilder::new()
            .await
            .local(HostScript {
                identity: Some("identity-local".to_string()),
                sessions: ["a", "b", "c"]
                    .map(|id| rest_harness::session(id, 100))
                    .into(),
                ..HostScript::default()
            })
            .await
            .start()
            .await;
        let host = rest_harness::local_id(&harness.store).await;
        harness.await_refreshed(host).await;
        let keys = ["a", "b", "c"].map(|id| SessionKey::new(host, id));
        let [_, b, c] = &keys;
        for key in &keys {
            assert!(harness.store.mark_blocked(key).await.unwrap().newly_blocked);
        }
        let exited = |id: &str| {
            let id = id.to_string();
            move |script: &mut HostScript| {
                for session in &mut script.sessions {
                    if session.id == id {
                        session.status =
                            farhelm_proto::SessionStatus::Exited { exit_code: Some(0) };
                    }
                }
            }
        };

        harness.fleet.edit(host, |script| {
            script.truncated = true;
            script.sessions.retain(|session| session.id != "c");
            exited("a")(script);
        });
        harness.manager.refresh_now(host);
        await_blocked(&harness, &keys, &[b, c]).await;

        // The listing above is processed whole before the next one, so once
        // `b` clears, `c` has already been judged on the truncated listing.
        harness.fleet.edit(host, exited("b"));
        harness.manager.refresh_now(host);
        await_blocked(&harness, &keys, &[c]).await;

        harness.fleet.edit(host, |script| script.truncated = false);
        harness.manager.refresh_now(host);
        await_blocked(&harness, &keys, &[]).await;
    }

    /// Only input the user's own hand produced in that terminal may count as attention: the desktop app reports focus
    /// by itself, and pointer motion and wheel reports can reach a window the user is not using, which the ntfy
    /// alerts' awareness rule must not take as evidence. Specified: keys, text, paste parts, button presses and
    /// releases and drags count; focus reports, buttonless motion, wheel reports, an unknown tag and a frame that is
    /// not a grid message do not.
    #[farhelm_testtrace::test]
    fn only_the_users_own_terminal_input_counts_as_attention() {
        let frame = |input: GridInput| input.encode().unwrap();
        let mouse = |kind, button| GridInput::Mouse {
            kind,
            button,
            x: 1,
            y: 1,
            mods: 0,
        };
        for counted in [
            GridInput::Key {
                key: "y".into(),
                code: "KeyY".into(),
                kc: 89,
                mods: 0,
                mac: false,
            },
            GridInput::Text { data: "y".into() },
            GridInput::Paste {
                data: "x".repeat(farhelm_proto::grid::GRID_MAX_PASTE_PART_BYTES),
                last: true,
            },
            mouse(MouseEventKind::Down, 0),
            mouse(MouseEventKind::Up, 0),
            mouse(MouseEventKind::Move, 0),
        ] {
            assert!(counts_as_attention(&frame(counted.clone())), "{counted:?}");
        }
        for ignored in [
            GridInput::Focus { focused: true },
            mouse(MouseEventKind::Move, -1),
            mouse(MouseEventKind::Wheel, 1),
        ] {
            assert!(!counts_as_attention(&frame(ignored.clone())), "{ignored:?}");
        }
        assert!(!counts_as_attention(br#"{"t":"teleport"}"#));
        assert!(!counts_as_attention(b"not a grid message"));
    }

    /// A burst of keystrokes into a blocked session must start one clear, and a failing store must not be asked again
    /// for every keystroke, nor may old failures pile up. Specified: a second claim fails while the first is in
    /// flight; after a failure the next claim fails within the retry interval and succeeds past it; after a success a
    /// new claim is granted; and a failure older than the interval for another session is dropped by any claim.
    #[farhelm_testtrace::test]
    fn terminal_clears_run_one_at_a_time_and_space_out_retries() {
        let clears = Clears::default();
        let key = SessionKey::new(1, "s");
        let other = SessionKey::new(1, "other");
        let start = Instant::now();
        let later = |by: Duration| start + by;
        assert!(clears.begin(&key, start));
        assert!(!clears.begin(&key, start), "one clear in flight at a time");
        clears.finish(&key, false, start);
        assert!(
            !clears.begin(&key, later(Duration::from_secs(1))),
            "a failure is retried only after a pause"
        );
        assert!(
            clears.begin(&key, later(CLEAR_RETRY + Duration::from_secs(1))),
            "the pause over, the clear is tried again"
        );
        clears.finish(&key, true, start);
        assert!(clears.begin(&key, start), "a success leaves nothing behind");
        clears.finish(&key, true, start);

        assert!(clears.begin(&other, start));
        clears.finish(&other, false, start);
        assert!(clears.begin(&key, later(CLEAR_RETRY + Duration::from_secs(1))));
        assert!(
            !clears.0.lock().unwrap().contains_key(&other),
            "a stale failure for another session is dropped"
        );
    }

    /// A terminal input message whose tag `InputShape` does not know never counts as attention, so a new `GridInput`
    /// variant would go unnoticed silently. The exhaustive match below stops compiling when a variant is added, and
    /// the samples must then gain one. Specified: one sample of every variant decodes as `InputShape` with its tag.
    #[farhelm_testtrace::test]
    fn every_grid_input_variant_has_a_known_tag() {
        fn tag(input: &GridInput) -> InputTag {
            match input {
                GridInput::Key { .. } => InputTag::Key,
                GridInput::Text { .. } => InputTag::Text,
                GridInput::Paste { .. } => InputTag::Paste,
                GridInput::Mouse { .. } => InputTag::Mouse,
                GridInput::Focus { .. } => InputTag::Focus,
            }
        }
        for sample in [
            GridInput::Key {
                key: "y".into(),
                code: "KeyY".into(),
                kc: 89,
                mods: 0,
                mac: false,
            },
            GridInput::Text { data: "y".into() },
            GridInput::Paste {
                data: "y".into(),
                last: true,
            },
            GridInput::Mouse {
                kind: MouseEventKind::Down,
                button: 0,
                x: 0,
                y: 0,
                mods: 0,
            },
            GridInput::Focus { focused: true },
        ] {
            let shape: InputShape = serde_json::from_slice(&sample.encode().unwrap())
                .unwrap_or_else(|error| panic!("{sample:?} does not decode: {error}"));
            assert_eq!(shape.t, tag(&sample), "{sample:?}");
        }
    }

    /// Typing into a blocked session's terminal is the user answering it. The clear runs on a task of its own, so the
    /// test waits for it on the fleet feed. Specified: after a text frame, the session's mark leaves the table.
    #[farhelm_testtrace::test]
    async fn terminal_input_clears_the_mark() {
        let (harness, key, _uri) = harness().await;
        assert!(
            harness
                .store
                .mark_blocked(&key)
                .await
                .unwrap()
                .newly_blocked
        );
        let keys = [key.clone()];
        await_blocked(&harness, &keys, &[&key]).await;
        terminal_input(
            &harness.state,
            &key,
            &GridInput::Text {
                data: "y".to_string(),
            }
            .encode()
            .unwrap(),
        );
        await_blocked(&harness, &keys, &[]).await;
    }
}
