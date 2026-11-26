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
//!   chosen terminal is home (the session pane makes it so whenever the set changes). Closing or reaping the chosen
//!   tab makes its nearest earlier surviving tab home, or the agent terminal when there is none. A dialog or question
//!   that holds focus keeps it.
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
//! - **A tab's × sits at its left, and shows only when asked for.** It used to sit at the right of its tab, always
//!   shown, so the last tab's × sat right beside New terminal and a press meant for New terminal kept closing a shell
//!   (the maintainer's TODO entry, "Stop accidental terminal tab closes when creating a new terminal"). Now each shell
//!   tab starts with a fixed leading slot showing a terminal mark, and the × takes that same place, instead of the
//!   mark, while the pointer is over the tab, keyboard focus is on the tab or its ×, the tab's close question is open,
//!   or its close is in flight ([`TerminalSet::strip`]). Nothing moves when it shows: the × is always drawn, over the
//!   slot and transparent while it is not shown, so it keeps its place in Tab's order (the tab, then its ×) and its
//!   anchor for the question, and the strip's layout never depends on the pointer. The Agent tab has neither mark
//!   nor ×.
//!
//! - **The choice survives a new view.** Reopening a session or reconnecting to the helm shows the remembered shell
//!   tab if it has a terminal in the set. A deliberate selection or a close fallback updates the choice, and either
//!   can clear it by choosing Agent. A missing tab or a refused initial attach during restore leaves the choice
//!   intact for another opening. A temporary reconnect keeps trying the remembered tab.
//!   Opening a session the sidebar filters hid can precede its first row. If the user is still waiting when that row
//!   comes in, it supplies the initial tab list. The terminal shown after restoration receives upload notices left by
//!   the old view.
//!
//! The close question names the tab as it was named when it opened (the old one renamed itself as tabs before it
//! closed). It acts on the tab's id either way.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, EntityId,
    FocusHandle, Global, HoverListenerMode, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Pixels, Render, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px, svg,
};

#[cfg(test)]
mod tests;

use super::CellMetrics;
use super::attached::{AttachedTerminal, Hold, InitialTabAttach, NoticeHold, TerminalTarget};
use super::attachments::Carried;
use super::tunnels::Tunnels;
use crate::assets::Icon;
use crate::components::{
    Button, HoverHelp, IconButton, TERMINAL_TAB_CLOSE_SIDE, TERMINAL_TAB_INSET, TOOLBAR_ICON_GAP,
    Tier,
};
use crate::focus::{self, Mark};
use crate::host_text;
use crate::layers::{self, Anchor, LayerId, Subject};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::prompt::{self, Answer, Question};
use crate::session::{self, UiSession};
use crate::theme;

/// How many tabs are attached at most (the old app's `MAX_MOUNTED_TAB_ISLANDS`). See the module docs.
pub(crate) const MAX_ATTACHED_TABS: usize = 32;

/// The strip's element id: the whole tab area, its scrolling tabs and New terminal pinned after them.
pub(crate) const STRIP_ID: &str = "session-terminal-tabs";

/// The element id of the strip's part that scrolls: the Agent tab and the shell tabs, without the divider and New
/// terminal, which stay put at the strip's right end ([`TerminalSet::strip`]).
pub(crate) const STRIP_TABS_ID: &str = "session-terminal-tabs-scroll";

/// The agent terminal's tab's element id.
pub(crate) const AGENT_TAB_ID: &str = "session-terminal-tab-agent";

/// The element id of the notice a tab past [`MAX_ATTACHED_TABS`] shows in place of a terminal.
pub(crate) const NOT_ATTACHED_ID: &str = "terminal-not-attached";

