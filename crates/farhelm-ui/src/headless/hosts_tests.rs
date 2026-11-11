//! The Hosts panel, headless.

use super::{
    Page, fixtures,
    server::{FakeHelm, Reply},
};
use serde_json::json;

/// Port of `e2e/tests/terminal-multihost.spec.ts`, "host-inline-update:
/// pointer starts progress and keeps the header on one line": a connected
/// host on an older farhelm shows an "old version" status and an inline
/// update button; clicking it sends one bodyless `POST
/// /api/hosts/{id}/update`, and while that is in flight the button gives way
/// to "updating…" progress.
///
/// Why: the inline button is the one-click path to an update, and it shares
/// its request with the host menu's. A refusal that arrives later must not
/// be the first sign the click did anything.
///
/// The browser version's other half (the header stays on one line, both
/// targets are hit-testable, the long name truncates, the keyboard variant's
/// focus handoff) is layout and focus, and stays there.
#[farhelm_testtrace::test]
async fn the_inline_host_update_sends_one_update_and_shows_progress() {
    let helm = FakeHelm::start().await;
    fixtures::idle_helm(&helm);
    let host = 8020;
    let name = "user@a-very-long-remote-host-name-that-needs-ellipsis";
    helm.json(
        "GET",
        "/api/hosts",
        200,
        json!({ "hosts": [fixtures::old_version_host(host, name)] }),
    );
    helm.json(
        "GET",
        &format!("/api/hosts/{host}/provisioning"),
        200,
        json!({ "host_id": host, "run_id": null, "operation": null, "status": "completed", "steps": [], "message": null }),
    );
    let update = helm.route_held("POST", &format!("/api/hosts/{host}/update"), |_| {
        Reply::json(409, json!({ "error": "held fixture update refused" }))
    });
    let mut page = Page::app(&helm);
    let toggle = page.wait_for(".hosts-panel-toggle").await;
    if page.dom().attr(toggle, "aria-pressed") != Some("true") {
        page.click(toggle);
    }
    let row = page.wait_for(&format!("[data-host-id=\"{host}\"]")).await;
    page.dump_only(row);
    let button_name = format!("update {name}");
    let button = page
        .wait_until("the inline update button", |p| {
            let found = p.roles_like(row, "button", &button_name);
            found.into_iter().find(|&b| p.name(b) == button_name)
        })
        .await;
    assert!(
        !page.hidden(button),
        "the update button is visible, not inside hidden content"
    );
    assert_eq!(page.roles_like(row, "status", "old version").len(), 1);

    page.click(button);
    let log = helm.requests_handle();
    let path = format!("/api/hosts/{host}/update");
    let sent = page
        .wait_until("the update request", |_| {
            log().into_iter().find(|r| r.path == path)
        })
        .await;
    assert_eq!(sent.method, "POST");
    assert_eq!(sent.body_len, 0, "the update request has no body");
    let pending = page
        .wait_for(".host-update-pending[data-update-phase=planning]")
        .await;
    assert_eq!(page.text(pending), "updating…");
    assert!(
        page.roles_like(row, "button", &button_name)
            .into_iter()
            .all(|b| page.name(b) != button_name),
        "the button gives way to progress"
    );
    update.release();
    assert_eq!(
        helm.requests().iter().filter(|r| r.path == path).count(),
        1,
        "one click, one update"
    );
}
