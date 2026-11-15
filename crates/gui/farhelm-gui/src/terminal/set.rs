//! The session view's terminals: the agent terminal and one per shell tab, all attached at once under one lease, with
//! the strip that chooses which one shows.
//!
//! SPEC.md, "Session view": the view "supports additional terminal tabs: plain shells spawned in the session's working
//! directory", which "survive client disconnects and supervisor restarts exactly like the agent terminal", and "a tab
//! whose process exits on its own is reaped automatically and silently". Ported from the old app's `tabs.rs` and the
//! tab half of `session_view.rs` (reference commit `24bc0580`). Here's the TLDR of the mechanics:
//!
//! - **The tabs are the helm's list.** The set follows the open session's pushed row: its `tabs`, in creation order,
//!   each named by its place ("Terminal 1", "Terminal 2"), as SPEC.md gives tabs no names. A tab opened from another
//!   client appears; one closed or reaped anywhere goes, and its terminal and stream with it. A reaped tab says nothing
//!   on its way out (its terminal's status line stays silent for a tab that was attached and then closed).
//! - **Every terminal stays attached; choosing one only changes which is drawn.** The old app attached every tab at
//!   once and rejected attach-on-choose, since each switch would pay a full replay. Only the chosen terminal is drawn
//!   here; it hands its measured pane to the set, which fits the hidden ones to the same pane, so a hidden tab attaches,
//!   replays its history and keeps real rows and columns (and is resized with the window and the text size).
//! - **One lease for the view.** The agent terminal and every tab attach under one [`AttachmentLease`], minted when the
//!   set is made, so the helm sees one viewer: a second window's view takes all of them over at once, and a reconnect
//!   of one is not a takeover of the others. They share one tunnel list too, the agent terminal's, since tunnels belong
//!   to the session: a tunnel a tab opens shows in the top row's pill.
//! - **A takeover holds back new tabs.** When another window takes the terminals over, each shows its own Detached line
//!   with Take control. A tab learned about meanwhile is parked rather than attached, because attaching it under this
//!   view's lease would take every terminal back without the user asking (the old app's takeover latch). Take control
//!   on any terminal takes all of them back, parked ones included.
//! - **At most 32 tabs are attached.** A bound on what the window builds from a tab list it did not write (the old
//!   app's `MAX_MOUNTED_TAB_ISLANDS`), not a feature limit: the strip lists every tab, and a tab past the bound shows
//!   that it is not attached instead of a terminal. The agent terminal does not count.
//! - **Focus.** Choosing a tab focuses its terminal through the focus coordinator, with the mark of the click; the
//!   chosen terminal is home (the session pane makes it so whenever the set changes). When the chosen tab goes, the
//!   agent terminal is chosen and becomes home, so a reaped tab that held focus leaves it on the agent terminal (SPEC.md,
//!   "A shell tab exits on its own while the user types in it. The session's agent tab takes focus").
//!
//! NOTE: New tab and Close are not here yet: the strip lists and chooses tabs, but opening and closing them is the
//! terminal area's next change. Selection is not kept across a change of session: reopening a session shows its agent
//! terminal, as the old app's view did after a reload.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, EntityId, FocusHandle,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, Role, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};

#[cfg(test)]
mod tests;

use super::CellMetrics;
use super::attached::{AttachedTerminal, NoticeHold, TerminalTarget};
use super::tunnels::Tunnels;
use crate::components::{Button, HoverHelp, Tier};
use crate::focus::{self, Mark};
use crate::session::{self, UiSession};
use crate::theme;

/// How many tabs are attached at most (the old app's `MAX_MOUNTED_TAB_ISLANDS`). See the module docs.
pub(crate) const MAX_ATTACHED_TABS: usize = 32;

/// The strip's element id.
pub(crate) const STRIP_ID: &str = "session-terminal-tabs";

/// The agent terminal's tab's element id.
pub(crate) const AGENT_TAB_ID: &str = "session-terminal-tab-agent";

/// The element id of the notice a tab past [`MAX_ATTACHED_TABS`] shows in place of a terminal.
pub(crate) const NOT_ATTACHED_ID: &str = "terminal-not-attached";

