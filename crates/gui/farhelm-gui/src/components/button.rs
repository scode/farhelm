//! Buttons, drawn as the old stylesheet's `.btn` tiers: ghost (the default), neutral, primary and danger, the outlined
//! danger of a permanent answer, plus the compact control that sits on the red banner, the ⋯ that opens a list row's
//! menu, the host rail's square entries, the removable filter chip, the button that reads as a link, the version
//! readout's controls that keep the look of their text, an entry of a navigation list such as Settings' sections, the
//! two swatches of the host settings' appearance picker, and a Hosts row's outlined inline Update.
//!
//! Either part of a split button is a button too ([`Button::split`]): the session header's Restart and Replace, a main
//! part and a caret joined with no gap, each its own control with its own focus and help.
//!
//! The session header's buttons ([`Button::compact`]) also have a condensed look ([`Button::condensed`]), the old
//! `.actions-compact` rule, which the header takes only while its row cannot hold its actions at their ordinary size
//! beside a tunnel pill. It changes only the box, the text size and the line height: names, help, focus and activation
//! stay as they are.
//!
//! A button shows its label's text, or other content in its place ([`Button::content`]: a host's mark, a mark beside a
//! host's name); the label is its accessible name either way. A button that is one of a set of choices says whether
//! it is the one in force ([`Button::pressed`]), as the rail's entries do.
//!
//! A button that opens something (a menu, a question) can say so with [`Button::expanded`]: assistive technology hears
//! whether what it opens is open, and while it is the button wears the old pressed look, the accent fill with the
//! inverse ink. A plain hover fill left behind after the pointer moves away would read as stale hover, not as "open";
//! the old header triggers and the row menu's ⋯ followed the same rule. A button that widens or narrows a region in
//! place rather than opening something over the page (the host rail's Expand control) reports the same state without
//! that look ([`Button::discloses`]).
//!
//! A button can also answer a secondary click (a right press, which macOS also makes of a Ctrl+click) with an action of
//! its own ([`Button::on_secondary_click`]): the host rail's avatars open their host's menu that way.
//!
//! A button activates on click, and on Enter or Space while it has keyboard focus (a held key does not repeat it:
//! SPEC.md, "Repeated or composing Enter cannot answer the new question"). Focus is the focus coordinator's: an enabled
//! button is a tab stop tracked through [`focus::track`], which applies the coordinator's click rule (on Linux a click
//! focuses it, on macOS it does not, as in WebKit there) in place of GPUI's own focus-on-press.
//! A disabled button is not a tab stop and takes no focus on a click (SPEC.md: Tab stays within a dialog's "visible,
//! enabled controls"). A busy button ([`Button::busy`]) is the old app's `aria-disabled` control, different in that it
//! keeps focus, its place in Tab's order and its hover help, and does nothing when pressed. The old app used it for
//! two cases: an action in flight (a Save while the save runs, so the control the user chose stays focused while its
//! work runs, where disabling it would drop focus out of the dialog's controls), and an action that cannot run for a
//! reason its hover help explains (the session header's Restart for a session that cannot resume, whose reason must
//! stay reachable by pointer and keyboard alike).
//!
//! A button with words in the session list's heading band (its order title) takes that band's padding
//! ([`Button::heading`]), as the old sheet's `.session-heading .sort-title` rule gave it; the band's icon buttons are
//! fixed squares ([`Button::square`]). The marked bar that takes the band's place while sessions are marked draws its
//! Stop and Delete a little smaller than an ordinary button ([`Button::toolbar`]), as the old `.marked-bar` rule did.
//!
//! The focus ring is the old `:focus-visible` ring (a 2-pixel accent outline one pixel outside the button, shown only
//! after keyboard input) together with the hover fill, which the old sheet also gave a focused button so a ghost
//! button's box is visible to a keyboard user. A few tiers keep their resting fill under the ring instead, as their old
//! rules did; `Button::fill` lists them.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, RenderOnce,
    Role, SharedString, StatefulInteractiveElement, Styled, Toggled, Window, div,
    prelude::FluentBuilder as _, px, svg,
};

use super::{Handler, HoverHelp};
use crate::assets::Icon;
use crate::focus;
use crate::theme::{self, Palette, Token, metrics};

