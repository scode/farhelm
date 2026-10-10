//! The cards that ask the user to approve an agent's `farhelm` command
//! (SPEC.md, Agent-spawned sessions).
//!
//! The helm holds each acting request from inside a session until the user
//! answers it here. This module only shows what the helm lists and sends the
//! answer back: the decision, the per-host setting and the expiry are all the
//! helm's (farhelm-helm's `approvals`), so nothing a card does can let a
//! request through that the helm did not hold for it.
//!
//! ## Shape
//!
//! Non-modal requests centered above the main pane, with one expanded card
//! and the other waiting requests below it as selectable headers. The oldest
//! request opens first; a user's selection survives listing refreshes until
//! that request leaves. The rest of the app stays usable, and a card stays
//! until it is answered (here or in another window) or the helm expires it;
//! either way the next listing drops it. The listing is re-read on every fleet
//! invalidation notice, the same way every other surface learns of changes:
//! the helm bumps the feed whenever a request starts or stops waiting.
//!
//! ## Whose words are whose
//!
//! Almost everything on a card was written by the agent asking: the session's
//! title, the folder, the command text, the template's name. A card that read
//! those values into a sentence would let an agent write its own approval
//! prompt, so every value sits in a labelled row as escaped,
//! direction-isolated peer text (`peer::display_peer`), and the only prose is
//! this module's own labels and buttons. Host names are rendered the same way:
//! they are user data the helm relays, not this UI's words.

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;
use farhelm_proto::SessionLaunch;
use farhelm_proto::approvals::{
    ApprovalAction, ApprovalAnswer, ApprovalSession, LaunchVerb, PendingApproval,
};
use farhelm_proto::launcher::{TemplateDestination, TemplateFields};

use crate::ApiBase;
use crate::api::AnswerOutcome;
use crate::feed::{fallback_polls_now, fallback_sleep};
use crate::peer::{PeerBlock, display_identity, display_peer};
use crate::reader::{SurfaceReader, Trigger, request_read};