/// An unattached tab within the cap failed to measure its font. Closing other tabs only helps outside the cap.
fn not_attached_text(tabs: &[Tab], chosen: Option<&str>) -> String {
    let mut within_cap = tabs.iter().take(MAX_ATTACHED_TABS);
    if within_cap.any(|tab| Some(tab.id.as_str()) == chosen) {
        return "Farhelm could not measure the terminal font for this tab.".to_owned();
    }
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

/// The element id of shell tab `id`'s × button. The wrapper drawn around it names whether the × shows in its debug
/// selector, this id followed by `:shown` or `:hidden` ([`TerminalSet::strip`]), since the frame's record carries no
/// opacity.
pub(crate) fn close_element_id(id: &str) -> String {
    format!("session-terminal-close-{id}")
}

/// The element id of the box around shell tab `id`'s button and its ×, which listens for the pointer over either and
/// draws the tab's hover fill under both. Not under the tabs' own `session-terminal-tab-` prefix, which names buttons.
fn pill_element_id(id: &str) -> String {
    format!("session-terminal-pill-{id}")
}

/// The terminal mark in a shell tab's leading slot: one of the standard 14-unit action icons, as the × that takes its
/// place is, so the two swap without either moving.
const LEADING_ICON: f32 = 14.;

/// How far a focused button's ring reaches outside its edge (`components::button`'s ring: 2 pixels wide, 1 pixel
/// clear), which [`TerminalSet::reveal_close`] keeps in view beside a focused ×.
const RING_ROOM: f32 = 3.;

/// Where a shell tab's × sits from the tab's left edge: centered on the leading slot, which starts
/// [`TERMINAL_TAB_INSET`] in. The × is wider than the mark, so it starts left of the slot.
const CLOSE_LEFT: f32 = TERMINAL_TAB_INSET + LEADING_ICON / 2. - TERMINAL_TAB_CLOSE_SIDE / 2.;

/// A shell tab's face, drawn in place of its name's text ([`Button::content`]): the leading slot with the terminal
/// mark, then the name, as far apart as a button's icon and label ([`TOOLBAR_ICON_GAP`]). The slot is always laid
/// out, so the tab is as wide with the mark showing as with the × over it; `revealed` only makes the mark transparent
/// while the × shows in its place. `ink` is the tab's own ink
/// ([`Button::ink`]), since an SVG takes none from the button around it.
fn shell_tab_face(label: String, ink: Hsla, revealed: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(TOOLBAR_ICON_GAP)
        .child(
            div()
                .flex_none()
                .size(px(LEADING_ICON))
                .opacity(if revealed { 0. } else { 1. })
                .child(
                    svg()
                        .path(Icon::Terminal.path())
                        .size(px(LEADING_ICON))
                        .text_color(ink),
                ),
        )
        .child(label)
}

/// Whether the pointer is over a shell tab's box (its button or its ×), as the box last reported it: one of the ×'s
/// reveal reasons. Element state under the tab's id, so a tab drawn again starts unhovered and a tab that scrolls or
/// closes away under a resting pointer cannot leave a × shown. Recorded from a hover listener that GPUI runs whatever
/// the last input was, unlike its hover styles, which stand down after a key press: a key typed while the pointer
/// rests on a tab must not hide the × the user is about to press. Local to the strip rather than shared with the
/// session list's `RowHover`, which does the same for a row's menu button, because only these two use it and they are
/// keyed by different things. A change redraws the strip.
#[derive(Clone)]
struct TabHover(Entity<bool>);

impl TabHover {
    /// Shell tab `id`'s hover state, made while the strip is drawn.
    fn of(id: &str, window: &mut Window, cx: &mut App) -> TabHover {
        TabHover(window.use_keyed_state(
            SharedString::from(format!("{}-hovered", pill_element_id(id))),
            cx,
            |_, _| false,
        ))
    }

    /// Record that the pointer is over the tab's box (`hovered`) or not; only a change redraws.
    fn set(&self, hovered: bool, cx: &mut App) {
        self.0.update(cx, |state, cx| {
            if *state != hovered {
                *state = hovered;
                cx.notify();
            }
        });
    }

    /// Whether the pointer is over the tab's box.
    fn hovered(&self, cx: &App) -> bool {
        *self.0.read(cx)
    }
}

/// The element id of the line under the strip saying why closing shell tab `id` was refused.
pub(crate) fn close_refusal_id(id: &str) -> String {
    format!("session-terminal-close-refusal-{id}")
}

/// New terminal's element id.
pub(crate) const ADD_ID: &str = "session-terminal-add";

/// `.tab-add`: a 24-pixel square.
pub(crate) const ADD_SIDE: f32 = 24.;

/// `.tab-add-divider`: 16 pixels high.
const ADD_DIVIDER_HEIGHT: f32 = 16.;

/// `.tab-add-divider`'s width: a 1-pixel rule.
const ADD_DIVIDER_WIDTH: f32 = 1.;

/// `.tab-add-divider`'s margin on each side, beyond the strip's gaps.
const ADD_DIVIDER_MARGIN: f32 = 2.;

/// The width New terminal takes when pinned at the strip's end: the gap before the divider, the divider and its
/// 2-pixel margins, the gap after it, and the button.
const PINNED_ADD_WIDTH: f32 =
    STRIP_GAP + ADD_DIVIDER_MARGIN + ADD_DIVIDER_WIDTH + ADD_DIVIDER_MARGIN + STRIP_GAP + ADD_SIDE;

/// The least width the scrolling tabs keep beside a pinned New terminal: a control's worth, so a focused tab or × is
/// still recognizable when scrolled into view (SPEC.md keeps the top row reachable down to a 650-pixel pane). A strip
/// narrower than New terminal and this together draws New terminal at the end of the scrolling tabs instead
/// ([`TerminalSet::pins_add`]).
const PINNED_TABS_FLOOR: f32 = ADD_SIDE;

/// Whether a strip `width` wide pins New terminal at its end ([`TerminalSet::pins_add`]). A zero width is taken for a
/// strip not laid out yet and pins; a strip really laid out at zero width (only in a pane narrower than the 650 pixels
/// SPEC.md keeps the row whole for, where clipping is allowed) shows no tab either way.
fn pins_add_at(width: Pixels) -> bool {
    width == px(0.) || width >= px(PINNED_ADD_WIDTH + PINNED_TABS_FLOOR)
}

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
/// was named when it opened and acts on its id, so a tab before it closing in the meantime closes the right one. The
/// set records the question's layer ([`TerminalSet::asking`]), so the × stays shown while its question is open, even
/// once the pointer has left the tab for the question.
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
    let asked = prompt::ask_anchored(
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
    if let Some(layer) = asked {
        set.update(cx, |set, cx| {
            set.asking = Some((id.to_owned(), layer));
            cx.notify();
        });
    }
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
    /// Follows the terminal's attach outcome while this set can still be restoring the tab.
    _attach: Option<Subscription>,
    /// Its button's focus handle in the strip, kept with the tab so focus stays on its button as tabs come and go.
    focus: FocusHandle,
    /// Its ×'s focus handle, kept with the tab for the same reason, and so the strip can tell when the × holds focus
    /// ([`TerminalSet::reveal_focused`]).
    close_focus: FocusHandle,
}

/// A control in the strip that keyboard focus can rest on, as [`TerminalSet::reveal_focused`] tells them apart: each
/// is revealed once when focus reaches it, and a tab and its × share one item of the strip but are revealed
/// differently (the item's start, or the × a few pixels into it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StripControl {
    /// The Agent tab, the strip's first item.
    Agent,
    /// Shell tab `n`'s button, the item that holds it and its × (item `n + 1`).
    Tab(usize),
    /// Shell tab `n`'s ×, over the start of that same item.
    Close(usize),
    /// New terminal: pinned after the scrolling tabs, where focus reaching it scrolls nothing, or, in a strip too
    /// narrow to pin it ([`TerminalSet::pins_add`]), the scrolling part's last item, after the divider.
    Add,
}

