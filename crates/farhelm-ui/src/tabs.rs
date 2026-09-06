//! Terminal tabs (PLAN_M4.md item 6): the tab-domain half of the feature —
//! renderer-free derivations plus the strip's one presentational piece.
//!
//! `session_view::SessionView` holds one xterm.js island per terminal —
//! the agent's plus one per open tab — all attached CONCURRENTLY. Most of
//! this module needs no renderer to reason about: which tabs to show
//! (`visible_tabs`'s optimistic-rendering
//! correction over the server's list), what to call them (`tab_label`),
//! which DOM nodes they own (`tab_terminal_element_id`/
//! `tab_banner_element_id`), how the terminal WebSocket's own path is
//! built (`terminal_ws_path`), and the tab strip's per-tab UI piece
//! (`TabStripItem`, the one item here that does render). The stateful
//! half — the signals, futures, and effect that drive all of this — lives
//! in `SessionView` itself; keeping the derivations here as free
//! functions is what makes them unit-testable without standing up a
//! component tree.

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::Tab;
use crate::api::{encode_path_segment, encode_query_value};

/// The DOM id of the agent terminal's mount point, and of its banner.
///
/// Unchanged from before tabs existed, deliberately: "the terminal" of a
/// session still unambiguously means its agent terminal, the browser suite
/// keys off these two ids throughout, and a session with no tabs must
/// render exactly the DOM it always did.
pub(crate) const AGENT_TERMINAL_ELEMENT_ID: &str = "terminal";
pub(crate) const AGENT_BANNER_ELEMENT_ID: &str = "term-banner";

/// The DOM id of the agent terminal's connecting placeholder — the surface
/// that stands in front of a terminal while its attach catches up
/// (PLAN_M5.md item 5).
///
/// New in M5, unlike the two ids above: an empty xterm used to mount
/// visible and the only overlay a pane had was its detach banner, so there
/// was nothing standing in front of the catch-up to reuse. Rendered empty
/// by `SessionView`; terminal.js owns its content, for the same reason it
/// owns the banner's — the reveal happens inside a `term.write()`
/// completion callback, which nothing on the Dioxus side can be driven
/// from.
pub(crate) const AGENT_CONNECTING_ELEMENT_ID: &str = "term-connecting";

/// How many TAB terminals this view will ever mount at once. The agent
/// terminal is always mounted and is not counted against it.
///
/// PLAN_M4.md item 6 deliberately adds no artificial cap on TABS, and this
/// is not one: it is a bound on what this client will do with a LIST it
/// did not author. The plan's reasoning — "tab count is bounded by the
/// user opening them by hand" — is a statement about a healthy supervisor,
/// and the tab list arrives over a connection that under `--ssh` reaches a
/// different machine. A supervisor that is compromised, or merely wrong
/// (a rediscovery bug adopting every window on a shared tmux server),
/// could list thousands, and this view would answer by constructing
/// thousands of xterm.js instances and opening thousands of WebSockets —
/// wedging the browser, and with it the user's only way to see what
/// happened. A trust boundary needs a bound even where the cooperative
/// case does not.
///
/// 32 is chosen to be far past hand-driven use (SPEC.md's tabs are for
/// "poking at the workspace next to the agent") while still being a
/// resource count a browser will not notice. Tabs beyond it are NOT hidden
/// — the strip lists every one the server reports, and their panes say
/// they are not attached — because silently dropping tabs would be its own
/// lie about what the session holds.
pub(crate) const MAX_MOUNTED_TAB_ISLANDS: usize = 32;

/// The two properties the number above has to keep, checked at COMPILE
/// time rather than by a test: both are statements about a constant, so a
/// violation should never get as far as being runnable. Below the floor it
/// stops being a trust boundary and becomes a feature limit a real user
/// could hit by hand; above the ceiling it stops bounding what a hostile
/// or buggy tab list can make this browser construct.
const _: () = assert!(MAX_MOUNTED_TAB_ISLANDS >= 16);
const _: () = assert!(MAX_MOUNTED_TAB_ISLANDS <= 256);

/// `tab_errors`' key for the add-tab control, which — unlike a close — has
/// no tab id of its own to be keyed by yet. A fixed non-id string rather
/// than an `Option` key so the map stays one flat, renderable structure;
/// it cannot collide with a real tab id, which is a supervisor-minted
/// opaque token and never this word.
pub(crate) const TAB_OPEN_ERROR_KEY: &str = "open";

