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
//!   set is made, so the helm sees one viewer, and a reconnect of one is not a takeover of the others. They share one
//!   tunnel list too, the agent terminal's, since tunnels belong to the session: a tunnel a tab opens shows in the top
//!   row's pill.
//! - **Each terminal attaches on its own.** A tab the set learns about opens its stream at its first fit, whatever the
//!   other terminals are doing, and a terminal whose stream stopped (a stalled viewer, a refused attach) waits for its
//!   own Reconnect. One UI holds the helm at a time, and a window that loses it goes inert as a whole, so there is no
//!   other window whose terminals a new tab's attach could take back (decided by the migration's orchestrator on
//!   2026-11-15).
//! - **At most 32 tabs are attached.** A bound on what the window builds from a tab list it did not write (the old
//!   app's `MAX_MOUNTED_TAB_ISLANDS`), not a feature limit: the strip lists every tab, and a tab past the bound shows
//!   that it is not attached instead of a terminal. The agent terminal does not count.
//! - **Focus.** Choosing a tab focuses its terminal through the focus coordinator, with the mark of the click; the
//!   chosen terminal is home (the session pane makes it so whenever the set changes). When the chosen tab goes, the
//!   agent terminal is chosen and becomes home, so a reaped tab that held focus leaves it on the agent terminal (SPEC.md,
//!   "A shell tab exits on its own while the user types in it. The session's agent tab takes focus").
//!
//! - **New terminal and Close.** New terminal (`POST /api/sessions/{id}/tabs`) runs on the session's own operation key,
//!   so a pending lifecycle request refuses it (SPEC.md, "Lifecycle operations"), and it is unavailable while a question,
//!   dialog or menu holds the keyboard, which covers an open lifecycle question (the session's questions hang from its
//!   row and header, and are not modal); its refusal shows on the session's row and header as "Open tab: " and the helm's
//!   words. If the user waited, the tab it opened is chosen and focused once the helm has answered and listed it, in
//!   either order. Each tab's × (`DELETE /api/sessions/{id}/tabs/{tab}`) runs on that tab's own key, so siblings close
//!   side by side and a close holds none of the session's lifecycle actions; it asks first, hung under the ×, naming the
//!   tab and that its shell and everything it started die, unless **Skip session confirmations** is on (SPEC.md: the
//!   switch skips "terminal-tab Close"). A refused close shows under the strip on that tab alone ("Close tab: " and the
//!   helm's words) until its next close.
//!
//! NOTE: Selection is not kept across a change of session: reopening a session shows its agent terminal, as the old
//! app's view did after a reload. The close question names the tab as it was named when it opened (the old one renamed
//! itself as tabs before it closed); it acts on the tab's id either way.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, EntityId,
    FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, Role,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window,
    div, prelude::FluentBuilder as _, px,
};

#[cfg(test)]
mod tests;

use super::CellMetrics;
use super::attached::{AttachedTerminal, Hold, NoticeHold, TerminalTarget};
use super::tunnels::Tunnels;
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, IconButton, Tier};
use crate::focus::{self, Mark};
use crate::host_text;
use crate::layers::{Anchor, Subject};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
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

/// The narrowest the strip is drawn while the top row shows a tunnel pill, room permitting: the old app's
/// `.session-topbar:has(> .session-tunnel-pill) .tab-strip { flex: 0 1 auto; min-width: 112px }`, which kept a pill
/// from squeezing the strip to nothing.
///
/// It is a ceiling on the floor, not a guarantee: the top row's fit gives the strip less when the row cannot hold the
/// pill whole and the actions beside it, and its module docs (`session_pane::header`'s `fit`) give the rule and why.
/// The strip's own element keeps `min_w_0`: the floor is the top row's to apply, and no floor applies without a pill.
pub(crate) const STRIP_FLOOR_BESIDE_PILL: f32 = 112.;

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

/// The element id of shell tab `id`'s × button.
pub(crate) fn close_element_id(id: &str) -> String {
    format!("session-terminal-close-{id}")
}