/// The old stylesheet's button tiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tier {
    /// `.btn`: no fill, quiet text, a hover fill. Most toolbar and row controls.
    #[default]
    Ghost,
    /// `.btn-neutral`: a raised fill and outline, for secondary answers such as Cancel.
    Neutral,
    /// `.btn-primary`: the solid accent fill of an ordinary main action.
    Primary,
    /// `.btn-danger`: the solid red fill of a destructive confirmation.
    Danger,
    /// `.btn-danger.btn-outline`: red text and a red outline on no fill, with a subdued red hover. The permanent answer
    /// beside a one-off danger answer (the YOLO confirmation's "Start, and don't ask again on this host" beside "Start
    /// YOLO session anyway"): a filled button draws the reflex click, and the answer that turns a question off for good
    /// should not be the reflex target, so it keeps the danger tier's color but not its fill. Unavailable, it takes the
    /// retired fill as [`Tier::Danger`] does; busy, it keeps its outline and loses only its hover, as the old
    /// `aria-disabled` rule left it.
    DangerOutline,
    /// `.btn-primary.btn-outline`: accent text and an accent outline on no fill, with the control hover fill. The
    /// permanent answer beside a one-off primary answer, for the same reason as [`Tier::DangerOutline`]: the host setup
    /// confirmation's "Set up, don't ask again" beside Set up. Unavailable, it takes the retired fill as
    /// [`Tier::Primary`] does; busy, it keeps its outline and loses only its hover.
    PrimaryOutline,
    /// `.btn.banner-reclaim`: a compact ghost button for the red banner (the old terminal's "Take control"). It has a
    /// pale red outline and white text so it reads against the banner's fill, a darker red hover, and a white focus
    /// ring because the accent ring is not calibrated for a red background. Use it only on a red banner.
    OnBanner,
    /// `.session-row-menu`: the ghost ⋯ at the end of a list row, in the old toggle's box (`padding: 0 5px` around the
    /// glyph, at the row's 14-pixel text and `line-height: 1.4`, so 19.6 pixels high). `on_fill` is for the selected
    /// row's solid accent fill, where it draws in the inverse ink with an accent-family hover and an inverse focus ring
    /// (`.session-row.selected .session-row-menu` and `.session-row.selected :focus-visible`).
    RowMenu { on_fill: bool },
    /// `.host-rail .btn`: an entry of the host rail, a 40-pixel square with no padding around what it shows (a word
    /// or a host's mark, through [`Button::content`]), quiet ink, and the raised `--bg-2` under the pointer. Its chosen
    /// entry is drawn through [`Button::pressed`].
    Rail,
    /// `.btn.btn-neutral.host-filter-chip`: the neutral tier in a chip's tighter box (`padding: 4px 6px`) that may
    /// shrink to its row, so the host name inside it can be cut short. Its content lays out its own parts and does the
    /// cutting.
    Chip,
    /// `.inline-link`: a button that reads as a link in running text: the link color, underlined, with no fill, edge
    /// or padding. The focus ring is every control's.
    Link,
    /// `.app-version-check`, `.app-version-toggle` and `.app-update-failure`: the sidebar header's version readout and
    /// its status, controls that keep the look of the text they sit in. No padding, edge or fill, at rest or under the
    /// pointer, and the inks are the content's own; keyboard focus draws every control's ring. `focus_fill` adds the
    /// control-hover fill under that ring, which only the update-menu toggle had (the old sheet's `.btn:focus-visible`
    /// fill outranked the toggle's `background: none`, and its own hover rule took the fill away again under the
    /// pointer).
    Readout { focus_fill: bool },
    /// `.app-settings-navigation button`: an entry of a navigation list (Settings' sections), as wide as its list, its
    /// icon and label at the left 8 pixels apart, 13-pixel text on a 16-pixel line (the old `line-height: normal`, which
    /// made it 34 pixels high), the primary ink at rest, and the label cut short with an ellipsis rather than wrapped.
    /// The entry whose pane is shown is drawn through [`Button::pressed`], in this tier as the old sheet drew it: the
    /// accent fill with the accent edge, which the pointer does not change, and the inverse ring one pixel inside the
    /// edge (`outline-offset: -3px`).
    Nav,
    /// `.templates-quick-mark`: the bolt beside a template in the Templates list, a ghost with no fill at rest, its glyph
    /// in the quiet `--fg-2` until it holds ⚡, then in the warning amber (`lit`). The old sheet drew no fill under the
    /// pointer, only an edge; here the pointer gets the ghost tier's hover fill, the nearest look the button draws. A
    /// lit bolt is also reported pressed ([`Button::pressed`]), without the pressed fill: the amber glyph is its look.
    /// Under the keyboard ring a bolt, lit or not, has no fill, as the old one had none ([`Button::fill`]).
    QuickMark { lit: bool },
    /// `.templates-number-key`: a template's number-key button, a ghost in a tight 4-pixel box whose text (a chord, in
    /// the monospace face, given as [`Button::content`]) is the quiet `--fg-2` with no key and the accent once one is
    /// bound (`bound`). The same hover note as [`Tier::QuickMark`].
    NumberKey { bound: bool },
    /// `.templates-row`: a template's row in the Templates list, a ghost as wide as its row (`width: 100%`) whose
    /// content (the template's name over its summary, given as [`Button::content`]) sits at its left, 10 by 12 pixels
    /// in, in the primary ink, with the control hover fill under the pointer. The row whose template the editor holds is
    /// drawn through [`Button::pressed`] with the accent fill and a more visible accent ink edge, one step up
    /// the accent family under the pointer, and the inverse ring inside the edge over the resting accent
    /// ([`Button::fill`]). The content draws its own lines, so a pressed row's summary takes `--on-accent-dim` there,
    /// not here. The Templates editor's "+ Add field" menu draws its entries in this tier too, left-aligned with the
    /// same hover fill.
    TemplateRow,
    /// `.launch-composer-recent-slots > button`: one row of the launcher's recent setups, a ghost as wide as its well and
    /// 36 pixels high, with square corners (the well rounds the strip), 14 pixels in at the sides, its content (the
    /// setup's cells, given as [`Button::content`]) laid out from the left in the quiet `--fg-1` ink at 13 pixels, and the
    /// control hover fill under the pointer. The cells draw their own inks (the agent's name in the primary ink). Its
    /// keyboard ring is drawn inside its edge, since the well clips anything outside it.
    RecentSetup,
    /// `.launch-composer-browser > button`: one entry of the launcher's Browse listing (Use, Parent, or a folder), as
    /// wide as the listing and at least 38 pixels high, on the recessed `--well` fill with the `--border` edge, its
    /// words centered in the primary ink at 14 pixels and semibold, the look the old stylesheet shared with its native
    /// selects, with no hover fill. The old edge was 1.5 pixels; this one is the frame's 1. Nothing disables one.
    BrowseRow,
    /// `.templates-remove`: the ✕ that removes a field from a template, a ghost 30 pixels wide and 34 high (the height
    /// of the field's control beside it, so the ✕ is centered on the control), in the quiet `--fg-1` ink that turns
    /// `--danger` with the control hover fill under the pointer.
    FieldRemove,
    /// `.host-icon-choice`: one of the host settings' icon choices, a 36-pixel square on the well with a dim edge, its
    /// content (the host's mark) centered, and the control hover fill under the pointer. Chosen
    /// ([`Button::pressed`]), it takes the accent fill with the accent edge, as a [`Tier::Nav`] entry does, keeps the
    /// fill's hover, and keeps its resting fill under the keyboard ring ([`Button::fill`]). The content is the caller's,
    /// so a chosen choice draws its mark in the inverse ink itself (the old sheet's one `!important` over the mark's own
    /// tint).
    IconChoice,
    /// `.session-tunnel-toggle`: the open-tunnels pill in the session's top row, the row's one distinctively colored
    /// control: the tunnel ink on the tunnel fill inside the tunnel edge, rounded to 8 pixels, padded 2 by 4 pixels at
    /// 11 pixels. Open ([`Button::expanded`]), it takes the accent fill with the inverse ink, as any open toggle does.
    Tunnel,
    /// `.tab`: one terminal of the session view's tab strip (the agent's or a shell tab's), a ghost in the old tab's box
    /// (`padding: 3px 10px` at 12 pixels) that never shrinks, so a long strip scrolls rather than squeezing its tabs.
    /// The shown terminal's tab is drawn through [`Button::pressed`], the accent fill with the inverse ink
    /// (`.tab.selected`); [`Button::fill`] has its one exception under the keyboard ring.
    TerminalTab,
    /// `.tab-close`: the × beside a shell tab in the session view's tab strip, a ghost in a tight box (`padding: 3px 7px`
    /// at 12 pixels) that never shrinks, in the quiet `--fg-1` ink that turns `--danger` on the subdued red fill
    /// (`--danger-bg-dim`) under the pointer, warning before a press that ends a shell. Busy
    /// ([`Button::busy`]) while its tab closes.
    TerminalTabClose,
    /// `.host-color-choice`: one of the host settings' color choices, a 26-pixel circle with 3 pixels around its
    /// content (the color's swatch), no fill or edge at rest and no hover fill. Chosen, it takes the accent fill inside
    /// an edge in the inverse ink.
    ColorChoice,
    /// `.btn.host-update-button`: a Hosts row's inline `↑ Update`, outlined in its own ink on no fill, in a compact box
    /// (`padding: 2px 5px`) that never shrinks, with the control hover fill. The ink carries the update's urgency
    /// without borrowing a filled tier: the warning amber for an optional update, the strong danger red for a
    /// `required` one (`[data-update-kind="required"]`).
    HostUpdate { required: bool },
    /// `.approval-waiting`: a waiting agent request's header below the expanded approval card, a full-width row on the
    /// raised `--bg-2` with the warning amber edge and the larger corner, `padding: 6px 12px`, the control hover fill,
    /// and the browser's default button text (about 13.3 pixels, as the old header set no size), its content laid out
    /// from the left by the caller ([`Button::content`]).
    Waiting,
}

/// The [`Tier::QuickMark`] box's padding, vertical then horizontal (`.templates-quick-mark`: `10px 5px`).
const QUICK_MARK_PAD: (f32, f32) = (10., 5.);
/// The [`Tier::Tunnel`] pill's padding, vertical then horizontal (`.session-tunnel-toggle`: `2px 4px`).
const TUNNEL_PAD: (f32, f32) = (2., 4.);
/// The [`Tier::TerminalTab`] box's padding, vertical then horizontal (`.tab`: `3px 10px`).
const TERMINAL_TAB_PAD: (f32, f32) = (3., 10.);
/// The [`Tier::TerminalTabClose`] box's padding, vertical then horizontal (`.tab-close`: `3px 7px`).
const TERMINAL_TAB_CLOSE_PAD: (f32, f32) = (3., 7.);
/// The [`Tier::NumberKey`] box's padding on every side.
const NUMBER_KEY_PAD: f32 = 4.;
/// The [`Tier::FieldRemove`] box: `.templates-remove`'s `width: 30px; height: 34px`.
const FIELD_REMOVE_SIZE: (f32, f32) = (30., 34.);
/// The [`Tier::TemplateRow`] box's padding, vertical then horizontal (`.templates-row`: `10px 12px`).
const TEMPLATE_ROW_PAD: (f32, f32) = (10., 12.);
/// The [`Tier::RecentSetup`] box: `height: 36px; padding: 0 14px`. The old row had no border; every button here has a
/// 1-pixel (transparent) edge, so the padding is a pixel less to keep the content 14 pixels in.
const RECENT_SETUP_HEIGHT: f32 = 36.;
const RECENT_SETUP_PAD_X: f32 = 13.;
/// The [`Tier::BrowseRow`] box's least height (`min-height: 38px`).
const BROWSE_ROW_MIN_HEIGHT: f32 = 38.;

