//! Launcher (New session) behavior, headless.

use super::{
    Page,
    fixtures::{self, CODEX_MODEL},
    server::{FakeHelm, Reply},
};
use serde_json::json;

/// Open New the way a person does: the header button, then the form, and
/// wait until the form has consumed its own first reads (the target host's
/// launch history and the templates), which it marks on itself.
///
/// A browser test gets this ordering for free, because a page load is slower
/// than an in-process fake. Here an action taken before those replies land
/// races the form's reseeding: a late history reply once retired a refused
/// launch's YOLO question before the test could see it.
async fn open_launcher(page: &mut Page) -> super::dom::NodeId {
    let new = page.wait_for(".new-session-button").await;
    page.click(new);
    let form = page.wait_for(".create-session-form").await;
    page.dump_only(form);
    page.wait_until("the launcher's history and templates", |p| {
        let history = p
            .dom()
            .attr(form, "data-history-fetched-revision")
            .unwrap_or("0");
        let templates = p.dom().attr(form, "data-initial-templates-ready");
        (history != "0" && templates == Some("true")).then_some(())
    })
    .await;
    form
}

/// Port of `e2e/tests/composer-word-search.spec.ts`, "composer word search
/// drives a structured launch": typing a harness, a model, an effort, YOLO
/// then Guarded into the launcher's search box, Enter after each, presses
/// the matching control each time, and Enter in the emptied box launches a
/// create whose `launch` carries exactly that selection.
///
/// Why: the search box is the keyboard path through the whole launcher, and
/// it once stopped at the harness. Along the way it pins that the search's
/// "Permissions" results group is the only group a loose "permissions" name
/// finds. The Steel foundation plan gave the permission row the same name,
/// which made the two indistinguishable to a screen reader; the browser
/// suite found that hours after the work was done.
///
/// The browser version's focus assertions (focus returns to the box after
/// each Enter) stay there: focus moves are page JavaScript.
#[farhelm_testtrace::test]
async fn word_search_drives_a_structured_launch() {
    let helm = FakeHelm::start().await;
    fixtures::idle_helm(&helm);
    helm.route("POST", "/api/sessions", |request| {
        let launch = request
            .json
            .as_ref()
            .and_then(|body| body.get("launch"))
            .cloned()
            .unwrap_or_default();
        Reply::json(
            200,
            fixtures::created_session(
                "11111111-1111-4111-8111-111111111111:fh-word",
                "/tmp",
                launch,
            ),
        )
    });
    let mut page = Page::app(&helm);
    let form = open_launcher(&mut page).await;
    let search = page.one_in(form, ".launch-composer-search input[role=combobox]");

    // Each accepted word presses one control. Enter takes the row the last
    // render highlighted, so the premise (that row is selected) is checked
    // before every key.
    let model_option = format!("Model: {CODEX_MODEL} (Codex)");
    let steps = [
        (
            "codex",
            "Harness: Codex",
            ".launch-composer-harness-choice",
            "Codex",
        ),
        (CODEX_MODEL, model_option.as_str(), "", ""),
        (
            "low",
            "Effort: low",
            ".launch-composer-effort-choice",
            "low",
        ),
        (
            "effort:default",
            "Effort: default",
            ".launch-composer-effort-choice",
            "default",
        ),
        (
            "low",
            "Effort: low",
            ".launch-composer-effort-choice",
            "low",
        ),
    ];
    for (word, option, group, pressed) in steps {
        page.input(search, word);
        let premise = format!("option {option:?} selected");
        page.wait_until(&premise, |p| {
            let row = p
                .roles_like(form, "option", option)
                .into_iter()
                .find(|&n| p.name(n) == option)?;
            (p.dom().attr(row, "aria-selected") == Some("true")).then_some(row)
        })
        .await;
        page.key(search, "Enter");
        if !group.is_empty() {
            let choice = page.one_in(form, group);
            let button = page.role_in(choice, "button", pressed);
            assert_eq!(
                page.dom().attr(button, "aria-pressed"),
                Some("true"),
                "{word} presses {pressed}"
            );
        } else {
            let model = page.role_in(form, "combobox", "model");
            assert_eq!(page.dom().attr(model, "value"), Some(CODEX_MODEL));
        }
        assert_eq!(
            page.dom().attr(search, "value"),
            Some(""),
            "accepting {word} clears the box"
        );
    }

    page.input(search, "perms:yolo");
    page.wait_until("the Permissions results group", |p| {
        (!p.roles_like(form, "group", "Permissions").is_empty()).then_some(())
    })
    .await;
    // The browser spec's loose `getByRole("group", { name: "Permissions" })`
    // was a strict locator: a second group answering to that name fails it.
    let groups = page.roles_like(form, "group", "Permissions");
    assert_eq!(
        groups.len(),
        1,
        "{}",
        page.failure(
            "exactly one group is named like \"Permissions\" while search shows that section"
        )
    );
    page.key(search, "Enter");
    let permissions = page.one_in(form, ".launch-composer-permissions-choice");
    assert_eq!(
        page.dom()
            .attr(page.role_in(permissions, "button", "yolo"), "aria-pressed"),
        Some("true")
    );

    page.input(search, "guarded");
    page.wait_until("Permissions: guarded selected", |p| {
        let row = p
            .roles_like(form, "option", "Permissions: guarded")
            .into_iter()
            .next()?;
        (p.dom().attr(row, "aria-selected") == Some("true")).then_some(())
    })
    .await;
    page.key(search, "Enter");
    assert_eq!(
        page.dom().attr(
            page.role_in(permissions, "button", "guarded"),
            "aria-pressed"
        ),
        Some("true")
    );

    let folder = page.role_in(form, "combobox", "folder");
    page.input(folder, "/tmp");
    let launch = page.one_in(form, ".create-session-submit");
    page.wait_until("Launch enabled", |p| {
        (p.dom().attr(launch, "aria-disabled") != Some("true")).then_some(())
    })
    .await;
    page.key(search, "Enter");

    let log = helm.requests_handle();
    let create = page
        .wait_until("the create request", |_| {
            log()
                .into_iter()
                .find(|r| r.method == "POST" && r.path == "/api/sessions")
        })
        .await;
    let selection = &create.json.expect("the create carries a JSON body")["launch"];
    assert_eq!(selection["harness"], "codex");
    assert_eq!(selection["model"], CODEX_MODEL);
    assert_eq!(selection["effort"], "low");
    assert_eq!(selection["permissions"], "guarded");
}

