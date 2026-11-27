//! The open session's top row and its lifecycle actions: Stop, Restart ▾, Replace ▾, Clone and Delete, at the right of
//! the row, with their questions anchored under their buttons and the split buttons' menus under their carets.
//!
//! SPEC.md, "Terminal experience" (the session view's one top row, and the 650-pixel rule), "Lifecycle operations" and
//! "Keyboard focus". Ported from the old app's `.session-topbar` and `.titlebar-actions` (`session_view.rs` and
//! `app.css` at the reference commit). Here's the TLDR of the rules:
//!
//! - **The row.** A plain flex row, at least 30 pixels high, on `--bg-1`: a flexible left region that may shrink to
//!   nothing ([`TABS_SLOT`], where the terminal area's tab strip and open-tunnels pill go; the pill is the terminal's
//!   to draw, [`AttachedTerminal::tunnel_pill`](crate::terminal::attached::AttachedTerminal::tunnel_pill)), then the
//!   actions, which never shrink. The row never wraps; a pane too narrow for it clips it. SPEC.md's floor is a
//!   650-pixel pane, where every action stays fully visible and keyboard reachable.
//! - **Ordinary and condensed actions.** The ordinary look uses the same 26-pixel buttons as the marked sessions
//!   bar. When the actions, tab strip and any tunnel pill need more room than the pane has, the actions take the
//!   condensed look. Lifecycle buttons use 11-pixel labels on a 15-pixel line with no side padding. An environment
//!   session's Suspend or Resume keeps its former condensed padding and label size. The choice uses widths measured
//!   in the current frame ([`fit`]). The tab strip scrolls as needed and the tunnel pill keeps its floor.
//! - **The tab strip's floor beside a pill.** While a pill shows, the strip is at least 112 pixels wide, the old
//!   `min-width: 112px` ([`crate::terminal::set::STRIP_FLOOR_BESIDE_PILL`]), but only as far as the row has room: when
//!   the width the pill needs to stay whole and the chosen actions leave less, the strip gets what is left, down to
//!   nothing, and scrolls. The fit decides the floor as the row is laid out, and its module docs give the rule and why
//!   ([`fit`]); without a pill there is none.
//! - **The actions, in the old order.** Stop, Restart, Replace, Clone, Delete, each a toolbar button with the old icon,
//!   label and hover help; Delete in the danger tier, the rest neutral. The terminal area adds the text size group at
//!   the start of the actions.
//! - **Restart and Replace are split buttons** ([`Split`]): the main part performs the plain action, question included,
//!   and the caret beside it opens a menu of that action and its "with…" variant, which opens the launcher
//!   (`launcher::entry::open_restart_with`, `open_replace_with`). The menu ([`CaretMenu`]) is a popover below its
//!   caret that names the caret as where closing returns focus; Escape or Tab closes it there, as SPEC.md asks of these
//!   menus alone. A choice that cannot run stays in the menu, greyed but focusable, with its reason as its description
//!   and help, and a caret whose choices all are unavailable still opens, muted, since its menu is where the reasons
//!   are. Clone opens the launcher too (`launcher::entry::open_clone`). Clone and the "with…" choices are unavailable
//!   while the launcher cannot open (`session_list::menu::launcher_wait`: a launch pending or a host change in
//!   flight), as the launcher's entry points refuse then, with that reason as their help and description; Clone stays
//!   focusable then, as Restart and Replace do while they wait. The launcher returns focus to what held it when it
//!   opened: Clone, or the caret, since a chosen menu item closes the menu first. A caret menu's anchor is named for
//!   its session, so the menu closes when another session opens (a Delete from elsewhere, say).
//! - **Into the launcher from Hosts.** Clone, Restart with… and Replace with… are New handoffs: once the launcher has
//!   opened, the sidebar shows Sessions in place of the Hosts panel (`session_list::show_sessions`), so the session
//!   the launch makes appears where the user can see it, as the old app did (SPEC.md: "All or a host avatar returns to
//!   Sessions, as does an accepted quick-switcher pick or New handoff"; the GPUI migration's orchestrator counted these
//!   three as New handoffs on 2026-11-16; the interrupted card's Replace with…, which took the same path in the old
//!   app, is one too, applied by the sidebar-and-session-view area on 2026-11-17, `super::card`). A refused one leaves
//!   the panel as it was.
//! - **The row's code does the work.** Each action reads the open session's row as the session list reads it and calls
//!   the same `begin` and `confirm` the row's menu calls (`session_list::stop`, `replace`, `delete`), which hold each
//!   action's ask-or-send rule: the same questions, the same frozen preconditions, the same commands, the same pending
//!   lines and refusal words. Restart, which the old row menu never had, is `session_list::restart`. What is the
//!   header's own: where each question is anchored, and two decisions that differ from the row's because SPEC.md says
//!   so: Delete asks even for a session with nothing alive (`delete::begin`'s flag), and Restart's button is greyed but
//!   focusable, its hover help and accessible name saying why, when the session cannot resume
//!   ([`crate::components::Button::busy`]).
//! - **The Delete shortcut** is this Delete (SPEC.md, "Desktop window chrome"): the main view hands it to
//!   [`SessionPane::header_action`].
//! - **Exclusion.** While the session has any request in flight, or the YOLO question its Replace met or the
//!   interrupted card's Replace question is open, every action is unavailable, carets and Clone included, as the row's
//!   menu items are (`session_list::menu::refused`), and Restart and Replace also wait for a host change
//!   (`session_list::menu::launch_wait`). A pending Stop shows its spinner and "Stopping…" on the Stop button, and a
//!   pending Delete shows its progress in place of the actions until it settles (they come back with a refusal).
//!   While a question an action asked is open, its button reads as pressed and a click on it does nothing; a press
//!   anywhere else closes the question unanswered.
//! - **The open session by key.** Each action reads the session's row from the store by key, and the helm keeps the
//!   open session there even while a filter or the list's cap leaves it out of the list (`session_list::filters`,
//!   "The query names the open session"), so the actions work on a session the list does not show exactly as on a
//!   listed one. With no row at all (the moment before the helm answers the query that names a newly opened session,
//!   or a selected key that names no session), the row draws no actions, since there is nothing they could ask from.
//! - **Focus.** Each question returns focus home (the terminal) on Escape and to where it was on any other close, which
//!   is SPEC.md's "Escape cancels the header's Stop, Restart, Replace or Delete question and returns focus to the
//!   terminal" ([`crate::prompt::Question::returns_home_on_escape`]).
//! - **A retired containerized session** supports Delete only (`session_list::retired`), so its row draws Delete
//!   alone, with the ordinary question, and none of Stop, Restart, Replace, Clone or the terminal's text size controls
//!   (it has no terminal: `super::retired`); a press or shortcut that reaches another action for it does nothing.
//! - **An environment session** (`session_list::environment`) gets its own control immediately left of Stop: Suspend,
//!   or Resume as the filled primary control while its environment is suspended or stopped, asking under itself when
//!   the host's activity says so, and while that is under way turning a spinner in its glyph's slot, named
//!   "Suspending…" or "Resuming…", at the same width, so the row still fits the 650-pixel floor. It is no [`Action`]
//!   ([`Control`] says why). While the environment is frozen, Stop, Restart and Replace are unavailable, and Clone and
//!   Delete stay (both wait while a Suspend or Resume runs). While it offers Resume, the Restart and Replace carets'
//!   "with…" choices stay available, because the launcher resumes the host before it sends the request. Stop's help says
//!   what plain Stop leaves running there, and its question offers Stop and suspend (`session_list::stop`). While it is
//!   being deleted every action is unavailable, Clone and Delete included, through `session_list::menu::refused` like
//!   any other exclusion, and Stop's, Clone's and Delete's help say why, as the old header's tooltips did. Its Delete,
//!   the shortcut's included, is the environment's own (`session_list::environment_delete`): always asked, whatever
//!   Skip session confirmations says, with Delete's help naming the lane a click asks in; and when the helm answers
//!   that it could not ask the environment's own Farhelm, the warned whole-environment question opens here again while
//!   this pane still shows the session (`SessionPane::ask_delete_again`). That second question is centered, since
//!   the Delete button it would hang under is not drawn while its Delete runs.
//! - **The YOLO question.** A Replace from the header that the source's host refuses for YOLO asks the launcher's
//!   question below the top row, among the header's lines, rather than at the source's row
//!   ([`crate::session_list::replace_yolo`]). It is drawn only while the source is the open session.
//!
//! The row is drawn by the pane on every frame, uncached, and reads everything it shows from the store, the operation
//! store and the shared clocks while it draws; the only state it keeps is where each button and caret was drawn, and
//! which question or caret menu it opened ([`Header`]). Whether its actions are condensed is not kept: each frame
//! measures it again.
//!
//! NOTE: Not ported from the old caret: `aria-haspopup`, which GPUI cannot report, and placing the menu at its caret's
//! right edge (the layer manager places a popover below its anchor). The text size group is the terminal area's
//! (`terminal/text_size.rs`); the row tells it which look it is drawn in, and it is as tall as the lifecycle buttons in
//! each (the maintainer's decision of 2026-11-16, where the old group was shorter).
//!
//! NOTE: The row's height is its minimum (30 pixels, or the title band's height), no longer the tallest thing in it:
//! its content is laid out by [`fit`] inside a box that asks for no height of its own. The ordinary 26-pixel buttons
//! fit inside it.

