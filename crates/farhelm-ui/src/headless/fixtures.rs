//! Canned helm replies shared by the headless tests.
//!
//! [`idle_helm`] scripts the reads every page makes before any test action:
//! one connected local host, no sessions, no templates, default
//! preferences. A test layers its own routes on top (later routes win), so
//! it states only what its surface is about.
//!
//! The bodies follow the helm's wire shapes. Rows that have a golden fixture
//! (`farhelm-helm/http-contract/host-list.json`, `session-list.json`) start
//! from it so they stay in step with what the helm serializes, and bodies
//! built here are checked against the UI's or the protocol's own types
//! (`Session`, `LaunchTemplate`), so a fixture that drifts from the wire
//! shape fails at the fixture instead of as a page that quietly ignores it.

use super::server::{FakeHelm, Reply};
use serde_json::{Value, json};

/// The local host, id 1, connected, as the helm's golden host list has it.
pub fn local_host() -> Value {
    let list: Value = serde_json::from_str(include_str!(
        "../../../farhelm-helm/http-contract/host-list.json"
    ))
    .expect("the golden host list parses");
    let local = list["hosts"]
        .as_array()
        .and_then(|hosts| hosts.iter().find(|host| host["kind"] == "local"))
        .expect("the golden host list has a local host");
    local.clone()
}

/// The Codex model a launcher test picks: the helm catalog's first Codex
/// entry, with the three-level effort list Codex ships with (`launches.rs`,
/// `CODEX_EFFORTS`). Three levels keep the effort control a row of buttons.
pub const CODEX_MODEL: &str = "gpt-6-luna";

/// A slice of the helm's release catalog (`farhelm-helm/src/launches.rs`,
/// `CATALOG`), in its wire shape. Copied rather than linked because the helm
/// keeps the catalog crate-private; the entries are the ones launcher tests
/// name, with the effort lists the helm gives them.
pub fn launch_catalog() -> Value {
    json!([
        { "id": CODEX_MODEL, "harness": "codex", "efforts": ["low", "medium", "high"] },
        { "id": "gpt-6-astra", "harness": "codex", "efforts": ["high"] },
        { "id": "claude-fable-5", "harness": "claude", "efforts": ["low", "medium", "high", "xhigh", "max"] },
    ])
}

/// A created session as the helm returns it from `POST /api/sessions`: the
/// golden listing's first row, moved to the local host, carrying the
/// structured `selection` the create asked for. The stored launch keeps the
/// golden row's compiled commands; only the selection is the request's.
pub fn created_session(id: &str, cwd: &str, selection: Value) -> Value {
    let list: Value = serde_json::from_str(include_str!(
        "../../../farhelm-helm/http-contract/session-list.json"
    ))
    .expect("the golden session list parses");
    let mut session = list["sessions"][0].clone();
    session["id"] = json!(id);
    session["host"] = json!(1);
    session["host_name"] = json!("host1");
    session["cwd"] = json!(cwd);
    session["canonical_cwd"] = json!(cwd);
    session["launch"]["selection"] = selection;
    if let Err(error) = serde_json::from_value::<crate::Session>(session.clone()) {
        panic!("created_session does not decode as the UI's Session: {error}\n{session:#}");
    }
    session
}

/// A connected ssh host whose farhelm is older than the helm's, from the
/// golden host list's ssh row: the shape a host offering an update has.
pub fn old_version_host(id: u64, name: &str) -> Value {
    let list: Value = serde_json::from_str(include_str!(
        "../../../farhelm-helm/http-contract/host-list.json"
    ))
    .expect("the golden host list parses");
    let mut host = list["hosts"]
        .as_array()
        .and_then(|hosts| hosts.iter().find(|host| host["kind"] == "ssh"))
        .expect("the golden host list has an ssh host")
        .clone();
    host["id"] = json!(id);
    host["name"] = json!(name);
    host["alias"] = Value::Null;
    host["destination"] = json!("user@remote-host");
    host["state"] = json!({
        "phase": "connected", "identity": "old-host", "build_version": "0.1.0",
        "old_version": true, "newer_version": false, "refresh": { "status": "ok", "sessions": 0 },
    });
    host
}

