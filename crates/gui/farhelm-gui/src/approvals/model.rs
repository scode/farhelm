//! What an approval card shows and offers, decided from one pending request without a window.
//!
//! SPEC.md, "Agent-spawned sessions", says what every card kind names; this module turns the helm's [`PendingApproval`]
//! into that content: the heading, the From line's parts, the labelled facts in the old app's order, the explanatory
//! notes, and the answer buttons with their wording and hover help. The drawing only lays these out, so the tests here
//! pin what a card says for every kind without a window. Ported from the old app's `approvals.rs` at the reference
//! commit (`selected_request`, `action_kind`, `action_summary`, `action_rows`, `launch_rows`, `template_rows`,
//! `session_rows`, `optional_value`, the answer-set rules and the Always-allow help).
//!
//! Almost every value on a card was written by the agent that asks: session titles, folders, command lines, template
//! names. A card that read those into a sentence would let an agent write its own approval prompt, so every such value
//! is host text ([`HostText`]) under its own label, and the only prose is this module's own labels, notes and buttons.
//! Host names are host text too: they are the user's data relayed by the helm, not the app's words. Session IDs, a
//! link's site and a template's host install are identities ([`Fact::Identity`], [`Fact::CopyIdentity`]), carried raw
//! here because they are drawn in the stricter identity spelling, which spells spaces and literal escape notation
//! distinctly so that two different references never look alike; Copy always copies the raw reference.
//!
//! A fact's label is kept as the old app's lowercase key (`"folder"`, `"session id"`), because the key also
//! decides how the value is laid out ([`Fact::shape`]); [`Fact::label`] gives the words the card shows. Nothing here
//! decides whether a request waits, may be answered, or what an answer does: the helm decides all of that, and refuses
//! an answer a card should not have offered.

use farhelm_proto::approvals::{
    ApprovalAction, ApprovalAnswer, ApprovalLaunchKind, ApprovalSession, LaunchVerb,
    PendingApproval, QuickLaunchCard, TemplateKeyCard,
};
use farhelm_proto::launcher::{TemplateDestination, TemplateFields, TemplateNameKind};
use farhelm_proto::{DeleteGuard, LaunchPermission, SessionLaunch};

use crate::environment_ui::{AgentLaunchPlace, ProviderUi};
use crate::host_text::HostText;
use crate::permission::words;
use crate::permission::{self, Shield};

// ---- The waiting list -----------------------------------------------------------------------------------------------

/// The request whose card is expanded: the one the user chose, while it still waits, otherwise the oldest.
///
/// The helm lists requests oldest first. A choice is kept by identity, never by position, so an answer or an expiry
/// elsewhere in the list cannot put another request under the user's eyes in its place; once the chosen request has
/// gone, the oldest remaining one opens (SPEC.md).
pub fn selected<'a>(
    waiting: &'a [PendingApproval],
    chosen: Option<&str>,
) -> Option<&'a PendingApproval> {
    waiting
        .iter()
        .find(|approval| Some(approval.id.as_str()) == chosen)
        .or_else(|| waiting.first())
}

/// The expanded card's count of the whole fleet's waiting requests, the collapsed ones included.
pub fn count(waiting: usize) -> String {
    format!("1 of {waiting} waiting")
}

// ---- One card -------------------------------------------------------------------------------------------------------

/// Everything one expanded card shows and offers, built from one pending request.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    /// The request's approval id: what every answer names, never either session's reference.
    pub id: String,
    /// The request's kind in the wire spelling (`"launch"`, `"host_edit"`), for element ids and tests.
    pub kind: &'static str,
    /// The action the card asks about, in the app's words ([`summary`]); the waiting list's headers use it too.
    pub heading: &'static str,
    /// The compact From line: who asked.
    pub from: Requester,
    /// The aligned list of labelled facts. Holds no [`Fact::Box`]: those are in `boxes`.
    pub facts: Vec<Fact>,
    /// The long texts (commands, prompts, wrapper words), each in a bounded box of its own after the fact list, so the
    /// layout can shrink them toward two lines before the fact list needs to scroll (SPEC.md).
    pub boxes: Vec<Fact>,
    /// The note drawn first under the facts, before any refusal the last answer left: a link's tunnel or local-network
    /// explanation.
    pub lead_note: Option<LeadNote>,
    /// The notes drawn after any refusal, in order.
    pub notes: Vec<&'static str>,
    /// The answer buttons in the order the answer bar draws them; Deny (or Dismiss) is always last, drawn apart.
    pub answers: Vec<Answer>,
}

/// The From line's parts, each kept apart so a session title cannot pose as the host or the ID.
#[derive(Clone, Debug, PartialEq)]
pub struct Requester {
    /// The host whose connection carried the request.
    pub host: HostText,
    /// The asking session's title, when the helm knows it; drawn quoted and ellipsized, its full text on hover.
    pub title: Option<HostText>,
    /// The complete entry-prefixed reference, raw. The line shows its bare ID shortened (host text's abbreviated
    /// identity spelling, which drops the entry prefix and keeps an escape whole at either end); the hover, the
    /// accessible name and Copy ID use the whole reference.
    pub id: String,
}

/// The From line's words before the full ID in its accessible name.
pub const FROM_ID_LABEL: &str = "From session ID";
/// The From line's Copy ID's accessible name: "from" keeps it apart from a target's "Copy session ID".
pub const FROM_COPY_NAME: &str = "Copy from session ID";
/// The From line's Copy button label.
pub const COPY_ID: &str = "Copy ID";
/// A target's or source's Copy button label.
pub const COPY: &str = "Copy";
/// Every session-reference Copy button's help: Copy takes the raw reference, never the escaped spelling shown.
pub const COPY_HELP: &str = "Copy: the complete session ID, including characters shown as escapes";

