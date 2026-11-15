//! Tests of the host menu's Retry and Adopt over the helm client's fake: what each sends under which key, the identity
//! Adopt carries, where a refusal shows, the launch refusal, and fixing the destination instead through Settings.
//!
//! Every test starts from the Hosts panel's harness (`hosts/tests.rs`) in a column of the sidebar's width and a window
//! as large as the reference screens', and every step ends in one of the shared harness's step helpers, so the focus
//! and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, HostStateView, HostView, KeyedEntry,
};
use gpui::{TestAppContext, VisualTestContext, px, size};
use serde_json::{Value, json};

use crate::hosts::LAUNCH_IN_FLIGHT;
use crate::hosts::tests::{hosts, open_panel_in_column, reference_hosts, start_work};
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{any_open, click, focused_id, keys, label, meta, one_command, settle};

/// The identity a mismatched `build-box` reports, as the menu shows it.
const REPORTED: &str = "c0ffee00-0000-4000-8000-000000000002";

/// The reference hosts with `build-box` in `state`.
fn listed_with(state: HostStateView) -> Vec<HostView> {
    let mut listed = reference_hosts();
    listed[1].state = state;
    listed
}

/// `build-box` recorded as one install and reporting `reported`.
fn mismatch(reported: &str) -> HostStateView {
    HostStateView::IdentityMismatch {
        recorded: "c1182bd3-a604-4d1e-855d-ccbf6c992b0b".into(),
        reported: reported.into(),
    }
}

/// The panel over `listed`, in a column and a window as large as the reference screens'.
fn panel<'a>(
    cx: &'a mut TestAppContext,
    listed: &[HostView],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (helm, _, cx) = open_panel_in_column(cx, vec![hosts(listed)]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    settle(cx);
    (helm, cx)
}

/// The work in flight on `build-box`'s key.
fn work(cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)?
            .read(cx)
            .in_flight(&OpKey::host("2"))
    })
}

/// SPEC.md, "Topology": an entry that reaches a machine another entry holds asks the user to remove that entry or
/// change this one's destination "and then press Retry"; the old menu offered Retry in every phase, first, as the
/// button it replaced. Specified: a duplicate `build-box`'s menu starts on Retry ("Try the connection again"); choosing
/// it closes the menu and sends `POST /api/hosts/2/retry` without a body under the host's key; the helm's empty answer
/// opens nothing and leaves focus on the row's toggle.
#[gpui::test]
fn retry_sends_one_attempt_under_the_hosts_key(cx: &mut TestAppContext) {
    let duplicate = HostStateView::Duplicate {
        twin: 3,
        identity: "df06c6e5-a058-4377-93bb-541c9e68e940".into(),
    };
    let (mut helm, cx) = panel(cx, &listed_with(duplicate));
    click(cx, "host-menu-toggle-2");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));
    assert_eq!(label(cx, "host-menu-retry").as_deref(), Some("Retry"));
    click(cx, "host-menu-retry");
    assert!(!any_open(cx), "the menu closed");
    let (sent, body) = one_command(&mut helm, Method::Post, "/api/hosts/2/retry");
    assert_eq!(body, Value::Null);
    assert_eq!(work(cx), Some(Work::RetryHost));
    helm.reply(sent, 200, Some(json!({})));
    settle(cx);
    assert_eq!(work(cx), None);
    assert!(
        !any_open(cx),
        "nothing to show: the pushed host list carries the outcome"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
}