/// Keep every waiting request reachable while showing one decision in full.
///
/// Mounted once, beside the fleet feed, so the cards survive selection
/// changes the way the feed does. Renders nothing while nothing waits.
///
/// The listing is read through the page's shared reader discipline
/// (`reader::request_read`): one read at a time, a failed read retried on its
/// own, and the fallback poll while the feed is down. A card is the only way
/// an agent's request can be answered, and its notice is announced exactly
/// once, so a read lost to a blip must not leave the request invisible for the
/// rest of its wait.
#[component]
pub(crate) fn ApprovalCards() -> Element {
    let base = use_context::<ApiBase>().0;
    let mut approvals = use_signal(Vec::<PendingApproval>::new);
    // Answers in flight, so a double click cannot send two.
    let answering = use_signal(HashSet::<String>::new);
    // Cards answered from this window, hidden at once rather than left
    // clickable until the next listing drops them; forgotten once a listing
    // no longer has them.
    let mut answered = use_signal(HashSet::<String>::new);
    // The last refusal per card (an "always allow" whose setting could not
    // be stored), shown on the card while its request keeps waiting.
    let mut errors = use_signal(HashMap::<String, String>::new);
    // Set when an answer arrived after its request had stopped waiting, so the
    // user learns the answer did not take effect even though the card is gone:
    // `Some(true)` when that answer was "always allow", whose setting the helm
    // stores before it looks for the request.
    let mut late = use_signal(|| None::<bool>);
    // Listing changes and switches both move a different decision under the
    // pointer. A generation keeps an older pause from arming a newer card.
    let mut armed = use_signal(|| true);
    let mut shown = use_signal(|| (Vec::<String>::new(), None::<String>));
    let mut expanded = use_signal(|| None::<String>);
    let mut arm_generation = use_signal(|| 0_u64);

    let read_base = base.clone();
    let read = move || {
        let base = read_base.clone();
        async move {
            match crate::api::fetch_approvals(&base).await {
                Ok(list) => {
                    let waiting: HashSet<String> =
                        list.iter().map(|card| card.id.clone()).collect();
                    errors.write().retain(|id, _| waiting.contains(id));
                    answered.write().retain(|id| waiting.contains(id));
                    approvals.set(list);
                    true
                }
                Err(_) => false,
            }
        }
    };
    let surface = use_signal(SurfaceReader::default);
    let request = move |trigger: Trigger| request_read(surface, trigger, read.clone());
    let mount = request.clone();
    use_hook(move || mount(Trigger::Explicit));
    let notice = request.clone();
    crate::feed::use_feed_reader(move || notice(Trigger::Notice));
    let poll = request.clone();
    use_future(move || {
        let poll = poll.clone();
        async move {
            loop {
                fallback_sleep().await;
                if fallback_polls_now() {
                    poll(Trigger::Scheduled);
                }
            }
        }
    });

    let cards: Vec<PendingApproval> = approvals
        .read()
        .iter()
        .filter(|card| !answered.read().contains(&card.id))
        .cloned()
        .collect();
    // The helm lists oldest first. Resolve selection against this listing,
    // rather than its previous index: expiry or an answer can remove any
    // request, and refreshing must not replace one the user chose to read.
    let selected = selected_request(&cards, expanded.read().as_deref());
    let selected_id = selected.map(|card| card.id.clone());
    let ids: Vec<String> = cards.iter().map(|card| card.id.clone()).collect();
    let presentation = (ids, selected_id.clone());
    if *shown.peek() != presentation {
        shown.set(presentation);
        let generation = arm_generation.peek().wrapping_add(1);
        arm_generation.set(generation);
        armed.set(false);
        spawn(async move {
            crate::reader::sleep_ms(ARM_DELAY_MS).await;
            if *arm_generation.peek() == generation {
                armed.set(true);
            }
        });
    }
    if cards.is_empty() && late().is_none() {
        return rsx! {};
    }
    rsx! {
        div {
            class: "approval-cards",
            role: "region",
            aria_label: "requests waiting for your approval",
            // Left live by `modal_isolation` while a dialog is open: an agent
            // waiting on a card must not wait on the dialog too.
            "data-modal-exempt": "true",
            onmounted: move |_| {
                // The region stays outside the shell for modal exemption;
                // its geometry follows the actual pane instead of assuming
                // a sidebar width or a fixed-height session header.
                document::eval(include_str!("../assets/approval-layout.js"));
            },
            if let Some(always) = late() {
                div { class: "approval-late", role: "status",
                    p {
                        "Your answer arrived after its request had stopped waiting, so it did not take effect."
                        if always {
                            " The host may still have been set to run farhelm commands without asking; its settings show whether it was."
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-neutral approval-late-dismiss",
                        "data-tooltip": "dismiss: hide this notice",
                        onclick: move |_| late.set(None),
                        "dismiss"
                    }
                }
            }
            if let Some(card) = selected {
                ApprovalCard {
                    key: "{card.id}",
                    card: card.clone(),
                    waiting: cards.len(),
                    busy: answering.read().contains(&card.id) || !armed(),
                    error: errors.read().get(&card.id).cloned(),
                    on_answer: {
                        let base = base.clone();
                        let request = request.clone();
                        let mut answering = answering;
                        move |(id, answer): (String, ApprovalAnswer)| {
                            if !answering.write().insert(id.clone()) {
                                return;
                            }
                            let base = base.clone();
                            let request = request.clone();
                            spawn(async move {
                                let result = crate::api::answer_approval(&base, &id, answer).await;
                                match result {
                                    Ok(outcome) => {
                                        errors.write().remove(&id);
                                        answered.write().insert(id.clone());
                                        if outcome == AnswerOutcome::Gone {
                                            late.set(Some(answer == ApprovalAnswer::AlwaysAllow));
                                        }
                                    }
                                    Err(error) => {
                                        errors.write().insert(id.clone(), error);
                                    }
                                }
                                answering.write().remove(&id);
                                request(Trigger::Explicit);
                            });
                        }
                    },
                }
            }
            for card in cards.iter().filter(|card| Some(&card.id) != selected_id.as_ref()) {
                button {
                    key: "{card.id}",
                    r#type: "button",
                    class: "approval-waiting",
                    "data-approval-id": "{card.id}",
                    aria_expanded: "false",
                    onclick: {
                        let id = card.id.clone();
                        move |_| {
                            armed.set(false);
                            expanded.set(Some(id.clone()));
                        }
                    },
                    span { class: "approval-waiting-action", "{action_summary(&card.action)}" }
                    span { class: "approval-waiting-host",
                        span { class: "approval-waiting-label", "from host" }
                        span { class: "peer-value", dir: "ltr", "{display_peer(&card.host_name)}" }
                    }
                }
            }
        }
    }
}

/// Preserve a chosen request by identity, falling back to the oldest waiting.
///
/// The list is already ordered by the helm. A vanished selection must not
/// retain its old position: answering or expiring it opens the oldest left.
fn selected_request<'a>(
    cards: &'a [PendingApproval],
    selected: Option<&str>,
) -> Option<&'a PendingApproval> {
    cards
        .iter()
        .find(|card| Some(card.id.as_str()) == selected)
        .or_else(|| cards.first())
}