/// The note a link card draws first.
#[derive(Clone, Debug, PartialEq)]
pub enum LeadNote {
    /// Tunnel and open is offered: which port on which host, and that opening directly would reach this Mac.
    Tunnel { port: u16, host: HostText },
    /// A local-network link without Tunnel and open: that it needs its own host permission.
    LocalNetwork,
}

impl LeadNote {
    /// The tunnel note's words around the remote host's name, which goes between them as host text: "This is port
    /// <port> on " and ". Opening it directly would reach this Mac instead."
    pub fn tunnel_words(port: u16) -> (String, &'static str) {
        (
            format!("This is port {port} on "),
            ". Opening it directly would reach this Mac instead.",
        )
    }

    /// The local-network note's words.
    pub const LOCAL_NETWORK: &'static str = "This link addresses your own machine or local network. It needs its \
                                                    own host permission, even when public links are allowed.";
}

/// Whether this app can offer Tunnel and open on an eligible link card.
///
/// SPEC.md offers it "in the Mac app", and the answer needs the terminal's tunnel consent, so the caller decides from
/// the platform it runs on ([`crate::actions::THIS_PLATFORM`]) and whether the consent entry point exists; taking it as
/// a value rather than a `cfg` lets Linux tests cover both answer sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunnelOffer {
    Offered,
    NotOffered,
}

impl Card {
    /// The card for `approval`.
    pub fn of(approval: &PendingApproval, tunnel: TunnelOffer) -> Self {
        let action = &approval.action;
        let tunnel_port = match action {
            ApprovalAction::OpenUrl {
                url,
                tunnel_available: true,
                ..
            } if tunnel == TunnelOffer::Offered => Some(
                url::Url::parse(url)
                    .ok()
                    .and_then(|parsed| parsed.port_or_known_default())
                    .unwrap_or_default(),
            ),
            _ => None,
        };
        let (boxes, facts) = facts(action)
            .into_iter()
            .partition(|fact| matches!(fact, Fact::Box(..)));
        let lead_note = match (tunnel_port, action) {
            (Some(port), _) => Some(LeadNote::Tunnel {
                port,
                host: HostText::from_host(approval.host_name.as_str()),
            }),
            (
                None,
                ApprovalAction::OpenUrl {
                    local_network: true,
                    preapproved: false,
                    ..
                },
            ) => Some(LeadNote::LocalNetwork),
            _ => None,
        };
        Card {
            id: approval.id.clone(),
            kind: kind(action),
            heading: summary(action),
            from: Requester::of(&approval.host_name, &approval.session),
            facts,
            boxes,
            lead_note,
            notes: notes(action),
            answers: answers(approval, tunnel_port.is_some()),
        }
    }
}

impl Requester {
    /// The From line's parts for a request from `host` by `session`.
    pub(super) fn of(host: &str, session: &ApprovalSession) -> Self {
        Requester {
            host: HostText::from_host(host),
            title: session.title.as_deref().map(HostText::from_host),
            id: session.id.clone(),
        }
    }
}

// ---- Kinds and headings ---------------------------------------------------------------------------------------------

/// The action's kind, in the wire spelling.
pub fn kind(action: &ApprovalAction) -> &'static str {
    match action {
        ApprovalAction::OpenUrl { .. } => "open_url",
        ApprovalAction::Launch { .. } => "launch",
        ApprovalAction::LaunchWithPrompt { .. } => "launch_with_prompt",
        ApprovalAction::LaunchWithAgentWrapper { .. } => "launch_with_agent_wrapper",
        ApprovalAction::LaunchEnvironment { .. } => "launch_environment",
        ApprovalAction::Rename { .. } => "rename",
        ApprovalAction::Stop { .. } => "stop",
        ApprovalAction::Prompt { .. } => "prompt",
        ApprovalAction::Delete { .. } => "delete",
        ApprovalAction::Restart { .. } => "restart",
        ApprovalAction::TemplateWrite { .. } => "template_write",
        ApprovalAction::TemplateWriteWithShortcuts { .. } => "template_write_with_shortcuts",
        ApprovalAction::TemplateDelete { .. } => "template_delete",
        ApprovalAction::HostAdd { .. } => "host_add",
        ApprovalAction::HostEdit { .. } => "host_edit",
        ApprovalAction::HostRemove { .. } => "host_remove",
    }
}

/// The action in the app's words, shared by the expanded card's heading and the waiting list's headers. Never quotes a
/// value: what the action acts on is in the facts.
pub fn summary(action: &ApprovalAction) -> &'static str {
    use LaunchVerb::{Clone, Create, Spawn, SpawnInherited};
    match action {
        ApprovalAction::OpenUrl { .. } => "Open a link in your browser",
        ApprovalAction::Launch { verb, .. }
        | ApprovalAction::LaunchWithPrompt { verb, .. }
        | ApprovalAction::LaunchWithAgentWrapper { verb, .. } => match verb {
            Spawn | Create => "Start a new session",
            SpawnInherited => "Start a new session running the asking session's own launch",
            Clone => "Start a copy of a session",
        },
        ApprovalAction::LaunchEnvironment { verb, .. } => match verb {
            Clone => "Copy a session into a new sandbox or container",
            Spawn | SpawnInherited | Create => "Start a session in a new sandbox or container",
        },
        ApprovalAction::Rename { .. } => "Rename a session",
        ApprovalAction::Stop { target } => match target.launch_kind {
            Some(ApprovalLaunchKind::Shell) => "Stop a session's shell and close its terminal tabs",
            Some(ApprovalLaunchKind::Agent) => "Stop a session's agent and close its terminal tabs",
            _ => "Stop a session's main program and close its terminal tabs",
        },
        ApprovalAction::Prompt { .. } => "Send a prompt to a session's agent",
        ApprovalAction::Delete { .. } => "Delete a session and its stored state",
        ApprovalAction::Restart { .. } => "Restart a session, resuming its conversation",
        ApprovalAction::TemplateWrite {
            replaces_existing: false,
            ..
        } => "Create a launch template",
        ApprovalAction::TemplateWriteWithShortcuts {
            replaces_existing: false,
            ..
        } => "Create a launch template and assign its shortcuts",
        ApprovalAction::TemplateWrite {
            replaces_existing: true,
            ..
        } => "Change a launch template",
        ApprovalAction::TemplateWriteWithShortcuts {
            replaces_existing: true,
            ..
        } => "Change a launch template and its shortcuts",
        ApprovalAction::TemplateDelete { .. } => "Delete a launch template",
        ApprovalAction::HostAdd { .. } => "Add a host",
        ApprovalAction::HostEdit { .. } => "Change a host's alias, icon or color",
        ApprovalAction::HostRemove { .. } => "Remove a host",
    }
}