use std::collections::HashMap;
use std::rc::Rc;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled, WeakEntity, Window, div, px,
};

use super::SessionPane;
use crate::actions::DELETE_SESSION_SHORTCUT_NAME;
use crate::actions::{FocusNext, FocusPrevious};
use crate::assets::Icon;
use crate::components::{
    BUTTON_JOIN_GAP, Button, Condensed, HoverHelp, IconButton, MenuHeader, MenuItem, MenuStart,
    SplitPart, TOOLBAR_ACTION_GAP, Tier, home_end_keys, spinner_icon, toggle_arrow_keys,
};
use crate::desktop::title_band::{self, Row};
use crate::environment_ui::ProviderUi;
use crate::focus::{self, Mark};
use crate::host_text::Line;
use crate::launcher::entry;
use crate::layers::{self, Anchor, LayerId, Popover, Subject};
use crate::operations::Work;
use crate::prompt::{self, ActionFrame, Question, ShownAction};
use crate::session;
use crate::session_list::replace_yolo::{self, Surface};
use crate::session_list::{
    self, delete, environment, environment_delete, menu, outcome, replace, restart, retired, stop,
};
use crate::theme::{self, metrics};

mod fit;

/// The id of the top row's left region, where the terminal area's tab strip and open-tunnels pill go. The pill shows
/// while the session has tunnels open; the strip always, its Agent tab at least. The row's layout (a region that takes
/// the width the actions leave, and shrinks to nothing first) is already the one they need.
pub(crate) const TABS_SLOT: &str = "session-header-tabs";

/// `.session-topbar`'s `min-height`.
const ROW_MIN_HEIGHT: f32 = 30.;
/// `.session-topbar`'s horizontal padding.
const ROW_PAD_X: f32 = 10.;
/// `.session-topbar`'s gap between the left region and the actions, and inside the left region between the strip and
/// the pill.
const ROW_GAP: f32 = 10.;
/// That gap while a tunnel pill shows (`.session-topbar:has(> .session-tunnel-pill) { gap: 6px }`).
const PILL_ROW_GAP: f32 = 6.;
/// That gap while the actions are condensed (`.actions-compact`'s `gap: 2px`).
const CONDENSED_ACTIONS_GAP: f32 = 2.;

/// One of the header's actions, in the old order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Action {
    Stop,
    Restart,
    Replace,
    Clone,
    Delete,
}

impl Action {
    /// Every action, in the order the row draws them.
    pub(crate) const ALL: [Action; 5] = [
        Action::Stop,
        Action::Restart,
        Action::Replace,
        Action::Clone,
        Action::Delete,
    ];

    /// The button's id, by which tests and the bridge find it.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Action::Stop => "session-header-stop",
            Action::Restart => "session-header-restart",
            Action::Replace => "session-header-replace",
            Action::Clone => "session-header-clone",
            Action::Delete => "session-header-delete",
        }
    }

    /// The visible label.
    fn label(self) -> &'static str {
        match self {
            Action::Stop => "Stop",
            Action::Restart => "Restart",
            Action::Replace => "Replace",
            Action::Clone => "Clone",
            Action::Delete => "Delete",
        }
    }

    /// The old icon.
    fn icon(self) -> Icon {
        match self {
            Action::Stop => Icon::Stop,
            Action::Restart => Icon::Restart,
            Action::Replace => Icon::Replace,
            Action::Clone => Icon::Clone,
            Action::Delete => Icon::Trash,
        }
    }

    /// Whether the action on `row` would start a session now, and so waits for a host change ([`menu::launch_wait`]).
    /// A Restart that cannot resume does not: it keeps its own reason, which outlasts the change. Exhaustive, so a new
    /// action decides.
    fn would_launch(self, row: &SessionRow) -> bool {
        match self {
            Action::Restart => restart::offered(row),
            Action::Replace => true,
            // Clone opens the launcher rather than starting a session, and waits while the launcher cannot open,
            // which covers a host change already (`menu::launcher_wait`).
            Action::Stop | Action::Clone | Action::Delete => false,
        }
    }

    /// Delete is the one destructive action, so it alone is in the danger tier; the rest are neutral.
    fn tier(self) -> Tier {
        match self {
            Action::Delete => Tier::Danger,
            Action::Stop | Action::Restart | Action::Replace | Action::Clone => Tier::Neutral,
        }
    }
}

/// The hover help of `action` on `row`, as listed and drawn now, with Skip session confirmations as `skipped`. Delete
/// names the platform's Delete shortcut, and drops "after you confirm" only when the question is skipped: a session
/// with open terminal tabs asks even then ([`delete::asks_despite_skipping`]). Restart's says whether the conversation
/// can be resumed and why not ([`restart::hover_help`]).
fn hover_help(action: Action, row: &SessionRow, skipped: bool) -> HoverHelp {
    let words = match action {
        Action::Stop => format!(
            "Stop the {} and close its terminal tabs",
            crate::session_list::stop::program_word(row)
        ),
        Action::Restart => {
            return HoverHelp::sentence(restart::hover_help(row, "header-restart-help"));
        }
        Action::Replace => replace::menu_help(row).to_owned(),
        Action::Clone => menu::CLONE_HELP.to_owned(),
        Action::Delete if skipped && !delete::asks_despite_skipping(row) => {
            format!("Delete: remove this session and its state ({DELETE_SESSION_SHORTCUT_NAME})")
        }
        Action::Delete => format!(
            "Delete: remove this session and its state, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})"
        ),
    };
    HoverHelp::tooltip(words)
}

/// The id of an environment session's own control, Suspend or Resume, immediately left of Stop ([`environment_button`]).
pub(crate) const ENVIRONMENT_ID: &str = "session-header-environment";

/// A control of the top row that can ask a question and has a button for it to hang under: one of the [`Action`]s, or
/// an environment session's Suspend ([`environment_button`]). The environment control is no [`Action`] because it is
/// drawn only for an environment session, with its label, glyph, tier and help from the environment's phase and
/// provider, so none of the actions' fixed answers apply to it; it shares only their anchoring and pressed look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Control {
    Action(Action),
    Environment,
}