/// The last chosen shell tab in each session, including the fallback after
/// a close. The app owns the choice across pane remounts and helm resets.
/// Tab ids are never reused, so a missing restore target can stay remembered.
#[derive(Default)]
struct ShownTerminals(HashMap<String, String>);

impl Global for ShownTerminals {}

/// Which boundary an automatic restore is waiting for. Carried upload notices wait too so they go to the final
/// visible terminal, or to the notice center if the user leaves while the restore is pending.
enum Restore {
    WaitingForRow(Mark),
    Attaching(String),
}

/// The direction a shortcut moves through the session's tab strip.
#[derive(Clone, Copy)]
pub(crate) enum TabStep {
    Previous,
    Next,
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
    /// Successful sibling closes can arrive before the roster changes. These
    /// ids are ineligible for selection until the roster drops them.
    confirmed_closed: HashSet<String>,
    /// The pending restore for this opening. An explicit choice takes precedence even if it chooses this same tab.
    restore: Option<Restore>,
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
    /// The control last scrolled into view (see `scroll`), and for New terminal whether it was pinned then
    /// ([`TerminalSet::pins_add`]), since it needs revealing again when it moves into the scrolling part.
    revealed: Cell<Option<(StripControl, bool)>>,
    /// The whole strip's bounds as last laid out, tabs and New terminal together: the width the top row gave the strip,
    /// which does not depend on where New terminal sits in it, so deciding from it cannot flip the choice back
    /// ([`TerminalSet::pins_add`]). Its scroll position is never used.
    strip_bounds: ScrollHandle,
    /// Whether the terminals are inert ([`TerminalSet::set_inert`]), which new tabs take on too.
    inert: bool,
    /// The tab whose close question was asked last, by id, and the question's layer: while that layer is open, the
    /// tab's × stays shown ([`TerminalSet::strip`]). Never cleared, only replaced: the layer manager says whether the
    /// layer is still open whenever the strip draws, so a question answered, cancelled or dismissed some other way
    /// needs no bookkeeping here. Only the last one matters, since a press on another tab's × closes the open question
    /// before it asks its own. The session header keeps its questions the same way (`session_pane::header`'s `asked`).
    asking: Option<(String, LayerId)>,
    /// A tab New terminal opened that the pushed row does not list yet, with the press that asked and its window: it is
    /// chosen once it is listed if the user waited.
    opened: Option<(String, Mark, AnyWindowHandle)>,
    /// Follows the pushed row's tab list.
    _tabs_follow: session::ObservingCurrent,
    /// Routes upload notices left by retired terminals, including a tab closed in this set.
    _carried: Subscription,
}