/// The element id of the line under the strip saying why closing shell tab `id` was refused.
pub(crate) fn close_refusal_id(id: &str) -> String {
    format!("session-terminal-close-refusal-{id}")
}

/// New terminal's element id.
pub(crate) const ADD_ID: &str = "session-terminal-add";

/// `.tab-add`: a 24-pixel square.
const ADD_SIDE: f32 = 24.;

/// `.tab-add-divider`: 16 pixels high.
const ADD_DIVIDER_HEIGHT: f32 = 16.;

/// The close question's first line (the old app's `CLOSE_TAB_CONSEQUENCE`): closing asks whatever the shell runs,
/// since the window cannot see what that is, and the close kills it either way.
pub(crate) const CLOSE_TAB_CONSEQUENCE: &str =
    "Closing kills this terminal's shell and every process it started:";

/// `.tab-error`: the refusal line's padding, vertical then horizontal.
const REFUSAL_PAD: (f32, f32) = (4., 10.);

/// Whether shell tab `id`'s close is in flight.
fn closing(key: &str, id: &str, cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations
            .read(cx)
            .in_flight(&OpKey::tab(key, id))
            .is_some()
    })
}

/// The × of shell tab `id` (named `label` now) was pressed: close it at once when **Skip session confirmations** is on
/// (SPEC.md: the switch skips "terminal-tab Close"), else ask first, hung under the ×. The question names the tab as it
/// was named when it opened and acts on its id, so a tab before it closing in the meantime closes the right one.
fn ask_to_close(
    set: &Entity<TerminalSet>,
    id: &str,
    label: &str,
    anchor: &Anchor,
    window: &mut Window,
    cx: &mut App,
) {
    let mark = Mark::take(cx);
    let skips = session::current_store(cx).is_some_and(|store| {
        crate::session_list::menu::skips_questions(store.read(cx).collections())
    });
    if skips {
        set.update(cx, |set, cx| set.close_tab(id, cx));
        return;
    }
    let question = Question::new(
        format!("Close {label}?"),
        Answer::new(
            "Confirm close",
            HoverHelp::tooltip(format!(
                "Close {label}, ending its shell and everything it started"
            )),
            Tier::Danger,
        ),
        id.to_owned(),
    )
    .line(CLOSE_TAB_CONSEQUENCE)
    .line(label.to_owned())
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip("Cancel: keep the tab open"),
        Tier::Neutral,
    ));
    let confirmed = set.downgrade();
    prompt::ask_anchored(
        question,
        anchor.clone(),
        Some(Subject::of(set)),
        &mark,
        move |id, _, cx| {
            let _ = confirmed.update(cx, |set, cx| set.close_tab(&id, cx));
        },
        window,
        cx,
    );
}

/// Show the new shell and focus it only if the user waited for the reply and row update. Choosing it hides the
/// current terminal, so even a switch without an explicit focus move could redirect the user's next key.
fn choose_in(
    set: &Entity<TerminalSet>,
    id: String,
    mark: Mark,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    if focus::has_moved_on(&mark, cx) {
        return;
    }
    let set = set.clone();
    let _ = window.update(cx, move |_, window, cx| {
        set.update(cx, |set, cx| set.choose(Some(id), &mark, window, cx));
    });
}

/// Says how much of the terminals' notices is held back now; shared by every terminal of the set (see
/// [`AttachedTerminal::hold_notices`]).
pub(crate) type SharedHold = Rc<dyn Fn(&App) -> Hold>;

/// One shell tab, in the helm's order.
struct Tab {
    /// The helm's id for it, never parsed.
    id: String,
    /// Its terminal, while it is among the first [`MAX_ATTACHED_TABS`].
    terminal: Option<Entity<AttachedTerminal>>,
    /// Its button's focus handle in the strip, kept with the tab so focus stays on its button as tabs come and go.
    focus: FocusHandle,
    /// Its ×'s focus handle, kept with the tab for the same reason, and so the strip can tell when the × holds focus
    /// ([`TerminalSet::reveal_focused`]).
    close_focus: FocusHandle,
}