// ---- The split buttons' carets and their menus ----------------------------------------------------------------------

/// One of the two split buttons: Restart and Replace, whose caret opens a menu of the plain action and its "with…"
/// variant (the old `action_menu::ActionKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Split {
    Restart,
    Replace,
}

impl Split {
    /// The split button whose main part is `action`, if it is one.
    fn of(action: Action) -> Option<Split> {
        match action {
            Action::Restart => Some(Split::Restart),
            Action::Replace => Some(Split::Replace),
            Action::Stop | Action::Clone | Action::Delete => None,
        }
    }

    /// The plain action, the main part's.
    pub(crate) fn action(self) -> Action {
        match self {
            Split::Restart => Action::Restart,
            Split::Replace => Action::Replace,
        }
    }

    /// The caret's id, by which tests and the bridge find it; its menu is `<id>-menu`, and the menu's items are
    /// `<id>-plain` and `<id>-with`.
    pub(crate) fn caret_id(self) -> &'static str {
        match self {
            Split::Restart => "session-header-restart-options",
            Split::Replace => "session-header-replace-options",
        }
    }

    /// The action's name, the menu's title and the plain item's label.
    fn label(self) -> &'static str {
        self.action().label()
    }

    /// The variant's label: "Restart with…" or "Replace with…".
    fn with_label(self) -> &'static str {
        match self {
            Split::Restart => "Restart with…",
            Split::Replace => "Replace with…",
        }
    }

    /// Open the variant's launcher on `session` (the launcher's entry points), returning whether it opened. An opened
    /// launcher is a New handoff, so Sessions then shows in place of the Hosts panel (the module docs' "Into the
    /// launcher from Hosts"). Call it outside any update of the main view or the session list, which
    /// [`session_list::show_sessions`] reads and updates.
    fn open_with(self, session: &str, window: &mut Window, cx: &mut App) -> bool {
        let opened = match self {
            Split::Restart => entry::open_restart_with(session, window, cx),
            Split::Replace => entry::open_replace_with(session, window, cx),
        };
        if opened {
            session_list::show_sessions(window, cx);
        }
        opened
    }
}

/// What a split button's menu offers for a session now: whether each choice can run, and the description that says what
/// it does or why it cannot. The caret reads it for its look and help, and the open menu for its items, so the two
/// agree.
struct Choices {
    plain_unavailable: bool,
    with_unavailable: bool,
    plain_description: Line,
    with_description: Line,
}

impl Choices {
    /// `split`'s choices for the session `key`, listed as `row`, with `launcher_wait` saying why the launcher cannot
    /// open now, if it cannot ([`menu::launcher_wait`]). Restart's plain choice is unavailable when the session cannot
    /// resume, and its variant when the launcher says why it is not offered
    /// ([`entry::restart_with_unavailable`]) or cannot open; Replace's plain choice is offered, and its
    /// variant only while the launcher can open. Both plain choices also wait for a host change
    /// ([`menu::launch_wait`]), as the main parts do. An unavailable choice's description is its reason; the others
    /// keep the menu's own wording. A host error stays a host part for the description and its tooltip.
    fn of(
        split: Split,
        key: &str,
        row: &SessionRow,
        launcher_wait: Option<&'static str>,
        cx: &App,
    ) -> Choices {
        // A plain action needs the supervisor now. The launcher variants can prepare a resumable environment first.
        let environment = environment::of_key(key, cx);
        let wait = environment
            .filter(|host| host.frozen())
            .map(|_| "Wait for this host to connect")
            .or_else(|| menu::launch_wait(cx));
        let launcher_wait = environment
            .filter(|host| !host.permits_launch_preparation())
            .map(|_| "Wait for this host to connect")
            .or(launcher_wait);
        match split {
            Split::Restart => {
                let reason = entry::restart_with_unavailable(key, cx);
                let offered = restart::offered(row);
                let plain = match wait.filter(|_| offered) {
                    Some(wait) => wait.into(),
                    None => restart::menu_description(row).into(),
                };
                Choices {
                    plain_unavailable: !offered || wait.is_some(),
                    with_unavailable: reason.is_some() || launcher_wait.is_some(),
                    plain_description: plain,
                    with_description: reason.map_or_else(
                        || {
                            launcher_wait
                                .unwrap_or("Resume this session's conversation with changed launch settings")
                                .into()
                        },
                        Line::from,
                    ),
                }
            }
            Split::Replace => Choices {
                plain_unavailable: wait.is_some(),
                with_unavailable: launcher_wait.is_some(),
                plain_description: wait
                    .unwrap_or(if row.info.launch.is_shell() {
                        "Start a fresh shell with the same settings"
                    } else {
                        "Swap this session for a fresh conversation with the same settings"
                    })
                    .into(),
                with_description: launcher_wait
                    .unwrap_or("Edit the settings, then swap this session for a new one")
                    .into(),
            },
        }
    }

    /// Whether neither choice can run, so the caret is muted and its help says to open it for the reasons.
    fn none_available(&self) -> bool {
        self.plain_unavailable && self.with_unavailable
    }
}

// ---- What the header keeps -------------------------------------------------------------------------------------------

/// What the header keeps between frames: where each button was last drawn, so its question can hang under it, the
/// question it last opened, during which that button reads as pressed and is inert, and the same for the carets and
/// their menus. Kept by the pane, and checked with the layer manager whenever it is read, so a question or menu closed
/// some other way needs no bookkeeping here.
#[derive(Default)]
pub(super) struct Header {
    /// The header and card share the pane's snapshot. The Delete shortcut has no button handler to carry it.
    action: ActionFrame<menu::Actionable, ActionOffer>,
    anchors: HashMap<Control, Anchor>,
    asked: Option<(Control, LayerId)>,
    /// Each caret as last drawn: the anchor its menu opens below, and its focus handle, which the menu names as where
    /// closing returns focus. Both are element state, made while the caret draws, so its handlers read them from here.
    carets: HashMap<Split, (Anchor, FocusHandle)>,
    /// The caret menu open now, if any, while its caret reads as open.
    menu: Option<(Split, LayerId)>,
}

impl Header {
    /// The snapshot this frame gave its session controls, also shared with the interrupted card and Delete shortcut.
    pub(super) fn action(&self) -> Option<ShownAction<menu::Actionable>> {
        self.action.current()
    }

    /// An empty pane has no action for the Delete shortcut to activate.
    pub(super) fn clear_action(&mut self) {
        self.action.clear();
    }

    /// The split button whose caret menu is open now, if any.
    pub(super) fn open_menu(&mut self, window: &Window, cx: &App) -> Option<Split> {
        let (split, layer) = self.menu?;
        if layers::is_open(layer, window, cx) {
            Some(split)
        } else {
            self.menu = None;
            None
        }
    }

    /// The control whose question is open now, if any.
    pub(super) fn open_question(&mut self, window: &Window, cx: &App) -> Option<Control> {
        let (control, layer) = self.asked?;
        if layers::is_open(layer, window, cx) {
            Some(control)
        } else {
            self.asked = None;
            None
        }
    }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// What the top row draws for the open session, read from the store and the operation store now.
struct Shown<'a> {
    /// The session's row as the store holds it, listed or carried outside the list.
    row: &'a SessionRow,
    action: ShownAction<menu::Actionable>,
    skipped: bool,
    /// The spinner's step, from the pane's spinner.
    step: u8,
    offer: ActionOffer,
}

/// Availability shown in the header and interrupted card. These can change independently of the session's row, such
/// as when the launcher finishes work on a different host.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ActionOffer {
    busy: bool,
    asking: Option<Control>,
    /// [`menu::launch_wait`].
    launch_wait: Option<&'static str>,
    /// Why the launcher cannot open now, if it cannot ([`menu::launcher_wait`]), for Clone and the "with…" choices.
    launcher_wait: Option<&'static str>,
    stopping: bool,
    /// An environment session's own control, `None` for any other session.
    environment: Option<EnvironmentShown>,
}

