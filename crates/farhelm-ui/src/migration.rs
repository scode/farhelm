//! The move to new terminals, as a session shows it (SPEC.md, "Terminal
//! experience"): the migration band at the top of the session view, and the
//! terminal kinds in the session's details bar.
//!
//! Every terminal is either old (a pane in the supervisor's private tmux
//! server) or new (a terminal host of its own), and never changes kind while
//! it lives. A session moves to new terminals only by getting new ones:
//! Restart, Clone or Replace for its agent, closing and reopening for a tab.
//! This module decides, from the session the helm reported, whether the
//! user should be told that, and what to call each terminal.
//!
//! Both surfaces stay silent until the session's supervisor creates new
//! terminals (`Session::supervisor_creates_host_terminals`): before that
//! there is nothing to move a session to, and a host kept on old terminals on
//! purpose (the README and docs screenshot harnesses' stacks) must look as it
//! always has. The one exception is the details bar of a session that
//! already has a new terminal, which names the kinds whatever the switch
//! says now: once one terminal is new, "old" is no longer what every
//! terminal is, and the user can only tell them apart from here.
//!
//! The decisions are pure functions of a [`Session`] ([`migration_case`],
//! [`terminal_kinds`]); the components only render them. The band's
//! dismissal is the one piece of state, and it is deliberately the
//! component's own: the session view is keyed by session
//! (`SessionView { key: ... }` in `lib.rs`), so opening another session and
//! coming back mounts a fresh band, which is what "back on the session's
//! next visit" means. It is kept per message, so hiding the old-agent
//! message does not hide the old-tabs one the session may come to need
//! later in the same visit. Nothing about it is stored or shared with other
//! clients. Tabs are named by the labels the strip shows them under
//! (`tabs::visible_tab_infos`), not by their place in the last detail read.

use crate::Session;
use crate::tabs::VisibleTabs;
use dioxus::prelude::*;
use farhelm_proto::TerminalKind;

/// The docs page the band links to: what is changing, why, and how to move
/// a session (`website/src/content/docs/docs/using/new-terminals.md`).
pub(crate) const DOCS_URL: &str = "https://farhelm.io/docs/using/new-terminals/";

/// Which of the band's two messages a session needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MigrationCase {
    /// The agent's terminal is old. `restart_available` is whether this
    /// session can be restarted at all (SPEC.md, "Lifecycle operations"):
    /// when it cannot, the band offers only Clone and Replace, which start a
    /// fresh conversation.
    OldAgent { restart_available: bool },
    /// The agent's terminal is new, and these tabs (by their labels, in the
    /// tab strip's order) are old. Restarting the agent would not move them;
    /// only closing them and opening new ones does.
    OldTabs { labels: Vec<String> },
}

/// Whether the session view shows the migration band for `session`, and
/// which message, ignoring dismissal (the component's own state).
///
/// `tabs` are the tabs the strip shows, in its order, with their kinds
/// ([`crate::tabs::visible_tab_infos`]): the band names old tabs by the
/// labels the user sees, which can differ from the server's list for a
/// moment after the user closes or opens one. Naming the server's
/// "Terminal 1" after the strip has renumbered would point the user at the
/// wrong shell.
///
/// No band while the session's supervisor does not create new terminals, for
/// a session shown from a disconnected host's last known state (its notice
/// already says what is shown may be out of date, and nothing can be moved
/// until it is back), or for a session whose agent has no terminal (a
/// terminal a reboot took has nothing left to move). `restart_available`
/// is the session view's own reading of whether Restart can run, so the
/// band never offers an action the header has greyed out.
pub(crate) fn migration_case(
    session: &Session,
    tabs: &VisibleTabs,
    restart_available: bool,
) -> Option<MigrationCase> {
    if !session.supervisor_creates_host_terminals || session.stale {
        return None;
    }
    match session.agent_terminal? {
        TerminalKind::Tmux => Some(MigrationCase::OldAgent { restart_available }),
        TerminalKind::Host => {
            let labels: Vec<String> = tabs
                .as_slice()
                .iter()
                .enumerate()
                .filter(|(_, tab)| tab.kind == Some(TerminalKind::Tmux))
                .map(|(index, _)| crate::tabs::tab_label(index))
                .collect();
            (!labels.is_empty()).then_some(MigrationCase::OldTabs { labels })
        }
    }
}