// ---- Facts ----------------------------------------------------------------------------------------------------------

/// One labelled fact on a card. The first field is the old app's lowercase key, which names the fact and decides its
/// layout; [`Fact::label`] is what the card shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Fact {
    /// A host value. Technical destinations stay on one line and scroll sideways; a URL and permission flags wrap in
    /// full, because their tails are part of the decision ([`Fact::shape`]).
    Value(&'static str, HostText),
    /// A host value that may span lines (a command, a prompt, wrapper words), kept whole in a bounded,
    /// keyboard-scrollable box.
    Box(&'static str, HostText),
    /// The app's own words (yes, none, the agent's default), never host text.
    Words(&'static str, &'static str),
    /// An identity (a link's site, a template's host install), drawn in the strict identity spelling.
    Identity(&'static str, String),
    /// A full session reference: drawn in the strict identity spelling, in full, with a Copy button that copies the
    /// raw reference.
    CopyIdentity(&'static str, String),
    /// The Permissions row: the session list's shield with the card's chip words.
    Permission(Chips),
}

/// How a fact's value is laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// On one line in the monospace face, scrolling sideways within itself: paths, repositories, SSH destinations.
    OneLine,
    /// Wrapped in full in the monospace face: a URL and permission flags, whose tails must stay visible, and a box of
    /// what a shell reads (a command, a resume command, wrapper words).
    WrappedMono,
    /// Drawn as a chip: a template's stored permission label and its YOLO assertion.
    Chip,
    /// Wrapped in the interface face: names, titles, the app's own words, and a prompt's box.
    Wrapped,
}

impl Fact {
    /// The label the card shows: the key in sentence case, with the requester's and the new session's redundant
    /// context dropped; a box's label keeps every word.
    pub fn label(&self) -> String {
        match self {
            Fact::Box(key, _) => sentence_case(key),
            Fact::Value(key, _)
            | Fact::Words(key, _)
            | Fact::Identity(key, _)
            | Fact::CopyIdentity(key, _) => sentence_case(compact(key)),
            Fact::Permission(_) => "Permissions".to_owned(),
        }
    }

    /// A copy button's accessible name and label words: "Copy " and the fact's compact label, its nouns kept lowercase
    /// ("Copy from session ID"). `None` for a fact without Copy.
    pub fn copy_name(&self) -> Option<String> {
        match self {
            Fact::CopyIdentity(key, _) => Some(format!("Copy {}", compact(key))),
            Fact::Value(..)
            | Fact::Box(..)
            | Fact::Words(..)
            | Fact::Identity(..)
            | Fact::Permission(_) => None,
        }
    }

    /// How the value is laid out.
    pub fn shape(&self) -> Shape {
        match self {
            Fact::Value(key, _) => match *key {
                "folder" | "managed checkout of" | "ssh destination" | "destination" => {
                    Shape::OneLine
                }
                "URL" | "permission flags" => Shape::WrappedMono,
                "permissions" => Shape::Chip,
                _ => Shape::Wrapped,
            },
            Fact::Words("asserted YOLO", _) => Shape::Chip,
            Fact::Identity(..) | Fact::CopyIdentity(..) => Shape::OneLine,
            // A prompt is prose and keeps the interface face (the old `.approval-card-prompt`); every other box holds
            // what a shell reads (a command, a resume command, wrapper words) in the monospace face (the old `pre`).
            Fact::Box("initial prompt" | "prompt", _) => Shape::Wrapped,
            Fact::Box(..) => Shape::WrappedMono,
            Fact::Words(..) | Fact::Permission(_) => Shape::Wrapped,
        }
    }
}

/// A key's compact words: the new session's and the resume command's redundant context dropped, acronyms kept in
/// their spelling.
fn compact(key: &'static str) -> &'static str {
    match key {
        "copied session id" => "copied session ID",
        "session id" => "session ID",
        "ssh destination" => "SSH destination",
        "new session on host" => "on host",
        "resume command" => "resume",
        _ => key,
    }
}

/// The app's fixed words with their first letter capitalized, as the card shows its labels and its own fact words
/// ("Generated", "None"); a leading ⚡ stays in front, and words that already start with an acronym (URL) are
/// unchanged.
pub fn sentence_case(words: &str) -> String {
    let mut shown = words.to_owned();
    if let Some((index, letter)) = words
        .char_indices()
        .find(|(_, letter)| letter.is_ascii_alphabetic())
    {
        shown.replace_range(index..index + 1, &letter.to_ascii_uppercase().to_string());
    }
    shown
}

/// The facts describing the action, in the old app's order.
pub fn facts(action: &ApprovalAction) -> Vec<Fact> {
    let mut facts = Vec::new();
    match action {
        ApprovalAction::OpenUrl { url, .. } => {
            // The site the browser will actually contact, parsed the way the browser parses it and spelled in ASCII:
            // credentials before an `@`, a backslash or a look-alike Unicode host can make the URL read as a
            // different site than the one it opens.
            if let Some(site) = url::Url::parse(url)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_owned))
            {
                facts.push(Fact::Identity("site", site));
            }
            facts.push(value("URL", url));
        }
        ApprovalAction::Launch {
            host_name,
            cwd,
            repo,
            title,
            launch,
            source,
            ..
        }
        | ApprovalAction::LaunchWithPrompt {
            host_name,
            cwd,
            repo,
            title,
            launch,
            source,
            ..
        }
        | ApprovalAction::LaunchWithAgentWrapper {
            host_name,
            cwd,
            repo,
            title,
            launch,
            source,
            ..
        } => {
            if let Some(source) = source {
                session_facts(Role::Source, source, &mut facts);
            }
            facts.push(value("new session on host", host_name));
            if let Some(repo) = repo {
                facts.push(value("managed checkout of", &format!("GitHub: {repo}")));
            }
            facts.push(value("folder", cwd));
            facts.push(
                title
                    .as_ref()
                    .map_or(Fact::Words("title", "generated"), |title| {
                        value("title", title)
                    }),
            );
            if let ApprovalAction::LaunchWithAgentWrapper { .. } = action
                && let Some(words) = launch
                    .agent_selection()
                    .and_then(|selection| selection.wrapper_prefix.as_ref())
            {
                facts.push(boxed("agent supplied wrapper", &words.join(" ")));
            }
            launch_facts(launch, Runs::Start, &mut facts);
            match action {
                ApprovalAction::LaunchWithPrompt { prompt, .. }
                | ApprovalAction::LaunchWithAgentWrapper {
                    prompt: Some(prompt),
                    ..
                } => facts.push(boxed("initial prompt", prompt)),
                _ => {}
            }
        }
        ApprovalAction::LaunchEnvironment {
            recipe,
            place,
            place_name,
            cwd,
            repo,
            title,
            launch,
            source,
            prompt,
            ..
        } => {
            if let Some(source) = source {
                session_facts(Role::Source, source, &mut facts);
            }
            let provider = ProviderUi(place.provider());
            facts.push(value("recipe", recipe));
            facts.push(match provider.agent_launch_place() {
                // The host's name is the helm's registry name, shown as data like every other host name on a card.
                AgentLaunchPlace::NamedHost(key) => {
                    value(key, place_name.as_deref().unwrap_or_default())
                }
                AgentLaunchPlace::Note(key, words) => Fact::Words(key, words),
            });
            facts.push(Fact::Words(
                "consequence",
                provider.agent_launch_consequence(),
            ));
            facts.push(match repo {
                // A managed checkout's folder does not exist until the environment does; the repository is the fact.
                Some(repo) => value("managed checkout of", &format!("GitHub: {repo}")),
                None => value("folder", cwd),
            });
            facts.push(
                title
                    .as_ref()
                    .map_or(Fact::Words("title", "generated"), |title| {
                        value("title", title)
                    }),
            );
            launch_facts(launch, Runs::Start, &mut facts);
            if let Some(prompt) = prompt {
                facts.push(boxed("initial prompt", prompt));
            }
        }
        ApprovalAction::Rename { target, title } => {
            session_facts(Role::Target, target, &mut facts);
            facts.push(value("new title", title));
        }
        ApprovalAction::Stop { target } => {
            session_facts(Role::Target, target, &mut facts);
        }
        ApprovalAction::Prompt {
            target,
            launch,
            screen_yolo,
            attached,
            prompt,
        } => {
            session_facts(Role::Target, target, &mut facts);
            // The target's shield as the session list draws it, observed bypass included; display evidence only, never
            // a launch this request would run.
            facts.extend(Chips::of(launch, *screen_yolo, Detail::Plain).map(Fact::Permission));
            // SPEC.md: the card "says when the target session is open in the window", the one client a helm accepts
            // (One GUI at a time).
            if *attached {
                facts.push(Fact::Words("client", "this session is open in the window"));
            }
            facts.push(Fact::Words(
                "delivery",
                "the prompt may interleave with user typing or follow text already in the composer",
            ));
            facts.push(boxed("prompt", prompt));
        }
        ApprovalAction::Delete { target, guard } => {
            session_facts(Role::Target, target, &mut facts);
            facts.push(Fact::Words(
                "what deletion stops",
                deletion_scope(*guard, target.launch_kind),
            ));
        }
        ApprovalAction::Restart {
            target,
            stop_if_running,
            launch,
        } => {
            session_facts(Role::Target, target, &mut facts);
            facts.push(Fact::Words(
                "if it is working",
                if *stop_if_running {
                    "stop it first"
                } else {
                    "refuse"
                },
            ));
            match launch {
                Some(launch) => launch_facts(launch, Runs::Resume, &mut facts),
                None => facts.push(Fact::Words("resume command", "unknown to this helm")),
            }
        }
        ApprovalAction::TemplateWrite {
            name,
            fields,
            host_name,
            ..
        }
        | ApprovalAction::TemplateDelete {
            name,
            fields,
            host_name,
        } => {
            facts.push(value("template", name));
            template_facts(fields, host_name.as_deref(), &mut facts);
        }
        ApprovalAction::TemplateWriteWithShortcuts {
            name,
            fields,
            host_name,
            shortcuts,
            ..
        } => {
            facts.push(value("template", name));
            template_facts(fields, host_name.as_deref(), &mut facts);
            match &shortcuts.quick_launch {
                Some(QuickLaunchCard::Set { moved_from }) => {
                    facts.push(Fact::Words("⚡ quick launch", "mark this template"));
                    if let Some(previous) = moved_from {
                        facts.push(value("⚡ moves from", previous));
                    }
                }
                Some(QuickLaunchCard::Clear) => {
                    facts.push(Fact::Words("⚡ quick launch", "clear this template"))
                }
                None => {}
            }
            match &shortcuts.key {
                Some(TemplateKeyCard::Set {
                    key,
                    replaced_key,
                    moved_from,
                }) => {
                    if let Some(previous) = replaced_key {
                        facts.push(value("number key removed", previous));
                    }
                    facts.push(value("number key assigned", key));
                    if let Some(previous) = moved_from {
                        facts.push(value("number key moves from", previous));
                    }
                }
                Some(TemplateKeyCard::Clear { key }) => {
                    facts.push(value("number key removed", key))
                }
                None => {}
            }
        }
        ApprovalAction::HostAdd { destination, alias } => {
            facts.push(value("ssh destination", destination));
            if let Some(alias) = alias {
                facts.push(value("alias", alias));
            }
        }
        // Each changed field shows its value now and the value approval writes; a field the request leaves alone is
        // absent. Icon and color words are the app's own fixed vocabulary; the alias and the host's names are host
        // text.
        ApprovalAction::HostEdit {
            host_name,
            destination,
            alias,
            icon,
            color,
        } => {
            facts.push(value("host", host_name));
            facts.push(
                destination
                    .as_ref()
                    .map_or(Fact::Words("destination", "this machine"), |destination| {
                        value("destination", destination)
                    }),
            );
            if let Some(alias) = alias {
                for (key, alias) in [("alias now", &alias.old), ("new alias", &alias.new)] {
                    facts.push(
                        alias
                            .as_ref()
                            .map_or(Fact::Words(key, "none"), |alias| value(key, alias)),
                    );
                }
            }
            if let Some(icon) = icon {
                facts.push(Fact::Words("icon now", icon.old.as_str()));
                facts.push(Fact::Words("new icon", icon.new.as_str()));
            }
            if let Some(color) = color {
                facts.push(Fact::Words("color now", color.old.as_str()));
                facts.push(Fact::Words("new color", color.new.as_str()));
            }
        }
        ApprovalAction::HostRemove {
            host_name,
            destination,
        } => {
            facts.push(value("host", host_name));
            facts.push(value("destination", destination));
        }
    }
    facts
}

/// A host value under `key`.
fn value(key: &'static str, raw: &str) -> Fact {
    Fact::Value(key, HostText::from_host(raw))
}

/// A long host value under `key`, in a box of its own.
fn boxed(key: &'static str, raw: &str) -> Fact {
    Fact::Box(key, HostText::from_host(raw))
}

/// Which session a card names besides the one that asked: the session a clone copies, or the session a lifecycle
/// action acts on. Each gets its own labels, so a clone card's source and a target never read alike.
#[derive(Clone, Copy)]
enum Role {
    Source,
    Target,
}

/// Name the processes this approval can stop and the liveness change that would invalidate its guard. The words
/// come from the helm's program category so an older or newer peer with an unknown kind gets neutral wording.
fn deletion_scope(guard: DeleteGuard, kind: Option<ApprovalLaunchKind>) -> &'static str {
    use ApprovalLaunchKind::{Agent, Shell};
    use DeleteGuard::{AgentEnded, NothingAlive, Unconditional};

    match (guard, kind) {
        (NothingAlive, Some(Agent)) => "nothing. Refuse if the agent or any terminal tab is alive",
        (NothingAlive, Some(Shell)) => "nothing. Refuse if the shell or any terminal tab is alive",
        (NothingAlive, _) => "nothing. Refuse if the main program or any terminal tab is alive",
        (AgentEnded, Some(Agent)) => "terminal tabs. Refuse if the agent is running again",
        (AgentEnded, Some(Shell)) => "terminal tabs. Refuse if the shell is running again",
        (AgentEnded, _) => "terminal tabs. Refuse if the main program is running again",
        (Unconditional, Some(Agent)) => "the agent and all terminal tabs",
        (Unconditional, Some(Shell)) => "the shell and all terminal tabs",
        (Unconditional, _) => "the main program and all terminal tabs",
    }
}