/// What the header draws for an environment session's own control and what its environment withholds, read from the
/// registry and the operation store now ([`environment::Environment`]), so nothing drawn borrows the store.
#[derive(Clone, Copy, PartialEq, Eq)]
struct EnvironmentShown {
    ui: ProviderUi,
    /// Suspended, stopped, changing or being deleted: Stop, Restart and Replace are unavailable.
    frozen: bool,
    /// The caret can offer an edited launch when the destination can wake.
    permits_launch_preparation: bool,
    /// The control reads Resume, in the primary tier, rather than Suspend.
    resume: bool,
    /// Stopped, so Resume's help says it starts the environment again rather than continuing its processes.
    stopped: bool,
    pending: Option<environment::Pending>,
    unavailable: bool,
    /// The reason this window's launch holds the chosen environment's lifecycle controls.
    launch_refusal: Option<&'static str>,
    /// Why every action is unavailable while the environment is being deleted, `None` otherwise
    /// (`environment::Environment::deleting`): Stop's, Clone's and Delete's help.
    deleting: Option<&'static str>,
    /// What Delete's question will offer, in the lane a click would ask in now (`environment_delete::help`): how
    /// Delete's help begins while the environment is not being deleted.
    delete_help: &'static str,
}

impl EnvironmentShown {
    /// `row`'s, filed under `key`, from `collections`; `None` for a session that is not an environment session.
    fn of(key: &str, row: &SessionRow, collections: &Collections, cx: &App) -> Option<Self> {
        let shown = environment::Environment::of(row, collections)?;
        Some(EnvironmentShown {
            ui: shown.ui,
            frozen: shown.frozen(),
            permits_launch_preparation: shown.permits_launch_preparation(),
            resume: shown.resumable(),
            stopped: shown.stopped(),
            pending: environment::pending(key, shown, cx),
            unavailable: environment::unavailable(key, shown, cx),
            launch_refusal: crate::launcher::launch::host_lifecycle_refusal(shown.host.id, cx),
            deleting: shown.deleting().map(|deleting| deleting.help),
            delete_help: environment_delete::help(shown, collections),
        })
    }
}

impl SessionPane {
    /// The top row for the open session filed under `key`, its spinner at `step`, recording where each button is drawn.
    /// With actions to draw, it builds them in both looks and leaves the choice to [`fit`], which makes it as the row is
    /// laid out; without (no store, no row, or a Delete in flight) the left region fills the row alone, beside the
    /// Delete's progress if there is one.
    pub(super) fn header_row(
        &mut self,
        key: &str,
        step: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = theme::palette(cx);
        // In the macOS window the row is the session's half of the window's title band: as tall as the sidebar's app
        // bar, and a press on its empty background drags the window (`desktop::title_band`).
        let min_height = if title_band::ON {
            title_band::HEIGHT
        } else {
            px(ROW_MIN_HEIGHT)
        };
        // The terminal area's part of the left region: the strip of the session's terminals, then the open-tunnels pill
        // while the session has tunnels open (the agent terminal's: SPEC.md's tunnels belong to the session). Buttons,
        // so they keep their presses inside the title band. Only the fit gives them their floors beside a pill (`fit`'s
        // module docs); a left region built without a fit has none.
        let set = self
            .mounted
            .current(cx)
            .as_ref()
            .and_then(|mounted| mounted.set.clone());
        let strip = set
            .as_ref()
            .map(|set| crate::terminal::set::TerminalSet::strip(set, window, cx));
        let agent = set.map(|set| set.read(cx).agent().clone());
        let pill_of = |window: &mut Window, cx: &mut Context<Self>| {
            agent.as_ref().and_then(|terminal| {
                terminal.update(cx, |terminal, cx| terminal.tunnel_pill(window, cx))
            })
        };
        let pill = pill_of(window, cx).map(|pill| pill.element);
        let shows_pill = pill.is_some();
        // `.session-topbar:has(> .session-tunnel-pill) { gap: 6px }`: a pill tightens the row's gaps.
        let gap = px(if shows_pill { PILL_ROW_GAP } else { ROW_GAP });
        let row = div()
            .id("session-header")
            .debug_selector(|| "session-header".into())
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(gap)
            .min_h(min_height)
            .px(px(ROW_PAD_X))
            .overflow_hidden()
            .bg(theme::color(palette.bg_1))
            .text_size(metrics::TEXT_MD);
        let left = fit::Left { strip, pill, gap };
        let row = title_band::row(row, Row::SessionTop);
        let Some(store) = session::current_store(cx) else {
            self.header.clear_action();
            return row.child(left.build(None)).into_any_element();
        };
        let collections = store.read(cx).collections();
        let row_now = collections.get::<SessionRow>(key).cloned();
        let skipped = menu::skips_questions(collections);
        let environment = row_now
            .as_ref()
            .filter(|row| !retired::is(row))
            .and_then(|row| EnvironmentShown::of(key, row, collections, cx));
        if let Some(label) = delete::in_flight(key, cx) {
            self.header.clear_action();
            self.header.anchors.clear();
            self.header.carets.clear();
            return row
                .child(left.build(None))
                // Text in the title band keeps its presses: it must not move the window (`desktop::title_band`).
                .child(title_band::claims_presses(
                    div().child(delete_progress(key, label, step, cx)),
                ))
                .into_any_element();
        }
        // No row, no actions (the module docs' "The open session by key").
        let Some(row_now) = row_now else {
            self.header.clear_action();
            self.header.anchors.clear();
            self.header.carets.clear();
            return row.child(left.build(None)).into_any_element();
        };
        let Some(target) = menu::Actionable::drawn(key, row_now.clone(), cx) else {
            self.header.clear_action();
            return row.child(left.build(None)).into_any_element();
        };
        let offer = ActionOffer {
            // `refused` counts the interrupted card's open Replace question too (`card::asks`), as any open
            // lifecycle question.
            busy: menu::refused(key, cx),
            asking: self.header.open_question(window, cx),
            launch_wait: menu::launch_wait(cx),
            launcher_wait: menu::launcher_wait(cx),
            stopping: stop::in_flight(key, cx),
            environment,
        };
        let shown = Shown {
            row: &row_now,
            action: self.header.action.capture(target, offer, window),
            skipped,
            step,
            offer,
        };
        // A retired session's row draws Delete alone (the module docs).
        let retired = retired::is(&row_now);
        let actions: &[Action] = if retired {
            &[Action::Delete]
        } else {
            &Action::ALL
        };
        // Questions and caret menus belong to the session that opened them. A new session's controls must not
        // provide an anchor for a layer that still targets the previous session.
        let mut anchors = HashMap::new();
        let mut carets = HashMap::new();
        let mut slots = Vec::new();
        for &action in actions {
            // An environment session's Suspend or Resume, immediately left of Stop (SPEC.md, "Sandbox sessions").
            if action == Action::Stop && shown.offer.environment.is_some() {
                let anchor = Anchor::for_element(
                    SharedString::from(format!("{ENVIRONMENT_ID}-anchor-{key}")),
                    window,
                    cx,
                );
                anchors.insert(Control::Environment, anchor);
            }
            let anchor = Anchor::for_element(
                SharedString::from(format!("{}-anchor-{key}", action.id())),
                window,
                cx,
            );
            anchors.insert(Control::Action(action), anchor);
            let caret = Split::of(action).map(|split| {
                // The session's key is in the anchor's id, so a caret menu closes with its session: the next session's
                // caret is another anchor, and an anchor that is no longer drawn closes its popover.
                let anchor = Anchor::for_element(
                    SharedString::from(format!("{}-anchor-{key}", split.caret_id())),
                    window,
                    cx,
                );
                let focus = focus::control_handle(
                    SharedString::from(format!("{}-focus-handle", split.caret_id())),
                    window,
                    cx,
                );
                carets.insert(split, (anchor, focus));
                (
                    split,
                    Choices::of(split, key, shown.row, shown.offer.launcher_wait, cx),
                )
            });
            slots.push((action, caret));
        }
        let drawn = Drawn {
            shown: &shown,
            slots: &slots,
            anchors: &anchors,
            carets: &carets,
            open: offer.asking,
            menu_open: self.header.open_menu(window, cx),
            pane: cx.weak_entity(),
            retired,
        };
        // The ordinary toolbar can use all the room even without a pill. Keep a condensed layout available for the
        // fit to give the tab strip room in either case.
        let ordinary = drawn.actions(false, cx);
        let condensed = drawn.actions(true, cx);
        self.header.anchors = anchors;
        self.header.carets = carets;
        row.child(fit::Fit {
            left,
            pill: shows_pill.then(|| pill_of(window, cx)).flatten(),
            ordinary,
            condensed,
        })
        .into_any_element()
    }

