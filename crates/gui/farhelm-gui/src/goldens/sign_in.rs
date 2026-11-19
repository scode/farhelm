//! SSH prompts over their own attention rows, using the production window owner and layer manager.

use crate::hosts::tests::{connected, host, hosts};
use crate::session_list::tests::{TEST_NOW, listing, preferences_state, session as row};
use crate::sign_in::tests::{questions, remote, startup_question};
use crate::{local_time, session};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{ConnectionStage, HostStateView, Preferences, SessionRow};
use gpui::{
    AnyElement, App, AppContext as _, IntoElement, ParentElement as _, Styled as _, Window, div, px,
};

use super::{SceneState, session_lists_at};

/// Two hosts share the frame. The left column pins both attention actions and the two key badges. The right pane
/// keeps the unavailable session card's Sign in visible under the question.
fn setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
    prompt: Option<&str>,
    notice: bool,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        listing(
            &["bench"],
            &[row("bench")
                .host(3)
                .host_name("bench-box")
                .stale()
                .activity(TEST_NOW - 900)
                .seen(Some(TEST_NOW - 900))
                .title("Benchmark the release build")],
        ),
        preferences_state(Preferences {
            welcome_seen: Some(true),
            ..Preferences::default()
        }),
        hosts(&[
            host(1, "local", connected()),
            remote(
                2,
                "build-box",
                HostStateView::Connecting {
                    attempt: 0,
                    stage: ConnectionStage::WaitingForSignIn,
                    last_error: None,
                },
            ),
            remote(3, "bench-box", HostStateView::SignInNeeded),
        ]),
        questions(
            &prompt
                .map(|prompt| startup_question("golden-question", 2, prompt, notice))
                .into_iter()
                .collect::<Vec<_>>(),
        ),
    ]);
    let owner = crate::sign_in::SignIn::attach(window, cx);
    state
        .lists
        .borrow_mut()
        .push(cx.new(crate::session_list::SessionList::new));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(owner));
}

/// A one-line verification code question with the field masked.
pub(super) fn sign_in_code(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, window, cx, Some("Verification code: "), false);
}

/// Preserve the menu's separate lines and keep each line isolated as host text.
pub(super) fn sign_in_duo(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(
        state,
        window,
        cx,
        Some(
            "Duo two-factor login\n\n1. Duo Push\n2. Phone call\n3. SMS passcode\nPasscode or option (1–3): ",
        ),
        false,
    );
}

/// Confirm that a long fingerprint and host-key question fit the popover.
pub(super) fn sign_in_host_key(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(
        state,
        window,
        cx,
        Some(
            "The authenticity of host 'build-box.example.invalid' can't be established.\nED25519 key fingerprint is SHA256:Q1z2A4nV3m7e1Xw5cBAuF6aRYl19bKeXk9srT87p2mA.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ",
        ),
        false,
    );
}

/// A security-key touch notice shows the instructions and only the Cancel action.
pub(super) fn sign_in_notice(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(
        state,
        window,
        cx,
        Some(
            "Confirm user presence for key ED25519-SK SHA256:v44lm31Y68LrXr10TTI0ihcMz4jX6XdS9kO01vUojWc",
        ),
        true,
    );
}

/// The stopped host's session shows its Sign in action even when there is no question to draw.
pub(super) fn sign_in_needed(state: &SceneState, window: &mut Window, cx: &mut App) {
    setup(state, window, cx, None, false);
}

pub(super) fn sign_in_surfaces(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    use crate::session_pane::unavailable::{self, Drawn, Notice};
    let notice = session::current_store(cx).and_then(|store| {
        let collections = store.read(cx).collections();
        let row = collections.get::<SessionRow>("bench")?;
        Some(Notice::of(row, collections, local_time::zone(cx)))
    });
    let card = notice.map(|notice| {
        unavailable::draw(
            Drawn {
                key: "bench",
                notice,
                focus: &state.ring,
                scroll: &gpui::ScrollHandle::new(),
                step: 0,
                alone: true,
            },
            window,
            cx,
        )
    });
    div()
        .flex()
        .gap(px(12.))
        .child(session_lists_at(state, px(490.), cx))
        .child(div().relative().w(px(530.)).h(px(490.)).children(card))
        .into_any_element()
}