/// A session as separate facts: its title (when the helm knows it), its full reference with Copy, and its host. Kept
/// apart rather than joined into one value, because a title is agent text and could otherwise imitate the reference or
/// the host and make the card name a different session or machine than the one it acts on.
fn session_facts(role: Role, session: &ApprovalSession, facts: &mut Vec<Fact>) {
    let (key, id_key, host_key) = match role {
        Role::Source => (
            "copying session",
            "copied session id",
            "copied session's host",
        ),
        Role::Target => ("session", "session id", "session's host"),
    };
    facts.push(match &session.title {
        Some(title) => value(key, title),
        None => Fact::Words(key, "title unknown"),
    });
    facts.push(Fact::CopyIdentity(id_key, session.id.clone()));
    facts.push(match &session.host_name {
        Some(host) => value(host_key, host),
        None => Fact::Words(host_key, "unknown to this helm"),
    });
}

/// Which of a launch's commands the approved action runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Runs {
    /// A new session: the start command now, and the resume command a later Restart runs.
    Start,
    /// A restart: only the resume command.
    Resume,
}

/// A launch's Permissions row, its choices and the full text of what it runs. A new session's card shows the start
/// command and the resume command, because a later plain Restart runs that resume command without another check
/// against the agent YOLO rule, on the grounds that the user approved it here; a restart's card shows only the resume
/// command, which is all a restart runs. What a card shows is the stored launch: settings that changed since never
/// reinterpret it.
fn launch_facts(launch: &SessionLaunch, runs: Runs, facts: &mut Vec<Fact>) {
    facts.extend(Chips::of(launch, false, Detail::Launch).map(Fact::Permission));
    match launch {
        SessionLaunch::Agent { selection, .. } if selection.harness.is_shell() => {
            facts.push(value("shell", crate::harness::name(selection.harness)));
            if runs == Runs::Start {
                facts.push(boxed("command", &launch.display_command()));
            }
        }
        SessionLaunch::Agent { selection, .. } => {
            facts.push(value("agent", &word(&selection.harness)));
            facts.push(optional("model", selection.model.clone()));
            facts.push(optional("effort", selection.effort.as_ref().map(word)));
            if let Some(words) = &selection.permission_flags {
                facts.push(value(
                    "permission flags",
                    &farhelm_proto::join_permission_flags(words),
                ));
            }
            if runs == Runs::Start {
                facts.push(boxed("command", &launch.display_command()));
            }
            facts.push(resume(launch));
        }
        SessionLaunch::Command(command) => {
            if runs == Runs::Start {
                facts.push(boxed("command", &command.command));
            }
            facts.push(resume(launch));
            if let Some(agent) = command.agent {
                facts.push(value("declared agent", &word(&agent)));
            }
        }
        SessionLaunch::Legacy { invocation, .. } => {
            if runs == Runs::Start {
                facts.push(boxed("command", invocation));
            }
            facts.push(resume(launch));
        }
    }
}