    /// The lines between the top row and the terminal for `key`: a Restart's or Replace's wait once it has lasted two
    /// seconds, then the session's refusal or notice, as the old header's progress and error lines were, then the YOLO
    /// question a Replace from this header met ([`replace_yolo`]). The wait sits below the row, never in it, so a long
    /// host name cannot push an action out of the 650-pixel floor; the question sits there for the same reason. None
    /// while the session shows its interrupted card (`card`), which draws all three itself, so nothing shows twice.
    pub(super) fn header_lines(
        &self,
        key: &str,
        step: u8,
        card: bool,
        cx: &App,
    ) -> Vec<AnyElement> {
        let mut lines = Vec::new();
        if card {
            return lines;
        }
        let ink = theme::color(theme::palette(cx).fg_0);
        let row = session::current_store(cx)
            .and_then(|store| store.read(cx).collections().get::<SessionRow>(key).cloned());
        if let Some(row) = &row {
            for work in [Work::Restart, Work::Replace] {
                if let Some(line) =
                    outcome::wait_line("session-header", work, true, key, row, step, ink, cx)
                {
                    lines.push(line.py(px(4.)).px(px(12.)).into_any_element());
                }
            }
        }
        // A refused Resume from the unavailable card shows on the card, where it was pressed.
        if let Some(line) = outcome::pane_line(key, cx)
            && !super::unavailable::shows_resume_refusal(key, cx)
        {
            lines.push(line.into_any_element());
        }
        lines.extend(replace_yolo::header_question(key, cx));
        lines
    }

    /// Whether the header shows anything that turns the spinner for `key`: a Stop in flight, or a Restart or Replace
    /// that has waited long enough to say so.
    pub(super) fn header_spins(key: &str, cx: &App) -> bool {
        stop::in_flight(key, cx)
            || outcome::waited_seconds(key, Work::Restart, cx).is_some()
            || outcome::waited_seconds(key, Work::Replace, cx).is_some()
            || environment::pending_of(key, cx).is_some()
    }

    /// The Delete shortcut uses the header's current render, subject to the same presentation check as its button.
    pub(crate) fn header_action(
        &mut self,
        action: Action,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(target) = self.header.action().as_ref().and_then(ShownAction::shown) {
            self.act_from_header(action, target, mark, window, cx);
        }
    }

    /// Run the activated header action or the plain choice from its caret menu. Hang a question under the main button
    /// only while the pane still owns that target. Live state may block a pending, stale or retired action.
    pub(super) fn act_from_header(
        &mut self,
        action: Action,
        target: &menu::Actionable,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &target.key;
        if self.open_key(cx).as_ref() != Some(key)
            || self.header.open_question(window, cx).is_some()
        {
            return;
        }
        let allowed = match action {
            Action::Stop => target.allows_active(true, cx),
            Action::Restart | Action::Replace => target.allows_active(false, cx),
            Action::Clone => target.allows(cx, |row| !retired::is(row)),
            Action::Delete => target.allows_delete(cx),
        };
        if !allowed {
            return;
        }
        let (row, skipped) = (&target.row, target.skips_questions);
        let asking = Asking {
            control: Control::Action(action),
            key,
            mark,
        };
        // Each action's own `begin` decides whether it asks or goes ahead, the same decision the row's menu makes, so
        // the two cannot drift apart; the header adds only where its questions hang.
        match action {
            Action::Stop => {
                if let Some(ask) = stop::begin(target, cx) {
                    self.ask(asking, ask, stop::confirm, window, cx);
                }
            }
            Action::Restart => {
                if let Some(ask) = restart::begin(key, row, skipped, cx) {
                    self.ask(asking, ask, restart::confirm, window, cx);
                }
            }
            Action::Replace => {
                if let Some(ask) = replace::begin(
                    key,
                    row,
                    skipped,
                    Surface::Header,
                    window.window_handle(),
                    cx,
                ) {
                    self.ask(asking, ask, replace::confirm, window, cx);
                }
            }
            Action::Clone => {
                // The launcher refuses while New is unavailable, as the button is drawn then; a press that beats
                // that frame opens nothing. An opened one is a New handoff, which shows Sessions in place of the
                // Hosts panel (the module docs' "Into the launcher from Hosts").
                if entry::open_clone(key, window, cx) {
                    session_list::show_sessions(window, cx);
                } else {
                    tracing::debug!("the launcher refused to open Clone from the header");
                }
            }
            Action::Delete => {
                // SPEC.md: the header's Delete "asks first in the same anchored way, even for a session that has
                // ended", unlike the row's, which deletes a session with nothing alive unasked.
                if let Some(ask) = delete::begin(target, true, cx) {
                    let again = self.ask_delete_again(window, cx);
                    self.ask(asking, ask, delete::confirming(Some(again)), window, cx);
                }
            }
        }
    }

    /// The user activated an environment session's own control at `mark`: Resume goes at once, and Suspend reads the
    /// host's activity first and may ask, under this control (`session_list::environment::begin_shown`, which the row's menu
    /// goes through too). Nothing happens while the control's question is open.
    fn environment_action(
        &mut self,
        target: &menu::Actionable,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open_key(cx).as_ref() != Some(&target.key)
            || self.header.open_question(window, cx).is_some()
        {
            return;
        }
        let (pane, mark) = (cx.weak_entity(), *mark);
        environment::begin_shown(
            target,
            false,
            window.window_handle(),
            cx,
            move |asked, ui, window, cx| {
                let key = asked.key.clone();
                let asking = Asking {
                    control: Control::Environment,
                    key: &key,
                    mark: &mark,
                };
                let _ = pane.update(cx, |pane, cx| {
                    pane.ask(
                        asking,
                        || environment::question(&asked, ui),
                        environment::confirm,
                        window,
                        cx,
                    )
                });
            },
        );
    }