/// The DOM id of a tab's mount point, and of its banner. Derived from the
/// tab id rather than from its position so that a tab's island keeps its
/// identity when a SIBLING is closed — a position-derived id would make
/// every tab after the closed one look like a different island to
/// terminal.js's reconciliation and force a pointless remount (and, with
/// it, a full replay) of terminals nothing happened to.
///
/// The tab id is supervisor-minted and opaque; it reaches the DOM only
/// through `getElementById`, which imposes no syntax on an id, so nothing
/// here has to assume it is a UUID even though today it always is.
pub(crate) fn tab_terminal_element_id(tab_id: &str) -> String {
    format!("terminal-{tab_id}")
}

/// The banner half of the pair above; see `tab_terminal_element_id` for
/// why both are derived from the id rather than the position.
pub(crate) fn tab_banner_element_id(tab_id: &str) -> String {
    format!("term-banner-{tab_id}")
}

/// A tab's connecting placeholder (PLAN_M5.md item 5), derived from the tab
/// id for the same reason its two siblings are.
///
/// Every terminal needs its own: each has its own socket and therefore its
/// own catch-up phase, so a session view can legitimately hold one terminal
/// still catching up next to another that went live minutes ago.
pub(crate) fn tab_connecting_element_id(tab_id: &str) -> String {
    format!("term-connecting-{tab_id}")
}

/// A tab's display label: purely positional, one-based (PLAN_M4.md item
/// 6). SPEC.md gives tabs no names and close is their only operation, so
/// v1 invents no naming surface — and because the position comes from
/// `SessionInfo::tabs`' creation order, two clients looking at the same
/// session agree on which tab is "Terminal 2".
pub(crate) fn tab_label(index: usize) -> String {
    format!("Terminal {}", index + 1)
}