/// The launch's resume command in a box, or the app's "none".
fn resume(launch: &SessionLaunch) -> Fact {
    launch
        .display_resume()
        .map_or(Fact::Words("resume command", "none"), |resume| {
            boxed("resume command", &resume)
        })
}

/// Every choice a template write or delete covers, including what the agent templates listing withholds (command
/// text). A field the template does not set is absent; an explicit removal shows as "none", so a card never presents
/// a cleared field as untouched or the other way round.
fn template_facts(fields: &TemplateFields, host_name: Option<&str>, facts: &mut Vec<Fact>) {
    if let Some(prompt) = &fields.prompt {
        facts.push(boxed("initial prompt", prompt));
    }
    if let Some(kind) = &fields.kind {
        facts.push(value("launch kind", &word(kind)));
    }
    if let Some(agent) = &fields.agent {
        facts.push(value("agent", &word(agent)));
    }
    if let Some(model) = &fields.model {
        facts.push(optional("model", model.clone()));
    }
    if let Some(effort) = &fields.effort {
        facts.push(optional("effort", effort.as_ref().map(word)));
    }
    if let Some(permissions) = &fields.permissions {
        // The template's stored label, worded as the templates summary words it; never classified as a launch, since
        // A template's omitted choice uses the default for the new launch.
        facts.push(value(
            "permissions",
            &crate::templates::summary::permission_label(fields.agent, *permissions),
        ));
    }
    // The words themselves, as a launch card shows them: an agent can write a custom template's flags, which can make
    // the agent's command line run arbitrary commands, and approving a card that said only "custom" would approve
    // flags the user never saw.
    if let Some(words) = &fields.permission_flags {
        facts.push(value(
            "permission flags",
            &farhelm_proto::join_permission_flags(words),
        ));
    }
    if let Some(wrapper) = &fields.wrapper_prefix {
        facts.push(
            wrapper
                .as_ref()
                .map_or(Fact::Words("wrapper prefix", "none"), |words| {
                    boxed("wrapper prefix", &words.join(" "))
                }),
        );
    }
    // Old template records stay decodable, but their container settings no longer describe a runnable choice; a card
    // must not present them as launch options.
    if let Some(container) = &fields.container {
        facts.push(Fact::Words(
            "retired container setting",
            if container.is_some() {
                farhelm_proto::RETIRED_CONTAINER_REASON
            } else {
                "remove the retired choice"
            },
        ));
    }
    if let Some(command) = &fields.command {
        facts.push(boxed("command", command));
    }
    if let Some(resume) = &fields.resume_command {
        facts.push(
            resume
                .as_ref()
                .map_or(Fact::Words("resume command", "none"), |resume| {
                    boxed("resume command", resume)
                }),
        );
    }
    if let Some(yolo) = fields.yolo {
        facts.push(Fact::Words(
            "asserted YOLO",
            if yolo { "yes" } else { "no" },
        ));
    }
    if let Some(identity) = &fields.host {
        facts.push(match host_name {
            Some(name) => value("host", name),
            // The install identity itself, when no registered host carries it now: the template still names it, and
            // the card must not guess why there is no name for it.
            None => Fact::Identity("host install", identity.clone()),
        });
    }
    match &fields.destination {
        Some(TemplateDestination::Folder(folder)) => facts.push(value("folder", folder)),
        Some(TemplateDestination::Github(repo)) => {
            facts.push(value("managed checkout of", &format!("GitHub: {repo}")))
        }
        None => {}
    }
    if let Some(name) = &fields.name {
        facts.push(value("session title", name));
        facts.push(Fact::Words(
            "session name kind",
            match fields.name_kind {
                Some(TemplateNameKind::Fixed) => "fixed name",
                None | Some(TemplateNameKind::Base) => "base name",
            },
        ));
    }
}