/// A control in the strip that keyboard focus can rest on, as [`TerminalSet::reveal_focused`] tells them apart: each
/// is revealed once when focus reaches it, and a tab and its × share one item of the strip but are revealed
/// differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StripControl {
    /// The Agent tab, the strip's first item.
    Agent,
    /// Shell tab `n`'s button, at the start of the item that holds it and its × (item `n + 1`).
    Tab(usize),
    /// Shell tab `n`'s ×, at the end of that same item.
    Close(usize),
    /// New terminal, the strip's last item (after the shell tabs' items and the divider).
    Add,
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
    /// The shell tabs, in the helm's order.
    tabs: Vec<Tab>,
    /// The chosen tab's id, `None` for the agent terminal. Only ever a listed tab's.
    chosen: Option<String>,
    /// The hold every terminal's notices answer to.
    hold: SharedHold,
    /// The Agent tab's focus handle in the strip.
    agent_focus: FocusHandle,
    /// New terminal's focus handle in the strip, held here so the strip can tell when it holds focus.
    add_focus: FocusHandle,
    /// The strip's sideways scroll, and the control it last scrolled into view for keyboard focus: a control Tab
    /// reaches is scrolled into view once, so the wheel can still move the strip while it holds focus. A `Cell` because
    /// the strip is built while the header draws, from a shared reference, and asking the scroll handle to reveal a
    /// control then is how GPUI expects it (the handle acts on it when it lays the strip out).
    scroll: ScrollHandle,
    revealed: Cell<Option<StripControl>>,
    /// Whether the terminals are inert ([`TerminalSet::set_inert`]), which new tabs take on too.
    inert: bool,
    /// A tab New terminal opened that the pushed row does not list yet, with the press that asked and its window: it is
    /// chosen once it is listed if the user waited.
    opened: Option<(String, Mark, AnyWindowHandle)>,
    /// Follows the pushed row's tab list.
    _tabs_follow: session::ObservingCurrent,
}