/// Port of `e2e/tests/yolo-guard.spec.ts`, "the YOLO question can launch
/// guarded and remember that permission": a YOLO Codex template launched on
/// a host that asks first is refused with the YOLO question; the question
/// leads with "start guarded session instead"; answering it sends exactly
/// one new create, Guarded, under a new intent key, without the YOLO
/// override and without marking the host safe.
///
/// Why: Guarded is a new launch, not consent to the refused one. A UI that
/// reused the refused request (its key or its override) could start the YOLO
/// session the person just declined, and one that marked the host safe
/// would stop asking for every later launch.
///
/// The browser version also holds the guarded create open to watch the
/// pressed permission change before success, checks focus on the question's
/// Cancel, and confirms one real session on the helm; those stay there.
#[farhelm_testtrace::test]
async fn the_yolo_question_launches_guarded_as_a_new_request() {
    let helm = FakeHelm::start().await;
    fixtures::idle_helm(&helm);
    let template = "yolo-guarded";
    helm.json(
        "GET",
        "/api/templates",
        200,
        json!({ "templates": [fixtures::template(template, json!({ "kind": "agent", "agent": "codex", "permissions": "yolo" }))] }),
    );
    // The host asks before YOLO launches: the first create is refused with
    // the helm's question header, any later one succeeds.
    let mut creates = 0;
    helm.route("POST", "/api/sessions", move |request| {
        creates += 1;
        if creates == 1 {
            return fixtures::yolo_refusal();
        }
        let launch = request
            .json
            .as_ref()
            .and_then(|body| body.get("launch"))
            .cloned()
            .unwrap_or_default();
        Reply::json(
            200,
            fixtures::created_session(
                "11111111-1111-4111-8111-111111111111:fh-guard",
                "/tmp",
                launch,
            ),
        )
    });
    let mut page = Page::app(&helm);
    let form = open_launcher(&mut page).await;
    let folder = page.role_in(form, "combobox", "folder");
    page.input(folder, "/tmp");
    let harness = page.one_in(form, ".launch-composer-harness-choice");
    page.click(page.role_in(harness, "button", "Codex"));
    let permissions = page.one_in(form, ".launch-composer-permissions-choice");
    page.click(page.role_in(permissions, "button", "yolo"));

    let search = page.one_in(form, ".launch-composer-search input[role=combobox]");
    page.input(search, &format!("tl:{template}"));
    let option_name = format!("Template: {template}");
    let option = page
        .wait_until("the template option", |p| {
            p.roles_like(form, "option", &option_name)
                .into_iter()
                .find(|&n| p.name(n) == option_name)
        })
        .await;
    page.click(option);
    let yolo = page.role_in(permissions, "button", "yolo");
    assert_eq!(
        page.dom().attr(yolo, "aria-pressed"),
        Some("true"),
        "the template keeps YOLO"
    );

    let launch = page.one_in(form, ".create-session-submit");
    page.wait_until("Launch enabled", |p| {
        (p.dom().attr(launch, "aria-disabled") != Some("true")).then_some(())
    })
    .await;
    page.click(launch);
    let confirmation = page.wait_for(".yolo-confirmation").await;
    let guarded = page.one_in(confirmation, ".yolo-launch-guarded");
    assert_eq!(page.text(guarded), "start guarded session instead");
    assert_eq!(page.dom().attr(guarded, "type"), Some("button"));
    assert!(
        page.dom()
            .attr(guarded, "class")
            .unwrap_or("")
            .split_whitespace()
            .any(|c| c == "btn-primary")
    );
    let first_button = page.select(confirmation, "button")[0];
    assert_eq!(
        first_button, guarded,
        "the guarded answer leads the question"
    );

    let creates_of = |helm: &FakeHelm| -> Vec<serde_json::Value> {
        helm.requests()
            .into_iter()
            .filter(|r| r.method == "POST" && r.path == "/api/sessions")
            .filter_map(|r| r.json)
            .collect()
    };
    assert_eq!(
        creates_of(&helm).len(),
        1,
        "the refusal sent exactly one create"
    );

    page.click(guarded);
    let log = helm.requests_handle();
    page.wait_until("the guarded create", |_| {
        (log()
            .iter()
            .filter(|r| r.method == "POST" && r.path == "/api/sessions")
            .count()
            == 2)
            .then_some(())
    })
    .await;
    let bodies = creates_of(&helm);
    assert_eq!(bodies[1]["launch"]["harness"], "codex");
    assert_eq!(bodies[1]["launch"]["permissions"], "guarded");
    assert!(bodies[1]["launch"].get("permission_flags").is_none());
    assert!(
        bodies[1].get("confirm_yolo").is_none(),
        "Guarded carries no YOLO override"
    );
    let (refused, retried) = (&bodies[0]["intent_key"], &bodies[1]["intent_key"]);
    assert!(refused.is_string() && retried.is_string());
    assert_ne!(
        refused, retried,
        "changing the permission retires the refused request's key"
    );
    page.wait_gone(".create-session-form").await;
    page.settle().await;
    assert!(
        !helm
            .requests()
            .iter()
            .any(|r| r.path.ends_with("/yolo-without-asking")),
        "answering Guarded never marks the host safe"
    );
}