/// A launch choice that may be left to the agent's default.
fn optional(key: &'static str, chosen: Option<String>) -> Fact {
    chosen.map_or(Fact::Words(key, "agent's default"), |chosen| {
        value(key, &chosen)
    })
}

/// A wire enum's own spelling, which is also the word the launcher and the command line use for it.
fn word<T: serde::Serialize>(chosen: &T) -> String {
    serde_json::to_value(chosen)
        .ok()
        .and_then(|chosen| chosen.as_str().map(str::to_owned))
        .unwrap_or_default()
}

// ---- Permission chips -----------------------------------------------------------------------------------------------

/// A Permissions row: the shield the session list would draw for the launch, the chip's words, and, on a launch or
/// restart card, the guarded mode's own name and the stored workspace trust (SPEC.md, "Agent-spawned sessions").
#[derive(Clone, Debug, PartialEq)]
pub struct Chips {
    /// The session list's classification ([`permission::permission`]), which decides the chip's drawing and whether it
    /// warns, so the card can never look safer than the session's row. Private because the shield type is
    /// crate-internal ([`crate::permission`]); [`Chips::icon`] and [`Chips::ink`] draw it.
    shield: Shield,
    /// Which of the shield's meanings the chip names.
    pub kind: ChipKind,
    /// The agent's own name for its guarded mode ("Guarded · Claude auto mode"), on a launch or restart card whose
    /// stored permission is Guarded.
    pub guarded: Option<&'static str>,
    /// The stored workspace trust, when the launch records one.
    pub trust: Option<bool>,
}