    /// How an environment Delete asked from this header (its Delete button, a caret's plain choice or the Delete
    /// shortcut) asks the whole-lane question from this header again once the helm answers "unasked"
    /// (`session_list::environment_delete`'s "Settling"): only while this pane still shows that session, no other
    /// question of the header is open and the session's commands are not refused. Otherwise nothing opens and the
    /// helm's words stay on the row, as the old header handed the question back only to a view still showing it. The
    /// question hangs under the Delete button when it was drawn in the last frame, and is centered otherwise, which is
    /// the usual case: the button is not drawn while the first Delete runs.
    fn ask_delete_again(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> environment_delete::AskAgain {
        let (pane, handle) = (cx.weak_entity(), window.window_handle());
        Rc::new(move |frozen: delete::Frozen, mark: Mark, cx: &mut App| {
            let pane = pane.clone();
            handle
                .update(cx, move |_, window, cx| {
                    pane.update(cx, |pane, cx| {
                        let key = frozen.key.clone();
                        if pane.open_key(cx).as_deref() != Some(key.as_str())
                            || pane.header.open_question(window, cx).is_some()
                        {
                            return false;
                        }
                        let Some(target) = menu::actionable(&key, cx) else {
                            return false;
                        };
                        let asking = Asking {
                            control: Control::Action(Action::Delete),
                            key: &key,
                            mark: &mark,
                        };
                        pane.ask(
                            asking,
                            || delete::question(&target.row, frozen.clone()),
                            delete::confirm,
                            window,
                            cx,
                        )
                    })
                    .unwrap_or(false)
                })
                .unwrap_or(false)
        })
    }

    /// Ask the question `question` builds for `asking`'s control about its session, hung under the action's button as
    /// last drawn (centered if it was not drawn), returning focus home on Escape. Nothing opens after the selection
    /// changes or the user moves on from the gesture. The question closes when its session goes away. Returns whether
    /// it opened.
    fn ask<T: Clone + 'static>(
        &mut self,
        asking: Asking,
        question: impl Fn() -> Question<T>,
        on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Asking { control, key, mark } = asking;
        // Suspend can wait for an activity read. Its answer must still belong to the session shown in the pane.
        if self.open_key(cx).as_deref() != Some(key) {
            return false;
        }
        let question = question().returns_home_on_escape();
        let subject = Subject::session(key, cx);
        let asked = match self.header.anchors.get(&control).cloned() {
            Some(anchor) => {
                prompt::ask_anchored(question, anchor, subject, mark, on_confirm, window, cx)
            }
            None => prompt::ask(question, subject, mark, on_confirm, window, cx),
        };
        let Some(layer) = asked else {
            return false;
        };
        self.header.asked = Some((control, layer));
        cx.notify();
        true
    }
}

/// Which control asks a question, about which session, for which of the user's gestures: what [`SessionPane::ask`]
/// needs besides the question itself.
#[derive(Clone, Copy)]
struct Asking<'a> {
    control: Control,
    key: &'a str,
    mark: &'a Mark,
}

/// What both looks of the actions are drawn from in one frame: what the row shows, each action in order with its split's
/// choices, each control's anchor and each caret's anchor and focus handle (made once, so the two looks share them),
/// the question and caret menu open now, and whether the session is retired (Delete alone, no text size).
struct Drawn<'a> {
    shown: &'a Shown<'a>,
    slots: &'a [(Action, Option<(Split, Choices)>)],
    anchors: &'a HashMap<Control, Anchor>,
    carets: &'a HashMap<Split, (Anchor, FocusHandle)>,
    open: Option<Control>,
    menu_open: Option<Split>,
    pane: WeakEntity<SessionPane>,
    retired: bool,
}

impl Drawn<'_> {
    /// The actions, in their ordinary look or `condensed` (`fit` decides which is drawn): the terminal area's text size
    /// group, then each control in its anchor, each split's two parts side by side.
    fn actions(&self, condensed: bool, cx: &App) -> AnyElement {
        let mut buttons: Vec<AnyElement> = Vec::new();
        for (action, caret) in self.slots {
            let action = *action;
            // An environment session's Suspend or Resume, immediately left of Stop (SPEC.md, "Sandbox sessions").
            if action == Action::Stop
                && let Some(environment) = self.shown.offer.environment
            {
                let asking = self.open == Some(Control::Environment);
                let button = environment_button(
                    environment,
                    self.shown.step,
                    asking,
                    condensed,
                    &self.pane,
                    &self.shown.action,
                );
                buttons.push(
                    div()
                        .relative()
                        .flex_none()
                        .child(button)
                        .children(self.anchors.get(&Control::Environment).map(Anchor::track))
                        .into_any_element(),
                );
            }
            let asking = self.open == Some(Control::Action(action));
            let button = action_button(action, self.shown, asking, condensed, &self.pane);
            // Each action is its own anchor, exactly as wide as its button, as the old `.restart-offer` wrappers were,
            // so its question hangs under it.
            let main = div().relative().flex_none().child(button).children(
                self.anchors
                    .get(&Control::Action(action))
                    .map(Anchor::track),
            );
            let Some(((split, choices), (anchor, focus))) = caret
                .as_ref()
                .and_then(|caret| Some((caret, self.carets.get(&caret.0)?)))
            else {
                buttons.push(main.into_any_element());
                continue;
            };
            // A resumable destination keeps the caret available for the variant that opens the launcher.
            let waiting_for_host = self
                .shown
                .offer
                .environment
                .is_some_and(|environment| !environment.permits_launch_preparation);
            let caret = self.caret_button(
                *split,
                choices,
                self.shown.offer.busy || waiting_for_host,
                self.menu_open == Some(*split),
                condensed,
                focus,
            );
            // The gap separates the two click targets. Stretch gives the caret the main part's height so the pair
            // still reads as one control. The caret is its menu's anchor.
            buttons.push(
                div()
                    .flex()
                    .flex_row()
                    .gap(BUTTON_JOIN_GAP)
                    .flex_none()
                    .child(main)
                    .child(div().relative().flex().child(caret).child(anchor.track()))
                    .into_any_element(),
            );
        }
        div()
            .id("session-header-actions")
            .debug_selector(|| "session-header-actions".into())
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(if condensed {
                px(CONDENSED_ACTIONS_GAP)
            } else {
                TOOLBAR_ACTION_GAP
            })
            // The terminal area's text size group starts the right-hand group (SPEC.md's top row), for a session that
            // has a terminal to size, drawn in the same look as the buttons so it stays exactly as tall as they are.
            .children((!self.retired).then(|| crate::terminal::text_size::controls(condensed, cx)))
            .children(buttons)
            .into_any_element()
    }
}

/// `action`'s button as `shown` says, pressed while its question is open (`asking`), activating through `pane`, in the
/// actions' ordinary look or `condensed`.
fn action_button(
    action: Action,
    shown: &Shown,
    asking: bool,
    condensed: bool,
    pane: &WeakEntity<SessionPane>,
) -> Button {
    let pane = pane.clone();
    let captured = shown.action.clone();
    let row = shown.row;
    let offer = shown.offer;
    // A launch waiting for a host change says why in place of what it does, and so does Clone while the launcher
    // cannot open.
    let wait = match action {
        Action::Clone => offer.launcher_wait,
        _ => offer.launch_wait.filter(|_| action.would_launch(row)),
    };
    let help = wait.map_or_else(
        || hover_help(action, row, shown.skipped),
        HoverHelp::tooltip,
    );
    // An environment session's Stop says what plain Stop leaves running there, and its freeze withholds Stop, Restart
    // and Replace (the old header's). While it is being deleted, Stop, Clone and Delete say why nothing is available.
    let help = match (action, offer.environment) {
        (
            Action::Stop | Action::Clone | Action::Delete,
            Some(EnvironmentShown {
                deleting: Some(reason),
                ..
            }),
        ) => HoverHelp::tooltip(reason),
        (Action::Stop, Some(environment)) => HoverHelp::tooltip(environment.ui.stop_help()),
        // An environment session's Delete always asks, so its help never takes the skipped wording, as the old header's
        // did not; it names the lane a click asks in.
        (Action::Delete, Some(environment)) => HoverHelp::tooltip(format!(
            "{}, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})",
            environment.delete_help
        )),
        _ => help,
    };
    let frozen = offer
        .environment
        .is_some_and(|environment| environment.frozen);
    let stopping = action == Action::Stop && offer.stopping;
    let label = if stopping {
        stop::PROGRESS_LABEL
    } else {
        action.label()
    };
    let mut button = Button::new(action.id(), label, help.below(), Role::Button)
        .icon(action.icon())
        .tier(action.tier())
        .toolbar()
        .condensed(condensed.then_some(Condensed::Words))
        .expanded(asking)
        .on_click(move |_, window, cx| {
            if let Some(target) = captured.shown() {
                let mark = Mark::take(cx);
                let _ = pane.update(cx, |pane, cx| {
                    pane.act_from_header(action, target, &mark, window, cx)
                });
            }
        });
    if stopping {
        button = button.spinner(shown.step);
    } else if action == Action::Stop && !frozen {
        // A frozen environment cannot start a Stop. Its disabled button can give that room to the tabs.
        button = button.reserve_label(stop::PROGRESS_LABEL);
    }
    if Split::of(action).is_some() {
        button = button.split(SplitPart::Main);
    }
    match action {
        // Busy rather than disabled while waiting, so the reason in the hover help stays reachable.
        Action::Restart => button
            .accessible_name(restart::offer_label(row.info.restart_offer))
            .busy(!restart::offered(row) || wait.is_some())
            .disabled(offer.busy || frozen),
        Action::Replace => button.busy(wait.is_some()).disabled(offer.busy || frozen),
        Action::Stop => button.disabled(offer.busy || row.stale || frozen),
        // Clone opens the launcher, so it waits while the launcher cannot open, as Restart and Replace wait.
        Action::Clone => button.busy(wait.is_some()).disabled(offer.busy),
        Action::Delete => button.disabled(offer.busy),
    }
}