/// How long the answer buttons stay disabled after the listing or selection changes.
/// Long enough to absorb the second click of a double-click and a click
/// already on its way, short enough not to be noticed when nothing moved.
const ARM_DELAY_MS: u64 = 700;

/// Show the selected request's entire decision and its own answer controls.
///
/// Collapsed headers carry no answer action. Every value needed to approve
/// this request therefore remains in this expanded card, including full
/// command text and any refusal that left the request waiting.
#[component]
fn ApprovalCard(
    card: PendingApproval,
    /// The whole fleet's waiting count, including collapsed requests.
    waiting: usize,
    /// An answer is in flight, or the presentation-change pause is active.
    busy: bool,
    /// The last answer's refusal, if any.
    error: Option<String>,
    on_answer: EventHandler<(String, ApprovalAnswer)>,
) -> Element {
    // Long rows follow the short facts, so a full-width folder or command
    // cannot split the compact grid into several mostly empty rows.
    let (full, facts): (Vec<_>, Vec<_>) = action_rows(&card.action)
        .into_iter()
        .partition(full_width_row);
    let id = card.id.clone();
    let heading = format!("approval-{}-heading", card.id);
    let answer = move |answer: ApprovalAnswer| {
        let id = id.clone();
        move |_| on_answer.call((id.clone(), answer))
    };
    let mut asking = Vec::new();
    session_rows(
        "asked by session",
        "asking session id",
        None,
        &card.session,
        &mut asking,
    );
    rsx! {
        section {
            class: "approval-card",
            role: "group",
            aria_labelledby: "{heading}",
            "data-approval-id": "{card.id}",
            "data-approval-kind": "{action_kind(&card.action)}",
            div { class: "approval-card-header",
                span { class: "approval-card-badge", "agent request" }
                h2 { id: "{heading}", class: "approval-card-heading", "{action_summary(&card.action)}" }
                span { class: "approval-card-count", "1 of {waiting} waiting" }
            }
            dl { class: "approval-card-requester",
                Row { label: "from host", value: card.host_name.clone() }
                for (index, row) in asking.into_iter().enumerate() {
                    CardDetail { key: "{index}", row }
                }
            }
            dl { class: "approval-card-rows",
                for (index, row) in facts.into_iter().chain(full).enumerate() {
                    CardDetail { key: "{index}", row }
                }
            }
            if let Some(error) = error {
                p { class: "approval-card-error", role: "alert", "{display_peer(&error)}" }
            }
            div { class: "approval-card-actions",
                button {
                    r#type: "button",
                    class: "btn btn-primary approval-allow",
                    "data-tooltip": "allow: let this one request go ahead",
                    disabled: busy,
                    onclick: answer(ApprovalAnswer::Allow),
                    "allow"
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral approval-always-allow",
                    "data-tooltip": "always allow: let this request go ahead, and stop asking about farhelm commands from this host",
                    disabled: busy,
                    onclick: answer(ApprovalAnswer::AlwaysAllow),
                    "always allow from "
                    span { class: "peer-value", dir: "ltr", "{display_peer(&card.host_name)}" }
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral approval-deny",
                    "data-tooltip": "deny: refuse this request; the agent is told you declined",
                    disabled: busy,
                    onclick: answer(ApprovalAnswer::Deny),
                    "deny"
                }
            }
        }
    }
}