impl MigrationCase {
    /// Which message this is, as the band's `data-migration` attribute and
    /// its dismissal name it. Two different sets of old tabs are the same
    /// message; an old agent and old tabs are not.
    fn message(&self) -> &'static str {
        match self {
            MigrationCase::OldAgent { .. } => "agent",
            MigrationCase::OldTabs { .. } => "tabs",
        }
    }
}

/// The band's words for one case: a bold first sentence that says what is
/// true of this session, then what to do about it, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BandText {
    pub(crate) headline: String,
    pub(crate) body: String,
}

/// Word the band for `case`.
///
/// Every message says "when convenient": nothing is broken, an old terminal
/// keeps working, and moving costs the user a restart or a fresh
/// conversation, so the band must not read as an error or a deadline. It
/// promises nothing about when old terminals stop working.
pub(crate) fn band_text(case: &MigrationCase) -> BandText {
    const NEW_TERMINALS: &str = "Farhelm now runs new terminals without tmux.";
    match case {
        MigrationCase::OldAgent {
            restart_available: true,
        } => BandText {
            headline: "This session still runs in an old terminal.".into(),
            body: format!(
                "{NEW_TERMINALS} Old terminals keep working, but when convenient, restart this \
                 session to move it to a new one; Restart keeps its conversation. Clone and \
                 Replace start in a new terminal too."
            ),
        },
        MigrationCase::OldAgent {
            restart_available: false,
        } => BandText {
            headline: "This session still runs in an old terminal.".into(),
            body: format!(
                "{NEW_TERMINALS} Old terminals keep working, but this session can't be \
                 restarted, so to move it to a new one, clone or replace it when convenient. \
                 Either one starts a fresh conversation unless you pick a conversation to \
                 continue."
            ),
        },
        MigrationCase::OldTabs { labels } => {
            let (headline, them) = match labels.as_slice() {
                [only] => (format!("{only} is still an old terminal."), "it"),
                _ => (
                    format!("{} are still old terminals.", join_labels(labels)),
                    "them",
                ),
            };
            BandText {
                headline,
                body: format!(
                    "{NEW_TERMINALS} The agent already runs in a new one, but restarting it does \
                     not move this session's tabs. When convenient, close {them} and open a new \
                     tab."
                ),
            }
        }
    }
}

/// "Terminal 1", "Terminal 1 and Terminal 2", "Terminal 1, Terminal 2 and
/// Terminal 3": the tab labels as a sentence names them.
fn join_labels(labels: &[String]) -> String {
    match labels {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// What the details bar says about a session's terminal kinds: a short
/// label, and a description naming every terminal's kind for the tooltip
/// and screen readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalKinds {
    pub(crate) label: String,
    pub(crate) description: String,
}

/// The session details' terminal kinds (SPEC.md, "Terminal experience"),
/// or `None` when the details bar shows none.
///
/// `tabs` are the tabs the strip shows, as for [`migration_case`], so the
/// description's tab labels are the strip's.
///
/// Shown while the session's supervisor creates new terminals, or while any
/// of the session's terminals is already new: in both cases the kinds can
/// differ between sessions, or within one. Otherwise every terminal the
/// session could have is old, as every terminal always was, and the bar
/// stays as it was. Terminals whose kind was not reported are left out, and
/// a session with none reported shows nothing.
pub(crate) fn terminal_kinds(session: &Session, tabs: &VisibleTabs) -> Option<TerminalKinds> {
    let terminals: Vec<(String, TerminalKind)> = session
        .agent_terminal
        .map(|kind| ("Agent".to_string(), kind))
        .into_iter()
        .chain(
            tabs.as_slice()
                .iter()
                .enumerate()
                .filter_map(|(index, tab)| {
                    tab.kind.map(|kind| (crate::tabs::tab_label(index), kind))
                }),
        )
        .collect();
    let any_new = terminals
        .iter()
        .any(|(_, kind)| *kind == TerminalKind::Host);
    let any_old = terminals
        .iter()
        .any(|(_, kind)| *kind == TerminalKind::Tmux);
    if terminals.is_empty() || !(session.supervisor_creates_host_terminals || any_new) {
        return None;
    }
    let plural = terminals.len() > 1;
    let label = match (any_old, any_new) {
        (true, true) => "Old and new terminals",
        (true, false) if plural => "Old terminals",
        (true, false) => "Old terminal",
        (false, _) if plural => "New terminals",
        (false, _) => "New terminal",
    };
    let description = terminals
        .iter()
        .map(|(name, kind)| format!("{name}: {}", kind_words(*kind)))
        .collect::<Vec<_>>()
        .join(". ");
    Some(TerminalKinds {
        label: label.to_string(),
        description: format!("{description}."),
    })
}

/// One terminal's kind in the details' words. Old terminals name tmux,
/// since that is what makes them old and what they still need installed.
fn kind_words(kind: TerminalKind) -> &'static str {
    match kind {
        TerminalKind::Tmux => "old terminal (tmux)",
        TerminalKind::Host => "new terminal",
    }
}