impl TerminalSet {
    /// The terminals of the session filed under `key`, made for `session`: the agent terminal at `metrics`, and the
    /// session's tabs as the store lists them. Restore the remembered tab only after its terminal is available to
    /// draw and focus. Every terminal holds back as much of its notices as `hold` says.
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
        let carried = cx.observe_global::<Carried>(|set, cx| set.take_carried(cx));
        let restore = cx
            .try_global::<ShownTerminals>()
            .is_some_and(|shown| shown.0.contains_key(&key))
            .then(|| Restore::WaitingForRow(Mark::take(cx)));
        let mut set = TerminalSet {
            session: session.downgrade(),
            key,
            lease,
            agent,
            tabs: Vec::new(),
            chosen: None,
            confirmed_closed: HashSet::new(),
            restore,
            hold,
            agent_focus: cx.focus_handle().tab_stop(true),
            add_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            revealed: Cell::new(None),
            strip_bounds: ScrollHandle::new(),
            inert: false,
            asking: None,
            opened: None,
            _tabs_follow: tabs_follow,
            _carried: carried,
        };
        set.follow_tabs(cx);
        set.take_carried(cx);
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
                AttachedTerminal::sharing(session, target, metrics, open, tunnels, false, cx);
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
    /// chosen or the chosen tab's font could not be measured.
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
    /// through the focus coordinator, which also makes it home. Only live entries in the current strip can be chosen.
    /// An explicit Agent choice clears the memory even when Agent is already shown as a fallback.
    pub fn choose(
        &mut self,
        id: Option<String>,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(wanted) = &id
            && (self.confirmed_closed.contains(wanted)
                || !self.tabs.iter().any(|tab| &tab.id == wanted))
        {
            return;
        }
        if self.set_choice(id, cx) {
            cx.notify();
        }
        self.take_carried(cx);
        match self.home(cx) {
            Some(home) => focus::choose_terminal(&home, mark, window, cx),
            None => focus::set_home(None, window, cx),
        }
    }

    /// Cycle the main terminal and the shell tabs with mounted terminals. Confirmed closes and tabs beyond the attach
    /// limit can still occupy painted positions, so keep the original strip index when scrolling to the choice.
    pub(crate) fn step(
        &mut self,
        direction: TabStep,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.inert || self.shown().is_none() {
            return;
        }
        let mut live = vec![(0, None)];
        live.extend(
            self.tabs
                .iter()
                .enumerate()
                .filter(|(_, tab)| {
                    tab.terminal.is_some() && !self.confirmed_closed.contains(&tab.id)
                })
                .map(|(index, tab)| (index + 1, Some(tab.id.as_str()))),
        );
        if live.len() == 1 {
            return;
        }
        let current = live
            .iter()
            .position(|&(_, id)| id == self.chosen.as_deref())
            .unwrap_or(0);
        let target = match direction {
            TabStep::Previous => (current + live.len() - 1) % live.len(),
            TabStep::Next => (current + 1) % live.len(),
        };
        let (strip_index, id) = live[target];
        self.choose(id.map(str::to_owned), mark, window, cx);
        self.scroll.scroll_to_item(strip_index);
    }

    /// A deliberate choice or close fallback replaces the restore target and
    /// its saved selection together. Initial-attach refusals use their own path.
    fn set_choice(&mut self, id: Option<String>, cx: &mut Context<Self>) -> bool {
        self.restore = None;
        let shown = cx.default_global::<ShownTerminals>();
        match &id {
            Some(id) => {
                shown.0.insert(self.key.clone(), id.clone());
            }
            None => {
                shown.0.remove(&self.key);
            }
        }
        let changed = self.chosen != id;
        self.chosen = id;
        changed
    }