/// Keep a requester value separate from the UI's own label.
///
/// The requesting host is peer text just like session details. Escape its
/// controls and isolate its direction so it cannot rewrite the surrounding
/// label; CSS lets the complete value wrap within the requester grid.
#[component]
fn Row(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "approval-card-row",
            dt { "{label}" }
            dd {
                span { class: "peer-value", dir: "ltr", "{display_peer(&value)}" }
            }
        }
    }
}

/// Regroup an existing decision row without changing which values are shown.
///
/// Commands and identities need the whole card width; short facts share the
/// grid. The row model still owns content and trust distinctions, so visual
/// compactness cannot turn peer text into Farhelm's own verdict.
#[component]
fn CardDetail(row: CardRow) -> Element {
    let full = full_width_row(&row);
    let class = if full {
        "approval-card-row approval-card-full"
    } else {
        "approval-card-row"
    };
    match row {
        CardRow::Value(label, value) => rsx! {
            div { class,
                dt { "{compact_label(label)}" }
                dd { span { class: "peer-value", dir: "ltr", "{display_peer(&value)}" } }
            }
        },
        CardRow::Block(label, text) => rsx! {
            div { class: "approval-card-row approval-card-full approval-card-command",
                dt { "{label}" }
                dd { PeerBlock { class: "approval-card-block", text } }
            }
        },
        CardRow::Note(label, words) => rsx! {
            div { class,
                dt { "{compact_label(label)}" }
                dd { class: "approval-card-note", "{words}" }
            }
        },
        CardRow::Identity(label, identity) => rsx! {
            div { class,
                dt { "{label}" }
                dd { span { class: "peer-value", dir: "ltr", "{display_identity(&identity)}" } }
            }
        },
    }
}

/// Reserve a whole line for text whose shape or identity needs room to read.
/// Missing-command notes remain ordinary facts; real command text never does.
fn full_width_row(row: &CardRow) -> bool {
    matches!(
        row,
        CardRow::Block(..) | CardRow::Identity(..) | CardRow::Value("folder", _)
    )
}