impl TerminalSet {
    /// The terminals of the session filed under `key`, made for `session`: the agent terminal at `metrics`, and the
    /// session's tabs as the store lists them. Every terminal holds back as much of its notices as `hold` says.
    pub fn new(
        session: &Entity<UiSession>,
        key: String,
        metrics: CellMetrics,
        hold: SharedHold,
        cx: &mut Context<Self>,
    ) -> Self {
        let lease = AttachmentLease::new();
        let agent = Self::make(session, &key, None, &lease, metrics, &hold, false, None, cx);
        let tabs_follow = session::observe_current(cx, |set: &mut Self, cx| set.follow_tabs(cx));
        let mut set = TerminalSet {
            session: session.downgrade(),
            key,
            lease,
            agent,
            tabs: Vec::new(),
            chosen: None,
            hold,
            agent_focus: cx.focus_handle().tab_stop(true),
            add_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            revealed: Cell::new(None),
            inert: false,
            opened: None,
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
        // A Restart detaches only the agent's terminal, so a tab keeps showing its own reconnect card: a tab's drop
        // during the Restart is its own outage, and holding its card would hide it until the helm answered.
        let agent = tab.is_none();
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
            let notices: NoticeHold = Box::new(move |cx| match hold(cx) {
                Hold::ReconnectCard if !agent => Hold::Nothing,
                held => held,
            });
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

    /// Whether the set is inert ([`TerminalSet::set_inert`]), for tests.
    #[cfg(test)]
    pub fn is_inert(&self) -> bool {
        self.inert
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
    /// strip dimmed and inert beneath the card"). Inert terminals hold no focus, so focus one of them held goes home
    /// through the focus coordinator. The session pane's unavailable card calls it while it shows over the set
    /// (`session_pane::unavailable`), and makes itself home meanwhile.
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

    /// Bring the tabs in line with the pushed row: keep the terminals of tabs still listed, make terminals for new
    /// ones, drop those no longer listed, and fall back to the agent terminal when the chosen tab went.
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
                close_focus: cx.focus_handle().tab_stop(true),
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
                // Not fitted here: the notify below redraws the shown terminal, whose measured pane fits it (`measured`).
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
        // The requested tab can be chosen after this update releases the set.
        if let Some((id, _, _)) = &self.opened
            && self.tabs.iter().any(|tab| &tab.id == id)
            && let Some((id, mark, window)) = self.opened.take()
        {
            let set = cx.entity();
            cx.defer(move |cx| choose_in(&set, id, mark, window, cx));
        }
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

    /// The strip for the top row's left region: the agent terminal's tab, then one per shell tab, each choosing its
    /// terminal and with its × beside it, then a divider and New terminal, in a row that scrolls sideways when it runs
    /// out of room (SPEC.md: "only the tab strip takes the remaining width and scrolls horizontally when needed").
    ///
    /// Each tab is a button reporting whether its terminal is the one shown (pressed), as the old strip's were, not a
    /// tab list, which could not hold the close buttons beside them. A tab, ×, or New terminal that keyboard focus
    /// reaches while scrolled out of view is scrolled into it ([`TerminalSet::reveal_focused`]).
    pub fn strip(set: &Entity<TerminalSet>, window: &mut Window, cx: &mut App) -> AnyElement {
        let this = set.read(cx);
        let key = this.key.clone();
        let chosen = this.chosen.clone();
        let inert = this.inert;
        this.reveal_focused(window);
        let listed: Vec<(String, FocusHandle, FocusHandle)> = this
            .tabs
            .iter()
            .map(|tab| (tab.id.clone(), tab.focus.clone(), tab.close_focus.clone()))
            .collect();
        let agent_focus = this.agent_focus.clone();
        let add_focus = this.add_focus.clone();
        let scroll = this.scroll.clone();
        let palette = theme::palette(cx);
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
                .into_any_element()
        };
        let mut items = vec![tab(
            AGENT_TAB_ID.to_owned(),
            "Agent".to_owned(),
            "Agent: show the session's agent terminal".to_owned(),
            None,
            &agent_focus,
        )];
        for (index, (id, focus, close_focus)) in listed.into_iter().enumerate() {
            let label = tab_label(index);
            let closing = closing(&key, &id, cx);
            let anchor = Anchor::for_element(
                SharedString::from(format!("{}-close-anchor", tab_element_id(&id))),
                window,
                cx,
            );
            let close = {
                let set = set.clone();
                let (id, label, anchor) = (id.clone(), label.clone(), anchor.clone());
                Button::new(
                    close_element_id(&id),
                    "×",
                    HoverHelp::tooltip(format!(
                        "Close {label}, ending its shell and everything it started"
                    )),
                    Role::Button,
                )
                .tier(Tier::TerminalTabClose)
                .accessible_name(format!("Close {label}"))
                .focus(&close_focus)
                .disabled(inert)
                .busy(closing)
                .on_click(move |_, window, cx| ask_to_close(&set, &id, &label, &anchor, window, cx))
            };
            // The tab and its ×: siblings, as the old `.tab-slot` held them, the × its own anchor for the question.
            items.push(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .child(tab(
                        tab_element_id(&id),
                        label.clone(),
                        format!("{label}: show this shell tab"),
                        Some(id.clone()),
                        &focus,
                    ))
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .child(close)
                            .child(anchor.track()),
                    )
                    .into_any_element(),
            );
        }
        // `.tab-add-divider`: a 1-pixel rule that keeps creating a tab apart from choosing one.
        items.push(
            div()
                .flex_none()
                .w(px(1.))
                .h(px(ADD_DIVIDER_HEIGHT))
                .mx(px(2.))
                .bg(theme::color(palette.border_dim))
                .into_any_element(),
        );
        // SPEC.md, "Lifecycle operations": "an open lifecycle confirmation or a pending request refuses another
        // lifecycle action, New terminal or Clone on that session". A request on the session's key is the second; any
        // question, dialog or menu holding the keyboard covers the first (the session's questions hang from its row and
        // header, and are not modal), so a press there only closes it, unanswered.
        let refused = crate::session_list::delete::busy(&key, cx)
            || crate::layers::focus_layer_open(window, cx);
        let add = {
            let set = set.clone();
            IconButton::new(
                ADD_ID,
                Icon::Add,
                "New terminal",
                HoverHelp::tooltip("New terminal: open a shell tab in the session's folder"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .square(px(ADD_SIDE))
            .focus(&add_focus)
            .disabled(inert || refused)
            .on_click(move |_, window, cx| {
                let mark = Mark::take(cx);
                let handle = window.window_handle();
                TerminalSet::open_tab(&set, mark, handle, cx);
            })
        };
        items.push(add.into_any_element());
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
            .track_scroll(&scroll)
            .when(inert, |strip| strip.opacity(INERT_OPACITY))
            .children(items)
            .into_any_element()
    }

    /// Scroll the strip to the control keyboard focus is on, once per control it reaches (see `revealed`): a tab, a
    /// tab's ×, or New terminal. Without this a control Tab reaches can hold focus while drawn wholly outside the
    /// strip's visible part, which happens readily beside a tunnel pill, where the top row may leave the strip only a
    /// few pixels (`session_pane::header`'s fit).
    ///
    /// The strip's items are the Agent tab, then one item per shell tab holding its button and its ×, then the divider
    /// and New terminal, which [`StripControl`] maps focus onto. A tab and New terminal are scrolled to as items; a ×
    /// sits at the end of its tab's item, so it is revealed by that item's end ([`Self::reveal_end`]). Focus moving
    /// from a tab to its own × still counts as reaching a new control.
    fn reveal_focused(&self, window: &Window) {
        let focused = if self.agent_focus.is_focused(window) {
            Some(StripControl::Agent)
        } else if self.add_focus.is_focused(window) {
            Some(StripControl::Add)
        } else {
            self.tabs.iter().enumerate().find_map(|(n, tab)| {
                if tab.focus.is_focused(window) {
                    Some(StripControl::Tab(n))
                } else if tab.close_focus.is_focused(window) {
                    Some(StripControl::Close(n))
                } else {
                    None
                }
            })
        };
        if focused != self.revealed.get() {
            match focused {
                Some(StripControl::Agent) => self.scroll.scroll_to_item(0),
                Some(StripControl::Tab(n)) => self.scroll.scroll_to_item(n + 1),
                Some(StripControl::Close(n)) => self.reveal_end(n + 1),
                // After the shell tabs' items and the divider.
                Some(StripControl::Add) => self.scroll.scroll_to_item(self.tabs.len() + 2),
                None => {}
            }
            self.revealed.set(focused);
        }
    }

    /// Scroll the strip so the end of its item `index` shows, where a tab's × sits. `scroll_to_item` shows an item
    /// wider than the strip from its start, which for a tab's item is the tab and leaves the × out of view, so such an
    /// item is lined up with the strip's right edge instead; an item that fits is scrolled to as usual, which shows it
    /// whole. The item's width comes from the frame the strip was last drawn in (the scroll handle keeps it), so an
    /// item not drawn yet is scrolled to as usual too.
    fn reveal_end(&self, index: usize) {
        let viewport = self.scroll.bounds();
        match self.scroll.bounds_for_item(index) {
            Some(item) if item.size.width > viewport.size.width => {
                let mut offset = self.scroll.offset();
                offset.x = viewport.right() - item.right();
                self.scroll.set_offset(offset);
            }
            _ => self.scroll.scroll_to_item(index),
        }
    }

    /// Open a shell tab in the session (New terminal), under the session's own key, so a pending lifecycle request
    /// refuses it and it holds the session's lifecycle actions while it runs. Once the helm has answered and listed
    /// the new tab, the tab is chosen and its terminal takes focus if the user has not moved on since `mark`, as the
    /// old app selected a tab it opened; a refusal shows on the session's row and header ("Open tab: " and the helm's
    /// words). Not keyed for retries: a duplicate tab is visible and closes on its own, and the button sends once per
    /// press.
    fn open_tab(set: &Entity<TerminalSet>, mark: Mark, window: AnyWindowHandle, cx: &mut App) {
        let key = set.read(cx).key.clone();
        let (Some(operations), Ok(path)) = (
            session::current_operations(cx),
            ApiPath::api(&["sessions", &key, "tabs"]),
        ) else {
            return;
        };
        let command = Command::new(
            Work::TabOpen,
            Method::Post,
            path,
            None,
            "POST /api/sessions/[session]/tabs",
        );
        let weak = set.downgrade();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::session(&key), command, cx, move |settled, cx| {
                let Outcome::Done(Some(body)) = &settled.outcome else {
                    return;
                };
                let Some(id) = body["tab"]["id"].as_str().map(str::to_owned) else {
                    return;
                };
                let Some(set) = weak.upgrade() else {
                    return;
                };
                if set.read(cx).tabs.iter().any(|tab| tab.id == id) {
                    choose_in(&set, id, mark, window, cx);
                } else {
                    // The reply can beat the pushed row. The mark must still hold when the tab is listed.
                    set.update(cx, |set, _| set.opened = Some((id, mark, window)));
                }
            })
        });
        if let Err(not_started) = started {
            tracing::debug!(?not_started, "New terminal sent nothing");
        }
    }