/// The [`Tier::RowMenu`] box's height: 14-pixel text at `line-height: 1.4`.
const ROW_MENU_HEIGHT: f32 = 19.6;
/// The [`Tier::RowMenu`] box's horizontal padding.
const ROW_MENU_PAD_X: f32 = 5.;
/// The [`Tier::Rail`] box's side (`.host-rail .btn`'s `width` and `height`).
const RAIL_SIZE: f32 = 40.;
/// The [`Tier::Chip`] box's padding, vertical then horizontal.
const CHIP_PAD: (f32, f32) = (4., 6.);
/// The [`Tier::Nav`] box's padding, vertical then horizontal.
const NAV_PAD: (f32, f32) = (8., 10.);
/// The [`Tier::Nav`] label's line height.
const NAV_LINE_HEIGHT: f32 = 16.;
/// The [`Tier::IconChoice`] square's side (`.host-icon-choice`'s `max-width: 36px` with `aspect-ratio: 1`).
const ICON_CHOICE_SIZE: f32 = 36.;
/// The [`Tier::HostUpdate`] box's padding, vertical then horizontal.
const HOST_UPDATE_PAD: (f32, f32) = (2., 5.);
/// The [`Tier::Waiting`] row's padding, vertical then horizontal.
const WAITING_PAD: (f32, f32) = (6., 12.);
/// The [`Tier::Waiting`] row's text size: a browser's default for a button that sets none (13.333 pixels).
const WAITING_TEXT: f32 = 13.333;
/// The [`Tier::ColorChoice`] circle's side and the room around its swatch.
const COLOR_CHOICE_SIZE: f32 = 26.;
const COLOR_CHOICE_PAD: f32 = 3.;

/// A heading button with words' padding, vertical then horizontal (`.session-heading .sort-title`: `padding: 2px 0`),
/// so the order title's words start at the band's own edge.
const HEADING_WORDS_PAD: (f32, f32) = (2., 0.);

/// A marked bar button's padding, vertical then horizontal, and its height, edge included (`.marked-bar .marked-stop`:
/// `height: 26px; padding: 3px 7px`). Its 12-pixel text is every button's.
const TOOLBAR_PAD: (f32, f32) = (3., 7.);
const TOOLBAR_HEIGHT: f32 = 26.;

/// Every button's line height but where a tier or look sets its own: `.btn`'s `line-height: 1.4` at its 12-pixel text,
/// 16.8 pixels rounded to 17.
const LINE: f32 = 17.;
/// The edge every tier but `Tier::Link` and `Tier::Readout` draws, transparent where the tier has no edge color (`.btn`'s
/// `border: 1px solid`). The render draws it with `border_1`; this names it for [`Button::compact_height`].
const EDGE: f32 = 1.;

/// A compact button's padding, vertical then horizontal: `.titlebar-actions .btn`'s `padding: 2px 8px`, the session
/// header's actions.
const COMPACT_PAD: (f32, f32) = (2., 8.);
/// A compact button's gap between its icon and its label: the header's main buttons' `gap: 4px`.
const COMPACT_GAP: f32 = 4.;
/// A compact button's icon: `.action-glyph`'s 13 pixels.
const COMPACT_ICON: f32 = 13.;
/// A compact split button caret's side padding: `.header-action-caret`'s `padding: 2px 4px`.
const CARET_PAD_X: f32 = 4.;

/// A condensed button's padding on every side: `.actions-compact .titlebar-actions .btn`'s `padding: 2px`. A
/// [`Condensed::Words`] button keeps it above and below only.
const CONDENSED_PAD: f32 = 2.;
/// A [`Condensed::Words`] button's text size, its line height and its gap between icon and label: the old rule's
/// `font-size: 11px` and `gap: 2px`, and `.btn`'s `line-height: 1.4` at that size (15.4 pixels, rounded as the ordinary
/// 16.8 is to 17). Such a button is therefore 2 pixels shorter than an ordinary one, as the old one was 1.4 pixels
/// shorter. The gap is public for content a condensed button shows in place of its label (the header's "Stopping…").
const CONDENSED_TEXT: f32 = 11.;
const CONDENSED_LINE: f32 = 15.;
pub const CONDENSED_GAP: Pixels = px(2.);

/// The session header's geometry a button is drawn in, if any ([`Button::compact`], [`Button::condensed`]): one field,
/// so a condensed button is always a compact one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compact {
    Ordinary,
    Condensed(Condensed),
}

/// How a compact button gives up width while the session header's row is short of it ([`Button::condensed`]). The old
/// stylesheet had two rules under `.actions-compact`, and which one a header button took depended on its class, so the
/// owner says which applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condensed {
    /// `.titlebar-actions .btn { padding: 2px }`: 2 pixels of padding all round, the text, icon and gap unchanged. Every
    /// header button the next rule does not name (an environment session's Suspend or Resume).
    Padding,
    /// The rule for `.header-stop`, `.restart-primary`, `.header-replace`, `.header-clone`, `.header-delete` and
    /// `.header-action-caret`: 11-pixel text, no padding at the sides (2 pixels above and below remain from the rule
    /// before), and 2 pixels between the icon and the label.
    Words,
}

/// Which part of a split button a button is ([`Button::split`]): the session header's Restart and Replace, whose main
/// part performs the ordinary action and whose caret opens a menu of that action and its "with…" variant (SPEC.md,
/// "Terminal experience"). The two parts are drawn side by side with no gap, the way the old `.header-action-split`
/// joined them, so each squares the corners where it meets the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitPart {
    /// The main part: its right corners are square.
    Main,
    /// The caret: its left corners are square, and an icon-only caret in the compact geometry keeps 4 pixels at its
    /// sides (`.header-action-caret`'s `padding: 2px 4px`) rather than the square an icon-only button gets. `muted`
    /// draws a neutral caret in the retired look while it is not open, for a menu none of whose choices can run now;
    /// unlike [`Button::busy`] it still opens its menu, which is where the reasons are (the old
    /// `[data-actions-unavailable="true"]` rule, a visual state rather than a disabled one).
    Caret { muted: bool },
}

/// A button with a visible text label (and optionally a leading icon).
#[derive(IntoElement)]
pub struct Button {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    tier: Tier,
    icon: Option<Icon>,
    show_label: bool,
    /// The accessible name, when it says more than the visible label ([`Button::accessible_name`]).
    accessible_name: Option<SharedString>,
    disabled: bool,
    /// Whether the button's action is in flight or cannot run: drawn as unavailable, ignoring activation, but still
    /// focusable ([`Button::busy`]).
    busy: bool,
    /// Whether the button's action would be refused now: drawn as unavailable, but still pressed ([`Button::refused`]).
    refused: bool,
    /// The session header's tighter geometry ([`Button::compact`]), condensed or not ([`Button::condensed`]).
    compact: Option<Compact>,
    /// The list heading band's padding for a button with words ([`Button::heading`]).
    heading: bool,
    /// The marked bar's geometry ([`Button::toolbar`]).
    toolbar: bool,
    /// Whether what the button opens is open; `None` for a button that opens nothing.
    expanded: Option<bool>,
    /// Whether the button's choice is the one in force; `None` for a button that is not a choice.
    pressed: Option<bool>,
    /// Whether a pressed button wears the quieter look ([`Button::quiet`]).
    quiet: bool,
    /// Whether a press leaves keyboard focus where it was ([`Button::keeps_focus_on_press`]).
    keeps_focus: bool,
    /// What the button shows in place of its label's text, when it shows more than words.
    content: Option<AnyElement>,
    /// A mark drawn over the button's corner, placed by its caller ([`Button::badge`]).
    badge: Option<AnyElement>,
    /// A fixed square side in place of the tier's padding ([`Button::square`]).
    side: Option<Pixels>,
    /// The part of a split button this is, if it is one ([`Button::split`]).
    split: Option<SplitPart>,
    /// Whether an open button ([`Button::expanded`]) wears the open look; false for a disclosure
    /// ([`Button::discloses`]).
    open_look: bool,
    /// Whether a [`Tier::Rail`] entry is a row as wide as the expanded rail ([`Button::rail_row`]).
    rail_row: bool,
    on_click: Option<Handler>,
    /// What a secondary click does ([`Button::on_secondary_click`]).
    on_secondary: Option<SecondaryHandler>,
    /// Keys the owner takes before the button's own Enter and Space ([`Button::on_key_down`]).
    on_key: Option<KeyHandler>,
    focus: Option<FocusHandle>,
}

/// A button owner's own key handler ([`Button::on_key_down`]).
type KeyHandler = Rc<dyn Fn(&KeyDownEvent, &mut Window, &mut App)>;

/// A button owner's secondary-click handler ([`Button::on_secondary_click`]).
type SecondaryHandler = Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;

