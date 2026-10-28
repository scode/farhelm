//! The Templates dialog, headless, opened the way a person opens it: from
//! the session list's header button.

use super::{Page, dom::NodeId, fixtures, server::FakeHelm};
use serde_json::json;

/// Open Templates from the header and wait until its list has loaded.
///
/// Through the header button on purpose. A port that mounted the dialog
/// directly would keep passing when the button is renamed or replaced,
/// which is how most of the Steel sidebar plan's late failures happened.
async fn open_templates(page: &mut Page) -> NodeId {
    page.wait_for(".new-session-button").await;
    let root = page.dom().root();
    page.click(page.role_in(root, "button", "Templates"));
    let dialog = page.wait_for(".templates-dialog[role=dialog]").await;
    page.dump_only(dialog);
    let sidebar = page.one_in(dialog, ".templates-sidebar");
    page.wait_until("the template list", |p| {
        (p.dom().attr(sidebar, "aria-busy") == Some("false")).then_some(())
    })
    .await;
    dialog
}

/// Add a field to the template being edited, by its menu name.
fn add_field(page: &mut Page, dialog: NodeId, name: &str) {
    page.click(page.one_in(dialog, ".templates-add"));
    let menu = page.one_in(dialog, ".templates-add-menu");
    page.click(page.role_in(menu, "button", name));
}

/// Port of `e2e/tests/templates.spec.ts`, "templates save explicit guarded
/// and default-YOLO permissions": for Codex (Guarded), OMP and Goose (YOLO,
/// their default), the approvals choice is offered, pressing it and saving
/// stores exactly `{kind: agent, agent, permissions}`.
///
/// Why: a permission that is also the agent's current default is still an
/// explicit stored edit. The editor once marked it unoffered, or refused to
/// save it, to avoid showing a duplicate reset label.
#[farhelm_testtrace::test]
async fn templates_save_explicit_default_permissions() {
    let helm = FakeHelm::start().await;
    fixtures::idle_helm(&helm);
    let store = fixtures::template_store(&helm, Vec::new());
    let mut page = Page::app(&helm);
    let dialog = open_templates(&mut page).await;
    let name = "explicit permissions v1.2";
    page.click(page.one_in(dialog, ".templates-new"));
    page.input(page.one_in(dialog, ".templates-name"), name);
    add_field(&mut page, dialog, "agent type");
    page.select_option(page.role_in(dialog, "combobox", "agent type"), "codex");
    add_field(&mut page, dialog, "approvals");
    for (agent, permission) in [("codex", "guarded"), ("omp", "yolo"), ("goose", "yolo")] {
        page.select_option(page.role_in(dialog, "combobox", "agent type"), agent);
        let choices = page.role_in(dialog, "group", "approvals");
        let offered = page.one_in(choices, &format!("button[value={permission}]"));
        assert!(
            !page.text(offered).contains("not offered"),
            "{agent} offers {permission}"
        );
        page.click(offered);
        assert_eq!(page.dom().attr(offered, "aria-pressed"), Some("true"));
        let save = page.one_in(dialog, ".templates-save");
        page.click(save);
        page.wait_gone(".templates-unsaved").await;
        let stored = store
            .lock()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .cloned()
            .expect("the template is stored");
        assert_eq!(
            stored["fields"],
            json!({ "kind": "agent", "agent": agent, "permissions": permission }),
            "{agent}"
        );
        let sidebar = page.one_in(dialog, ".templates-sidebar");
        page.wait_until("the list after save", |p| {
            (p.dom().attr(sidebar, "aria-busy") == Some("false")).then_some(())
        })
        .await;
    }
}

/// Port of `e2e/tests/templates.spec.ts`, "a refused template Delete keeps
/// the helm error and stored template": a Delete the helm refuses (409,
/// plain text) shows exactly the helm's text, keeps the row and the Delete
/// control, offers no Undo, and sends one request.
///
/// Why: a known refusal is not a lost reply. The dialog's lost-reply
/// handling (report uncertainty, re-read) must not swallow the helm's own
/// explanation or pretend the template might be gone.
#[farhelm_testtrace::test]
async fn a_refused_template_delete_keeps_the_helm_error_and_the_template() {
    let helm = FakeHelm::start().await;
    fixtures::idle_helm(&helm);
    let name = "delete-refused";
    let fields = json!({ "kind": "agent", "agent": "claude", "name": "kept original" });
    let store = fixtures::template_store(&helm, vec![fixtures::template(name, fields.clone())]);
    let refusal = "this template is still in use";
    helm.route("DELETE", &format!("/api/templates/{name}"), move |_| {
        super::server::Reply::text(409, refusal)
    });
    let mut page = Page::app(&helm);
    let dialog = open_templates(&mut page).await;
    let row = page
        .select(dialog, ".templates-row")
        .into_iter()
        .find(|&r| page.text(r).contains(name))
        .unwrap_or_else(|| panic!("{}", page.failure("the stored template's row")));
    page.click(row);
    let name_field = page.one_in(dialog, ".templates-name");
    page.wait_until("the row's template in the editor", |p| {
        (p.dom().attr(name_field, "value") == Some(name)).then_some(())
    })
    .await;
    let delete = page.one_visible_in(dialog, ".templates-delete");
    assert_eq!(page.text(delete), "delete");
    page.click(delete);
    let error = page
        .wait_for(".templates-dialog p.templates-error[role=status]")
        .await;
    assert_eq!(page.text(error), refusal);
    assert!(
        page.dom().is_attached(row) && page.text(row).contains(name),
        "the row stays"
    );
    let delete = page.one_visible_in(dialog, ".templates-delete");
    assert!(
        page.dom().attr(delete, "disabled").is_none(),
        "Delete stays available"
    );
    assert!(
        page.select(dialog, ".templates-undo").is_empty(),
        "a refusal offers no Undo"
    );
    let deletes = helm
        .requests()
        .iter()
        .filter(|r| r.method == "DELETE")
        .count();
    assert_eq!(deletes, 1);
    assert_eq!(
        store.lock().unwrap()[0]["fields"],
        fields,
        "the stored template is untouched"
    );
}