/// Port of `e2e/tests/sidebar.spec.ts`, "composer destination block keeps
/// host, browse, folder, and grouped choices together": the destination
/// block's Browse button names the committed host; focusing the folder field
/// offers the host's recent folder and the home resets; the seven
/// destination controls come in reading order (host, Browse, folder, recent
/// folder, home reset, local-home reset, name); choosing the recent folder
/// fills the field and closes the options, and "local home" resets the
/// folder to `~` on the local host.
///
/// Why: these are every input that decides where a session launches, and
/// they are read top to bottom. The browser version also pins the 36 px
/// control heights and label centering at two widths; those stay there.
#[farhelm_testtrace::test]
async fn the_destination_block_keeps_its_choices_together_and_in_order() {
    let helm = FakeHelm::start().await;
    fixtures::idle_helm(&helm);
    let recent = "/fixture/destination-link";
    helm.json(
        "GET",
        "/api/launch-history",
        200,
        json!({ "launches": [], "folders": [{
            "host": 1, "canonical_cwd": recent, "canonical_proven": true,
            "display_cwd": recent, "created_at": 1, "creation_seq": 1,
        }] }),
    );
    let mut page = Page::app(&helm);
    let form = open_launcher(&mut page).await;
    let destination = page.one_in(form, ".launch-composer-destination");
    let host = page.one_in(destination, ".create-session-host");
    let host_label = page.text(page.one_in(host, "bdi"));
    assert!(
        !host_label.is_empty(),
        "the host picker shows its committed label"
    );
    let browse = page.role_in(destination, "button", "browse this path");
    assert!(
        page.text(browse).contains(&host_label),
        "Browse names the host: {:?}",
        page.text(browse)
    );

    let folder = page.role_in(destination, "combobox", "folder");
    page.focus(folder);
    let options = page
        .wait_until("the folder options", |p| {
            let found = p.roles_like(destination, "listbox", "folders");
            found.into_iter().find(|&l| p.name(l) == "folders")
        })
        .await;
    let recent_option = page.role_in(options, "option", recent);
    assert_eq!(
        page.dom().attr(recent_option, "aria-selected"),
        Some("false")
    );
    for label in ["home", "local home"] {
        assert!(
            page.dom()
                .descendants(options)
                .iter()
                .any(|&n| page.dom().children(n).is_empty() && page.text(n) == label),
            "{}",
            page.failure(&format!("the folder options offer {label:?}"))
        );
    }
    let order = [
        ".create-session-host",
        "button[aria-label=\"browse this path\"]",
        "input[aria-label=folder]",
        ".launch-composer-folder-options button[role=option]",
        ".launch-composer-folder-options button[aria-label=\"reset folder to home\"]",
        ".launch-composer-folder-options button[aria-label=\"reset destination to local home\"]",
        ".launch-composer-name",
    ];
    let document: Vec<_> = page.dom().descendants(page.dom().root());
    let positions: Vec<usize> = order
        .iter()
        .map(|selector| {
            let first = *page
                .select(destination, selector)
                .first()
                .unwrap_or_else(|| panic!("{}", page.failure(selector)));
            document.iter().position(|&n| n == first).expect("attached")
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "reading order {order:?} at {positions:?}"
    );

    page.click(recent_option);
    assert_eq!(page.dom().attr(folder, "value"), Some(recent));
    page.wait_until("the folder options to close", |p| {
        p.roles_like(destination, "listbox", "folders")
            .is_empty()
            .then_some(())
    })
    .await;
    assert_eq!(
        template_host_pinned(&mut page, form),
        None,
        "the inherited local host starts unpinned"
    );
    page.blur(folder);
    page.focus(folder);
    let options = page
        .wait_until("the folder options again", |p| {
            p.roles_like(destination, "listbox", "folders")
                .into_iter()
                .next()
        })
        .await;
    let local_home = page
        .dom()
        .descendants(options)
        .into_iter()
        .find(|&n| page.role(n).as_deref() == Some("option") && page.text(n).contains("local home"))
        .unwrap_or_else(|| panic!("{}", page.failure("the local home option")));
    page.click(local_home);
    assert_eq!(page.dom().attr(folder, "value"), Some("~"));
    page.wait_until("the folder options to close after local home", |p| {
        p.roles_like(destination, "listbox", "folders")
            .is_empty()
            .then_some(())
    })
    .await;
    assert_eq!(page.dom().attr(host, "data-host-id"), Some("1"));
    // The host was local all along, so the visible host cannot tell an
    // explicit local pin from a folder-only reset. Template inclusion can.
    assert_eq!(
        template_host_pinned(&mut page, form),
        Some("true"),
        "local home pins the host"
    );
}

/// Open "save as template" and read whether its host checkbox is checked
/// (`Some("true")`) or not (`None`), then cancel. The host is offered for
/// the template exactly when the draft pinned it explicitly.
fn template_host_pinned(page: &mut Page, form: super::dom::NodeId) -> Option<&'static str> {
    page.click(page.role_in(form, "button", "save as template"));
    let panel = page.role_in(form, "region", "save as template");
    let checkbox = page.one_in(panel, "input[type=checkbox][data-field=host]");
    let checked = page.dom().attr(checkbox, "checked").map(|_| "true");
    page.click(page.role_in(panel, "button", "cancel"));
    checked
}