impl Button {
    /// A button. `id` must be stable and unique among its siblings (it keys the button's element state, the tooltip
    /// owner among it, and the test bridge finds the button by it);
    /// `label` is its visible text and accessible name; `role` is what assistive technology and the test bridge see
    /// (usually [`Role::Button`], [`Role::MenuItem`] inside a menu).
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        Button {
            id: id.into(),
            label: label.into(),
            help,
            role,
            tier: Tier::default(),
            icon: None,
            show_label: true,
            accessible_name: None,
            disabled: false,
            busy: false,
            refused: false,
            compact: None,
            heading: false,
            toolbar: false,
            expanded: None,
            pressed: None,
            quiet: false,
            keeps_focus: false,
            content: None,
            badge: None,
            side: None,
            split: None,
            open_look: true,
            rail_row: false,
            on_click: None,
            on_secondary: None,
            on_key: None,
            focus: None,
        }
    }

    /// Draw it in one of the stylesheet's tiers.
    pub fn tier(mut self, tier: Tier) -> Self {
        self.tier = tier;
        self
    }

    /// A leading icon before the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// An unavailable button: drawn dimmed (or with the retired fill for primary and danger) and ignoring activation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The old `aria-disabled` button: one whose action is in flight (a Save while the save runs) or cannot run for a
    /// reason its hover help explains (the session header's Restart for a session that cannot resume). Unlike
    /// [`Button::disabled`], it stays a tab stop, keeps focus and keeps its hover help, so the button the user pressed
    /// is still where focus is when the work settles, and a reason stays reachable. It ignores activation: Enter and
    /// Space stop at the button, as they do at an enabled one, instead of going on to whatever holds it. A busy neutral
    /// button shows `cursor: not-allowed`, as the old sheet gave the `aria-disabled` header Restart; no other tier
    /// does, since the old busy Save kept the button's cursor.
    ///
    /// Its look by tier: the filled tiers (neutral, chip, primary, danger) take the retired look, the old sheet's
    /// `--bg-2` fill, `--border-dim` edge and `--fg-2` text; the unfilled ones (ghost, the danger outline, the row's ⋯,
    /// the banner's, the rail's, the Settings section list's and the link) keep their look and lose only their hover
    /// fill. GPUI cannot report the state to assistive technology, so a button whose action cannot run should say so in
    /// its name ([`Button::accessible_name`]).
    ///
    /// The look is applied on the next redraw, so a handler must still keep its own guard for a press that arrives
    /// before it (Rename's Save checks its draft).
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// A main action that would be refused now (the launcher's Launch while its draft is incomplete): drawn as
    /// unavailable, with the retired fill for primary and danger, but still focusable and still pressed, so a press
    /// runs the handler, which says why nothing happened. The old launcher's submit button was `aria-disabled` rather
    /// than `disabled` for exactly this (`create_form.rs` at reference commit `24bc0580`; SPEC.md, "Creation": "an
    /// incomplete attempt says why beside Launch, as does a refused Launch press").
    pub fn refused(mut self, refused: bool) -> Self {
        self.refused = refused;
        self
    }

    /// The accessible name, when the visible label alone would leave something out: the session header's Restart shows
    /// "Restart" but is named for its offer ("Resume conversation", "Restart unavailable"), as the old button's
    /// `aria-label` was. Without it the label is the name.
    pub fn accessible_name(mut self, name: impl Into<SharedString>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// The session header's geometry (`.titlebar-actions .btn`): 2 by 8 pixels of padding, 4 between the icon and the
    /// label, and a 13-pixel icon, so the header's actions keep their compact height and fit a 650-pixel pane.
    pub fn compact(mut self) -> Self {
        self.compact.get_or_insert(Compact::Ordinary);
        self
    }

    /// The session header's geometry ([`Button::compact`]), condensed as `condensed` says, or ordinary for `None`: the
    /// header's look while its row cannot hold its actions at their ordinary size (see [`Condensed`]).
    pub fn condensed(mut self, condensed: Option<Condensed>) -> Self {
        self.compact = Some(condensed.map_or(Compact::Ordinary, Compact::Condensed));
        self
    }

    /// The outer height, edge included, of a session header button with words in the compact geometry, condensed as
    /// `condensed` says or ordinary for `None`: 23 pixels ordinary and with [`Condensed::Padding`], 21 with
    /// [`Condensed::Words`]. It is computed from the same line, padding and edge the render draws, for a control drawn
    /// beside the header's buttons that must be exactly as tall as they are (the terminal text size group). It repeats
    /// the render's choice of padding and edge rather than reading them from it, so the session header's tests check it
    /// against a drawn Stop button in both looks. It does not hold for an icon-only button, whose box follows its 13-pixel icon instead (19
    /// pixels) unless something stretches it, as a split button's caret is stretched to its main part.
    pub fn compact_height(condensed: Option<Condensed>) -> Pixels {
        let (line, pad) = match condensed {
            None => (LINE, COMPACT_PAD.0),
            Some(Condensed::Padding) => (LINE, CONDENSED_PAD),
            Some(Condensed::Words) => (CONDENSED_LINE, CONDENSED_PAD),
        };
        px(line + 2. * pad + 2. * EDGE)
    }

    /// The session list heading band's padding for a button that shows words or other content (the order title,
    /// `.session-heading .sort-title`): 2 pixels above and below and none at its sides, so its words line up with the
    /// band's edge. Hover help prefers the space below this band. The tier still gives the colors. The band's icon buttons (the old
    /// Templates and New) are 24-pixel squares instead ([`Button::square`]).
    pub fn heading(mut self) -> Self {
        self.heading = true;
        self.help_below()
    }

    /// The session list's marked bar geometry for a button with words (its Stop and Delete): 3 by 7 pixels of padding in
    /// a 26-pixel box, the old `.marked-bar .marked-stop` and `.marked-delete` rule, so the bar's words fit beside "N
    /// marked" and its icon buttons in a narrow sidebar. Hover help prefers below. The tier still gives the colors.
    pub fn toolbar(mut self) -> Self {
        self.toolbar = true;
        self.help_below()
    }

    /// Top-band controls and dialog answers show help below when the space is free.
    pub(crate) fn help_below(mut self) -> Self {
        self.help = self.help.below();
        self
    }

    /// The button opens something, and `expanded` says whether it is open now: `aria-expanded`, and the pressed look
    /// while it is (see the module docs), except in [`Tier::Readout`], which keeps the look of its text. On a button
    /// that is also [`Button::pressed`], the pressed look wins.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// The button widens or narrows a region in place, and `expanded` says whether the region is expanded now: the
    /// `aria-expanded` of [`Button::expanded`], without its open look, since nothing opens over the page. The host
    /// rail's Expand control, whose old button reported `aria-expanded` and kept the rail's quiet look either way.
    pub fn discloses(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self.open_look = false;
        self
    }

    /// The button is one of a set of choices (the host rail's entries), and `pressed` says whether it is the one in
    /// force now: `aria-pressed`, which assistive technology hears as a toggle's state, and while pressed the old
    /// `[aria-pressed="true"]` look, the accent fill with the inverse ink and an inverse focus ring drawn inside the
    /// edge. A pressed button keeps that fill under the pointer and the keyboard ring (one step up the accent family),
    /// where the old sheet's later `.btn` hover and focus fills replaced it and left white ink on a light fill; a
    /// pressed `Tier::Nav` entry keeps its fill unchanged under the pointer, as the old section list did
    /// ([`Button::fill`] lists the pressed tiers that keep their resting fill under the ring).
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    /// While `quiet`, a pressed button wears a quieter look than the accent fill: the control hover fill, the primary
    /// ink and an accent edge, with its keyboard ring in the accent inside the edge, and the same fill under the
    /// pointer. The host rail's chosen entry wears it while Hosts management is shown, so that the filled Hosts control
    /// stays the one that marks the view on screen, as the old sheet drew it (the
    /// `.host-rail .btn[aria-pressed="true"]` rule under `.app-sidebar:has(.hosts-panel-toggle[aria-pressed="true"])`).
    /// It changes only the look: the button still reports itself pressed. It does nothing on a button that is not
    /// pressed.
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Show `content` in place of the label's text (a host's mark, or a mark beside a host's name), centered in the
    /// button's box. The label stays the button's accessible name, so content that a screen reader should not read
    /// twice needs no hiding; content holding host text draws it through the host-text element, as everywhere.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Draw `badge` over the button, after its icon and content: a small corner mark such as the Hosts control's `↑`
    /// when some host has an update. The badge places itself (absolutely, against the button's own box, which is
    /// positioned for it) and draws its own look, since the old sheet gave each such mark its own rule. It is a picture
    /// only: what it means belongs in the button's accessible name and hover help, so the badge needs no role.
    pub fn badge(mut self, badge: impl IntoElement) -> Self {
        self.badge = Some(badge.into_any_element());
        self
    }

    /// Draw the button as a square of `side` with its icon or content centered, in place of its tier's padding and
    /// height: the Hosts heading's add button (`.hosts-heading .add-host-button`, a 24-pixel neutral square around the
    /// 14-pixel plus), which the tier's own padding would make 28 pixels. The tier still decides its colors and ring.
    /// For a button that shows an icon or content: a text label would be clipped to the square.
    pub fn square(mut self, side: Pixels) -> Self {
        self.side = Some(side);
        self
    }

    /// Draw a [`Tier::Rail`] entry as a row of the expanded rail (`html.host-rail-open .host-rail .btn`): as wide as the
    /// rail's column rather than the 40-pixel square, still 40 pixels high, its content laid out from the left. The
    /// content keeps its glyph in the rail's 40-pixel icon box ([`super::rail_entry`]), so the glyph stays where the
    /// collapsed square had it and only the label beside it is new. Ignored in every other tier.
    pub fn rail_row(mut self) -> Self {
        self.rail_row = true;
        self
    }

    /// Draw the button as one part of a split button (see [`SplitPart`]). The owner lays the two parts out side by side
    /// with no gap, and each is a control of its own: its own label, hover help, focus and activation.
    pub fn split(mut self, part: SplitPart) -> Self {
        self.split = Some(part);
        self
    }

    /// Keys the owner handles while the button has focus, before the button's own Enter and Space activation: a key
    /// `handler` takes, by stopping its propagation, goes no further, and any other key goes on to the activation and
    /// past it. For a button that opens a list and drives it from the keyboard (the launcher's host picker, whose
    /// arrows browse its hosts, whose letters type ahead, and whose Enter picks while the list is open), the old
    /// buttons' own `onkeydown`. A disabled button takes no keys, so `handler` never runs for it; a busy one keeps focus
    /// and still hands its keys to `handler` (only its own activation stops), so a handler that must not act while the
    /// button is busy checks that itself.
    pub fn on_key_down(
        mut self,
        handler: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_key = Some(Rc::new(handler));
        self
    }

    /// On Linux, a press leaves keyboard focus where it was instead of focusing the button, as the Linux click rule
    /// otherwise would (the focus coordinator's [`focus::OnPress::Keep`]); Tab still reaches it. For a pointer control
    /// that stands in for the window system's own, whose clicks never take focus: the Linux window's drawn title bar
    /// (`desktop::linux_title_bar`; the migration's coordinator granted the exception on 2026-11-14).
    ///
    /// Nothing changes on macOS, where a click focuses no button anyway, and where tracking a button as `Keep` would
    /// put it in Tab's order even with the system's Keyboard navigation setting off.
    pub fn keeps_focus_on_press(mut self) -> Self {
        self.keeps_focus = cfg!(target_os = "linux");
        self
    }

    /// What activating the button does.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// What a secondary click does: a right press, which macOS's platform layer also makes of a Ctrl+left press (GPUI
    /// turns it into a right press before any element sees it). It runs on the press, as a context menu opens, takes the
    /// press so nothing behind the button sees it, and leaves keyboard focus where it was, as the coordinator leaves it
    /// for every press but the primary one. A disabled or busy button ignores it. The host rail's avatars open their
    /// host's menu this way (the old `oncontextmenu`).
    pub fn on_secondary_click(
        mut self,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_secondary = Some(Rc::new(handler));
        self
    }

    /// Use `handle` as the button's focus handle, so whoever owns it can make the button where focus starts (the prompt
    /// does, for its answers). Without it the button keeps its own handle in its element state.
    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    /// Colors for this tier and state: (fill, hover fill, edge, text). Ghost buttons have no fill or edge at rest.
    pub(super) fn colors(&self, palette: &Palette) -> (Option<Token>, Token, Option<Token>, Token) {
        // Before the pressed looks: a lit bolt is reported pressed but keeps its own look.
        match self.tier {
            Tier::QuickMark { lit } => {
                let ink = if lit { palette.warn } else { palette.fg_2 };
                return (None, palette.control_hover, None, ink);
            }
            Tier::NumberKey { bound } => {
                let ink = if bound { palette.accent } else { palette.fg_2 };
                return (None, palette.control_hover, None, ink);
            }
            _ => {}
        }
        if self.pressed == Some(true) && self.quiet {
            return (
                Some(palette.control_hover),
                palette.control_hover,
                Some(palette.accent),
                palette.fg_0,
            );
        }
        if self.pressed == Some(true) && self.tier == Tier::Nav {
            return (
                Some(palette.accent_fill),
                palette.accent_fill,
                Some(palette.accent_edge),
                palette.on_fill,
            );
        }
        if self.pressed == Some(true) && matches!(self.tier, Tier::IconChoice | Tier::TemplateRow) {
            // In Dark appearance the usual edge is the same color as the fill. The stronger edge keeps the
            // selected template distinct from the dialog even when its editor has keyboard focus.
            let edge = if self.tier == Tier::TemplateRow {
                palette.accent
            } else {
                palette.accent_edge
            };
            return (
                Some(palette.accent_fill),
                palette.accent_fill_hover,
                Some(edge),
                palette.on_fill,
            );
        }
        if self.pressed == Some(true) && self.tier == Tier::ColorChoice {
            return (
                Some(palette.accent_fill),
                palette.accent_fill,
                Some(palette.on_fill),
                palette.on_fill,
            );
        }
        if self.pressed == Some(true) {
            return (
                Some(palette.accent_fill),
                palette.accent_fill_hover,
                None,
                palette.on_fill,
            );
        }
        // A muted caret reads as retired until its menu opens, and keeps a hover fill, since a press still opens it.
        if self.expanded != Some(true)
            && self.tier == Tier::Neutral
            && self.split == Some(SplitPart::Caret { muted: true })
        {
            return (
                Some(palette.bg_2),
                palette.control_hover,
                Some(palette.border_dim),
                palette.fg_2,
            );
        }
        // After the pressed looks: a button that is both a choice in force and open (the host rail's Hosts while host
        // management shows) keeps its pressed look, as the old sheet drew it. The readout's update toggle had no open
        // look at all: it keeps the look of its text while its menu is open.
        if self.expanded == Some(true)
            && self.open_look
            && !matches!(self.tier, Tier::Readout { .. })
        {
            return (
                Some(palette.accent_fill_hover),
                palette.accent_fill_hover,
                None,
                palette.on_fill,
            );
        }
        let retired = (
            Some(palette.bg_2),
            palette.bg_2,
            Some(palette.border_dim),
            palette.fg_2,
        );
        match self.tier {
            Tier::Ghost
            | Tier::RowMenu { on_fill: false }
            | Tier::FieldRemove
            | Tier::TerminalTab
            | Tier::RecentSetup => (None, palette.control_hover, None, palette.fg_1),
            Tier::TerminalTabClose => (None, palette.danger_bg_dim, None, palette.fg_1),
            // Returned above, before the pressed looks.
            Tier::QuickMark { .. } | Tier::NumberKey { .. } => {
                (None, palette.control_hover, None, palette.fg_2)
            }
            Tier::RowMenu { on_fill: true } => {
                (None, palette.accent_fill_hover, None, palette.on_fill)
            }
            Tier::OnBanner => (
                None,
                palette.danger_fill_hover,
                Some(palette.danger_soft),
                palette.on_fill,
            ),
            // No hover change: the old entries were bare buttons, without the `.btn` hover fill.
            Tier::BrowseRow => (
                Some(palette.well),
                palette.well,
                Some(palette.border),
                palette.fg_0,
            ),
            Tier::Neutral | Tier::Chip if self.busy => retired,
            Tier::Neutral | Tier::Chip => (
                Some(palette.bg_2),
                palette.control_hover,
                Some(palette.border),
                palette.fg_0,
            ),
            Tier::Rail => (None, palette.bg_2, None, palette.fg_1),
            Tier::Waiting => (
                Some(palette.bg_2),
                palette.control_hover,
                Some(palette.warn),
                palette.fg_0,
            ),
            Tier::Nav | Tier::TemplateRow => (None, palette.control_hover, None, palette.fg_0),
            Tier::IconChoice => (
                Some(palette.well),
                palette.control_hover,
                Some(palette.border_dim),
                palette.fg_0,
            ),
            Tier::ColorChoice => (None, theme::TRANSPARENT, None, palette.fg_0),
            Tier::Tunnel => (
                Some(palette.tunnel_bg),
                palette.tunnel_bg,
                Some(palette.tunnel_border),
                palette.tunnel_fg,
            ),
            Tier::Link => (None, theme::TRANSPARENT, None, palette.link),
            Tier::Readout { .. } => (None, theme::TRANSPARENT, None, palette.fg_1),
            Tier::HostUpdate { required } => {
                let ink = if required {
                    palette.danger_strong
                } else {
                    palette.warn
                };
                (None, palette.control_hover, Some(ink), ink)
            }
            Tier::Primary if self.disabled || self.busy || self.refused => retired,
            Tier::Danger if self.disabled || self.busy || self.refused => retired,
            Tier::Primary => (
                Some(palette.accent_fill),
                palette.accent_fill_hover,
                Some(palette.accent_edge),
                palette.on_fill,
            ),
            Tier::Danger => (
                Some(palette.danger_bg),
                palette.danger_fill_hover,
                Some(palette.danger_soft),
                palette.on_fill,
            ),
            Tier::DangerOutline if self.disabled => retired,
            Tier::DangerOutline => (
                None,
                palette.danger_bg_dim,
                Some(palette.danger),
                palette.danger,
            ),
            Tier::PrimaryOutline if self.disabled => retired,
            Tier::PrimaryOutline => (
                None,
                palette.control_hover,
                Some(palette.accent),
                palette.accent,
            ),
        }
    }

    /// The fill the button draws, from the fills [`Button::colors`] gives it: `hovered` says the pointer is over an
    /// active button, `focused` that an enabled one shows the keyboard ring. The pointer draws the hover fill, and so
    /// does the ring, which the old sheet gave every `.btn` (`.btn:focus-visible`) so a ghost button's box shows. These
    /// keep their resting fill under the ring instead, as the old sheet drew them (decided by the migration's
    /// orchestrator on 2026-11-16 for the tab and 2026-11-17 for the rest):
    ///
    /// - the shown terminal's tab, even with the pointer over it, so a keyboard user can still tell which terminal is
    ///   shown (`.tab.selected:focus-visible { background: var(--accent-fill) }`, which outranked its hover rule);
    /// - a template row and a host icon choice, chosen or not, whose old buttons were not `.btn` and whose focus rules
    ///   drew only the ring, so the pointer still draws their hover fill over it;
    /// - a quick-launch bolt, lit or not, also not a `.btn`, which drew no fill at all (the pointer still draws the
    ///   hover fill, the departure [`Tier::QuickMark`] describes);
    /// - the Settings section list's entries, which drew the ring alone, and the readout's controls but the update-menu
    ///   toggle (see [`Tier::Readout`]).
    ///
    /// NOTE: The unit tests call this directly. Its two call sites in `render` (the background away from the pointer
    /// and the hover style) have no render-level test: nothing in the test harness reads a drawn background, and no
    /// golden draws any of the tiers above focused in the state listed. A change there must keep passing `focused` to
    /// both.
    pub(super) fn fill(&self, palette: &Palette, hovered: bool, focused: bool) -> Option<Token> {
        let (rest, hover, _, _) = self.colors(palette);
        match self.tier {
            Tier::TerminalTab if self.pressed == Some(true) && focused => rest,
            _ if hovered => Some(hover),
            _ if !focused => rest,
            Tier::Nav | Tier::Readout { focus_fill: false } | Tier::QuickMark { .. } => rest,
            Tier::TemplateRow | Tier::IconChoice => rest,
            Tier::Readout { focus_fill: true } => Some(palette.control_hover),
            _ => Some(hover),
        }
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let (_, _, edge, text) = self.colors(palette);
        let radius = match self.tier {
            Tier::ColorChoice => px(COLOR_CHOICE_SIZE / 2.),
            Tier::Waiting => metrics::RADIUS_MD,
            Tier::RecentSetup => px(0.),
            _ => metrics::RADIUS_SM,
        };
        let bordered = !matches!(self.tier, Tier::Link | Tier::Readout { .. });
        let focus = self.focus.clone().unwrap_or_else(|| {
            focus::control_handle(
                ElementId::Name(format!("{}-focus", self.id).into()),
                window,
                cx,
            )
        });
        let ring = focus::shows_ring(&focus, window, cx);
        // The lifecycle buttons' condensed look: smaller text and line, no side padding, a narrower gap.
        let words = self.compact == Some(Compact::Condensed(Condensed::Words));
        // `.banner-reclaim` overrides `.btn`'s padding to fit inside the banner's 4-pixel band.
        let (pad_y, pad_x) = match self.tier {
            _ if self.heading => (px(HEADING_WORDS_PAD.0), px(HEADING_WORDS_PAD.1)),
            _ if self.toolbar => (px(TOOLBAR_PAD.0), px(TOOLBAR_PAD.1)),
            Tier::OnBanner => metrics::BANNER_BUTTON_PAD,
            Tier::RowMenu { .. } => (px(0.), px(ROW_MENU_PAD_X)),
            // Condensed before the compact geometry it replaces, the caret's included.
            _ if words => (px(CONDENSED_PAD), px(0.)),
            _ if self.compact == Some(Compact::Condensed(Condensed::Padding)) => {
                (px(CONDENSED_PAD), px(CONDENSED_PAD))
            }
            _ if self.compact.is_some() && matches!(self.split, Some(SplitPart::Caret { .. })) => {
                (px(COMPACT_PAD.0), px(CARET_PAD_X))
            }
            Tier::Ghost | Tier::Neutral | Tier::Primary | Tier::Danger
                if self.compact.is_some() =>
            {
                (px(COMPACT_PAD.0), px(COMPACT_PAD.1))
            }
            Tier::Rail
            | Tier::Link
            | Tier::Readout { .. }
            | Tier::IconChoice
            | Tier::FieldRemove => (px(0.), px(0.)),
            Tier::ColorChoice => (px(COLOR_CHOICE_PAD), px(COLOR_CHOICE_PAD)),
            Tier::Chip => (px(CHIP_PAD.0), px(CHIP_PAD.1)),
            Tier::Nav => (px(NAV_PAD.0), px(NAV_PAD.1)),
            Tier::QuickMark { .. } => (px(QUICK_MARK_PAD.0), px(QUICK_MARK_PAD.1)),
            Tier::NumberKey { .. } => (px(NUMBER_KEY_PAD), px(NUMBER_KEY_PAD)),
            Tier::Tunnel => (px(TUNNEL_PAD.0), px(TUNNEL_PAD.1)),
            Tier::TerminalTab => (px(TERMINAL_TAB_PAD.0), px(TERMINAL_TAB_PAD.1)),
            Tier::TerminalTabClose => (px(TERMINAL_TAB_CLOSE_PAD.0), px(TERMINAL_TAB_CLOSE_PAD.1)),
            Tier::TemplateRow => (px(TEMPLATE_ROW_PAD.0), px(TEMPLATE_ROW_PAD.1)),
            Tier::RecentSetup => (px(0.), px(RECENT_SETUP_PAD_X)),
            Tier::HostUpdate { .. } => (px(HOST_UPDATE_PAD.0), px(HOST_UPDATE_PAD.1)),
            Tier::Waiting => (px(WAITING_PAD.0), px(WAITING_PAD.1)),
            Tier::Ghost
            | Tier::Neutral
            | Tier::Primary
            | Tier::Danger
            | Tier::DangerOutline
            | Tier::PrimaryOutline
            | Tier::BrowseRow => metrics::BUTTON_PAD,
        };
        // A fixed square centers what it shows instead of padding it.
        let (pad_y, pad_x) = if self.side.is_some() {
            (px(0.), px(0.))
        } else {
            (pad_y, pad_x)
        };
        let pressed = self.pressed == Some(true);
        let ring_color = match self.tier {
            // The quiet look's fill is light in the light palette, where the inverse ring would not show.
            _ if pressed && self.quiet => palette.accent,
            _ if pressed => palette.on_fill,
            Tier::OnBanner | Tier::RowMenu { on_fill: true } => palette.on_fill,
            Tier::Ghost
            | Tier::Neutral
            | Tier::Primary
            | Tier::Danger
            | Tier::DangerOutline
            | Tier::PrimaryOutline
            | Tier::Rail
            | Tier::Chip
            | Tier::Link
            | Tier::Readout { .. }
            | Tier::Nav
            | Tier::QuickMark { .. }
            | Tier::NumberKey { .. }
            | Tier::TemplateRow
            | Tier::RecentSetup
            | Tier::BrowseRow
            | Tier::FieldRemove
            | Tier::IconChoice
            | Tier::ColorChoice
            | Tier::Tunnel
            | Tier::TerminalTab
            | Tier::TerminalTabClose
            | Tier::HostUpdate { .. }
            | Tier::Waiting
            | Tier::RowMenu { on_fill: false } => palette.accent,
        };
        // An icon-only button is otherwise as high as its icon plus its padding and square; the row menu's box is the
        // old toggle's, set by its text line and its own side padding; a rail entry is a fixed square.
        let (pad_x, height) = match self.tier {
            _ if self.side.is_some() => (pad_x, self.side),
            _ if self.toolbar => (pad_x, Some(px(TOOLBAR_HEIGHT))),
            Tier::RowMenu { .. } => (pad_x, Some(px(ROW_MENU_HEIGHT))),
            Tier::Rail => (pad_x, Some(px(RAIL_SIZE))),
            // The bolt keeps its tall, narrow box (`padding: 10px 5px`), not the square an icon-only button gets.
            Tier::QuickMark { .. } => (pad_x, None),
            Tier::IconChoice => (pad_x, Some(px(ICON_CHOICE_SIZE))),
            Tier::ColorChoice => (pad_x, Some(px(COLOR_CHOICE_SIZE))),
            _ if self.show_label || self.content.is_some() => (pad_x, None),
            // A split button's caret keeps its own side padding, and takes its height from the main part beside it.
            _ if matches!(self.split, Some(SplitPart::Caret { .. })) => (pad_x, None),
            _ => (pad_y, None),
        };
        // A rail entry, an appearance choice and a button given a side are squares that never give up their width; an
        // expanded rail's row is as wide as its column instead.
        let rail_row = self.rail_row && self.tier == Tier::Rail;
        let square = self.side.is_some()
            || (self.tier == Tier::Rail && !rail_row)
            || matches!(self.tier, Tier::IconChoice | Tier::ColorChoice);
        // `.btn.host-update-button`'s `flex: 0 0 auto`: the row's name gives way first.
        let rigid = matches!(
            self.tier,
            Tier::HostUpdate { .. } | Tier::TerminalTab | Tier::TerminalTabClose
        );
        let enabled = !self.disabled;
        // A busy button stays focusable (`enabled`), but nothing it does acts then (`active`).
        let active = enabled && !self.busy;
        let handler = self.on_click.clone().filter(|_| active);
        // A busy button still takes Enter and Space, doing nothing with them, so a press while busy does nothing at
        // all, rather than going on up to whatever holds the button, as it would from a button that does not take
        // them.
        let key_handler: Option<Handler> = if enabled && self.busy {
            Some(Rc::new(|_: &ClickEvent, _: &mut Window, _: &mut App| {}))
        } else {
            handler.clone()
        };

        let mut button = div()
            .id(ElementId::Name(self.id.clone()))
            .relative()
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(
                self.accessible_name
                    .clone()
                    .unwrap_or_else(|| self.label.clone()),
            )
            .flex()
            .items_center()
            .justify_center()
            .gap(match self.compact {
                Some(Compact::Condensed(Condensed::Words)) => CONDENSED_GAP,
                Some(_) => px(COMPACT_GAP),
                None => px(6.),
            })
            .py(pad_y)
            .px(pad_x)
            .when_some(height, |button, height| button.h(height))
            .when_some(height.filter(|_| square), |button, side| {
                button.w(side).flex_none()
            })
            .when(rigid, |button| button.flex_none())
            .when(rail_row, |button| {
                button.w_full().flex_none().justify_start()
            })
            // A chip yields its width to the row it sits in (`min-width: 0; max-width: 100%`), and its content cuts
            // the host's name short.
            .when(self.tier == Tier::Chip, |button| {
                button.min_w_0().max_w_full()
            })
            .when_some(self.expanded, |button, expanded| {
                button.aria_expanded(expanded)
            })
            .when_some(self.pressed, |button, pressed| {
                button.aria_toggled(if pressed {
                    Toggled::True
                } else {
                    Toggled::False
                })
            })
            .rounded(radius)
            // Where a split button's two parts meet, their corners are square.
            .when(self.split == Some(SplitPart::Main), |button| {
                button.rounded_tr(px(0.)).rounded_br(px(0.))
            })
            .when(
                matches!(self.split, Some(SplitPart::Caret { .. })),
                |button| button.rounded_tl(px(0.)).rounded_bl(px(0.)),
            )
            // `.inline-link` and the readout have no edge at all (`border: 0`), so they sit on the text line like the
            // words around them; every other tier keeps the 1-pixel edge.
            .when(bordered, |button| button.border_1())
            .when(self.tier == Tier::Link, |button| button.underline())
            .text_size(metrics::TEXT_SM)
            .line_height(px(LINE))
            .when(words, |button| {
                button
                    .text_size(px(CONDENSED_TEXT))
                    .line_height(px(CONDENSED_LINE))
            })
            .text_color(theme::color(text))
            // A transparent edge where a tier has none keeps every tier the same size.
            .border_color(theme::color(edge.unwrap_or(theme::TRANSPARENT)))
            // `.tab-close:disabled { opacity: 0.5 }`: a × whose tab is closing, or that cannot be pressed, reads so.
            .when(
                self.tier == Tier::TerminalTabClose && (self.busy || self.disabled),
                |button| button.opacity(0.5),
            )
            // `.session-tunnel-toggle`: `gap: 3px`, `font-size: 11px`, `line-height: 1.4`, `border-radius: 8px`.
            .when(self.tier == Tier::Tunnel, |button| {
                button
                    .gap(px(3.))
                    .text_size(px(11.))
                    .line_height(px(15.))
                    .rounded(px(8.))
            })
            .when(self.tier == Tier::Nav, |button| {
                button
                    .justify_start()
                    .gap(px(8.))
                    .min_w_0()
                    .text_size(metrics::TEXT_MD)
                    .line_height(px(NAV_LINE_HEIGHT))
            })
            // The waiting header fills the region's width and lays its content out from the left (`text-align:
            // left`), at the browser's default button size with its `normal` line height.
            .when(self.tier == Tier::Waiting, |button| {
                button
                    .justify_start()
                    .w_full()
                    .min_w_0()
                    .text_size(px(WAITING_TEXT))
                    .line_height(px(16.))
            })
            // A template's row fills its row's width and lays its lines out from the left (`text-align: left`); the
            // lines may shrink, so a long name or summary is cut rather than widening the list.
            .when(self.tier == Tier::TemplateRow, |button| {
                button.w_full().min_w_0().justify_start()
            })
            // A recent setup's row fills the strip's well at a fixed height, square-cornered inside the well's own
            // rounding, its cells laid out from the left at the strip's 13 pixels.
            .when(self.tier == Tier::RecentSetup, |button| {
                button
                    .w_full()
                    .min_w_0()
                    .h(px(RECENT_SETUP_HEIGHT))
                    .justify_start()
                    .text_size(metrics::TEXT_MD)
            })
            .when(self.tier == Tier::BrowseRow, |button| {
                button
                    .w_full()
                    .min_w_0()
                    .min_h(px(BROWSE_ROW_MIN_HEIGHT))
                    .text_size(metrics::TEXT_ROW_TITLE)
                    .font_weight(FontWeight::SEMIBOLD)
            })
            .when(self.tier == Tier::FieldRemove, |button| {
                button
                    .w(px(FIELD_REMOVE_SIZE.0))
                    .h(px(FIELD_REMOVE_SIZE.1))
                    .flex_none()
            });
        // The fill away from the pointer, and (for an active button, below) the one under it: both from `fill`, which
        // says how keyboard focus changes each.
        let focused = ring && enabled;
        button = button.when_some(self.fill(palette, false, focused), |button, fill| {
            button.bg(theme::color(fill))
        });
        if ring && pressed {
            // A pressed button's ring is the inverse ink inside its edge (`outline-color: var(--on-fill);
            // outline-offset: -4px`), as the old rail drew it: the accent ring would not read against the accent fill.
            // The old navigation entries drew it a pixel further out (`outline-offset: -3px`). So did the chosen color
            // swatch, whose ring is round: a browser's outline follows the button's radius, so the old ring was a
            // circle inside the swatch's round edge. Here the ring box is the circle's side less the 1-pixel edge and
            // the inset on each side, and its radius is half that, or the ring is a small square cut into the circle.
            let (inset, radius) = match self.tier {
                Tier::Nav => (px(1.), metrics::RADIUS_SM - px(1.)),
                Tier::ColorChoice => (px(1.), px(COLOR_CHOICE_SIZE / 2. - 2.)),
                _ => (px(2.), px(1.)),
            };
            button = button.child(
                div()
                    .absolute()
                    .top(inset)
                    .left(inset)
                    .right(inset)
                    .bottom(inset)
                    .border_2()
                    .border_color(theme::color(ring_color))
                    .rounded(radius),
            );
        } else if ring && self.tier == Tier::RecentSetup {
            // A recent setup's row fills a well that clips what lies outside it, so its ring sits inside its edge, as
            // the old one did (`box-shadow: inset 0 0 0 1.5px var(--accent)`), square like the row.
            button = button.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .border_2()
                    .border_color(theme::color(ring_color)),
            );
        } else if ring {
            // The old `outline: 2px solid var(--accent); outline-offset: 1px`, drawn as a ring element because GPUI has
            // no outline: two pixels wide, one pixel clear of the button's edge.
            button = button.child(
                div()
                    .absolute()
                    .top(px(-3.))
                    .left(px(-3.))
                    .right(px(-3.))
                    .bottom(px(-3.))
                    .border_2()
                    .border_color(theme::color(ring_color))
                    .rounded(metrics::RADIUS_SM + px(3.)),
            );
        }
        if active {
            // The field's ✕ also turns red under the pointer (`.templates-remove:hover { color: var(--danger) }`).
            // A tab's × does too (`.tab-close:hover`).
            let hover_ink = matches!(self.tier, Tier::FieldRemove | Tier::TerminalTabClose)
                .then_some(palette.danger);
            let hover_fill = self.fill(palette, true, focused);
            button = button.cursor_pointer().hover(move |style| {
                let style = match hover_fill {
                    Some(fill) => style.bg(theme::color(fill)),
                    None => style,
                };
                match hover_ink {
                    Some(ink) => style.text_color(theme::color(ink)),
                    None => style,
                }
            });
        } else if enabled && self.busy && self.tier == Tier::Neutral {
            // The old sheet's `cursor: not-allowed` on the `aria-disabled` header Restart, which still answers the
            // hover. Only the neutral tier: the old busy Save (a primary) kept the button's cursor.
            button = button.cursor_not_allowed();
        } else if !enabled
            && matches!(
                self.tier,
                Tier::Ghost
                    | Tier::Neutral
                    | Tier::OnBanner
                    | Tier::RowMenu { .. }
                    | Tier::Rail
                    | Tier::Chip
                    | Tier::Link
                    | Tier::Readout { .. }
                    | Tier::Nav
                    | Tier::QuickMark { .. }
                    | Tier::NumberKey { .. }
                    | Tier::TemplateRow
                    | Tier::RecentSetup
                    | Tier::BrowseRow
                    | Tier::FieldRemove
                    | Tier::IconChoice
                    | Tier::ColorChoice
                    | Tier::HostUpdate { .. }
                    | Tier::Waiting
            )
        {
            button = button.opacity(0.5);
        }
        if let Some(icon) = self.icon {
            button = button.child(
                svg()
                    .path(icon.path())
                    .size(px(if self.compact.is_some() {
                        COMPACT_ICON
                    } else {
                        14.
                    }))
                    .text_color(theme::color(text)),
            );
        }
        if let Some(content) = self.content {
            button = button.child(content);
        } else if self.show_label && self.tier == Tier::Nav {
            button = button.child(div().min_w_0().truncate().child(self.label));
        } else if self.show_label {
            button = button.child(self.label);
        }
        if let Some(badge) = self.badge {
            button = button.child(badge);
        }
        if let Some(on_secondary) = self.on_secondary.clone().filter(|_| active) {
            button = button.on_mouse_down(MouseButton::Right, move |event, window, cx| {
                cx.stop_propagation();
                on_secondary(event, window, cx);
            });
        }
        // Rail and list help rings sit just inside these buttons' borders. A selected button uses the same
        // inverse ink for its ring as for its words so both remain visible against the accent fill.
        let list_help = matches!(
            self.help.placement,
            crate::layers::HelpPlacement::List { .. }
        );
        let help = if self.tier == Tier::Rail || list_help {
            let inset = if bordered { px(1.) } else { px(0.) };
            let mut help = self.help.ring_radius((radius - inset).max(px(0.)));
            if text == palette.on_fill {
                help = help.ring_on_fill();
            }
            if list_help { help } else { help.beside_rail() }
        } else {
            self.help
        };
        let button =
            super::with_hover_help(button, &self.id, &help, true, Some(&focus), window, cx);
        let button = if enabled {
            let on_press = if self.keeps_focus {
                focus::OnPress::Keep
            } else {
                focus::OnPress::Control
            };
            let button = focus::track(button, &focus, on_press);
            // Registered before the activation below, so the owner's handler sees each key first.
            match self.on_key {
                Some(on_key) => {
                    button.on_key_down(move |event, window, cx| on_key(event, window, cx))
                }
                None => button,
            }
        } else {
            // Not tracked, so neither Tab nor a click can focus it; a press of any mouse button is still this
            // control's, so it does not reach the page's empty-space handler and move focus.
            button.on_any_mouse_down(|_, window, _| window.prevent_default())
        };
        super::activate_on_click(
            super::activate_on_enter_or_space(button, key_handler),
            handler,
        )
    }
}