/// The tabs to render, in order: the server's authoritative list, then any
/// this view opened that the server has not listed back yet, minus any it
/// has closed that the server still lists.
///
/// Both corrections are the same optimistic-rendering bargain
/// `list::ListView` makes for a deleted row, for the same reason — a tab
/// list refreshed by a 3-second poll would otherwise take up to a full
/// interval to show the user the result of their own click — but they
/// point in opposite directions and both edges matter:
///
/// - `opened` appends. A tab-open reply carries the new tab's id and
///   deliberately says NOTHING about ordering (farhelm-proto's
///   `TabOpened`), so an optimistic tab goes at the END and gets its real
///   position from the next refresh. That is honest for the common case
///   (the newest tab is last in creation order) and self-correcting when
///   another client's concurrent open makes it wrong.
/// - `closed` suppresses. A DELETE that has already succeeded must not
///   leave a tab on screen — clicking it would attach a terminal the
///   supervisor has destroyed — until a poll that was already in flight
///   before the close finally returns.
///
/// Neither set is trusted to be accurate forever: `SessionView`'s poll
/// prunes an `opened` id once the server lists it and a `closed` id once
/// the server stops, so a tab another client re-opens (impossible today —
/// ids are never reused — or a supervisor that answered oddly) cannot be
/// permanently hidden by a stale entry.
/// `opened` carries `(id, observed_from)` pairs, but only the ids matter
/// here — the sequence numbers are the POLL's business (see
/// `session_view::SessionView`), deciding when an entry may be retired,
/// never whether it is shown.
pub(crate) fn visible_tabs(
    server: &[Tab],
    opened: &[(String, u64)],
    closed: &HashSet<String>,
) -> Vec<String> {
    let mut ids: Vec<String> = server.iter().map(|tab| tab.id.clone()).collect();
    for (id, _) in opened {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    ids.retain(|id| !closed.contains(id));
    ids
}

/// The helm WebSocket path one terminal of a session attaches on.
///
/// `lease` rides on EVERY terminal of the view, the agent's included
/// (PLAN_M4.md item 3): the supervisor groups a session's channels by it
/// to enforce SPEC.md's one-attached-client rule across all of them, so a
/// view that leased only its tabs would have its own agent terminal
/// take over — and be taken over by — the tabs sitting beside it.
///
/// `tab` is absent for the agent terminal, which is what the pre-M4
/// (agent-only) reading of this endpoint already means, so the agent's URL
/// differs from an old build's only by the lease.
pub(crate) fn terminal_ws_path(session_id: &str, tab_id: Option<&str>, lease: &str) -> String {
    terminal_ws_path_on(session_id, tab_id, lease, "term")
}

/// The same terminal's path for an UNATTENDED attach — the route that
/// refuses rather than displacing another client (PLAN_M6.md item 7).
///
/// A different PATH rather than a flag on the one above, and built here
/// rather than spliced together in terminal.js, for two reasons that point
/// the same way. The safety one: a helm that predates this milestone does
/// not serve this route at all, so an automatic reconnect reaching a
/// rolled-back helm fails its handshake instead of quietly performing the
/// displacing attach a flag would have let it ignore — the browser has no
/// version handshake to catch that with, and its build-stamp check can be
/// a poll interval stale. The tidiness one: every id in these URLs comes
/// from a supervisor, so the escaping belongs where the encoders are, not
/// in string surgery on the far side of the eval boundary.
pub(crate) fn terminal_ws_unowned_path(
    session_id: &str,
    tab_id: Option<&str>,
    lease: &str,
) -> String {
    terminal_ws_path_on(session_id, tab_id, lease, "term/unowned")
}

/// The shared body of the two paths above; `route` is the only difference.
fn terminal_ws_path_on(session_id: &str, tab_id: Option<&str>, lease: &str, route: &str) -> String {
    let session = encode_path_segment(session_id);
    let lease = encode_query_value(lease);
    match tab_id {
        None => format!("/api/sessions/{session}/{route}?lease={lease}"),
        Some(tab) => {
            let tab = encode_query_value(tab);
            format!("/api/sessions/{session}/{route}?tab={tab}&lease={lease}")
        }
    }
}

/// The safety-critical half of the inline close-tab confirmation, the
/// counterpart to `status::confirm_consequence` for sessions and worded from
/// the same rule: say what the click actually destroys before naming the
/// thing.
///
/// Unconditional, with no status to branch on — a tab has none. That is
/// not an omission: SPEC.md makes close "kills that shell and its
/// processes" whatever the shell is currently doing, and a tab whose shell
/// already exited still closes (the supervisor's own idempotency), so
/// there is no state in which a softer wording would be more honest. The
/// daemonized-child clause is in it because the supervisor's tab-scoped
/// reap really does go after them, which is exactly the consequence a user
/// cannot see coming from the word "close".
pub(crate) const CLOSE_TAB_CONSEQUENCE: &str =
    "closing kills this terminal's shell and every process it started:";

/// `tab_errors`' entries in a stable display order, newest-agnostic and
/// purely lexical by key.
///
/// A `HashMap` iterates in no defined order, and the error lines sit in a
/// list the user reads: without this, every unrelated re-render could
/// reshuffle them, which reads as messages appearing and disappearing
/// rather than accumulating. The ORDER itself carries no meaning and is not
/// claimed to — only its stability does.
pub(crate) fn sorted_tab_errors(errors: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = errors
        .iter()
        .map(|(key, message)| (key.clone(), message.clone()))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

/// One tab in the strip: its selection button plus its own close
/// affordance, as a child component for the same reason `list::SessionRow`
/// is one — an rsx `for` body cannot bind locals, and two event handlers in
/// one iteration each need their own owned copy of the tab id. Props give
/// both of them one without the loop having to clone by hand.
///
/// The close control is a SIBLING button, not nested inside the selection
/// button: HTML forbids interactive content inside a `<button>`, so the
/// obvious "× inside the tab" markup would be invalid with undefined
/// browser behavior — the same constraint that split `SessionRow`'s open
/// action out of its row wrapper. Being a real button also means Enter and
/// Space activation, focus styling, and screen-reader semantics come for
/// free.
///
/// There is deliberately no such item for the agent terminal: it is
/// rendered directly by `SessionView` precisely because it must NOT carry
/// a close affordance.
#[component]
pub(crate) fn TabStripItem(
    tab_id: String,
    label: String,
    selected: bool,
    busy: bool,
    on_select: EventHandler<String>,
    on_close: EventHandler<String>,
) -> Element {
    let select_id = tab_id.clone();
    let close_id = tab_id.clone();
    rsx! {
        div { class: "tab-slot", "data-tab-id": "{tab_id}",
            button {
                r#type: "button",
                class: if selected { "btn tab selected" } else { "btn tab" },
                "data-terminal": "{tab_id}",
                onclick: move |_| on_select.call(select_id.clone()),
                "{label}"
            }
            button {
                r#type: "button",
                class: "tab-close",
                // The visible glyph is a multiplication sign, which reads
                // as nothing useful to a screen reader — the accessible
                // name has to carry the tab's identity instead, or every
                // tab's close button would announce identically.
                "aria-label": "close {label}",
                disabled: busy,
                onclick: move |_| on_close.call(close_id.clone()),
                "×"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tab with the given id — the whole of `Tab`, which is why this is
    /// a one-liner rather than a builder.
    fn tab(id: &str) -> Tab {
        Tab { id: id.into() }
    }

    /// The server's list is the spine of the rendered order, and the two
    /// optimistic corrections ride on top of it: a just-opened tab the
    /// server has not listed yet appears at the END (a tab-open reply
    /// carries no ordering — see `visible_tabs`), and a just-closed tab
    /// disappears immediately even while a poll in flight still lists it.
    /// Getting either edge wrong is directly visible to the user as a
    /// click that does nothing or a tab that vanishes and comes back.
    #[farhelm_testtrace::test]
    fn visible_tabs_applies_the_optimistic_corrections_over_the_server_list() {
        let server = vec![tab("a"), tab("b")];

        assert_eq!(
            visible_tabs(&server, &[], &HashSet::new()),
            vec!["a".to_string(), "b".to_string()],
            "with nothing pending, the server's list is the whole answer, in its order"
        );

        assert_eq!(
            visible_tabs(&server, &[("c".to_string(), 0)], &HashSet::new()),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            "an optimistic open lands at the end until a refresh places it"
        );

        let closed: HashSet<String> = ["a".to_string()].into_iter().collect();
        assert_eq!(
            visible_tabs(&server, &[], &closed),
            vec!["b".to_string()],
            "a closed tab is gone at once, even while the server still lists it"
        );
    }

    /// The one case both corrections meet: a poll that has caught up
    /// already lists the optimistically-added tab, and it must appear
    /// ONCE, in the server's position — not twice, and not pinned to the
    /// end. `SessionView`'s poll prunes the optimistic entry, but this
    /// deduplication is what keeps the render correct in the window before
    /// that prune runs.
    #[farhelm_testtrace::test]
    fn visible_tabs_never_duplicates_an_optimistic_tab_the_server_now_lists() {
        let server = vec![tab("a"), tab("b"), tab("c")];
        assert_eq!(
            visible_tabs(&server, &[("b".to_string(), 0)], &HashSet::new()),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    /// Labels are positional and one-based, derived from list position
    /// rather than from anything the tab itself carries — SPEC.md gives
    /// tabs no names, so this IS the naming rule, not a placeholder for
    /// one.
    #[farhelm_testtrace::test]
    fn tab_labels_are_one_based_positions() {
        assert_eq!(tab_label(0), "Terminal 1");
        assert_eq!(tab_label(1), "Terminal 2");
    }

    /// Each terminal gets its own mount point and its own banner, and the
    /// agent's two ids are exactly the ones the pre-tabs UI used — a
    /// session with no tabs must render the DOM it always did, and the
    /// browser suite keys off both names throughout. The per-tab ids must
    /// be distinct from the agent's and from each other, since terminal.js
    /// uses them as the identity of an island.
    #[farhelm_testtrace::test]
    fn terminal_element_ids_are_distinct_per_terminal() {
        assert_eq!(AGENT_TERMINAL_ELEMENT_ID, "terminal");
        assert_eq!(AGENT_BANNER_ELEMENT_ID, "term-banner");
        assert_eq!(tab_terminal_element_id("t1"), "terminal-t1");
        assert_eq!(tab_banner_element_id("t1"), "term-banner-t1");
        assert_ne!(tab_terminal_element_id("t1"), tab_terminal_element_id("t2"));
        assert_ne!(tab_terminal_element_id("t1"), tab_banner_element_id("t1"));
    }

    /// Each terminal's connecting placeholder (PLAN_M5.md item 5) is its
    /// own element, distinct from every other id the same pane carries.
    /// This is what keeps one terminal's catch-up from painting over a
    /// sibling that is already live — panes are stacked and every tab
    /// attaches concurrently, so the ids are the only thing separating
    /// them. The agent's is asserted as a literal because terminal.js and
    /// the browser suite both name it.
    #[farhelm_testtrace::test]
    fn connecting_placeholder_ids_are_distinct_per_terminal() {
        assert_eq!(AGENT_CONNECTING_ELEMENT_ID, "term-connecting");
        assert_eq!(tab_connecting_element_id("t1"), "term-connecting-t1");
        assert_ne!(
            tab_connecting_element_id("t1"),
            tab_connecting_element_id("t2")
        );
        for id in [
            tab_terminal_element_id("t1"),
            tab_banner_element_id("t1"),
            AGENT_CONNECTING_ELEMENT_ID.to_string(),
        ] {
            assert_ne!(tab_connecting_element_id("t1"), id);
        }
    }

    /// The lease rides on EVERY terminal, the agent's included: a view
    /// that leased only its tabs would have its own terminals take each
    /// other over (PLAN_M4.md item 3). The agent's URL carries no `?tab=`
    /// at all, which is the pre-M4 reading the helm still honors.
    #[farhelm_testtrace::test]
    fn every_terminal_path_carries_the_lease_and_only_tabs_carry_a_selector() {
        assert_eq!(
            terminal_ws_path("s1", None, "lease-1"),
            "/api/sessions/s1/term?lease=lease-1"
        );
        assert_eq!(
            terminal_ws_path("s1", Some("t1"), "lease-1"),
            "/api/sessions/s1/term?tab=t1&lease=lease-1"
        );
    }

    /// The unattended attach goes to a DIFFERENT ROUTE, not to the same
    /// one with a flag on it — the property PLAN_M6.md item 7's safety
    /// rests on (a helm that predates the contract does not serve this
    /// path at all, so it cannot silently perform the displacing attach
    /// instead). Pinned as the literal URL because that is what makes an
    /// old helm answer with something that is not a WebSocket.
    ///
    /// Everything else about the two must stay identical: same session,
    /// same selector, same lease, same escaping — a recovery that attached
    /// a different TERMINAL would be a bug the safety property would hide.
    #[farhelm_testtrace::test]
    fn the_unattended_path_differs_only_in_its_route() {
        assert_eq!(
            terminal_ws_unowned_path("s1", None, "lease-1"),
            "/api/sessions/s1/term/unowned?lease=lease-1"
        );
        assert_eq!(
            terminal_ws_unowned_path("s1", Some("t1"), "lease-1"),
            "/api/sessions/s1/term/unowned?tab=t1&lease=lease-1"
        );
        assert_eq!(
            terminal_ws_unowned_path("s1", Some("t&lease=stolen"), "mine"),
            "/api/sessions/s1/term/unowned?tab=t%26lease%3Dstolen&lease=mine",
            "the escaping is the shared body's, not a second implementation"
        );
    }

    /// A tab id or lease containing query syntax must not be able to
    /// re-split the URL and change which terminal gets attached — the tab
    /// id in particular comes from a supervisor that, under `--ssh`, is a
    /// different machine. Percent-encoding is what makes that structural
    /// rather than a matter of trusting the id's shape.
    #[farhelm_testtrace::test]
    fn query_values_are_percent_encoded() {
        assert_eq!(encode_query_value("plain-id_1.0~"), "plain-id_1.0~");
        assert_eq!(encode_query_value("a&b=c"), "a%26b%3Dc");
        assert_eq!(encode_query_value("a b"), "a%20b");
        // Per byte, so multi-byte UTF-8 encodes to several escapes rather
        // than being mangled into one.
        assert_eq!(encode_query_value("é"), "%C3%A9");
        assert_eq!(
            terminal_ws_path("s1", Some("t&lease=stolen"), "mine"),
            "/api/sessions/s1/term?tab=t%26lease%3Dstolen&lease=mine",
            "an injected parameter must stay part of the tab VALUE, never become a second key"
        );
    }

    /// Close is destructive in a way the word "close" understates: the
    /// supervisor's tab-scoped reap goes after the shell's daemonized
    /// descendants too (SPEC.md: close "kills that shell and its
    /// processes"). The consequence line is what tells the user that
    /// before they confirm, so it must promise the kill and mention what
    /// else goes with it.
    #[farhelm_testtrace::test]
    fn close_tab_consequence_states_the_kill_and_its_reach() {
        assert_eq!(
            CLOSE_TAB_CONSEQUENCE,
            "closing kills this terminal's shell and every process it started:",
            "the exact sentence is the contract — it is the last thing a user reads before \
             destroying a shell and everything under it, so a reworded or softened version is a \
             change to review, not an implementation detail to drift"
        );
    }

    /// Error lines are rendered in a list the user reads, and a `HashMap`
    /// has no iteration order — without a sort, an unrelated re-render
    /// could reshuffle them, which reads as messages being replaced rather
    /// than accumulating. What is pinned is STABILITY (the same set always
    /// renders the same way), not the particular order.
    #[farhelm_testtrace::test]
    fn tab_errors_render_in_a_stable_order() {
        let errors: HashMap<String, String> = [
            ("tab-b".to_string(), "close tab: boom".to_string()),
            (TAB_OPEN_ERROR_KEY.to_string(), "open tab: nope".to_string()),
            ("tab-a".to_string(), "close tab: bang".to_string()),
        ]
        .into_iter()
        .collect();
        let keys: Vec<String> = sorted_tab_errors(&errors)
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, vec!["open", "tab-a", "tab-b"]);
    }
}