/// The old "identity-mismatch-surfaced" and "adopt-requires-current-identity": the decision is between the two
/// identities, the item names the one adopting accepts, and the request carries that very identity, so the helm can
/// refuse an approval of an identity the host no longer reports instead of silently adopting a third install; the
/// refusal belongs in a focused dialog, not a row line. Specified: a mismatched `build-box`'s menu offers Adopt between
/// Retry and Update, as Down walks them (the old keyboard contract), whose line reads "Accept identity" and the
/// reported id; choosing it sends `POST /api/hosts/2/adopt` with
/// `{"reported": <that id>}` under the host's key; the helm's refusal opens "Adopt host · build-box" with its words and
/// Close focused, and nothing shows under the row.
#[gpui::test]
fn adopt_sends_the_identity_it_showed_and_a_refusal_opens_the_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, &listed_with(mismatch(REPORTED)));
    click(cx, "host-menu-toggle-2");
    assert_eq!(label(cx, "host-menu-adopt").as_deref(), Some("Adopt"));
    // The old order: Retry, then Adopt, then Update, as Down walks them.
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));
    keys(cx, "down");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-adopt"));
    keys(cx, "down");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-update"));
    let shown = format!("Accept identity {REPORTED}");
    assert_eq!(
        label(cx, "host-menu-adopt-identity").as_deref(),
        Some(shown.as_str())
    );
    click(cx, "host-menu-adopt");
    let (sent, body) = one_command(&mut helm, Method::Post, "/api/hosts/2/adopt");
    assert_eq!(body, json!({ "reported": REPORTED }));
    assert_eq!(work(cx), Some(Work::AdoptHost));
    let words =
        format!("host 2 now reports identity-since-changed, not {REPORTED}; look again and decide");
    helm.reply(sent, 409, Some(json!(words)));
    settle(cx);
    assert_eq!(
        label(cx, "host-refusal-title").as_deref(),
        Some("Adopt host · build-box")
    );
    assert_eq!(
        label(cx, "host-refusal-reason").as_deref(),
        Some(words.as_str())
    );
    assert_eq!(focused_id(cx).as_deref(), Some("host-refusal-done"));
    assert_eq!(
        label(cx, "host-error-2"),
        None,
        "the dialog showed it, so the row does not"
    );
}

/// "It must send the identity the user was shown": the menu shows the identity the host reported when it opened and
/// sends that one, never one read at the press, so a host that reports another identity meanwhile is refused by the
/// helm rather than adopted unseen. GPUI dispatches a key press (Enter) after drawing a fresh frame, and this test
/// platform draws after every update, so an item that read the store while drawing would send the newer identity;
/// this test's push lands before the press and tells the two apart. Specified: with the menu open on `build-box`
/// reporting one identity, a push makes it report another, which the row's open details show; the item still names the
/// first, choosing it sends the first, and the helm's refusal opens "Adopt host · build-box"; the menu opened again
/// names the new one.
#[gpui::test]
fn adopt_sends_the_identity_the_menu_showed_not_one_read_at_the_press(cx: &mut TestAppContext) {
    let (mut helm, panel, cx) =
        open_panel_in_column(cx, vec![hosts(&listed_with(mismatch(REPORTED)))]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    cx.update(|_, cx| panel.update(cx, |panel, cx| panel.show_details(true, cx)));
    settle(cx);
    click(cx, "host-menu-toggle-2");
    let since = "c0ffee00-0000-4000-8000-0000000000ff";
    let changed = listed_with(mismatch(since)).remove(1);
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1", "2", "3"]),
        upsert: vec![KeyedEntry {
            key: "2".into(),
            value: serde_json::to_value(&changed).unwrap(),
        }],
        remove: Vec::new(),
    });
    settle(cx);
    assert!(
        label(cx, "host-detail-2-1")
            .unwrap_or_default()
            .contains(since),
        "the premise: the push reached the store before the press"
    );
    let first = format!("Accept identity {REPORTED}");
    assert_eq!(
        label(cx, "host-menu-adopt-identity").as_deref(),
        Some(first.as_str())
    );
    click(cx, "host-menu-adopt");
    let (sent, body) = one_command(&mut helm, Method::Post, "/api/hosts/2/adopt");
    assert_eq!(
        body,
        json!({ "reported": REPORTED }),
        "the identity shown, not the store's at the press"
    );
    helm.reply(
        sent,
        409,
        Some(json!(format!("host 2 now reports {since}, not {REPORTED}"))),
    );
    settle(cx);
    assert_eq!(
        label(cx, "host-refusal-title").as_deref(),
        Some("Adopt host · build-box")
    );
    keys(cx, "escape");
    click(cx, "host-menu-toggle-2");
    let now = format!("Accept identity {since}");
    assert_eq!(
        label(cx, "host-menu-adopt-identity").as_deref(),
        Some(now.as_str()),
        "a new opening shows the store's identity"
    );
}