    /// Return the nearest earlier tab in the old strip that still appears in the roster and has no confirmed close.
    /// The old strip supplies the closing tab's position. `None` shows the main terminal.
    fn previous_surviving_tab(
        &self,
        closed: &str,
        still_listed: impl Fn(&str) -> bool,
    ) -> Option<String> {
        let index = self.tabs.iter().position(|tab| tab.id == closed)?;
        self.tabs[..index]
            .iter()
            .rev()
            .find(|tab| !self.confirmed_closed.contains(&tab.id) && still_listed(&tab.id))
            .map(|tab| tab.id.clone())
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
    /// ones, and choose a predecessor from the old order when the selected tab disappears.
    fn follow_tabs(&mut self, cx: &mut Context<Self>) {
        let listed: Option<Vec<String>> = session::current_store(cx).and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<SessionRow>(&self.key)
                .map(|row| row.info.tabs.iter().map(|tab| tab.id.clone()).collect())
        });
        // An omitted row tells us nothing about its terminals. A real row, including one listing no tabs, settles
        // the roster and any remembered choice. The pane itself closes if the listing proves the session gone.
        let Some(listed) = listed else {
            return;
        };
        let waiting_for_row = self
            .restore
            .take_if(|restore| matches!(restore, Restore::WaitingForRow(_)));
        // Most pushes change something else about the row (its activity, its status): nothing to do then. A tab whose
        // terminal could not be made (the font did not measure) is tried again at the next change of the list.
        let unchanged = listed.len() == self.tabs.len()
            && listed.iter().zip(&self.tabs).all(|(id, tab)| *id == tab.id);
        if unchanged {
            if waiting_for_row.is_some() {
                self.take_carried(cx);
            }
            return;
        }
        let Some(session) = self.session.upgrade() else {
            return;
        };
        let listed_ids: HashSet<&str> = listed.iter().map(String::as_str).collect();
        self.confirmed_closed
            .retain(|id| listed_ids.contains(id.as_str()));
        if let Some(chosen) = &self.chosen
            && !listed_ids.contains(chosen.as_str())
        {
            let fallback = self.previous_surviving_tab(chosen, |id| listed_ids.contains(id));
            self.set_choice(fallback, cx);
        }
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
                _attach: None,
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
                let tab_id = tab.id.clone();
                tab._attach = Some(cx.observe(&terminal, move |set, terminal, cx| {
                    set.follow_tab_attach(&tab_id, &terminal, cx);
                }));
                // Not fitted here: the notify below redraws the shown terminal, whose measured pane fits it (`measured`).
                tab.terminal = Some(terminal);
            }
            self.tabs.push(tab);
        }
        // What is left in `old` was not listed: its terminals go now, and their streams with them.
        drop(old);
        if let Some(Restore::WaitingForRow(mark)) = waiting_for_row
            && !focus::has_moved_on(&mark, cx)
        {
            self.chosen = cx
                .try_global::<ShownTerminals>()
                .and_then(|shown| shown.0.get(&self.key))
                .filter(|wanted| {
                    self.tabs
                        .iter()
                        .take(MAX_ATTACHED_TABS)
                        .any(|tab| &tab.id == *wanted && tab.terminal.is_some())
                })
                .cloned();
            self.restore = self.chosen.clone().map(Restore::Attaching);
        }
        // One push can remove the restoring tab and list the tab New requested. Let the deferred choice
        // settle before delivering notices, so they reach the terminal the user will see after this push.
        if let Some((id, _, _)) = &self.opened
            && self.tabs.iter().any(|tab| &tab.id == id)
            && let Some((id, mark, window)) = self.opened.take()
        {
            let set = cx.entity();
            cx.defer(move |cx| {
                choose_in(&set, id, mark, window, cx);
                // The user may have moved on. In that case `choose` did not run to deliver the notices.
                set.update(cx, |set, cx| set.take_carried(cx));
            });
        } else {
            self.take_carried(cx);
        }
        cx.notify();
    }

    /// Settle restoration only for the tab this opening chose automatically. If its first attach is refused, the main
    /// terminal becomes home and receives the notices. Keep the remembered choice for the next opening.
    fn follow_tab_attach(
        &mut self,
        tab: &str,
        terminal: &Entity<AttachedTerminal>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(&self.restore, Some(Restore::Attaching(wanted)) if wanted == tab) {
            return;
        }
        match terminal.read(cx).initial_tab_attach() {
            InitialTabAttach::Pending => return,
            InitialTabAttach::Refused => self.chosen = None,
            InitialTabAttach::Settled => {}
        }
        self.restore = None;
        self.take_carried(cx);
        cx.notify();
    }

    /// Deliver old upload results once the remembered tab's attach settles. A tab past the attach limit has no
    /// visible strip. The hidden Agent can still keep failures already shown, while interruptions wait for a
    /// visible terminal or a departure.
    fn take_carried(&mut self, cx: &mut Context<Self>) {
        if self.restore.is_some() {
            return;
        }
        let shown = self.shown();
        let attachments = shown.unwrap_or(&self.agent).read(cx).attachments().clone();
        attachments.update(cx, |attachments, cx| {
            attachments.take_carried(shown.is_some(), cx)
        });
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
    /// terminal and with its × over the leading slot at its left, in a part that scrolls sideways when it runs out of
    /// room, then a divider and New terminal pinned at the strip's right end (SPEC.md: "only the tab strip takes the
    /// remaining width, and its tabs scroll horizontally when needed"). A strip too narrow to pin New terminal puts the
    /// divider and New terminal at the end of the scrolling part instead ([`TerminalSet::pins_add`]).
    ///
    /// The strip and its scrolling part grow into free width but keep an automatic basis, their content's width, so
    /// that beside a tunnel pill a short row takes width from the strip and the pill in proportion to their content, as
    /// the old app's `.tab-strip { flex: 0 1 auto }` did, down to the strip's floor (`session_pane::header`'s fit). That
    /// content width is the same whether New terminal is pinned or not (the same items and gaps either way), so the
    /// strip's width, which decides the pin, does not depend on the pin.
    ///
    /// Each tab is a button reporting whether its terminal is the one shown (pressed), as the old strip's were, not a
    /// tab list, which could not hold the close buttons beside them. A tab, ×, or New terminal that keyboard focus
    /// reaches while scrolled out of view is scrolled into it ([`TerminalSet::reveal_focused`]).
    ///
    /// A shell tab's × shows, in place of its terminal mark, while the pointer is over the tab's box (not while the set
    /// is inert), keyboard focus is on the tab or its ×, its close question is open ([`TerminalSet::asking`]), or its
    /// close is in flight (see the module docs for why it moved there). How it is drawn holds three invariants:
    ///
    /// - **It is always drawn.** Only its opacity follows the reveal. Mounting it only on hover would change the strip's
    ///   elements with the pointer, which GPUI's frame cannot take (a hover read while laying out and again while
    ///   painting can disagree; `components::copy_field` has the rule against `group_hover` layout changes), and would
    ///   take the × out of Tab's order and its question's anchor out of the frame until the pointer came back.
    /// - **It is drawn over the tab, as the tab's sibling, not inside it.** Inside the button it would be a control in a
    ///   control. Drawn after the tab, it comes after it in Tab's order. Its box stops a primary press there, after the
    ///   × has armed its own click, so the tab under it neither arms its click nor takes focus from the same press (the
    ///   pattern `components::mark_toggle` documents). The gaps between tabs stay empty background for the macOS title
    ///   band's drag.
    /// - **Nothing moves when it shows.** The tab lays out its leading slot whether the mark or the × shows, and the ×
    ///   is placed absolutely over that slot, so tabs, the divider and New terminal keep their places.
    ///
    /// The box around a tab and its × listens for the pointer over either and draws an ordinary tab's hover fill
    /// itself, so the fill stays whole while the pointer is on the ×, and after a key press, when GPUI's own hover
    /// styles stand down.
    pub fn strip(set: &Entity<TerminalSet>, window: &mut Window, cx: &mut App) -> AnyElement {
        let this = set.read(cx);
        let key = this.key.clone();
        let chosen = this.chosen.clone();
        let inert = this.inert;
        let asking = this.asking.clone();
        this.reveal_focused(window);
        let listed: Vec<(String, FocusHandle, FocusHandle)> = this
            .tabs
            .iter()
            .map(|tab| (tab.id.clone(), tab.focus.clone(), tab.close_focus.clone()))
            .collect();
        let agent_focus = this.agent_focus.clone();
        let add_focus = this.add_focus.clone();
        let scroll = this.scroll.clone();
        let strip_bounds = this.strip_bounds.clone();
        let pins_add = this.pins_add();
        let strip_bounds_now = strip_bounds.clone();
        let palette = theme::palette(cx);
        // `revealed` is `None` for the Agent tab, which has no leading slot, and otherwise whether a shell tab's ×
        // shows over its slot now.
        let tab = |id: String,
                   label: String,
                   help: String,
                   value: Option<String>,
                   focus: &FocusHandle,
                   revealed: Option<bool>| {
            let set = set.clone();
            let pressed = chosen == value;
            let button = Button::new(
                id,
                label.clone(),
                HoverHelp::tooltip(help).below(),
                Role::Button,
            )
            .tier(Tier::TerminalTab)
            .pressed(pressed)
            .disabled(inert)
            .focus(focus)
            .on_click(move |_, window, cx| {
                let mark = Mark::take(cx);
                let value = value.clone();
                set.update(cx, |set, cx| set.choose(value, &mark, window, cx));
            });
            match revealed {
                None => button.into_any_element(),
                Some(revealed) => {
                    let ink = theme::color(button.ink(palette));
                    button
                        .content(shell_tab_face(label, ink, revealed))
                        .into_any_element()
                }
            }
        };
        // A shell session's main terminal runs the shell itself, and its row already says "No agent", so its tab names
        // the shell rather than an agent that is not there. A row not listed yet keeps the agent wording.
        let shell = session::current_store(cx).is_some_and(|store| {
            store
                .read(cx)
                .collections()
                .get::<SessionRow>(&key)
                .is_some_and(|row| row.info.launch.is_shell())
        });
        let (main_label, main_help) = if shell {
            ("Shell", "Shell: show the session's shell")
        } else {
            ("Agent", "Agent: show the session's agent terminal")
        };
        let mut items = vec![tab(
            AGENT_TAB_ID.to_owned(),
            main_label.to_owned(),
            main_help.to_owned(),
            None,
            &agent_focus,
            None,
        )];
        for (index, (id, focus, close_focus)) in listed.into_iter().enumerate() {
            let label = tab_label(index);
            let closing = closing(&key, &id, cx);
            let selected = chosen.as_deref() == Some(id.as_str());
            let anchor = Anchor::for_element(
                SharedString::from(format!("{}-close-anchor", tab_element_id(&id))),
                window,
                cx,
            );
            let hover = TabHover::of(&id, window, cx);
            // An inert strip takes no presses, so the pointer over it reveals nothing; a question or a close already
            // under way still shows whose it is.
            let hovered = hover.hovered(cx) && !inert;
            let asked = asking
                .as_ref()
                .is_some_and(|(asked, layer)| *asked == id && layers::is_open(*layer, window, cx));
            let revealed = hovered
                || focus.is_focused(window)
                || close_focus.is_focused(window)
                || asked
                || closing;
            let close = {
                let set = set.clone();
                let (id, label, anchor) = (id.clone(), label.clone(), anchor.clone());
                IconButton::new(
                    close_element_id(&id),
                    Icon::Close,
                    format!("Close {label}"),
                    HoverHelp::tooltip(format!(
                        "Close {label}, ending its shell and everything it started"
                    ))
                    .below(),
                    Role::Button,
                )
                .tier(Tier::TerminalTabClose { on_fill: selected })
                .focus(&close_focus)
                .disabled(inert)
                .busy(closing)
                .on_click(move |_, window, cx| ask_to_close(&set, &id, &label, &anchor, window, cx))
            };
            let close_selector = close_element_id(&id);
            // The × over the tab's leading slot: placed by a box that takes no input, around a box as large as the ×
            // that stops the press and is the question's anchor.
            let close = div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(CLOSE_LEFT))
                .flex()
                .items_center()
                .child(
                    div()
                        .relative()
                        .flex_none()
                        .opacity(if revealed { 1. } else { 0. })
                        // Tests read the reveal here, since the frame's record carries no opacity. The closure runs
                        // only in builds that record selectors, so other builds never format the string.
                        .debug_selector(move || {
                            format!(
                                "{close_selector}:{}",
                                if revealed { "shown" } else { "hidden" }
                            )
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(close)
                        .child(anchor.track()),
                );
            items.push(
                div()
                    .id(SharedString::from(pill_element_id(&id)))
                    .relative()
                    .flex_none()
                    .rounded(theme::metrics::RADIUS_SM)
                    .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
                    .on_hover(move |hovered, _, cx| hover.set(*hovered, cx))
                    // The shown tab's accent fill is its own, and the tab draws its hover accent under the pointer.
                    .when(hovered && !selected, |pill| {
                        pill.bg(theme::color(palette.control_hover))
                    })
                    .child(tab(
                        tab_element_id(&id),
                        label.clone(),
                        format!("{label}: show this shell tab"),
                        Some(id.clone()),
                        &focus,
                        Some(revealed),
                    ))
                    .child(close)
                    .into_any_element(),
            );
        }
        // `.tab-add-divider`: a 1-pixel rule that keeps creating a tab apart from choosing one.
        let divider = div()
            .flex_none()
            .w(px(ADD_DIVIDER_WIDTH))
            .h(px(ADD_DIVIDER_HEIGHT))
            .mx(px(ADD_DIVIDER_MARGIN))
            .bg(theme::color(palette.border_dim));
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
                HoverHelp::tooltip("New terminal: open a shell tab in the session's folder")
                    .below(),
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
        // The tabs scroll in a part of their own that takes the strip's free width, and the divider and New terminal
        // come after it, outside it: New terminal stays at the tab area's right end however many tabs are open, and in
        // view while the tabs scroll. A new tab therefore never takes New terminal's place under the pointer, where a
        // second press would land on the new tab's × (the plan's 2026-11-27 decision; SPEC.md, "Session view").
        // A strip too narrow to pin it keeps New terminal at the end of the scrolling tabs ([`Self::pins_add`]).
        let mut pinned_add = Vec::new();
        let end = if pins_add {
            &mut pinned_add
        } else {
            &mut items
        };
        end.push(divider.into_any_element());
        end.push(add.into_any_element());
        let tabs = div()
            .id(STRIP_TABS_ID)
            .debug_selector(|| STRIP_TABS_ID.into())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(STRIP_GAP))
            .flex_auto()
            .min_w_0()
            .overflow_x_scroll()
            .track_scroll(&scroll)
            .children(items);
        let strip = div()
            .on_children_prepainted(move |_, window, cx| {
                // The choice was made from the last frame's width, and this frame's is known only now (`track_scroll`
                // records it before the children are prepainted). When the strip changed size across the threshold
                // (a tunnel pill appearing beside it in a narrow pane, say), draw again with the new choice rather
                // than wait for something else to redraw the row: a pinned New terminal in a strip too narrow for it
                // leaves the tabs no room at all. The strip's own width does not depend on the choice, so the second
                // frame settles it. The redraw is deferred to after this frame, since one asked for while the frame
                // is prepared is dropped (as `hosts::progress::card`'s tracker notes).
                if pins_add_at(strip_bounds_now.bounds().size.width) != pins_add {
                    window.defer(cx, |window, _| window.refresh());
                }
            })
            .id(STRIP_ID)
            .debug_selector(|| STRIP_ID.into())
            .role(Role::Group)
            .aria_label("Terminals")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(STRIP_GAP))
            .flex_auto()
            .min_w_0()
            .overflow_x_hidden()
            .track_scroll(&strip_bounds)
            .when(inert, |strip| strip.opacity(INERT_OPACITY))
            .child(tabs)
            .children(pinned_add);
        strip.into_any_element()
    }

    /// Scroll the strip to the control keyboard focus is on, once per control it reaches (see `revealed`): a tab, a
    /// tab's ×, or New terminal. Without this a control Tab reaches can hold focus while drawn wholly outside the
    /// strip's visible part, which happens readily beside a tunnel pill, where the top row may leave the strip only a
    /// few pixels (`session_pane::header`'s fit).
    ///
    /// The scrolling part's items are the Agent tab, then one item per shell tab holding its button and its ×, which
    /// [`StripControl`] maps focus onto; New terminal sits after that part, always in view, unless the strip is too
    /// narrow to pin it ([`Self::pins_add`]), when it is the part's last item, after the divider. A tab and an unpinned
    /// New terminal are scrolled to as items, which shows an item wider than the strip from its start; a × is revealed
    /// by [`Self::reveal_close`]. Focus moving from a tab to its own × still counts as reaching a new control.
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
        // New terminal moves between the pinned end and the scrolling part when the strip's width crosses
        // `pins_add`'s threshold, so it counts as a new control to reveal when that happens under focus, as when a
        // tunnel pill narrows the strip while New terminal holds focus.
        let focused =
            focused.map(|control| (control, control == StripControl::Add && self.pins_add()));
        if focused != self.revealed.get() {
            match focused.map(|(control, _)| control) {
                Some(StripControl::Agent) => self.scroll.scroll_to_item(0),
                Some(StripControl::Tab(n)) => self.scroll.scroll_to_item(n + 1),
                Some(StripControl::Close(n)) => self.reveal_close(n + 1),
                // Pinned outside the scrolling part, so always in view; otherwise after the shell tabs and the divider.
                Some(StripControl::Add) if self.pins_add() => {}
                Some(StripControl::Add) => self.scroll.scroll_to_item(self.tabs.len() + 2),
                None => {}
            }
            self.revealed.set(focused);
        }
    }

    /// Whether New terminal is pinned at the strip's end, outside the scrolling tabs: unless the strip, as last laid
    /// out, is narrower than New terminal and a control's worth of tabs ([`PINNED_TABS_FLOOR`]). Pinned, adding a tab
    /// never moves New terminal, so a second press where it was lands on it again rather than on the new tab's × (the
    /// plan's 2026-11-27 decision). A strip that narrow appears only beside a tunnel pill in the narrowest pane the top
    /// row supports, where a pinned New terminal would leave the tabs less than a control's width; there it goes back to
    /// the end of the scrolling tabs, so every control can still be scrolled into view when focused. Before the strip's
    /// first layout its width is unknown, and it is pinned. The strip corrects a choice its new width overturns in the
    /// next frame ([`TerminalSet::strip`]).
    fn pins_add(&self) -> bool {
        pins_add_at(self.strip_bounds.bounds().size.width)
    }

    /// Scroll the strip so the × of the shell tab in its item `index` shows, with room for its focus ring, moving it
    /// only as far as that takes. A × already in view with its ring stays where it is, so Tab from a tab to its own ×
    /// does not shift the strip. Otherwise the strip moves the least that brings the × and its ring in from whichever
    /// side it was cut off; a strip too narrow for both lines the × itself up with its left edge, since beside a tunnel
    /// pill the top row may leave the strip only a few pixels (`session_pane::header`'s fit). Scrolling to the item
    /// would not do: an item wider than the strip shows from its start, and the × sits [`CLOSE_LEFT`] pixels into it.
    /// The item's place comes from the frame the strip was last drawn in (the scroll handle keeps it, placed as if
    /// unscrolled), so an item not drawn yet is scrolled to as usual.
    fn reveal_close(&self, index: usize) {
        let Some(item) = self.scroll.bounds_for_item(index) else {
            self.scroll.scroll_to_item(index);
            return;
        };
        let viewport = self.scroll.bounds();
        let mut offset = self.scroll.offset();
        let close = item.left() + px(CLOSE_LEFT);
        let (from, to) = (
            close - px(RING_ROOM),
            close + px(TERMINAL_TAB_CLOSE_SIDE + RING_ROOM),
        );
        if to - from > viewport.size.width {
            offset.x = viewport.left() - close;
        } else if from + offset.x < viewport.left() {
            offset.x = viewport.left() - from;
        } else if to + offset.x > viewport.right() {
            offset.x = viewport.right() - to;
        } else {
            return;
        }
        self.scroll.set_offset(offset);
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
    /// side. Its terminal goes when the pushed row stops listing it. A successful close remembers the fallback
    /// immediately so that terminal receives carried notices and another close can use the surviving strip.
    /// A refusal shows under the strip until the tab's next close.
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
                            if set.tabs.iter().any(|tab| tab.id == closed) {
                                set.confirmed_closed.insert(closed.clone());
                            }
                            if set.chosen.as_deref() == Some(closed.as_str()) {
                                let fallback = set.previous_surviving_tab(&closed, |_| true);
                                set.set_choice(fallback, cx);
                                set.take_carried(cx);
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
                    .child(not_attached_text(&self.tabs, self.chosen.as_deref())),
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