/// What a tab past [`MAX_ATTACHED_TABS`] shows (`.terminal-not-mounted`'s words at the reference commit).
fn not_attached_text() -> String {
    format!(
        "This session reports more than {MAX_ATTACHED_TABS} terminal tabs; this one is listed but not attached \
         (close some to attach it)."
    )
}

/// `.terminal-not-mounted`: `padding: 2em` at 14 pixels.
const NOT_ATTACHED_PAD: f32 = 28.;
/// `.terminal-not-mounted`: `font-size: 14px`.
const NOT_ATTACHED_TEXT_SIZE: f32 = 14.;

/// `.tab-strip`: `gap: 4px`.
const STRIP_GAP: f32 = 4.;

/// `.tab-strip[inert]`: `opacity: 0.45`.
const INERT_OPACITY: f32 = 0.45;

/// The element id of the tab for shell tab `id`.
pub(crate) fn tab_element_id(id: &str) -> String {
    format!("session-terminal-tab-{id}")
}

/// A tab's place-based name: "Terminal 1" for the first.
pub(crate) fn tab_label(index: usize) -> String {
    format!("Terminal {}", index + 1)
}

/// Says whether the terminals' notices are held back now; shared by every terminal of the set (see
/// [`AttachedTerminal::hold_notices`]).
pub(crate) type SharedHold = Rc<dyn Fn(&App) -> bool>;

/// One shell tab, in the helm's order.
struct Tab {
    /// The helm's id for it, never parsed.
    id: String,
    /// Its terminal, while it is among the first [`MAX_ATTACHED_TABS`].
    terminal: Option<Entity<AttachedTerminal>>,
    /// Its button's focus handle in the strip, kept with the tab so focus stays on its button as tabs come and go.
    focus: FocusHandle,
    /// Hears its terminal's changes, for the takeover rule.
    _watch: Option<Subscription>,
}

/// The open session's terminals. See the module docs.
pub struct TerminalSet {
    /// The UI session the terminals were made under, whose opener each new one takes.
    session: WeakEntity<UiSession>,
    /// The session's key.
    key: String,
    /// The view's lease, shared by every terminal of the set.
    lease: AttachmentLease,
    /// The agent terminal, always attached.
    agent: Entity<AttachedTerminal>,
    /// Hears the agent terminal's changes, for the takeover rule.
    _agent_watch: Subscription,
    /// The shell tabs, in the helm's order.
    tabs: Vec<Tab>,
    /// The chosen tab's id, `None` for the agent terminal. Only ever a listed tab's.
    chosen: Option<String>,
    /// The terminals that wait for Take control after another window took them over, by entity id. While any does,
    /// tabs learned about are parked (see the module docs).
    waiting: HashSet<EntityId>,
    /// The hold every terminal's notices answer to.
    hold: SharedHold,
    /// The Agent tab's focus handle in the strip.
    agent_focus: FocusHandle,
    /// The strip's sideways scroll, and the tab it last scrolled into view for keyboard focus: a tab Tab reaches
    /// is scrolled into view once, so the wheel can still move the strip while a tab holds focus. A `Cell` because the
    /// strip is built while the header draws, from a shared reference, and asking the scroll handle to reveal a tab
    /// then is how GPUI expects it (the handle acts on it when it lays the strip out).
    scroll: ScrollHandle,
    revealed: Cell<Option<usize>>,
    /// Whether the terminals are inert ([`TerminalSet::set_inert`]), which new tabs take on too.
    inert: bool,
    /// Follows the pushed row's tab list.
    _tabs_follow: session::ObservingCurrent,
}

impl TerminalSet {
    /// The terminals of the session filed under `key`, made for `session`: the agent terminal at `metrics`, and the
    /// session's tabs as the store lists them. Every terminal holds its notices back while `hold` says so.
    pub fn new(
        session: &Entity<UiSession>,
        key: String,
        metrics: CellMetrics,
        hold: SharedHold,
        cx: &mut Context<Self>,
    ) -> Self {
        let lease = AttachmentLease::new();
        let agent = Self::make(session, &key, None, &lease, metrics, &hold, false, None, cx);
        let agent_watch = cx.observe(&agent, |set, _, cx| set.member_changed(cx));
        let tabs_follow = session::observe_current(cx, |set: &mut Self, cx| set.follow_tabs(cx));
        let mut set = TerminalSet {
            session: session.downgrade(),
            key,
            lease,
            agent,
            _agent_watch: agent_watch,
            tabs: Vec::new(),
            chosen: None,
            waiting: HashSet::new(),
            hold,
            agent_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            revealed: Cell::new(None),
            inert: false,
            _tabs_follow: tabs_follow,
        };
        set.follow_tabs(cx);
        set
    }