/// An environment session's own control, immediately left of Stop, activating through `pane`: Suspend with the pause
/// glyph, or while the environment is suspended or stopped Resume with the play glyph as the filled primary control
/// (SPEC.md, "Sandbox sessions"), each with its provider's help. While a request or the change it asked for is under
/// way it is unavailable, the spinner turns in the glyph's slot, and its label names the direction ("Resume" while
/// resuming, else "Suspend"), so the control keeps its width and the actions stay whole at the 650-pixel floor; its
/// accessible name and hover help say what is under way ("Resuming…"). Unavailable too while the environment changes
/// and while the session's commands are refused. `condensed` takes only the old rule for every header button, 2 pixels
/// of padding all round: the rule that also shrinks the text named the lifecycle buttons alone.
fn environment_button(
    environment: EnvironmentShown,
    step: u8,
    asking: bool,
    condensed: bool,
    pane: &WeakEntity<SessionPane>,
    captured: &ShownAction<menu::Actionable>,
) -> Button {
    let resume = match environment.pending {
        Some(pending) => pending == environment::Pending::Resuming,
        None => environment.resume,
    };
    let (label, icon, tier) = if resume {
        ("Resume", Icon::Play, Tier::Primary)
    } else {
        ("Suspend", Icon::Pause, Tier::Neutral)
    };
    let help = environment
        .launch_refusal
        .unwrap_or_else(|| match environment.pending {
            Some(pending) => pending.label(),
            None if environment.resume => environment.ui.resume_help(environment.stopped),
            None => environment.ui.suspend_help(),
        });
    let pane = pane.clone();
    let captured = captured.clone();
    let button = Button::new(
        ENVIRONMENT_ID,
        label,
        HoverHelp::tooltip(help).below(),
        Role::Button,
    )
    .tier(tier)
    .toolbar()
    .condensed(condensed.then_some(Condensed::Padding))
    .expanded(asking)
    .disabled(environment.unavailable)
    .on_click(move |_, window, cx| {
        if let Some(target) = captured.shown() {
            let mark = Mark::take(cx);
            let _ = pane.update(cx, |pane, cx| {
                pane.environment_action(target, &mark, window, cx)
            });
        }
    });
    match environment.pending {
        Some(pending) => button.accessible_name(pending.label()).spinner(step),
        None => button.icon(icon),
    }
}

impl Drawn<'_> {
    /// `split`'s caret, offering `choices`, drawn open while its menu is (`open`), in the actions' ordinary look or
    /// `condensed`, tracking `focus`, and opening its menu through `pane`. It is unavailable only while the session's
    /// commands are refused (`busy`), as the old caret was: while neither choice can run it is muted but still opens the
    /// menu, which is where the reasons are (SPEC.md: "the menu remains available to inspect reasons"). Down and Up open the
    /// closed menu on its first or last choice, as the old caret did.
    fn caret_button(
        &self,
        split: Split,
        choices: &Choices,
        busy: bool,
        open: bool,
        condensed: bool,
        focus: &FocusHandle,
    ) -> IconButton {
        let help = if choices.none_available() {
            "Neither option is available. Open the menu for the reasons.".to_owned()
        } else {
            format!(
                "{} options: use the current settings or edit them first",
                split.label()
            )
        };
        let (clicked, keyed) = (self.pane.clone(), self.pane.clone());
        let (click_target, key_target) = (self.shown.action.clone(), self.shown.action.clone());
        IconButton::new(
            split.caret_id(),
            Icon::ChevronDown,
            format!("{} options", split.label()),
            HoverHelp::tooltip(help).below(),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .toolbar()
        .condensed(condensed.then_some(Condensed::Words))
        .split(SplitPart::Caret {
            muted: choices.none_available(),
        })
        .expanded(open)
        .disabled(busy)
        .focus(focus)
        .on_key_down(toggle_arrow_keys(move |start, window, cx| {
            if let Some(target) = key_target.shown() {
                let _ = keyed.update(cx, |pane, cx| {
                    pane.open_caret_menu(target, split, start, window, cx)
                });
            }
        }))
        .on_click(move |_, window, cx| {
            if let Some(target) = click_target.shown() {
                let _ = clicked.update(cx, |pane, cx| {
                    pane.toggle_caret_menu(target, split, window, cx)
                });
            }
        })
    }
}

impl SessionPane {
    /// `split`'s caret was activated (a click, Enter or Space): open its menu on the first choice, or close it if it is
    /// open.
    fn toggle_caret_menu(
        &mut self,
        target: &menu::Actionable,
        split: Split,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.header.open_menu(window, cx) == Some(split) {
            self.close_caret_menu(window, cx);
        } else {
            self.open_caret_menu(target, split, MenuStart::First, window, cx);
        }
    }

    /// Open `split`'s menu below its caret with focus on the choice `start` names, unless a caret menu is open
    /// already, the caret was not drawn, or the open session's commands are refused. The menu keeps Tab, as every menu
    /// does, and names the caret as where closing returns focus; its own Tab closes it ([`CaretMenu`]).
    fn open_caret_menu(
        &mut self,
        target: &menu::Actionable,
        split: Split,
        start: MenuStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.header.open_menu(window, cx).is_some() {
            return;
        }
        if self.open_key(cx).as_ref() != Some(&target.key) || !target.allows_launch_with(cx) {
            return;
        }
        let Some((anchor, caret)) = self.header.carets.get(&split).cloned() else {
            return;
        };
        let mark = Mark::take(cx);
        let items: [FocusHandle; 2] = std::array::from_fn(|_| cx.focus_handle().tab_stop(true));
        let start = match start {
            MenuStart::First => items[0].clone(),
            MenuStart::Last => items[1].clone(),
        };
        let pane = cx.weak_entity();
        let view = cx.new(|_| CaretMenu {
            pane,
            key: target.key.clone(),
            split,
            items,
            action: ActionFrame::default(),
        });
        let opened = layers::open_popover(
            Popover::menu(view, cx.focus_handle(), anchor, start)
                // The caret, not what held focus before: on macOS the click that opened the menu left the caret
                // unfocused, and SPEC.md returns focus to the caret when the menu closes.
                .returns_to(focus::ReturnTo::Control(caret)),
            &mark,
            window,
            cx,
        );
        self.header.menu = opened.map(|layer| (split, layer));
        cx.notify();
    }