/// SPEC.md, "Topology": instead of adopting, the user may "fix the destination", and the remedy line says so; the old
/// remedy pointed at the destination editor, which here is the host settings dialog. Specified: a mismatched
/// `build-box`'s details name both remedies and the five permissions adopting resets; its menu's Settings is available
/// and opens the host settings dialog with its destination field.
#[gpui::test]
fn fixing_the_destination_instead_is_the_host_settings_dialog(cx: &mut TestAppContext) {
    let (_helm, panel, cx) =
        open_panel_in_column(cx, vec![hosts(&listed_with(mismatch(REPORTED)))]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    cx.update(|_, cx| panel.update(cx, |panel, cx| panel.show_details(true, cx)));
    settle(cx);
    let remedy = label(cx, "host-detail-2-2").unwrap_or_default();
    assert!(
        remedy.contains("or fix the destination")
            && remedy.contains("all five of this host's permissions"),
        "{remedy}"
    );
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-settings");
    assert!(
        label(cx, "host-settings-edit-destination").is_some(),
        "the destination field and its Edit show"
    );
}

/// The migration's rule (2026-11-14): a host registry change refuses to start while a session launch is in flight, with
/// a reason, and Adopt rewrites the registry's entry. Retry changes none, but on a connected host it drops the
/// connection a launch's create may still be waiting on, so it waits too, as both did behind the old app's page-wide
/// lock (decided by the migration's settings-and-hosts area on 2026-11-15). Each line stays until its own action
/// starts: another action starting does not make it untrue. Specified: with a launcher New in flight, Retry sends
/// nothing and the Hosts panel's line says "Retry build-box: not started, …", and Adopt sends nothing and the line says
/// "Adopt build-box: not started, …"; once the launch has settled, a Retry goes out and the Adopt line stays, and then
/// an Adopt goes out and the line goes.
#[gpui::test]
fn a_launch_holds_both_back_and_each_line_lasts_until_its_own_action_starts(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = panel(cx, &listed_with(mismatch(REPORTED)));
    let launch = start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-retry");
    assert_eq!(helm.commands(), []);
    let retry_line = format!("Retry build-box: {LAUNCH_IN_FLIGHT}");
    assert_eq!(
        label(cx, "hosts-retry-not-started").as_deref(),
        Some(retry_line.as_str())
    );
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-adopt");
    assert_eq!(helm.commands(), []);
    let adopt_line = format!("Adopt build-box: {LAUNCH_IN_FLIGHT}");
    assert_eq!(
        label(cx, "hosts-adopt-not-started").as_deref(),
        Some(adopt_line.as_str())
    );

    helm.reply(launch, 201, Some(json!({})));
    settle(cx);
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-retry");
    let (retried, _) = one_command(&mut helm, Method::Post, "/api/hosts/2/retry");
    assert_eq!(
        label(cx, "hosts-adopt-not-started").as_deref(),
        Some(adopt_line.as_str()),
        "a Retry starting leaves the Adopt line, which is still true"
    );
    helm.reply(retried, 200, Some(json!({})));
    settle(cx);
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-adopt");
    one_command(&mut helm, Method::Post, "/api/hosts/2/adopt");
    assert_eq!(
        label(cx, "hosts-adopt-not-started"),
        None,
        "an Adopt starting ends it"
    );
}

/// SPEC.md, "Keyboard focus": a result that arrives after the user moved on opens no dialog and shows where it belongs
/// (the goal's "No queue for results that arrive later"). Specified: after Retry and a key press, the helm's refusal
/// opens nothing and shows under `build-box`'s row.
#[gpui::test]
fn a_refusal_after_the_user_moved_on_shows_under_the_row(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, &reference_hosts());
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-retry");
    let (sent, _) = one_command(&mut helm, Method::Post, "/api/hosts/2/retry");
    keys(cx, "shift-tab");
    helm.reply(sent, 404, Some(json!("host 2 is not registered")));
    settle(cx);
    assert!(!any_open(cx));
    assert_eq!(
        label(cx, "host-error-2").as_deref(),
        Some("host 2 is not registered")
    );
}

/// Retry and Adopt are host work but neither is a run, and only Adopt changes the registry's entry (the launch rule
/// above). Specified: [`Work::AdoptHost`] changes the host registry and [`Work::RetryHost`] does not, neither creates a
/// session, and neither shows as live work on the rail.
#[test]
fn adopt_is_registry_work_and_neither_is_live_work() {
    assert!(Work::AdoptHost.changes_host_registry());
    assert!(!Work::RetryHost.changes_host_registry());
    for work in [Work::AdoptHost, Work::RetryHost] {
        assert!(!work.creates_a_session());
        assert_eq!(crate::hosts::status::live(None, Some(work)), None);
    }
}