/// What the chip names. Several share a shield; the words tell them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipKind {
    Yolo,
    Guarded,
    Unknown,
    Approve,
    SmartApprove,
    Chat,
    /// A command launch asserted YOLO: the launching user's or agent's unchecked claim.
    AssertedYolo,
    /// A command launch asserted not YOLO: an unchecked claim too.
    AssertedNotYolo,
}

/// How much a Permissions row says: a launch or restart card adds the guarded mode's name and the workspace trust; a
/// prompt card shows only the target's chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Detail {
    Launch,
    Plain,
}

/// The chip explains what the recorded trust choice allows. Directory scope depends on the harness.
pub const TRUSTED_WORKSPACE: &str = "Enabled on the agent's Settings page. Repository hooks, settings and commands can run without asking.";

impl Chips {
    /// The card has a Permissions row only for launches with a shield.
    fn of(launch: &SessionLaunch, screen_yolo: bool, detail: Detail) -> Option<Self> {
        let shield = permission::permission(launch, screen_yolo)?;
        let kind = ChipKind::of(shield, launch);
        let selection = launch
            .agent_selection()
            .filter(|_| detail == Detail::Launch);
        Some(Chips {
            shield,
            kind,
            guarded: selection
                .filter(|_| kind == ChipKind::Guarded)
                .and_then(|selection| words::guarded_mode_name(selection.harness)),
            trust: selection.and_then(|selection| selection.workspace_trust),
        })
    }

    /// The session list's shield for the launch.
    pub(crate) fn shield(&self) -> Shield {
        self.shield
    }

    /// The shield's drawing, as the session list draws it.
    pub fn icon(&self) -> crate::assets::Icon {
        self.shield().icon()
    }

    /// The shield's ink, as an unselected session row inks it.
    pub fn ink(&self, palette: &crate::theme::Palette) -> crate::theme::Token {
        self.shield().ink(palette)
    }

    /// Warn for any launch represented by the YOLO shield, including an
    /// agent launch without a recorded permission mode.
    pub fn warning(&self) -> bool {
        self.shield() == Shield::Yolo
    }

    /// The words beside the chip: the guarded mode's own name when the card has it, else the meaning's description.
    pub fn description(&self) -> &str {
        self.guarded.unwrap_or(self.kind.description())
    }
}

impl ChipKind {
    /// The chip's meaning, from the session list's shield and the launch. The shield decides; the launch only names
    /// which of the shield's meanings it is, so this can never name a meaning safer than the shield.
    fn of(shield: Shield, launch: &SessionLaunch) -> Self {
        match (shield, launch) {
            (Shield::Yolo, SessionLaunch::Command(command)) if command.yolo => {
                ChipKind::AssertedYolo
            }
            (Shield::Yolo, _) => ChipKind::Yolo,
            (Shield::Shielded, SessionLaunch::Agent { selection, .. }) => {
                match selection.harness.stored_permission(selection.permissions) {
                    Some(LaunchPermission::Guarded) => ChipKind::Guarded,
                    Some(LaunchPermission::Approve) => ChipKind::Approve,
                    Some(LaunchPermission::SmartApprove) => ChipKind::SmartApprove,
                    Some(LaunchPermission::Chat) => ChipKind::Chat,
                    // The shield never calls these shielded; were that rule to change, say nothing reassuring.
                    Some(LaunchPermission::Yolo | LaunchPermission::Custom) | None => {
                        ChipKind::Unknown
                    }
                }
            }
            (Shield::Shielded, SessionLaunch::Command(_)) => ChipKind::AssertedNotYolo,
            (Shield::Shielded, SessionLaunch::Legacy { .. }) | (Shield::Unknown, _) => {
                ChipKind::Unknown
            }
        }
    }

    /// The chip's words.
    pub const fn label(self) -> &'static str {
        match self {
            ChipKind::Yolo => "YOLO",
            ChipKind::Guarded => "Guarded",
            ChipKind::Unknown => "Unknown",
            ChipKind::Approve => "Approve",
            ChipKind::SmartApprove => "Smart approve",
            ChipKind::Chat => "Chat",
            ChipKind::AssertedYolo => "Asserted YOLO",
            ChipKind::AssertedNotYolo => "Asserted not YOLO",
        }
    }

    /// What the meaning is, beside the chip.
    pub const fn description(self) -> &'static str {
        match self {
            ChipKind::Yolo => "Farhelm applies the host's YOLO rules to this launch",
            ChipKind::Guarded => "Guarded permission mode",
            ChipKind::Unknown => {
                "Unclassified — a command from before launch kinds, which Farhelm never classified"
            }
            ChipKind::AssertedYolo => {
                "YOLO, as asserted when the command was launched, by the user or by an agent — not checked by Farhelm"
            }
            ChipKind::AssertedNotYolo => {
                "Not YOLO, as asserted when the command was launched, by the user or by an agent — not checked by \
                 Farhelm"
            }
            ChipKind::Approve => "Approve permission mode",
            ChipKind::SmartApprove => "Smart approve permission mode",
            ChipKind::Chat => "Chat permission mode",
        }
    }
}

// ---- Notes ----------------------------------------------------------------------------------------------------------