/// Shorten only labels whose context is already supplied by the card section.
///
/// Peer values remain untouched. Session target/source labels keep their
/// distinctions; only the requester and new-session facts have shorter names.
/// The requester's labels keep a "from" prefix, matching its "from host" row:
/// rename, stop and restart cards label the session they act on as plain
/// "session" and "session id", and two identically labelled sessions on one
/// card would leave the user guessing which one is about to be stopped.
fn compact_label(label: &'static str) -> &'static str {
    match label {
        "asked by session" => "from session",
        "asking session id" => "from session id",
        "new session on host" => "on host",
        "resume command" => "resume",
        _ => label,
    }
}

/// Decision content before presentation separates facts from full-width text.
///
/// These variants preserve the distinction between peer text, fixed UI words,
/// and install identities regardless of which grid position they occupy.
#[derive(Debug, Clone, PartialEq)]
enum CardRow {
    /// A labelled peer fact, allowed to wrap to fit the available width.
    Value(&'static str, String),
    /// A peer value that may span lines (command text), kept in its shape.
    Block(&'static str, String),
    /// This UI's own words (yes, no, a default), not peer text.
    Note(&'static str, &'static str),
    /// An install identity, shown the way the hosts panel shows identities
    /// (`peer::display_identity`), which is stricter than ordinary peer text.
    Identity(&'static str, String),
}

/// The action's kind, for the card's data attribute (tests and styling).
fn action_kind(action: &ApprovalAction) -> &'static str {
    match action {
        ApprovalAction::Launch { .. } => "launch",
        ApprovalAction::Rename { .. } => "rename",
        ApprovalAction::Stop { .. } => "stop",
        ApprovalAction::Restart { .. } => "restart",
        ApprovalAction::TemplateWrite { .. } => "template_write",
        ApprovalAction::TemplateDelete { .. } => "template_delete",
    }
}

/// One line in this UI's own words saying what approving would do.
fn action_summary(action: &ApprovalAction) -> &'static str {
    match action {
        ApprovalAction::Launch {
            verb: LaunchVerb::Spawn | LaunchVerb::Create,
            ..
        } => "start a new session",
        ApprovalAction::Launch {
            verb: LaunchVerb::SpawnInherited,
            ..
        } => "start a new session running the asking session's own launch",
        ApprovalAction::Launch {
            verb: LaunchVerb::Clone,
            ..
        } => "start a copy of a session",
        ApprovalAction::Rename { .. } => "rename a session",
        ApprovalAction::Stop { .. } => "stop a session's agent",
        ApprovalAction::Restart { .. } => "restart a session, resuming its conversation",
        ApprovalAction::TemplateWrite {
            replaces_existing: false,
            ..
        } => "create a launch template",
        ApprovalAction::TemplateWrite {
            replaces_existing: true,
            ..
        } => "change a launch template",
        ApprovalAction::TemplateDelete { .. } => "delete a launch template",
    }
}

/// The rows that describe the action itself.
fn action_rows(action: &ApprovalAction) -> Vec<CardRow> {
    let mut rows = Vec::new();
    match action {
        ApprovalAction::Launch {
            host_name,
            cwd,
            title,
            launch,
            source,
            ..
        } => {
            if let Some(source) = source {
                session_rows(
                    "copying session",
                    "copied session id",
                    Some("copied session's host"),
                    source,
                    &mut rows,
                );
            }
            rows.push(CardRow::Value("new session on host", host_name.clone()));
            rows.push(CardRow::Value("folder", cwd.clone()));
            match title {
                Some(title) => rows.push(CardRow::Value("title", title.clone())),
                None => rows.push(CardRow::Note("title", "generated")),
            }
            launch_rows(launch, Runs::Start, &mut rows);
            // Farhelm's own verdict only where it has one: a command launch's
            // YOLO is the agent's unchecked claim, already shown as "asserted
            // YOLO", and restating it here in Farhelm's words would present
            // the claim as a finding.
            if !matches!(launch, SessionLaunch::Command(_)) {
                rows.push(CardRow::Note("YOLO", yolo_words(launch.yolo())));
            }
        }
        ApprovalAction::Rename { target, title } => {
            session_rows(
                "session",
                "session id",
                Some("session's host"),
                target,
                &mut rows,
            );
            rows.push(CardRow::Value("new title", title.clone()));
        }
        ApprovalAction::Stop { target } => {
            session_rows(
                "session",
                "session id",
                Some("session's host"),
                target,
                &mut rows,
            );
        }
        ApprovalAction::Restart {
            target,
            stop_if_running,
            launch,
        } => {
            session_rows(
                "session",
                "session id",
                Some("session's host"),
                target,
                &mut rows,
            );
            rows.push(CardRow::Note(
                "if it is working",
                if *stop_if_running {
                    "stop it first"
                } else {
                    "refuse"
                },
            ));
            match launch {
                Some(launch) => launch_rows(launch, Runs::Resume, &mut rows),
                None => rows.push(CardRow::Note("resume command", "unknown to this helm")),
            }
        }
        ApprovalAction::TemplateWrite {
            name,
            fields,
            host_name,
            ..
        } => {
            rows.push(CardRow::Value("template", name.clone()));
            template_rows(fields, host_name.as_deref(), &mut rows);
        }
        ApprovalAction::TemplateDelete {
            name,
            fields,
            host_name,
        } => {
            rows.push(CardRow::Value("template", name.clone()));
            template_rows(fields, host_name.as_deref(), &mut rows);
        }
    }
    rows
}

/// Which of a launch's commands the approved action runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Runs {
    /// A new session: the start command now, the resume command at a later
    /// restart.
    Start,
    /// A restart: only the resume command.
    Resume,
}

/// A launch's choices and the full text of what it runs. A new session's card
/// shows the start command and the resume command a later restart runs,
/// because a plain restart is not checked against the YOLO rule again; a
/// restart's card shows only the resume command, which is all a restart runs.
fn launch_rows(launch: &SessionLaunch, runs: Runs, rows: &mut Vec<CardRow>) {
    let start_label = "command";
    let resume_text = |argv: &[String]| {
        shell_words::join(argv.iter().filter(|element| {
            element.as_str() != farhelm_proto::session_launch::FARHELM_ARGS_PLACEHOLDER
        }))
    };
    match launch {
        SessionLaunch::Agent {
            selection, resume, ..
        } => {
            rows.push(CardRow::Value("agent", word(&selection.harness)));
            rows.push(optional_value("model", selection.model.clone()));
            rows.push(optional_value(
                "effort",
                selection.effort.as_ref().map(word),
            ));
            rows.push(optional_value(
                "permissions",
                selection.permissions.as_ref().map(word),
            ));
            if let Some(trust) = selection.workspace_trust {
                rows.push(CardRow::Note(
                    "workspace trust",
                    if trust { "trusted" } else { "not trusted" },
                ));
            }
            if runs == Runs::Start {
                rows.push(CardRow::Block(start_label, launch.display_command()));
            }
            match resume {
                Some(resume) => rows.push(CardRow::Block("resume command", resume_text(resume))),
                None => rows.push(CardRow::Note("resume command", "none")),
            }
        }
        SessionLaunch::Command(command) => {
            if runs == Runs::Start {
                rows.push(CardRow::Block(start_label, command.command.clone()));
            }
            match &command.resume {
                Some(resume) => rows.push(CardRow::Block("resume command", resume.clone())),
                None => rows.push(CardRow::Note("resume command", "none")),
            }
            if let Some(agent) = command.agent {
                rows.push(CardRow::Value("declared agent", word(&agent)));
            }
            rows.push(CardRow::Note(
                "asserted YOLO",
                if command.yolo { "yes" } else { "no" },
            ));
        }
        SessionLaunch::Legacy {
            invocation,
            resume_template,
            ..
        } => {
            if runs == Runs::Start {
                rows.push(CardRow::Block(start_label, invocation.clone()));
            }
            match resume_template {
                Some(resume) => rows.push(CardRow::Block("resume command", resume_text(resume))),
                None => rows.push(CardRow::Note("resume command", "none")),
            }
        }
    }
}

/// Every field a template sets, command text included.
fn template_rows(fields: &TemplateFields, host_name: Option<&str>, rows: &mut Vec<CardRow>) {
    if let Some(kind) = &fields.kind {
        rows.push(CardRow::Value("launch kind", word(kind)));
    }
    if let Some(agent) = &fields.agent {
        rows.push(CardRow::Value("agent", word(agent)));
    }
    if let Some(model) = &fields.model {
        rows.push(optional_value("model", model.clone()));
    }
    if let Some(effort) = &fields.effort {
        rows.push(optional_value("effort", effort.as_ref().map(word)));
    }
    if let Some(permissions) = &fields.permissions {
        rows.push(optional_value(
            "permissions",
            permissions.as_ref().map(word),
        ));
    }
    if let Some(trust) = &fields.workspace_trust {
        rows.push(CardRow::Note(
            "workspace trust",
            match trust {
                Some(true) => "trusted",
                Some(false) => "not trusted",
                None => "agent's default",
            },
        ));
    }
    if let Some(command) = &fields.command {
        rows.push(CardRow::Block("command", command.clone()));
    }
    if let Some(resume) = &fields.resume_command {
        match resume {
            Some(resume) => rows.push(CardRow::Block("resume command", resume.clone())),
            None => rows.push(CardRow::Note("resume command", "none")),
        }
    }
    if let Some(yolo) = fields.yolo {
        rows.push(CardRow::Note(
            "asserted YOLO",
            if yolo { "yes" } else { "no" },
        ));
    }
    if let Some(identity) = &fields.host {
        match host_name {
            Some(name) => rows.push(CardRow::Value("host", name.to_string())),
            // The install identity itself, when no registered host carries
            // it now: the template still names it, and the card must not
            // guess why there is no name for it.
            None => rows.push(CardRow::Identity("host install", identity.clone())),
        }
    }
    match &fields.destination {
        Some(TemplateDestination::Folder(folder)) => {
            rows.push(CardRow::Value("folder", folder.clone()))
        }
        Some(TemplateDestination::Github(repo)) => {
            rows.push(CardRow::Value("managed checkout of", repo.clone()))
        }
        None => {}
    }
    if let Some(name) = &fields.name {
        rows.push(CardRow::Value("session title", name.clone()));
    }
}

/// A launch choice that may be left to the agent's default.
fn optional_value(label: &'static str, value: Option<String>) -> CardRow {
    match value {
        Some(value) => CardRow::Value(label, value),
        None => CardRow::Note(label, "agent's default"),
    }
}

/// The launch's YOLO verdict in this UI's words. Unknown (a launch from
/// before launch kinds) says so rather than reading as "no": on a permission
/// prompt, guessing the safe answer is the wrong way to be wrong.
fn yolo_words(yolo: Option<bool>) -> &'static str {
    match yolo {
        Some(true) => "yes: the agent runs with no approval prompts",
        Some(false) => "no",
        None => "unknown: this launch predates YOLO classification",
    }
}