/// Which message the user dismissed during this visit, if any
/// ([`MigrationCase::message`]).
///
/// Keyed by the message rather than a plain flag: once the user has hidden
/// "this session still runs in an old terminal" and restarted, the session
/// can need the other message (its tabs did not move), which says something
/// the first never did, so it shows even in the same visit.
type Dismissed = Signal<Option<&'static str>>;

/// The band's dismissal, owned by the band itself so that it lasts exactly
/// as long as this visit to the session (see the module docs).
fn use_band_dismissal() -> Dismissed {
    use_signal(|| None)
}

/// What × does: hide `case`'s message for the rest of the visit.
fn dismiss(mut dismissed: Dismissed, case: &MigrationCase) {
    dismissed.set(Some(case.message()));
}

/// The migration band: loud, above the terminal, dismissible until the
/// session's next visit, linking to the docs page that explains the move.
///
/// `tabs` are the tabs the strip shows ([`migration_case`]). Announced as a
/// status without taking focus, like the other bands above the terminal,
/// and takes layout space rather than covering the terminal (the terminal's
/// resize observer refits it when the band comes and goes). Its actions are
/// only the link and the dismissal: the header's own Restart, Clone and
/// Replace are where moving happens, with their usual confirmations.
#[component]
pub(crate) fn MigrationBand(
    session: Session,
    tabs: VisibleTabs,
    restart_available: bool,
) -> Element {
    let dismissed = use_band_dismissal();
    // Tests find this mounted band through it (`tests::dismissal::BandProbe`) to click
    // its × and read what it rendered; nothing outside the tests provides one.
    #[cfg(all(test, native_desktop))]
    if let Some(probe) = try_use_context::<tests::dismissal::BandProbe>() {
        probe.0.set(Some(dioxus::core::current_scope_id()));
    }
    let case = migration_case(&session, &tabs, restart_available)
        .filter(|case| dismissed() != Some(case.message()));
    let Some(case) = case else {
        return rsx! {};
    };
    let text = band_text(&case);
    let message = case.message();
    rsx! {
        div { class: "migration-band", role: "status", aria_live: "polite", "data-migration": message,
            div { class: "migration-band-copy",
                strong { "{text.headline} " }
                "{text.body}"
            }
            div { class: "migration-band-actions",
                a { class: "migration-band-link", href: DOCS_URL, target: "_blank", rel: "noopener",
                    "data-tooltip": "Read what is changing and how to move a session, in your browser",
                    onclick: move |event| {
                        event.prevent_default();
                        crate::app_bar::open_external_page(DOCS_URL);
                    },
                    "What's changing ↗"
                }
                button { r#type: "button", class: "btn btn-neutral migration-band-dismiss",
                    aria_label: "Hide this note until you next open this session",
                    "data-tooltip": "Hide this note until you next open this session",
                    onclick: move |_| dismiss(dismissed, &case),
                    "×"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RestartOffer, SessionStatus, Tab};
    use std::collections::{HashMap, HashSet};

    /// A live session on a host whose supervisor creates new terminals, with
    /// an old agent terminal and no tabs; each test changes what it is about.
    fn session(id: &str) -> Session {
        Session {
            screen_yolo: false,
            claude_trust_prompt: false,
            workspace_trust_prompt: false,
            id: id.into(),
            title: "work".into(),
            cwd: "/tmp".into(),
            canonical_cwd: None,
            invocation: "agent".into(),
            agent_kind: crate::SessionAgentKind::Unrecognized,
            launch: None,
            status: SessionStatus::Running,
            annotation: None,
            restart_offer: RestartOffer::Resume,
            created_at: 0,
            last_activity_at: 0,
            tabs: Vec::new(),
            agent_terminal: Some(TerminalKind::Tmux),
            supervisor_creates_host_terminals: true,
            host: Some(1),
            host_identity: None,
            host_name: Some("box".into()),
            stale: false,
            github_repo: None,
            working_copy: None,
            seen_activity_at: None,
            pinned: false,
            notifications: Vec::new(),
            notifications_read_through: 0,
        }
    }

    fn tab(id: &str, kind: Option<TerminalKind>) -> Tab {
        Tab {
            id: id.into(),
            kind,
        }
    }

    /// `tabs` as the strip would show them with nothing pending: the only
    /// way to build a [`VisibleTabs`] is the strip's own function.
    fn strip(tabs: Vec<Tab>) -> VisibleTabs {
        crate::tabs::visible_tab_infos(&tabs, &[], &HashMap::new(), &HashSet::new())
    }

    /// Spec (goal s7, first case): an old agent terminal gets the band that
    /// says to restart when convenient, and names Restart only while the
    /// session can be restarted; otherwise it offers Clone and Replace and
    /// says they start a fresh conversation by default. Matters because the
    /// band must never point at an action the header greys out.
    #[farhelm_testtrace::test]
    fn an_old_agent_is_told_to_restart_or_else_to_clone_or_replace() {
        let old = session("s");
        let case =
            migration_case(&old, &strip(Vec::new()), true).expect("an old agent gets the band");
        assert_eq!(
            case,
            MigrationCase::OldAgent {
                restart_available: true
            }
        );
        let text = band_text(&case);
        assert!(text.headline.contains("old terminal"), "{text:?}");
        assert!(text.body.contains("restart this session"), "{text:?}");
        assert!(text.body.contains("when convenient"), "{text:?}");

        let case =
            migration_case(&old, &strip(Vec::new()), false).expect("still banded without Restart");
        let text = band_text(&case);
        assert!(!text.body.contains("restart this session"), "{text:?}");
        assert!(text.body.contains("clone or replace it"), "{text:?}");
        assert!(
            text.body.contains("fresh conversation unless you pick"),
            "{text:?}"
        );
        assert!(text.body.contains("when convenient"), "{text:?}");
    }

    /// Spec (goal s7, second case): a new agent with old tabs gets the band
    /// that names those tabs, by their strip labels, and says restarting the
    /// agent would not move them. New tabs and tabs whose kind was not
    /// reported are not named.
    #[farhelm_testtrace::test]
    fn a_new_agent_with_old_tabs_is_told_to_reopen_them() {
        let mut mixed = session("s");
        mixed.agent_terminal = Some(TerminalKind::Host);
        let mut tabs = vec![
            tab("a", Some(TerminalKind::Tmux)),
            tab("b", Some(TerminalKind::Host)),
            tab("c", Some(TerminalKind::Tmux)),
            tab("d", None),
        ];
        let case =
            migration_case(&mixed, &strip(tabs.clone()), true).expect("old tabs get the band");
        assert_eq!(
            case,
            MigrationCase::OldTabs {
                labels: vec!["Terminal 1".into(), "Terminal 3".into()]
            }
        );
        let text = band_text(&case);
        assert_eq!(
            text.headline,
            "Terminal 1 and Terminal 3 are still old terminals."
        );
        assert!(
            text.body.contains("restarting it does not move"),
            "{text:?}"
        );
        assert!(
            text.body.ends_with("close them and open a new tab."),
            "{text:?}"
        );

        tabs.truncate(1);
        let text =
            band_text(&migration_case(&mixed, &strip(tabs.clone()), true).expect("one old tab"));
        assert_eq!(text.headline, "Terminal 1 is still an old terminal.");
        assert!(
            text.body.ends_with("close it and open a new tab."),
            "{text:?}"
        );
    }

    /// Spec: the band and the details label tabs as the strip does, from the
    /// strip's visible list: a tab the user just closed is gone from both at
    /// once and the later tabs are renumbered, even while the next detail
    /// read (which would drop it from the session) has not arrived; a tab
    /// just opened is counted with the kind its open reply gave.
    ///
    /// Why: the band tells the user which tab to close. Numbered from the
    /// server's list while the strip has already renumbered, "close Terminal
    /// 1" would name a different shell, one that may be new and running
    /// something the user wants.
    #[farhelm_testtrace::test]
    fn tabs_are_named_by_the_strips_labels_while_a_close_or_open_is_pending() {
        let mut mixed = session("s");
        mixed.agent_terminal = Some(TerminalKind::Host);
        // The detail read the view last accepted, held from before the close.
        mixed.tabs = vec![
            tab("a", Some(TerminalKind::Tmux)),
            tab("b", Some(TerminalKind::Host)),
            tab("c", Some(TerminalKind::Tmux)),
        ];
        let closed = HashSet::from(["a".to_string()]);
        let visible = crate::tabs::visible_tab_infos(&mixed.tabs, &[], &HashMap::new(), &closed);
        assert_eq!(
            visible
                .as_slice()
                .iter()
                .map(|tab| tab.id.as_str())
                .collect::<Vec<_>>(),
            crate::tabs::visible_tabs(&mixed.tabs, &[], &closed),
            "the same list, in the same order, as the strip"
        );
        let text = band_text(&migration_case(&mixed, &visible, true).expect("c is still old"));
        assert_eq!(text.headline, "Terminal 2 is still an old terminal.");
        assert_eq!(
            terminal_kinds(&mixed, &visible)
                .expect("kinds shown")
                .description,
            "Agent: new terminal. Terminal 1: new terminal. Terminal 2: old terminal (tmux)."
        );

        // An optimistic open, not yet in any detail read, with its kind from
        // the open reply; and closing the last old tab ends the band.
        let opened = vec![("d".to_string(), 0)];
        let opened_kinds = HashMap::from([("d".to_string(), TerminalKind::Tmux)]);
        let closed = HashSet::from(["a".to_string(), "c".to_string()]);
        let visible = crate::tabs::visible_tab_infos(&mixed.tabs, &opened, &opened_kinds, &closed);
        let text = band_text(&migration_case(&mixed, &visible, true).expect("d is old"));
        assert_eq!(text.headline, "Terminal 2 is still an old terminal.");
        let closed = HashSet::from(["a".to_string(), "c".to_string(), "d".to_string()]);
        let visible = crate::tabs::visible_tab_infos(&mixed.tabs, &opened, &opened_kinds, &closed);
        assert_eq!(migration_case(&mixed, &visible, true), None);
    }

    /// Spec: no band when there is nothing to move, nothing to move it to, or
    /// nothing to act on: every terminal new, the supervisor not creating new
    /// terminals (the media harnesses' stacks, goal s4), a disconnected
    /// host's last known state, and an agent with no terminal.
    #[farhelm_testtrace::test]
    fn no_band_without_an_old_terminal_or_a_switch_that_is_on() {
        let mut all_new = session("s");
        all_new.agent_terminal = Some(TerminalKind::Host);
        assert_eq!(
            migration_case(
                &all_new,
                &strip(vec![tab("a", Some(TerminalKind::Host))]),
                true
            ),
            None
        );

        let mut switch_off = session("s");
        switch_off.supervisor_creates_host_terminals = false;
        assert_eq!(migration_case(&switch_off, &strip(Vec::new()), true), None);
        switch_off.agent_terminal = Some(TerminalKind::Host);
        assert_eq!(
            migration_case(
                &switch_off,
                &strip(vec![tab("a", Some(TerminalKind::Tmux))]),
                true
            ),
            None
        );

        let mut stale = session("s");
        stale.stale = true;
        assert_eq!(migration_case(&stale, &strip(Vec::new()), true), None);

        let mut gone = session("s");
        gone.agent_terminal = None;
        assert_eq!(migration_case(&gone, &strip(Vec::new()), true), None);
    }

    /// Spec (goal s7): session details name every terminal's kind, once the
    /// supervisor creates new terminals or the session already has a new
    /// one, and stay as they always were otherwise, so a host still on old
    /// terminals (and every screenshot of one) shows no change.
    #[farhelm_testtrace::test]
    fn details_name_each_terminals_kind_only_once_kinds_can_differ() {
        let old = session("s");
        let tabs = [tab("a", Some(TerminalKind::Host)), tab("b", None)];
        assert_eq!(
            terminal_kinds(&old, &strip(tabs.to_vec())),
            Some(TerminalKinds {
                label: "Old and new terminals".into(),
                description: "Agent: old terminal (tmux). Terminal 1: new terminal.".into(),
            })
        );
        assert_eq!(
            terminal_kinds(&old, &strip(Vec::new()))
                .expect("switch on")
                .label,
            "Old terminal"
        );
        let mut new = session("s");
        new.agent_terminal = Some(TerminalKind::Host);
        assert_eq!(
            terminal_kinds(&new, &strip(vec![tab("a", Some(TerminalKind::Host))]))
                .expect("switch on")
                .label,
            "New terminals"
        );

        let mut switch_off = session("s");
        switch_off.supervisor_creates_host_terminals = false;
        assert_eq!(
            terminal_kinds(&switch_off, &strip(Vec::new())),
            None,
            "all old, switch off: unchanged bar"
        );
        switch_off.agent_terminal = Some(TerminalKind::Host);
        assert_eq!(
            terminal_kinds(&switch_off, &strip(Vec::new()))
                .expect("a new terminal is always named")
                .label,
            "New terminal"
        );

        let mut unreported = session("s");
        unreported.agent_terminal = None;
        assert_eq!(terminal_kinds(&unreported, &strip(Vec::new())), None);
    }

    /// Clicking the real band's × needs Dioxus's serialized event adapter,
    /// which only the desktop build compiles in, so these run in the
    /// desktop feature's test run (`cargo nextest run -p farhelm-ui
    /// --features desktop`), as the approval card's and the updater's click
    /// tests do.
    #[cfg(native_desktop)]
    pub(super) mod dismissal {
        use super::*;
        use dioxus::core::ScopeId;
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;

        /// Where the real [`MigrationBand`] reports to a test: the scope it is
        /// mounted in on its latest render, which is how the test finds its ×
        /// and what it rendered. The band fills it only in the desktop
        /// feature's test builds, and only when a test provides one.
        #[derive(Clone, Default)]
        pub(in crate::migration) struct BandProbe(
            pub(in crate::migration) Rc<Cell<Option<ScopeId>>>,
        );

        /// What the probe shows next: one session and the tabs the strip shows.
        struct Fixture {
            shown: RefCell<(Session, VisibleTabs)>,
            band: BandProbe,
        }

        impl PartialEq for Fixture {
            fn eq(&self, other: &Self) -> bool {
                std::ptr::eq(self, other)
            }
        }

        /// Mounts the real [`MigrationBand`] once per session, keyed by the
        /// session's id in a dynamic slot as `SessionView` is in `lib.rs` (a
        /// `match` arm's element): Dioxus honours a key only among the children
        /// of such a slot, and diffs a template's own root in place whatever its
        /// key, so a keyed root here would prove nothing about the view. Whether
        /// `lib.rs` really keys the view so is the browser spec's to show.
        fn probe(fixture: Rc<Fixture>) -> Element {
            use_context_provider(|| fixture.band.clone());
            let (session, tabs) = fixture.shown.borrow().clone();
            let shown = Some(session);
            rsx! {
                match shown {
                    Some(session) => rsx! {
                        MigrationBand { key: "{session.id.raw()}", session, tabs, restart_available: true }
                    },
                    None => rsx! {},
                }
            }
        }

        /// Render whatever changed, then return the text the band rendered: its
        /// dynamic text nodes (the headline and the body), empty when it
        /// rendered nothing. Read from what the band's scope rendered, not from
        /// its decision, so a band that decided to show and drew nothing fails.
        fn render(dom: &mut VirtualDom, fixture: &Fixture) -> String {
            dom.mark_dirty(ScopeId::APP);
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
            rendered_text(dom, fixture)
        }

        fn rendered_text(dom: &VirtualDom, fixture: &Fixture) -> String {
            let scope = fixture.band.0.get().expect("the band is mounted");
            let node = dom.get_scope(scope).expect("the band's scope").root_node();
            node.dynamic_nodes
                .iter()
                .filter_map(|node| match node {
                    dioxus::core::DynamicNode::Text(text) => Some(text.value.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        }

        /// Click the band's ×, the last control it renders with a click listener
        /// (the docs link comes first), through the listener actually mounted,
        /// as the approval card's tests do.
        fn click_dismiss(dom: &mut VirtualDom, fixture: &Fixture) {
            let scope = fixture.band.0.get().expect("the band is mounted");
            let node = dom.get_scope(scope).expect("the band's scope").root_node();
            let listener = node
                .dynamic_attrs
                .iter()
                .enumerate()
                .filter_map(|(index, attributes)| {
                    attributes
                        .iter()
                        .any(|attribute| attribute.name == "onclick")
                        .then_some(index)
                })
                .next_back()
                .expect("the band renders its ×");
            let button = node
                .mounted_dynamic_attribute(listener, dom)
                .expect("the × is mounted");
            dioxus::html::set_event_converter(Box::new(dioxus::html::SerializedHtmlEventConverter));
            let data: Rc<dyn std::any::Any> = Rc::new(dioxus::html::PlatformEventData::new(
                Box::new(dioxus::html::SerializedMouseData::default()),
            ));
            dom.runtime()
                .handle_event("click", dioxus::core::Event::new(data, true), button);
        }

        const OLD_AGENT: &str = "This session still runs in an old terminal.";

        /// Spec (goal s7): clicking × on the real band hides it for the rest of
        /// the visit, including when a refresh brings a new snapshot of the same
        /// session, and it comes back the next time the session is opened (not
        /// on a timer); a different message that the session comes to need later
        /// in the same visit (old tabs, once the agent has moved) shows despite
        /// the earlier dismissal.
        ///
        /// Why: the band is the only reminder an old session gets; a dismissal
        /// that stuck would silence it for good, one that a routine refresh
        /// undid would make it impossible to dismiss, one that hid the next
        /// message would leave the user believing Restart moved the tabs, and a
        /// × that did nothing would leave the band in the way for good.
        #[farhelm_testtrace::test]
        fn the_band_hides_for_the_visit_and_returns_on_the_next() {
            let fixture = Rc::new(Fixture {
                shown: RefCell::new((session("old"), strip(Vec::new()))),
                band: BandProbe::default(),
            });
            let mut dom = VirtualDom::new_with_props(probe, fixture.clone());
            dom.rebuild_in_place();
            assert!(
                rendered_text(&dom, &fixture).contains(OLD_AGENT),
                "premise: the band shows"
            );

            click_dismiss(&mut dom, &fixture);
            assert_eq!(render(&mut dom, &fixture), "", "× hides the band");

            // A detail refresh of the same session: same key, new snapshot.
            fixture.shown.borrow_mut().0.title = "renamed".into();
            assert_eq!(render(&mut dom, &fixture), "", "a refresh keeps it hidden");

            // The agent moved, but a tab did not: the other message shows.
            {
                let mut shown = fixture.shown.borrow_mut();
                shown.0.agent_terminal = Some(TerminalKind::Host);
                shown.1 = strip(vec![tab("a", Some(TerminalKind::Tmux))]);
            }
            assert!(
                render(&mut dom, &fixture).contains("Terminal 1 is still an old terminal."),
                "a new message is not covered by the old dismissal"
            );

            // Another session, then this one again: a new visit.
            let mut other = session("other");
            other.agent_terminal = Some(TerminalKind::Host);
            *fixture.shown.borrow_mut() = (other, strip(Vec::new()));
            assert_eq!(
                render(&mut dom, &fixture),
                "",
                "the other session has no band"
            );
            *fixture.shown.borrow_mut() = (session("old"), strip(Vec::new()));
            assert!(
                render(&mut dom, &fixture).contains(OLD_AGENT),
                "back on the next visit"
            );
        }
    }
}