    /// Close shell tab `id` (its × and, unless skipped, its question), under the tab's own key, so siblings close side by
    /// side. Its terminal goes when the pushed row stops listing it; a chosen tab that closed shows the agent terminal
    /// at once. A refusal shows under the strip until the tab's next close.
    fn close_tab(&mut self, id: &str, cx: &mut Context<Self>) {
        let (Some(operations), Ok(path)) = (
            session::current_operations(cx),
            ApiPath::api(&["sessions", &self.key, "tabs", id]),
        ) else {
            return;
        };
        let command = Command::new(
            Work::TabClose,
            Method::Delete,
            path,
            None,
            "DELETE /api/sessions/[session]/tabs/[tab]",
        );
        let set = cx.weak_entity();
        let closed = id.to_owned();
        let started = operations.update(cx, |operations, cx| {
            operations.start(
                &OpKey::tab(&self.key, id),
                command,
                cx,
                move |settled, cx| {
                    if matches!(settled.outcome, Outcome::Done(_)) {
                        let _ = set.update(cx, |set, cx| {
                            if set.chosen.as_deref() == Some(closed.as_str()) {
                                set.chosen = None;
                                cx.notify();
                            }
                        });
                    }
                },
            )
        });
        if let Err(not_started) = started {
            tracing::debug!(?not_started, "a tab's close sent nothing");
        }
    }
}