/// A button that shows only an icon. Its label is still required: it is the accessible name, and the hover help
/// carries the words a sighted user would otherwise lack.
#[derive(IntoElement)]
pub struct IconButton(Button);

impl IconButton {
    /// An icon-only button; see [`Button::new`] for the arguments.
    pub fn new(
        id: impl Into<SharedString>,
        icon: Icon,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
    ) -> Self {
        let mut button = Button::new(id, label, help, role).icon(icon);
        button.show_label = false;
        IconButton(button)
    }

    /// See [`Button::tier`].
    pub fn tier(self, tier: Tier) -> Self {
        IconButton(self.0.tier(tier))
    }

    /// See [`Button::disabled`].
    pub fn disabled(self, disabled: bool) -> Self {
        IconButton(self.0.disabled(disabled))
    }

    /// See [`Button::pressed`].
    pub fn pressed(self, pressed: bool) -> Self {
        IconButton(self.0.pressed(pressed))
    }

    /// See [`Button::busy`]: the Templates list's bolts keep focus while the mark's write is in flight.
    pub fn busy(self, busy: bool) -> Self {
        IconButton(self.0.busy(busy))
    }

    /// See [`Button::expanded`].
    pub fn expanded(self, expanded: bool) -> Self {
        IconButton(self.0.expanded(expanded))
    }