/// A session as separate labelled rows: its title (when the helm knows it),
/// its id, and its host. Kept apart, not joined into one value, because a
/// title is agent text and could otherwise imitate the id or the host and make
/// the card name a different session or machine than the one it acts on.
///
/// `host_label` names the host row; `None` leaves it out, for the asking
/// session, whose host the card already names as the one the request came
/// from.
fn session_rows(
    label: &'static str,
    id_label: &'static str,
    host_label: Option<&'static str>,
    session: &ApprovalSession,
    rows: &mut Vec<CardRow>,
) {
    match &session.title {
        Some(title) => rows.push(CardRow::Value(label, title.clone())),
        None => rows.push(CardRow::Note(label, "title unknown")),
    }
    rows.push(CardRow::Value(id_label, session.id.clone()));
    if let Some(host_label) = host_label {
        match &session.host_name {
            Some(host) => rows.push(CardRow::Value(host_label, host.clone())),
            None => rows.push(CardRow::Note(host_label, "unknown to this helm")),
        }
    }
}

/// A wire enum's own spelling, which is also the word the launcher and the
/// CLI use for it.
fn word<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::CommandLaunch;

    /// Spec: a command launch's card shows the command and the resume
    /// command in full, and says the YOLO assertion in this UI's own words.
    ///
    /// Why: SPEC.md lets a later plain restart run the resume command without
    /// another YOLO check, on the grounds that the user approved it here; a
    /// card that hid it would have the user approve something they never saw.
    #[test]
    fn a_command_launch_card_shows_both_commands() {
        let action = ApprovalAction::Launch {
            verb: LaunchVerb::Create,
            host_name: "box".to_string(),
            cwd: "/w".to_string(),
            title: None,
            launch: SessionLaunch::Command(CommandLaunch {
                command: "agent --go".to_string(),
                yolo: false,
                agent: None,
                resume: Some("agent --resume {conversation}".to_string()),
            }),
            source: None,
        };
        let rows = action_rows(&action);
        assert!(rows.contains(&CardRow::Block("command", "agent --go".to_string())));
        assert!(rows.contains(&CardRow::Block(
            "resume command",
            "agent --resume {conversation}".to_string()
        )));
        assert!(rows.contains(&CardRow::Note("asserted YOLO", "no")));
        assert!(
            !rows
                .iter()
                .any(|row| matches!(row, CardRow::Note("YOLO", _))),
            "a command launch's claim is not restated as Farhelm's verdict: {rows:?}"
        );
        assert!(rows.contains(&CardRow::Note("title", "generated")));
    }

    /// Spec: a restart card shows the resume command the restart runs, for a
    /// legacy launch too, and not the start command it does not run.
    ///
    /// Why: a plain restart is allowed without the YOLO rule because it re-runs
    /// what the user already approved, so the card is where the user sees what
    /// that is; showing the start command instead would show the wrong thing.
    #[test]
    fn a_restart_card_shows_the_resume_command_only() {
        let action = ApprovalAction::Restart {
            target: ApprovalSession {
                id: "s".to_string(),
                title: None,
                host_name: None,
            },
            stop_if_running: false,
            launch: Some(SessionLaunch::Legacy {
                invocation: "agent --start".to_string(),
                agent_kind: farhelm_proto::AgentKind::Generic,
                resume_template: Some(vec!["agent".to_string(), "--resume".to_string()]),
            }),
        };
        let rows = action_rows(&action);
        assert!(rows.contains(&CardRow::Block(
            "resume command",
            "agent --resume".to_string()
        )));
        assert!(
            !rows
                .iter()
                .any(|row| matches!(row, CardRow::Block("command", _))),
            "{rows:?}"
        );
    }

    /// Spec: a template delete's card shows the host the template names, by
    /// name when the helm knows it and by install identity otherwise.
    ///
    /// Why: SPEC.md has the card show the template being deleted, and a
    /// pinned host is part of it; claiming the install is unknown would be
    /// false for a template pinned to a connected host.
    #[test]
    fn a_template_delete_card_shows_its_host() {
        let fields = TemplateFields {
            host: Some("install-1".to_string()),
            ..TemplateFields::default()
        };
        let named = action_rows(&ApprovalAction::TemplateDelete {
            name: "t".to_string(),
            fields: fields.clone(),
            host_name: Some("box".to_string()),
        });
        assert!(named.contains(&CardRow::Value("host", "box".to_string())));
        let unnamed = action_rows(&ApprovalAction::TemplateDelete {
            name: "t".to_string(),
            fields,
            host_name: None,
        });
        assert!(unnamed.contains(&CardRow::Identity("host install", "install-1".to_string())));
    }

    /// Spec: a template write's card lists every field the template will set,
    /// including the command text the agent templates listing withholds.
    ///
    /// Why: SPEC.md has the card show the whole resulting template, because a
    /// template's command line may later run on any host.
    #[test]
    fn a_template_card_shows_its_command_text() {
        let fields = TemplateFields {
            command: Some("run-me".to_string()),
            resume_command: Some(None),
            yolo: Some(true),
            ..TemplateFields::default()
        };
        let action = ApprovalAction::TemplateWrite {
            name: "t".to_string(),
            replaces_existing: false,
            fields,
            host_name: None,
        };
        let rows = action_rows(&action);
        assert!(rows.contains(&CardRow::Block("command", "run-me".to_string())));
        assert!(rows.contains(&CardRow::Note("resume command", "none")));
        assert!(rows.contains(&CardRow::Note("asserted YOLO", "yes")));
        assert_eq!(action_summary(&action), "create a launch template");
    }
}