    /// Close the caret menu if one is open, which returns focus to its caret.
    fn close_caret_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, layer)) = self.header.menu.take() {
            layers::close(layer, window, cx);
            cx.notify();
        }
    }

    /// A choice of `split`'s menu for `target` was activated at `mark`: close the menu, which returns
    /// focus to the caret, then go on as the plain action's button goes on ([`SessionPane::act_from_header`], so the
    /// same question hangs under the main part), or open the variant's launcher, which returns focus to the caret when
    /// it closes (SPEC.md: "Cancelling Restart with returns to that persistent caret"). Nothing happens when the open
    /// session has changed.
    fn caret_choice(
        &mut self,
        split: Split,
        target: &menu::Actionable,
        with: bool,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_caret_menu(window, cx);
        if self.open_key(cx).as_ref() != Some(&target.key) {
            return;
        }
        if !with {
            self.act_from_header(split.action(), target, mark, window, cx);
        } else if target.allows_launch_with(cx) && !split.open_with(&target.key, window, cx) {
            tracing::debug!(
                ?split,
                "the launcher refused to open from the header's menu"
            );
        }
    }
}

/// A split button's open menu (the old `ActionMenu`'s flyout): a header naming the action, then the plain choice and
/// its "with…" variant, each with an icon, a label and a line saying what it does or why it cannot. Drawn from the
/// store while it is open, so availability and reasons follow the session.
///
/// An unavailable choice stays focusable, so its reason is reachable by keyboard, and does nothing when activated
/// ([`MenuItem::disabled`]). Up and Down walk the two choices (the layer manager's menu context), Home and End go to
/// the first and the last ([`home_end_keys`]), and Escape closes the menu with focus back on the caret, all as the old
/// menu did. Tab closes it the same way, which SPEC.md asks of these menus alone ("Escape or Tab closes the menu and
/// returns focus to its caret"): the menu takes Tab's action itself, before the focus coordinator's trap would keep it
/// inside, so it needs nothing from the layer manager.
pub(crate) struct CaretMenu {
    pane: WeakEntity<SessionPane>,
    key: String,
    split: Split,
    /// The two choices' focus handles, plain first: the menu starts on either, and Home and End move there.
    items: [FocusHandle; 2],
    action: ActionFrame<menu::Actionable, [bool; 2]>,
}

#[cfg(test)]
impl CaretMenu {
    /// Draw the real Restart menu for a pushed session without a toolbar or a live activation target.
    pub(crate) fn restart_for_golden(key: &str, cx: &mut App) -> Self {
        Self {
            pane: WeakEntity::new_invalid(),
            key: key.to_owned(),
            split: Split::Restart,
            items: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            action: ActionFrame::default(),
        }
    }
}

/// The old `.header-action-menu`'s width.
const CARET_MENU_WIDTH: f32 = 260.;

impl Render for CaretMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.split.caret_id();
        let panel = layers::panel(cx)
            .id(SharedString::from(format!("{id}-menu")))
            .debug_selector(move || format!("{id}-menu"))
            .w(px(CARET_MENU_WIDTH));
        let Some(store) = session::current_store(cx) else {
            self.action.clear();
            return panel;
        };
        let Some(row) = store
            .read(cx)
            .collections()
            .get::<SessionRow>(&self.key)
            .cloned()
        else {
            self.action.clear();
            return panel;
        };
        let shell = row.info.launch.is_shell();
        let launcher_wait = menu::launcher_wait(cx);
        let choices = Choices::of(self.split, &self.key, &row, launcher_wait, cx);
        let busy = menu::refused(&self.key, cx);
        let Some(target) = menu::Actionable::drawn(&self.key, row, cx) else {
            self.action.clear();
            return panel;
        };
        let offer = [
            busy || choices.plain_unavailable,
            busy || choices.with_unavailable,
        ];
        let captured = self.action.capture(target, offer, window);
        let summary = if choices.none_available() {
            "Unavailable for this session"
        } else {
            match self.split {
                Split::Restart => "Resume a conversation in this session",
                Split::Replace if shell => "Start a fresh shell",
                Split::Replace => "Start a fresh conversation",
            }
        };
        let item = |with: bool| {
            let (label, icon, description, unavailable) = if with {
                (
                    self.split.with_label(),
                    Icon::Adjust,
                    choices.with_description.clone(),
                    choices.with_unavailable,
                )
            } else {
                (
                    self.split.label(),
                    self.split.action().icon(),
                    choices.plain_description.clone(),
                    choices.plain_unavailable,
                )
            };
            let (pane, captured, split) = (self.pane.clone(), captured.clone(), self.split);
            let item_id = format!("{id}-{}", if with { "with" } else { "plain" });
            let help = match &description {
                Line::App(words) => HoverHelp::tooltip(words.clone()),
                Line::Sentence(sentence) => {
                    HoverHelp::sentence(sentence.clone().with_id(format!("{item_id}-help")))
                }
            };
            let item = MenuItem::new(item_id, icon, label, help, Role::MenuItem);
            let item = match description {
                Line::App(words) => item.description(words),
                Line::Sentence(sentence) => item.description_sentence(sentence),
            };
            item.disabled(busy || unavailable)
                .focus(&self.items[usize::from(with)])
                .on_click(move |_, window, cx| {
                    if let Some(target) = captured.shown() {
                        let mark = Mark::take(cx);
                        let _ = pane.update(cx, |pane, cx| {
                            pane.caret_choice(split, target, with, &mark, window, cx)
                        });
                    }
                })
        };
        let (first, last) = (self.items[0].clone(), self.items[1].clone());
        let (tabbed, shift_tabbed) = (self.pane.clone(), self.pane.clone());
        panel
            // Home and End, the old menu's: the layer manager's menu context binds only Up and Down.
            .on_key_down(home_end_keys(first, last))
            // Tab and Shift+Tab close the menu with focus back on the caret (see the type's docs). Handled before the
            // root's handler, which would walk the menu's choices.
            .on_action(move |_: &FocusNext, window, cx| tab_out(&tabbed, window, cx))
            .on_action(move |_: &FocusPrevious, window, cx| tab_out(&shift_tabbed, window, cx))
            .child(MenuHeader::app(self.split.label(), summary))
            .child(
                div()
                    .id(SharedString::from(format!("{id}-items")))
                    .role(Role::Menu)
                    .aria_label(self.split.label())
                    .flex()
                    .flex_col()
                    .child(item(false))
                    .child(item(true)),
            )
    }
}

/// Tab or Shift+Tab in `pane`'s open caret menu: close it, which returns focus to its caret, through the layer
/// manager's Tab close ([`layers::close_on_tab`]), which swallows only the key's held repeats. Since the focus
/// coordinator now swallows fresh presses only for Escape, a plain close or a dismissal would behave the same; the Tab
/// close is kept so a second Tab still goes on from the caret to the next control, as SPEC.md's caret rule means.
fn tab_out(pane: &WeakEntity<SessionPane>, window: &mut Window, cx: &mut App) {
    let open = pane
        .upgrade()
        .and_then(|pane| pane.read(cx).header.menu.map(|(_, layer)| layer));
    if let Some(layer) = open {
        layers::close_on_tab(layer, window, cx);
    }
}

/// While Delete is pending, the spinner and the same label the row draws ([`delete::in_flight`]) replace the
/// header actions.
fn delete_progress(key: &str, label: &'static str, step: u8, cx: &App) -> impl IntoElement {
    let ink = theme::color(theme::palette(cx).accent);
    div()
        .id(SharedString::from(format!(
            "session-header-delete-progress-{key}"
        )))
        .debug_selector(|| "session-header-delete-progress".into())
        .role(Role::Status)
        .aria_label(label)
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .whitespace_nowrap()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(spinner_icon(step, px(12.), ink))
        .child(label)
}