    /// A terminal of this set: the agent's for `tab` `None`, else that tab's, sharing `tunnels` (the agent terminal's,
    /// for a tab) when given. It tells the set the pane it measures, so the set can fit the others.
    #[allow(
        clippy::too_many_arguments,
        reason = "called from the constructor, before the set exists to carry these"
    )]
    fn make(
        session: &Entity<UiSession>,
        key: &str,
        tab: Option<String>,
        lease: &AttachmentLease,
        metrics: CellMetrics,
        hold: &SharedHold,
        inert: bool,
        tunnels: Option<Entity<Tunnels>>,
        cx: &mut Context<Self>,
    ) -> Entity<AttachedTerminal> {
        let open = session.read(cx).terminal_opener();
        let target = TerminalTarget {
            session: key.to_owned(),
            tab,
            lease: lease.clone(),
        };
        let set = cx.weak_entity();
        let hold = hold.clone();
        cx.new(|cx| {
            let mut terminal =
                AttachedTerminal::sharing(session, target, metrics, open, tunnels, cx);
            let notices: NoticeHold = Box::new(move |cx| hold(cx));
            terminal.hold_notices(notices);
            let shown = cx.entity_id();
            terminal.on_measured(Rc::new(move |pane, scale, cx| {
                let _ = set.update(cx, |set, cx| set.measured(shown, pane, scale, cx));
            }));
            if inert {
                terminal.set_inert(true, cx);
            }
            terminal
        })
    }

    /// The agent terminal.
    pub fn agent(&self) -> &Entity<AttachedTerminal> {
        &self.agent
    }

    /// The terminal drawn now: the chosen tab's, or the agent's; `None` while a tab past [`MAX_ATTACHED_TABS`] is
    /// chosen.
    pub fn shown(&self) -> Option<&Entity<AttachedTerminal>> {
        match &self.chosen {
            None => Some(&self.agent),
            Some(id) => self
                .tabs
                .iter()
                .find(|tab| &tab.id == id)
                .and_then(|tab| tab.terminal.as_ref()),
        }
    }

    /// Where focus rests in this set: the shown terminal's handle, `None` while no terminal is shown.
    pub fn home(&self, cx: &App) -> Option<FocusHandle> {
        self.shown()
            .map(|terminal| terminal.read(cx).focus_handle().clone())
    }

    /// Every terminal of the set: the agent's first, then the attached tabs' in order.
    pub fn terminals(&self) -> impl Iterator<Item = &Entity<AttachedTerminal>> {
        std::iter::once(&self.agent).chain(self.tabs.iter().filter_map(|tab| tab.terminal.as_ref()))
    }

    /// Tab `id`'s button's focus handle in the strip, for tests that move keyboard focus there.
    #[cfg(test)]
    pub fn tab_focus(&self, id: &str) -> Option<FocusHandle> {
        self.tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.focus.clone())
    }

    /// The listed tabs' ids, in order, for tests.
    #[cfg(test)]
    pub fn tab_ids(&self) -> Vec<String> {
        self.tabs.iter().map(|tab| tab.id.clone()).collect()
    }

    /// The chosen tab's id, `None` for the agent terminal, for tests.
    #[cfg(test)]
    pub fn chosen(&self) -> Option<&str> {
        self.chosen.as_deref()
    }

    /// Show tab `id`'s terminal (`None` for the agent's) and focus it, with `mark` taken at the press that chose it,
    /// through the focus coordinator, which also makes it home. A tab that is not listed changes nothing.
    pub fn choose(
        &mut self,
        id: Option<String>,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(wanted) = &id
            && !self.tabs.iter().any(|tab| &tab.id == wanted)
        {
            return;
        }
        if self.chosen != id {
            self.chosen = id;
            cx.notify();
        }
        match self.home(cx) {
            Some(home) => focus::choose_terminal(&home, mark, window, cx),
            None => focus::set_home(None, window, cx),
        }
    }

    /// Make every terminal inert, or not ([`AttachedTerminal::set_inert`]), tabs the set learns about later included;
    /// the strip is drawn dimmed and takes no presses meanwhile (SPEC.md: a terminal "keeps its last screen and tab
    /// strip dimmed and inert beneath the card"). The sidebar area's unreachable-host card calls it; nothing does yet.
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn set_inert(&mut self, inert: bool, cx: &mut Context<Self>) {
        if self.inert == inert {
            return;
        }
        self.inert = inert;
        let terminals: Vec<_> = self.terminals().cloned().collect();
        for terminal in terminals {
            terminal.update(cx, |terminal, cx| terminal.set_inert(inert, cx));
        }
        cx.notify();
    }

    /// Bring the tabs in line with the pushed row: keep the terminals of tabs still listed, make terminals for new ones
    /// (parked while another window holds the terminals), drop those no longer listed, and fall back to the agent
    /// terminal when the chosen tab went.
    fn follow_tabs(&mut self, cx: &mut Context<Self>) {
        let listed: Vec<String> = session::current_store(cx)
            .and_then(|store| {
                store
                    .read(cx)
                    .collections()
                    .get::<SessionRow>(&self.key)
                    .map(|row| row.info.tabs.iter().map(|tab| tab.id.clone()).collect())
            })
            .unwrap_or_default();
        // Most pushes change something else about the row (its activity, its status): nothing to do then. A tab whose
        // terminal could not be made (the font did not measure) is tried again at the next change of the list.
        let unchanged = listed.len() == self.tabs.len()
            && listed.iter().zip(&self.tabs).all(|(id, tab)| *id == tab.id);
        if unchanged {
            return;
        }
        let Some(session) = self.session.upgrade() else {
            return;
        };
        // By id, so a long list from the supervisor costs one lookup per tab.
        let mut old: HashMap<String, Tab> = std::mem::take(&mut self.tabs)
            .into_iter()
            .map(|tab| (tab.id.clone(), tab))
            .collect();
        let metrics = crate::terminal::CellMetrics::measure(super::text_size::current(cx), cx);
        for (index, id) in listed.into_iter().enumerate() {
            let mut tab = old.remove(&id).unwrap_or_else(|| Tab {
                id,
                terminal: None,
                focus: cx.focus_handle().tab_stop(true),
                _watch: None,
            });
            if tab.terminal.is_none()
                && index < MAX_ATTACHED_TABS
                && let Some(metrics) = metrics
            {
                let terminal = Self::make(
                    &session,
                    &self.key,
                    Some(tab.id.clone()),
                    &self.lease,
                    metrics,
                    &self.hold,
                    self.inert,
                    Some(self.agent.read(cx).tunnels().clone()),
                    cx,
                );
                if !self.waiting.is_empty() {
                    terminal.update(cx, |terminal, cx| terminal.park(cx));
                }
                // Not fitted here: the notify below redraws the shown terminal, whose measured pane fits it (`measured`).
                tab._watch = Some(cx.observe(&terminal, |set, _, cx| set.member_changed(cx)));
                tab.terminal = Some(terminal);
            }
            self.tabs.push(tab);
        }
        // What is left in `old` was not listed: its terminals go now, and their streams with them.
        drop(old);
        if let Some(chosen) = &self.chosen
            && !self.tabs.iter().any(|tab| &tab.id == chosen)
        {
            self.chosen = None;
        }
        self.member_changed(cx);
        cx.notify();
    }

    /// The shown terminal `shown` measured its pane, as it does after every frame it is drawn in: fit every other
    /// terminal of the set to it. So a tab that appears is fitted, and attaches, after the next frame; a resize of the
    /// window or the text size reaches the hidden tabs the same way.
    fn measured(
        &mut self,
        shown: EntityId,
        pane: Bounds<Pixels>,
        scale: f32,
        cx: &mut Context<Self>,
    ) {
        let hidden: Vec<_> = self
            .terminals()
            .filter(|terminal| terminal.entity_id() != shown)
            .cloned()
            .collect();
        for terminal in hidden {
            terminal.update(cx, |terminal, cx| terminal.fit_hidden(pane, scale, cx));
        }
    }

    /// A terminal of the set changed: keep the takeover rule. Another window taking the terminals over makes them wait
    /// for Take control; once the user takes control on any of them (a terminal that waited and is still in the set
    /// waits no more), every other one that waits takes control too, parked ones included, since the helm hands all of
    /// a lease's terminals to the newest attach anyway. A waiting terminal that leaves the set (its tab closed or reaped
    /// in the other window) is not the user taking control, and releases nothing.
    fn member_changed(&mut self, cx: &mut Context<Self>) {
        let members: Vec<_> = self.terminals().cloned().collect();
        let now: HashSet<EntityId> = members
            .iter()
            .filter(|terminal| terminal.read(cx).taken_over())
            .map(|terminal| terminal.entity_id())
            .collect();
        let taken_back = members.iter().any(|terminal| {
            let id = terminal.entity_id();
            self.waiting.contains(&id) && !now.contains(&id)
        });
        if taken_back {
            self.waiting.clear();
            for terminal in members
                .into_iter()
                .filter(|terminal| now.contains(&terminal.entity_id()))
            {
                terminal.update(cx, |terminal, cx| terminal.take_control(cx));
            }
        } else {
            self.waiting = now;
        }
    }

    /// The strip for the top row's left region: the agent terminal's tab, then one per shell tab, each choosing its
    /// terminal, in a row that scrolls sideways when it runs out of room (SPEC.md: "only the tab strip takes the
    /// remaining width and scrolls horizontally when needed").
    ///
    /// Each tab is a button reporting whether its terminal is the one shown (pressed), as the old strip's were, not a
    /// tab list: the next change puts a close button beside each, which a tab list cannot hold. A tab that keyboard
    /// focus reaches while scrolled out of view is scrolled into it.
    pub fn strip(&self, set: &Entity<TerminalSet>, window: &Window) -> AnyElement {
        let chosen = self.chosen.clone();
        let inert = self.inert;
        let focused = std::iter::once(&self.agent_focus)
            .chain(self.tabs.iter().map(|tab| &tab.focus))
            .position(|handle| handle.is_focused(window));
        if focused != self.revealed.get() {
            if let Some(index) = focused {
                self.scroll.scroll_to_item(index);
            }
            self.revealed.set(focused);
        }
        let tab = |id: String,
                   label: String,
                   help: String,
                   value: Option<String>,
                   focus: &FocusHandle| {
            let set = set.clone();
            let pressed = chosen == value;
            Button::new(id, label, HoverHelp::tooltip(help), Role::Button)
                .tier(Tier::TerminalTab)
                .pressed(pressed)
                .disabled(inert)
                .focus(focus)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    let value = value.clone();
                    set.update(cx, |set, cx| set.choose(value, &mark, window, cx));
                })
        };
        let mut tabs = vec![tab(
            AGENT_TAB_ID.to_owned(),
            "Agent".to_owned(),
            "Agent: show the session's agent terminal".to_owned(),
            None,
            &self.agent_focus,
        )];
        for (index, listed) in self.tabs.iter().enumerate() {
            let label = tab_label(index);
            tabs.push(tab(
                tab_element_id(&listed.id),
                label.clone(),
                format!("{label}: show this shell tab"),
                Some(listed.id.clone()),
                &listed.focus,
            ));
        }
        div()
            .id(STRIP_ID)
            .debug_selector(|| STRIP_ID.into())
            .role(Role::Group)
            .aria_label("Terminals")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(STRIP_GAP))
            .min_w_0()
            .overflow_x_scroll()
            .track_scroll(&self.scroll)
            .when(inert, |strip| strip.opacity(INERT_OPACITY))
            .children(tabs)
            .into_any_element()
    }
}

impl Render for TerminalSet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.shown() {
            Some(terminal) => div().size_full().child(terminal.clone()),
            None => div().size_full().child(
                div()
                    .id(NOT_ATTACHED_ID)
                    .debug_selector(|| NOT_ATTACHED_ID.into())
                    .p(px(NOT_ATTACHED_PAD))
                    .text_size(px(NOT_ATTACHED_TEXT_SIZE))
                    .text_color(theme::color(theme::palette(cx).fg_dim))
                    .child(not_attached_text()),
            ),
        }
    }
}