/// A helm with one connected local host and nothing else on it.
pub fn idle_helm(helm: &FakeHelm) {
    helm.json("GET", "/api/preferences", 200, json!({}));
    helm.json("GET", "/api/hosts", 200, json!({ "hosts": [local_host()] }));
    helm.json(
        "GET",
        "/api/sessions",
        200,
        json!({ "sessions": [], "total": 0, "matching": 0 }),
    );
    helm.json("GET", "/api/templates", 200, json!({ "templates": [] }));
    helm.json("GET", "/api/launch-catalog", 200, launch_catalog());
    helm.json(
        "GET",
        "/api/launch-history",
        200,
        json!({ "launches": [], "folders": [] }),
    );
    helm.json("GET", "/api/approvals", 200, json!({ "approvals": [] }));
}

/// A stored launch template in the helm's wire shape, checked against
/// [`LaunchTemplate`](farhelm_proto::launcher::LaunchTemplate) so a typo in a
/// test's fields fails here instead of as a template the page ignores.
pub fn template(name: &str, fields: Value) -> Value {
    let template = json!({ "name": name, "fields": fields });
    serde_json::from_value::<farhelm_proto::launcher::LaunchTemplate>(template.clone())
        .unwrap_or_else(|error| panic!("template {name:?} is not a LaunchTemplate: {error}"));
    template
}

/// The helm's refusal of a YOLO launch on a host that asks first, as
/// `farhelm-helm/src/lib.rs` builds it: 409, the error text as a plain body,
/// the YOLO question header, and the header saying the create was definitely
/// not accepted (which is what lets the UI offer a fresh request at all).
pub fn yolo_refusal() -> Reply {
    use farhelm_proto::http::*;
    Reply::text(409, "this host asks before starting a YOLO session")
        .header(YOLO_CONFIRMATION_HEADER, YOLO_CONFIRMATION_REQUIRED)
        .header(CREATE_OUTCOME_HEADER, CREATE_OUTCOME_DEFINITELY_UNACCEPTED)
}

/// The template name a `/api/templates/{name}` request is about. The UI
/// percent-encodes the name into the path and the helm decodes it, so the
/// store must too, or a name with a space or a dot would be stored under its
/// encoded spelling.
fn template_name(path: &str) -> String {
    let segment = path.rsplit('/').next().unwrap_or_default();
    percent_encoding::percent_decode_str(segment)
        .decode_utf8()
        .unwrap_or_else(|error| panic!("template name {segment:?} is not UTF-8: {error}"))
        .into_owned()
}

/// The helm's template store, held in the fake: `GET /api/templates` lists
/// what `PUT /api/templates/{name}` stored (the body is the template's
/// fields), and `DELETE` removes it. Returns the store so a test can read
/// what was saved, the way the browser specs read `/api/templates` back.
pub fn template_store(
    helm: &FakeHelm,
    initial: Vec<Value>,
) -> std::sync::Arc<std::sync::Mutex<Vec<Value>>> {
    use std::sync::{Arc, Mutex};
    let store = Arc::new(Mutex::new(initial));
    let list = store.clone();
    helm.route("GET", "/api/templates", move |_| {
        Reply::json(200, json!({ "templates": *list.lock().unwrap() }))
    });
    let put = store.clone();
    helm.route("PUT", "/api/templates/*", move |request| {
        let name = template_name(&request.path);
        // Refuse a body the helm would refuse, as a reply the page shows,
        // rather than panicking inside the server task where no test sees it.
        let stored = json!({ "name": name, "fields": request.json.clone().unwrap_or_default() });
        if let Err(error) =
            serde_json::from_value::<farhelm_proto::launcher::LaunchTemplate>(stored.clone())
        {
            return Reply::text(400, &format!("not a template: {error}"));
        }
        let mut templates = put.lock().unwrap();
        templates.retain(|t| t["name"] != name.as_str());
        templates.push(stored);
        Reply::json(200, json!({}))
    });
    let delete = store.clone();
    helm.route("DELETE", "/api/templates/*", move |request| {
        let name = template_name(&request.path);
        delete
            .lock()
            .unwrap()
            .retain(|t| t["name"] != name.as_str());
        Reply::json(200, json!({}))
    });
    store
}