/// The notes drawn after any refusal, in order.
fn notes(action: &ApprovalAction) -> Vec<&'static str> {
    let mut notes = Vec::new();
    if let ApprovalAction::LaunchWithAgentWrapper { .. } = action {
        notes.push(
            "The agent supplied this wrapper. Farhelm cannot check what it runs; this request needs \
             your Allow.",
        );
    }
    if let ApprovalAction::HostRemove { .. } = action {
        notes.push(
            "Farhelm will forget this host. Its supervisor and sessions keep running, and adding the destination again \
             finds them.",
        );
    }
    if let ApprovalAction::HostAdd { .. }
    | ApprovalAction::HostEdit { .. }
    | ApprovalAction::HostRemove { .. } = action
    {
        notes.push(
            "Each host change needs your approval, even when the requesting host can run Farhelm commands without \
             asking.",
        );
    }
    notes
}

// ---- Answers --------------------------------------------------------------------------------------------------------

/// One answer button.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// What the button answers.
    pub answer: ApprovalAnswer,
    /// Its words.
    pub label: AnswerLabel,
    /// Its hover help.
    pub help: AnswerHelp,
    /// Whether it is the card's main action, drawn in the primary fill.
    pub primary: bool,
}

/// An answer button's words: the app's own, or a sentence that names the requesting host as host text.
#[derive(Clone, Debug, PartialEq)]
pub enum AnswerLabel {
    Words(&'static str),
    /// "Always allow on <host> only" ([`ON_HOST_ONLY`] around the host).
    OnHostOnly(HostText),
    /// "Always allow from <host>" ([`FROM_HOST`] before the host).
    FromHost(HostText),
}

/// The narrow grant's label around the requesting host's name.
pub const ON_HOST_ONLY: (&str, &str) = ("Always allow on ", " only");
/// The fleet-wide grant's label before the requesting host's name.
pub const FROM_HOST: &str = "Always allow from ";
/// The narrow grant's help around the requesting host's name, which appears twice: between the first and second
/// pieces and between the second and third.
pub const OWN_HOST_HELP: [&str; 3] = [
    "Always allow: let agents in ",
    "'s sessions start, stop, restart, rename and delete sessions on ",
    " without asking. Requests for other hosts still ask.",
];

/// An answer button's hover help.
#[derive(Clone, Debug, PartialEq)]
pub enum AnswerHelp {
    Words(&'static str),
    /// The narrow grant's help, which names the requesting host twice ([`OWN_HOST_HELP`] around it), so a reader
    /// learns that it covers session actions on that host alone.
    OwnHost(HostText),
}

/// Help for the answers that send Allow immediately. Adding a host first opens the Add host dialog.
pub const ALLOW_HELP: &str = "Allow: let this one request go ahead";
/// Deny's help, also on Dismiss.
pub const DENY_HELP: &str = "Deny: refuse this request; the agent is told you declined";
/// Tunnel and open's help. The old app's button carried none; the component layer requires one, so this states what
/// SPEC.md says the answer does.
pub const TUNNEL_HELP: &str = "Tunnel and open: reach this port on the remote host through a tunnel, \
                               then open it in your browser";

/// The answer set SPEC.md gives the request, in the answer bar's order.
///
/// Always allow is offered only on a card whose action may be granted for good: never on a preapproved link, a link
/// offering Tunnel and open, or a card that needs a direct Allow (host changes, an agent's wrapper prefix, a template
/// write that leaves a prefix, a new environment). The narrower "on <host> only" is offered only when the helm marks
/// the request eligible, before the fleet-wide one. A host add's Allow opens the add dialog rather than answering.
fn answers(approval: &PendingApproval, tunnel_offered: bool) -> Vec<Answer> {
    let action = &approval.action;
    let preapproved = matches!(
        action,
        ApprovalAction::OpenUrl {
            preapproved: true,
            ..
        }
    );
    let mut answers = Vec::new();
    if tunnel_offered {
        answers.push(Answer {
            answer: ApprovalAnswer::TunnelAndOpen,
            label: AnswerLabel::Words("Tunnel and open"),
            help: AnswerHelp::Words(TUNNEL_HELP),
            primary: true,
        });
    }
    answers.push(Answer {
        answer: ApprovalAnswer::Allow,
        label: AnswerLabel::Words(match action {
            ApprovalAction::HostAdd { .. } => "Open add dialog",
            _ if tunnel_offered || preapproved => "Open",
            _ => "Allow",
        }),
        help: AnswerHelp::Words(match action {
            ApprovalAction::HostAdd { .. } => "Open add dialog: review the host before adding it",
            _ => ALLOW_HELP,
        }),
        primary: !tunnel_offered,
    });
    if !preapproved && !tunnel_offered && !action.requires_direct_allow() {
        let host = HostText::from_host(approval.host_name.as_str());
        if approval.own_host_session_action {
            answers.push(Answer {
                answer: ApprovalAnswer::AlwaysAllowOwnHost,
                label: AnswerLabel::OnHostOnly(host.clone()),
                help: AnswerHelp::OwnHost(host.clone()),
                primary: false,
            });
        }
        answers.push(Answer {
            answer: ApprovalAnswer::AlwaysAllow,
            label: AnswerLabel::FromHost(host),
            help: AnswerHelp::Words(always_allow_help(action)),
            primary: false,
        });
    }
    answers.push(Answer {
        answer: ApprovalAnswer::Deny,
        label: AnswerLabel::Words(if preapproved && !tunnel_offered {
            "Dismiss"
        } else {
            "Deny"
        }),
        help: AnswerHelp::Words(DENY_HELP),
        primary: false,
    });
    answers
}

/// Always allow's help: a link card's grant covers only its category of link, never a broader host permission.
fn always_allow_help(action: &ApprovalAction) -> &'static str {
    match action {
        ApprovalAction::OpenUrl {
            local_network: true,
            ..
        } => "Always allow: stop asking about local-network links from this host",
        ApprovalAction::OpenUrl {
            local_network: false,
            ..
        } => "Always allow: stop asking about public links from this host",
        _ => "Always allow: stop asking about farhelm commands from this host",
    }
}