impl Render for TerminalSet {
    /// The refused closes' lines (one per listed tab whose close the helm refused, in the strip's order, as the old
    /// `.tab-error` lines under the strip), then the shown terminal, or the not-attached notice, in the rest.
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let operations = session::current_operations(cx);
        let refusals: Vec<AnyElement> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                let failure = operations
                    .as_ref()?
                    .read(cx)
                    .failure(&OpKey::tab(&self.key, &tab.id))?
                    .clone();
                let id = close_refusal_id(&tab.id);
                Some(
                    div()
                        .id(SharedString::from(id.clone()))
                        .debug_selector(move || id.clone())
                        .flex()
                        .flex_row()
                        .flex_none()
                        .py(px(REFUSAL_PAD.0))
                        .px(px(REFUSAL_PAD.1))
                        .text_size(theme::metrics::TEXT_SM)
                        .text_color(theme::color(palette.danger))
                        .child("Close tab: ")
                        .child(host_text::element(&failure.message))
                        .into_any_element(),
                )
            })
            .collect();
        let shown = match self.shown() {
            Some(terminal) => div().flex_1().min_h_0().child(terminal.clone()),
            None => div().flex_1().min_h_0().child(
                div()
                    .id(NOT_ATTACHED_ID)
                    .debug_selector(|| NOT_ATTACHED_ID.into())
                    .p(px(NOT_ATTACHED_PAD))
                    .text_size(px(NOT_ATTACHED_TEXT_SIZE))
                    .text_color(theme::color(palette.fg_dim))
                    .child(not_attached_text()),
            ),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(refusals)
            .child(shown)
    }
}