    /// See [`Button::on_click`].
    pub fn on_click(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        IconButton(self.0.on_click(handler))
    }

    /// See [`Button::focus`].
    pub fn focus(self, handle: &FocusHandle) -> Self {
        IconButton(self.0.focus(handle))
    }

    /// See [`Button::square`].
    pub fn square(self, side: Pixels) -> Self {
        IconButton(self.0.square(side))
    }

    /// See [`Button::badge`]: the Hosts control's `↑`.
    pub fn badge(self, badge: impl IntoElement) -> Self {
        IconButton(self.0.badge(badge))
    }

    /// See [`Button::keeps_focus_on_press`].
    pub fn keeps_focus_on_press(self) -> Self {
        IconButton(self.0.keeps_focus_on_press())
    }

    /// See [`Button::compact`]: the session header's split carets.
    pub fn compact(self) -> Self {
        IconButton(self.0.compact())
    }

    /// See [`Button::condensed`]: the session header's carets while its row is short of width.
    pub fn condensed(self, condensed: Option<Condensed>) -> Self {
        IconButton(self.0.condensed(condensed))
    }

    /// See [`Button::split`].
    pub fn split(self, part: SplitPart) -> Self {
        IconButton(self.0.split(part))
    }

    /// See [`Button::on_key_down`]: a caret that opens its menu with Down and Up.
    pub fn on_key_down(
        self,
        handler: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        IconButton(self.0.on_key_down(handler))
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.0.render(window, cx)
    }
}
