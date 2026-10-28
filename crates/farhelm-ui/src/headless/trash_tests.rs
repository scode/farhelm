//! The checkout Trash dialog, headless.

use super::{
    Page, fixtures,
    server::{FakeHelm, Reply},
};
use farhelm_proto::ArchivedCheckout;
use serde_json::json;
use std::sync::{Arc, Mutex};

/// Port of `e2e/tests/github-checkouts.spec.ts`, "single trash delete
/// removes one real archive and preserves its neighbour": the header's
/// Trash button opens a dialog named "trash"; the local host's band lists
/// both archives; the selected archive's "delete … permanently" button sends
/// one delete naming exactly that archive's id; afterwards no confirmation
/// remains and the neighbour is still listed.
///
/// Why: deleting an archived checkout is permanent, and the request must
/// name the one archive whose button was pressed. The browser version also
/// proves the real directories (the deleted one gone, the neighbour's
/// uncommitted work intact) and the focus return; those stay there.
#[farhelm_testtrace::test]
async fn a_single_trash_delete_names_only_its_archive() {
    let helm = FakeHelm::start().await;
    fixtures::idle_helm(&helm);
    let archive = |id: &str, name: &str| ArchivedCheckout {
        id: id.into(),
        name: name.into(),
        repository: "octo/widgets".into(),
        path: format!("/home/user/checkouts/.farhelm-trash/{name}"),
        archived_at: Some(1_700_000_000),
        bytes: None,
    };
    let trash = Arc::new(Mutex::new(vec![
        archive("archive-selected", "widgets-selected"),
        archive("archive-neighbour", "widgets-neighbour"),
    ]));
    let listed = trash.clone();
    helm.route("POST", "/api/checkout-trash/list", move |_| {
        Reply::json(
            200,
            json!({ "checkouts": *listed.lock().unwrap(), "issues": [] }),
        )
    });
    let deleted = trash.clone();
    helm.route("POST", "/api/checkout-trash/delete", move |request| {
        let ids: Vec<String> =
            serde_json::from_value(request.json.clone().unwrap_or_default()["ids"].clone())
                .unwrap_or_default();
        deleted
            .lock()
            .unwrap()
            .retain(|entry| !ids.contains(&entry.id));
        Reply::json(200, json!({ "removed": ids, "issues": [] }))
    });
    let mut page = Page::app(&helm);
    let open = page.wait_for(".checkout-trash-button").await;
    page.click(open);
    let root = page.dom().root();
    let dialog = page
        .wait_until("a dialog named trash", |p| {
            let found = p.roles_like(root, "dialog", "trash");
            found.into_iter().find(|&d| p.name(d) == "trash")
        })
        .await;
    page.dump_only(dialog);
    let band = page
        .wait_for(".checkout-trash-host[data-host-id=\"1\"]")
        .await;
    let button = page
        .wait_until("the selected archive's delete button", |p| {
            p.roles_like(band, "button", "delete widgets-selected permanently")
                .into_iter()
                .find(|&b| p.name(b) == "delete widgets-selected permanently")
        })
        .await;
    page.wait_until("the delete button enabled", |p| {
        p.dom().attr(button, "disabled").is_none().then_some(())
    })
    .await;
    page.click(button);
    let log = helm.requests_handle();
    let request = page
        .wait_until("the delete request", |_| {
            log()
                .into_iter()
                .find(|r| r.path == "/api/checkout-trash/delete")
        })
        .await;
    assert_eq!(
        request.json.expect("a JSON body")["ids"],
        json!(["archive-selected"])
    );
    page.wait_gone(".checkout-trash-confirm").await;
    page.wait_until("one entry left in the band", |p| {
        (p.select(band, ".checkout-trash-entry").len() == 1).then_some(())
    })
    .await;
    assert!(
        page.text(band).contains("widgets-neighbour"),
        "the neighbour stays listed"
    );
}
