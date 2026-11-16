//! The quick switcher: Cmd+K on a Mac, Ctrl+Shift+K on Linux, a dialog that finds any session by typing part of its
//! title, host or folder and opens the one the user picks.
//!
//! SPEC.md, "Desktop window chrome" (the Cmd+K paragraph). Ported from the old app's `list/quick_switcher.rs` and the
//! `.quick-switcher-*` rules in its stylesheet at reference commit `24bc0580`; the old app's behavior is the reference
//! where SPEC.md says less. Here's the TLDR of its rules:
//!
//! - **Opening.** The shortcut opens it from anywhere on the page, a focused terminal included (the main view handles
//!   the action on its ground, and the chord's `TERMINAL_KEYS` row keeps it from the program). It does nothing, though
//!   the key is still taken, while any dialog or menu is open, while the window is inert, and before the helm's
//!   preferences have arrived with the snapshot, which stands in for SPEC.md's "before preferences finish loading".
//! - **One read per opening.** Opening sends one `GET /api/sessions?sort=activity` with no filter, because the
//!   switcher searches every host whatever the sidebar shows, and matches locally over that snapshot as the user
//!   types. The snapshot does not follow pushed changes while the switcher is open, as the old one did not. Each
//!   opening is a switcher of its own, and its read's reply holds it only weakly, so a reply that arrives after its
//!   switcher closed reaches nobody, and never a switcher opened since. The read is a
//!   plain command, not an operation: nothing waits on it but this dialog. Until it answers the switcher says
//!   "Loading sessions…" and Enter picks nothing; a refusal says "Couldn't load sessions:" and the helm's words.
//! - **Matching.** The old matcher exactly ([`matching_positions`], [`ranked`]): the trimmed query as a
//!   case-insensitive subsequence (lowercase mapping, not full case folding) of the title, the row's `host_name` or
//!   its raw working directory; title matches first, then matches only in the host or the directory, each group in
//!   the helm's activity order. Matching reads the raw values, so how a value is escaped on screen never changes
//!   which sessions match.
//! - **Marks.** A value whose raw form matched has its matched characters drawn in the accent at weight 600 (the old
//!   `mark`; the fill's ink on the selected row), found again on the escaped spelling the user sees
//!   ([`mark_positions`], [`host_text::marked`]). A value that did not match is never marked; one that matched only
//!   through a character shown as an escape marks less or nothing, since no escape is drawn partly marked.
//! - **Host marks.** A session row draws its host's chosen mark before the host's name, the sidebar row's own mark
//!   ([`row_marks::HostMark::of`], drawn by [`row_marks::host_mark`]) in the `--on-fill` ink on the selected row. The
//!   switcher's read names each session's host only by id, so the mark comes from the registry the store holds as the
//!   row is drawn; a host the registry cannot place yet draws no mark and keeps its name, as in the sidebar.
//! - **Choosing.** The first match is selected after every edit; Up and Down move the selection, stopping at both
//!   ends, and scroll it into view. Focus stays in the search field the whole time: the rows are not focusable
//!   ([`OptionRow`]), and the field hands Up and Down over rather than moving its caret ([`TextInput::on_vertical`]).
//!   Hover only paints. Enter picks the selected row and a click picks the clicked one.
//! - **New session named X.** While the trimmed query is not empty, a row pinned below the scrolling matches offers
//!   `New session named "X"` for the trimmed query (SPEC.md's "New session named X" paragraph; the old row's words,
//!   escaped as host text, and its hover help). A query that starts with `tl:`, in any case, offers no New row: SPEC.md
//!   replaces sessions and the New row with templates there (below), so that a template search never names a session
//!   `tl:…`. It comes after the last match in arrow-key order, so it is selected when nothing matches, a failed read
//!   included, but the arrows cannot reach it and Enter picks nothing while the read is loading (the old switcher had
//!   no selection at all until the reply). A click picks it even then, as the old row's did. Picking it closes the
//!   switcher first, since New refuses while any dialog is open, which returns focus to the switcher's opener; New then
//!   opens with that name ([`super::open_new`]) and takes the opener as its own, so closing New returns focus to where
//!   it was before the switcher opened. A refused New (a launch pending) leaves focus there and opens nothing, as the
//!   old switcher did.
//! - **Picking.** A pick closes the switcher, then opens the session through the list's own [`selection::choose`], so
//!   it is selected, remembered with the helm and its terminal takes focus, the already open session's included
//!   (SPEC.md, "Where focus rests": "Choosing a session, including the one already selected, focuses its terminal").
//!   It closes first because a terminal never takes focus from inside a layer ([`crate::focus::choose_terminal`]). A
//!   pick goes ahead while a session action is pending; nothing here asks the operation store. Once the session is
//!   open, the session list returns from the Hosts panel to Sessions and, if its host or status filter hides the
//!   pick, resets both to All (`SessionList::picked_from_switcher`; SPEC.md: "Opening a session hidden by the host
//!   choice or status filter resets both to All after the open is accepted", and "All or a host avatar returns to
//!   Sessions, as does an accepted quick-switcher pick").
//! - **Closing.** Escape, Close and a press outside the card close it and return focus to what held it when it opened
//!   ([`ReturnTo::Opener`]). The veil takes that press, so it never also reaches the row or terminal under it.
//! - **Templates.** A query that starts with `tl:` in any ASCII case ([`template_rows::template_query`]) turns the
//!   card into the templates section: "Templates" for the heading, the templates whose names hold the rest of the
//!   query in the helm's order ([`template_rows::rows`], the launcher's own template search) in place of the
//!   sessions, and no New or Quick launch row. Each row draws the name and summary only, as the old section did; the
//!   ⚡ and number-key badges were the Cmd+J list's. No match says "No templates match", and a template collection
//!   the helm cannot read says "Couldn't load templates:" and the helm's words in place of the rows. The templates are
//!   pushed state, read from the store on every frame, so they need no read of their own: the section works while
//!   the session read is loading or after it failed (SPEC.md: "After a failed session read, New and template search
//!   remain usable"). The arrows and Enter walk the rows as they walk sessions, and a pick closes the switcher, then
//!   hands the name to [`template_rows::pick_from_switcher`], which opens New with the template applied and never
//!   launches; it does nothing while a dialog holds the keyboard, hence the order. A pick that opened New returns the
//!   list from the Hosts panel, as the New row's does, since both are New handoffs.
//! - **Quick launch.** With nothing typed, a row pinned below the matches offers "Quick launch: " and the ⚡ template's
//!   name, or "Mark a template" ([`template_rows::quick_launch_row_label`]), with the ⚡ chord beside it. It is placed
//!   and reached as the New row is (the two never show together): after the last match in arrow-key order, so the
//!   first recent session stays selected, unreachable while the read is loading, and selected after a failed one. A
//!   pick closes the switcher, then runs [`template_rows::quick_launch_from_switcher`], the ⚡ chord's own path:
//!   it launches unseen, opens New for a template that needs more choices, or opens Templates to mark one. A launch
//!   it started or a New it opened returns the list from the Hosts panel, as the old switcher's did (its quick launch
//!   went through the list's `open_new`, which hid the panel); Templates opened to mark one leaves the panel shown.
//! - **The templates hint.** A quiet section above the footer names `tl:`: "Templates" and "Type tl: to search
//!   templates. Picking one opens New with it applied." normally, and in the templates section only "Picking a
//!   template opens New with it applied; nothing launches yet.", in the old words.
//! - **The template list (Cmd+J).** The same dialog in its templates-only mode ([`Mode::Templates`]; SPEC.md, the
//!   Cmd+J paragraph), as the old app's `SwitcherMode::Templates` was: opened by its own chord under the same refusals
//!   and one more, any launcher held out of sight ([`open`] says which), with the same focus, Escape and outside-press
//!   rules, and named "Template launcher". It sends no session read and shows no session, New or Quick launch row: the
//!   whole query, with no prefix, filters templates as `tl:` does, and each row adds the template's ⚡ and number-key
//!   chords to its name and summary. A template named exactly the trimmed query (case-sensitively) is selected over an
//!   earlier match until the arrows choose another row, so typing a whole name and pressing Enter starts that template
//!   even when another name contains it. The selection is kept as a template's name, not a row's place, because the rows
//!   follow the templates the helm pushes while the list is open and a pick launches unseen. What is drawn selected is
//!   latched on every frame, so a push that adds or removes a row never moves the choice to another template, and a
//!   chosen template that goes stays chosen, drawn on no row ([`QuickSwitcher::chosen`]). A pick closes the list, then
//!   quick launches the chosen name through [`template_rows::launch_from_list`]: it starts unseen, or opens New with the
//!   template applied when it needs more choices, or with a refusal when no template has that name any more (SPEC.md:
//!   "a name missing from the current catalog shows a preparation error in New"), even when the list then says "No
//!   templates match". With nothing chosen, Enter does nothing. The hint says "Enter starts the highlighted template. A
//!   partial template opens New to complete it."
//!
//! The card is drawn here rather than in the shared dialog frame, because the old switcher had neither the frame's
//! title header nor its answer footer: one padded column on the dialog surface, whose results list is the only part
//! that scrolls. The layer manager places it near the window's top and caps its height
//! ([`DialogPlacement::Top`]); the card keeps the old side margin by capping its own width.
//!
//! NOTE: The old layout for windows narrower than 600 pixels is not ported, because the window's minimum width is 720.

#[cfg(test)]
mod template_list_tests;
#[cfg(test)]
mod tests;

use farhelm_helm_client::ApiPath;
use farhelm_proto::ui_channel::{SessionList, SessionRow};
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, FontWeight, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window,
    div, prelude::FluentBuilder as _, px,
};

use super::menu::{self, AgentName};
use super::row_marks::{self, HelpPlace, HostMark};
use super::selection;
use super::status::{self, Badge};
use crate::components::{
    Button, HoverHelp, OptionRow, Role, TextInput, TextInputEvent, TextInputState, Vertical,
    dialog_shadow,
};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::launcher::entry;
use crate::layers::{self, Dialog, DialogPlacement, LayerId};
use crate::session;
use crate::templates::rows::{self as template_rows, Row as TemplateRow};
use crate::templates::summary;
use crate::templates::{self, QUICK_LAUNCH_SHORTCUT_NAME};
use crate::theme::{self, Palette, metrics};
use crate::{assets, operations, shortcuts, ticker};

// ---- Layout: the old `.quick-switcher-*` rules ------------------------------------------------------------------------

/// The card's widest (`width: min(760px, 100%)`).
const CARD_MAX_WIDTH: Pixels = px(760.);
/// What the card leaves free on each side of a narrower window: the old backdrop's 16-pixel side padding, which the
/// layer manager leaves to the view. (Its height cap, `max-height: 75dvh`, and its place near the top are the layer
/// manager's: `DialogPlacement::Top`.)
const CARD_MARGIN: Pixels = px(16.);
/// The card's padding and the gap between its parts.
const CARD_PAD: Pixels = px(16.);
const CARD_GAP: Pixels = px(10.);
/// A row's fixed columns (`grid-template-columns: 24px minmax(0, 2fr) 70px minmax(0, 1fr) minmax(0, 2fr)`), the gap
/// between them, and the row's own padding and edge, which the flexible columns share what is left after.
const STATUS_COLUMN: Pixels = px(24.);
const AGENT_COLUMN: Pixels = px(70.);
const COLUMN_GAP: Pixels = px(10.);
const ROW_CHROME: Pixels = px(22.);
/// `.quick-switcher-host`'s gap between the host's mark and its name.
const HOST_MARK_GAP: Pixels = px(6.);

/// A result row's host mark id prefix; the session's key follows it, then [`row_marks::host_mark`]'s own `-host`
/// (`quick-switcher-session-<key>-host`). Not the row's own prefix, so that every id starting with
/// `quick-switcher-row-` still names a row, as the tests find rows by it.
const HOST_MARK_ID_PREFIX: &str = "quick-switcher-session-";

/// The words of the cap notice, shown when the helm's listing cap cut the snapshot.
pub(crate) const CAP_NOTICE: &str =
    "Only the most recently active sessions were searched; the helm's listing limit was reached.";

/// The New row's element id (the old `.quick-switcher-new`).
pub(crate) const NEW_ROW_ID: &str = "quick-switcher-new";

/// The New row's hover help, the old row's `data-tooltip`.
pub(crate) const NEW_ROW_HELP: &str = "Open New with this name; nothing launches yet";

/// The Quick launch row's element id (the old `.quick-switcher-quick-launch`).
pub(crate) const QUICK_ROW_ID: &str = "quick-switcher-quick-launch";

/// The Quick launch row's hover help, the old row's `data-tooltip`.
pub(crate) const QUICK_ROW_HELP: &str =
    "Quick launch: start the marked template directly, or open Templates to mark one";

/// A template row's element id prefix (the old `.quick-switcher-template`); the template's name follows it.
pub(crate) const TEMPLATE_ROW_ID: &str = "quick-switcher-template-";

/// A template row's hover help, the old section's `data-tooltip`.
pub(crate) const TEMPLATE_ROW_HELP: &str =
    "Open New with this template applied; nothing launches yet";

/// A template row's hover help in the template list, the old templates-only mode's `data-tooltip`: there a pick starts
/// the template rather than opening New.
pub(crate) const TEMPLATE_LIST_ROW_HELP: &str = "Start a session from this template";

/// `.quick-switcher-template-keys`: the gap between a template row's key chips.
const KEY_CHIP_GAP: Pixels = px(5.);

// ---- Matching -----------------------------------------------------------------------------------------------------------

/// Where `query` occurs in `text` as a case-insensitive subsequence, as the original characters' indices; `None` when
/// it does not. Ported unchanged from the old matcher, whose rules are the spec.
///
/// Both sides are compared through `char::to_lowercase`, which is lowercase mapping, not full case folding or
/// Unicode normalization. Each query character takes the first matching character after the previous one (leftmost,
/// greedy), so of several alignments the earliest wins. A character that lowercases to several (`İ` becomes `i` and a
/// combining dot) is listed once. An empty query matches everything, at no positions.
pub(crate) fn matching_positions(text: &str, query: &str) -> Option<Vec<usize>> {
    let mut folded = text
        .chars()
        .enumerate()
        .flat_map(|(index, ch)| ch.to_lowercase().map(move |ch| (index, ch)));
    let mut positions = Vec::new();
    for wanted in query.chars().flat_map(char::to_lowercase) {
        let (index, _) = folded.find(|(_, ch)| *ch == wanted)?;
        if positions.last() != Some(&index) {
            positions.push(index);
        }
    }
    Some(positions)
}

/// The characters to mark in `shown`, the escaped spelling the user sees of the value `raw`: none unless `query` matches
/// `raw` itself (the value took part in the match), and then where `query` occurs in `shown`, which may be nowhere. A
/// value is matched on its raw form ([`ranked`]) but marked on what is shown, so a title found by typing an invisible
/// character, which shows as its `<U+…>` escape, is listed with nothing marked, and a value that did not match is not
/// marked because its escapes happen to spell the query, as in the old switcher.
pub(crate) fn mark_positions(raw: &str, shown: &str, query: &str) -> Vec<usize> {
    if matching_positions(raw, query).is_none() {
        return Vec::new();
    }
    matching_positions(shown, query).unwrap_or_default()
}

/// The sessions of `sessions` (the helm's activity order) that `query` finds, as indices into it, in the order the
/// switcher lists them: every title match first, then every session that matches only in its host's name or its
/// working directory, each group keeping the helm's order. No score and no other sort: a scattered title match still
/// comes before a contiguous match in a folder, and among title matches the most recently active comes first.
///
/// The query is trimmed first. Matching reads the raw values (the title, `host_name` and `cwd` as the host wrote
/// them), so a hidden character in a title is found by typing it, and typing its escaped spelling (`<U+200B>`) finds
/// nothing.
pub(crate) fn ranked(sessions: &[SessionRow], query: &str) -> Vec<usize> {
    let query = query.trim();
    let mut titles = Vec::new();
    let mut elsewhere = Vec::new();
    for (index, row) in sessions.iter().enumerate() {
        if matching_positions(&row.info.title, query).is_some() {
            titles.push(index);
        } else if matching_positions(&row.host_name, query).is_some()
            || matching_positions(&row.info.cwd, query).is_some()
        {
            elsewhere.push(index);
        }
    }
    titles.extend(elsewhere);
    titles
}

// ---- Opening ------------------------------------------------------------------------------------------------------------

/// Which list the dialog is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The quick switcher (Cmd+K): sessions, the New or Quick launch row, and templates for a `tl:` query.
    Sessions,
    /// The template list (Cmd+J): templates only, filtered by the whole query, a pick starting the template (the module
    /// docs' "The template list").
    Templates,
}

/// The shortcuts ([`crate::actions::QuickSwitcher`] for the switcher, [`crate::actions::TemplateList`] for the template
/// list, as `mode` says): open the dialog on top of the window's page. The main view calls it for the action wherever
/// focus is, and the action is taken whatever this does, as `ONCE_PER_PRESS` requires. `list` is the window's session
/// list, which a pick asks to show what it opened.
///
/// Does nothing while a dialog or menu holds the keyboard (SPEC.md: "It does nothing while another modal is open"; a
/// popover that never takes focus, such as a terminal link's destination display, is not a modal), while the window is
/// inert, or before the snapshot has brought the helm's preferences. For the template list SPEC.md names the modals:
/// "New, Clone, Replace with and hidden quick-launch preparation". The first three are dialogs. The last is asked of the
/// launcher ([`entry::quick_launch_available`]), which refuses whenever its slot holds a launcher out of sight: a quick
/// launch being prepared, a launch waiting for its reply, or a refused launch waiting behind its notice. That is the old
/// key listener's "preparing" context, which kept Cmd+J from opening in all three. The same answer also refuses during
/// a host change in flight, which is new: the old list opened then and only its pick was refused.
pub(crate) fn open(
    list: WeakEntity<super::SessionList>,
    mode: Mode,
    window: &mut Window,
    cx: &mut App,
) {
    // A key pressed inside a layer reaches the main view's ground too, since the overlay is drawn inside it; this is
    // what keeps a second switcher from opening over the first, or over any other dialog or menu.
    let launcher_held = match mode {
        Mode::Sessions => false,
        Mode::Templates => !entry::quick_launch_available(cx),
    };
    if layers::focus_layer_open(window, cx) || launcher_held {
        return;
    }
    let Some(current) = session::connection(cx).and_then(|c| c.read(cx).session().cloned()) else {
        return;
    };
    if selection::preferences(current.read(cx).store().read(cx).collections()).is_none() {
        return;
    }
    let mark = Mark::take(cx);
    let switcher = cx.new(|cx| QuickSwitcher::new(list, mode, window, cx));
    let start = switcher.read(cx).input.focus_handle(cx);
    // Near the window's top, and closed by a press outside the card, as the old switcher was: the layer manager's
    // placement for exactly this dialog. The press goes the way Escape goes, so focus returns to the opener.
    let Some(layer) = layers::open_dialog(
        Dialog {
            view: switcher.clone().into(),
            container: cx.focus_handle(),
            start,
            // Nothing it shows can go away under it: it searches a snapshot of its own, or the templates the store holds
            // as it draws.
            subject: None,
            // None, because the card itself never takes focus (below), so Enter on it could never be pressed.
            main_action: None,
            // The old switcher kept focus in its field whatever was clicked inside the card, rows included.
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Opener,
            on_dismiss: None,
            placement: DialogPlacement::Top,
            dismiss_on_outside_press: true,
        },
        &mark,
        window,
        cx,
    ) else {
        return;
    };
    switcher.update(cx, |switcher, _| switcher.layer = Some(layer));
    #[cfg(test)]
    cx.set_global(tests::Opened(switcher.downgrade()));
    match mode {
        Mode::Sessions => read_sessions(switcher.downgrade(), cx),
        // The template list lists no session, so it reads none; its read stays loading, unseen.
        Mode::Templates => {}
    }
}

/// Send the switcher's one read, `GET /api/sessions?sort=activity`, through the shared read helper
/// ([`operations::read`]), and hand the answer to `switcher` if it is still open. It is fetched although sessions are
/// pushed state, because the switcher searches every session in the helm's activity order up to the helm's listing
/// cap, which the pushed listing (filtered and sorted as the sidebar asks) does not hold. Failures use the helper's
/// wording. The reply holds the switcher weakly and
/// each opening makes a switcher of its own, so a reply after its switcher closed reaches nobody, and never a switcher
/// opened since. A reset retires the UI session, which drops the wait, and closes every dialog, the switcher included.
fn read_sessions(switcher: WeakEntity<QuickSwitcher>, cx: &mut App) {
    let path =
        ApiPath::api(&["sessions"]).and_then(|path| path.with_query(&[("sort", "activity")]));
    let path = match path {
        Ok(path) => path,
        // The path is fixed, so this cannot happen; were it to, the switcher says so rather than loading forever.
        Err(error) => {
            let _ = switcher.update(cx, |switcher, cx| {
                switcher.answered(Read::Failed(HostText::from_host(error.to_string())), cx)
            });
            return;
        }
    };
    // The helper sends nothing without a UI session, and then never answers; `open` returned already in that case, in
    // this same update, so a switcher that exists always has its read on the way.
    operations::read::<SessionList>(path, "GET /api/sessions", cx, move |answer, cx| {
        let read = match answer {
            Ok(list) => Read::Loaded(list),
            Err(reason) => Read::Failed(reason),
        };
        // A switcher that closed is gone, so the update fails and the reply is dropped.
        let _ = switcher.update(cx, |switcher, cx| switcher.answered(read, cx));
    });
}

// ---- The view -----------------------------------------------------------------------------------------------------------

/// Where the switcher's read stands.
enum Read {
    /// No reply yet: the switcher lists nothing and Enter picks nothing.
    Loading,
    /// The helm refused or the reply could not be read; the reason may quote a host, so it is host text.
    Failed(HostText),
    /// The listing the switcher searches until it closes.
    Loaded(SessionList),
}

/// The open switcher: its field, its read, which row is selected, and its layer.
///
/// Its rows, in arrow-key order, are the matches and then the New row (something typed) or the Quick launch row
/// (nothing typed); for a `tl:` query, and always in the template list, the templates alone ([`Rows`]).
pub(crate) struct QuickSwitcher {
    /// The window's session list, told about every pick so that it can show the session the pick opened.
    list: WeakEntity<super::SessionList>,
    /// The switcher or the template list, for the dialog's life.
    mode: Mode,
    /// The search field. Its text is the query; the switcher keeps no copy of it.
    input: TextInputState,
    read: Read,
    /// The selected row's position among the rows (the matches, then the New row), kept within them wherever it is
    /// used ([`Rows::selected`]), since the rows change with every edit and when the read arrives. The switcher's
    /// alone: the template list keeps a name instead ([`QuickSwitcher::chosen`]).
    selected: usize,
    /// The template list's choice, by name: the template an edit left selected (the one named exactly the query, or
    /// else the first listed), the one the arrows moved to, or the one last drawn selected (latched on each frame).
    /// `None` while nothing has been listed since the last edit.
    choice: Option<String>,
    /// Whether the arrows have moved the template list's choice since the last edit, which ends its preference for an
    /// exact name.
    arrowed: bool,
    /// The results list's scroll position, which follows the selected row.
    scroll: ScrollHandle,
    /// The running pulse's ticks, held exactly while the window is active and a listed row pulses, and worked out at
    /// the top of each frame, as the session list does.
    pulse: Option<ticker::Pulsing>,
    /// Set right after the layer opens; taken when the switcher closes itself, so it closes once.
    layer: Option<LayerId>,
    /// The field's events.
    _events: Subscription,
}

impl QuickSwitcher {
    /// A switcher for `list` in `mode`, loading, with an empty field and the first row selected.
    fn new(
        list: WeakEntity<super::SessionList>,
        mode: Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = TextInputState::new(window, cx);
        let placeholder = match mode {
            Mode::Sessions => "Jump to a session · tl: for templates",
            Mode::Templates => "Start a template",
        };
        input.set_placeholder(placeholder, window, cx);
        let events = input.subscribe(window, cx, |switcher, event, window, cx| match event {
            TextInputEvent::Changed => switcher.edited(cx),
            TextInputEvent::Submitted { .. } => switcher.pick_selected(window, cx),
        });
        // The list opens with nothing typed, so its first template is chosen, as after an edit.
        let choice = match mode {
            Mode::Sessions => None,
            Mode::Templates => template_rows::rows("", cx)
                .ok()
                .and_then(|rows| rows.into_iter().next())
                .map(|row| row.name),
        };
        QuickSwitcher {
            list,
            mode,
            input,
            read: Read::Loading,
            selected: 0,
            choice,
            arrowed: false,
            scroll: ScrollHandle::new(),
            pulse: None,
            layer: None,
            _events: events,
        }
    }

    /// The read answered.
    fn answered(&mut self, read: Read, cx: &mut Context<Self>) {
        self.read = read;
        cx.notify();
    }

    /// The matches for the field's text now, as indices into the listing; none until it has loaded.
    fn matches(&self, cx: &App) -> Vec<usize> {
        match &self.read {
            Read::Loaded(list) => ranked(&list.sessions, &self.input.text(cx)),
            Read::Loading | Read::Failed(_) => Vec::new(),
        }
    }

    /// The rows for the field's text now: the matches and whether the New or Quick launch row shows, or for a `tl:`
    /// query the templates alone, and with them which rows the arrows walk. The template list's rows are the templates
    /// the whole query finds, with the one named exactly the trimmed query, if any.
    fn rows(&self, cx: &App) -> Rows {
        let query = self.input.text(cx);
        let only_templates = |filter: &str, exact: Option<&str>| {
            let templates = template_rows::rows(filter, cx);
            let exact = exact.and_then(|name| {
                let rows = templates.as_ref().ok()?;
                rows.iter().position(|row| row.name == name)
            });
            Rows {
                matches: Vec::new(),
                new: false,
                quick: false,
                loading: false,
                exact,
                templates: Some(templates),
            }
        };
        match self.mode {
            Mode::Templates => return only_templates(&query, Some(query.trim())),
            Mode::Sessions => {}
        }
        if let Some(filter) = template_rows::template_query(&query) {
            return only_templates(filter, None);
        }
        Rows {
            matches: self.matches(cx),
            new: shows_new_row(&query),
            quick: query.trim().is_empty(),
            loading: matches!(self.read, Read::Loading),
            exact: None,
            templates: None,
        }
    }

    /// The template list's chosen template, by name, for the template list's `rows`: until the arrows move after an
    /// edit, a listed template named exactly the query (SPEC.md: it "is highlighted over an earlier substring match, and
    /// Enter starts it; arrowing to another row changes that choice"), which also picks up an exact name a push brings
    /// after the user typed, as the old list did; otherwise the name the last edit or arrow chose, listed or not, so a
    /// push never hands the choice to another template; and with none, the first template listed now. `None` when
    /// nothing is chosen and nothing is listed, or the templates cannot be listed.
    fn chosen(&self, rows: &Rows) -> Option<String> {
        let Some(Ok(listed)) = &rows.templates else {
            return None;
        };
        match rows.exact {
            Some(exact) if !self.arrowed => Some(listed[exact].name.clone()),
            _ => self
                .choice
                .clone()
                .or_else(|| listed.first().map(|row| row.name.clone())),
        }
    }

    /// The row drawn selected, if any: the switcher's by position; the template list's where its chosen template is
    /// listed, so none while that template is gone.
    fn selection(&self, rows: &Rows) -> Option<Selected> {
        match self.mode {
            Mode::Sessions => rows.selected(self.selected),
            Mode::Templates => {
                let name = self.chosen(rows)?;
                let Some(Ok(listed)) = &rows.templates else {
                    return None;
                };
                listed
                    .iter()
                    .position(|row| row.name == name)
                    .map(Selected::Template)
            }
        }
    }

    /// The user edited the query: the first match is selected again, or in the template list the template named
    /// exactly the query, else the first one listed, and shown.
    fn edited(&mut self, cx: &mut Context<Self>) {
        match self.mode {
            Mode::Sessions => {
                self.selected = 0;
                self.scroll.scroll_to_item(0);
            }
            Mode::Templates => {
                self.arrowed = false;
                self.choice = None;
                let rows = self.rows(cx);
                self.choice = self.chosen(&rows);
                if let Some(Selected::Template(position)) = self.selection(&rows) {
                    self.scroll.scroll_to_item(position);
                }
            }
        }
        cx.notify();
    }

    /// Up or Down in the field: move the selection one row, stopping at either end, and scroll it into view. With
    /// nothing the arrows can reach there is nothing to move. The New and Quick launch rows are pinned outside the
    /// scrolling list, so they are always in view and selecting one scrolls nothing.
    ///
    /// In the template list the arrows move from the chosen template's row, or to the first row while the chosen
    /// template is not listed, and choose the template they arrive at by name.
    fn step(&mut self, direction: Vertical, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let count = rows.reachable();
        if count == 0 {
            return;
        }
        match self.mode {
            Mode::Templates => {
                // `count` is the listed templates', so they are listed.
                let Some(Ok(listed)) = &rows.templates else {
                    return;
                };
                let next = match (self.selection(&rows), direction) {
                    (Some(Selected::Template(current)), Vertical::Up) => current.saturating_sub(1),
                    (Some(Selected::Template(current)), Vertical::Down) => {
                        (current + 1).min(count - 1)
                    }
                    _ => 0,
                };
                self.arrowed = true;
                self.choice = Some(listed[next].name.clone());
                self.scroll.scroll_to_item(next);
            }
            Mode::Sessions => {
                let current = self.selected.min(count - 1);
                self.selected = match direction {
                    Vertical::Up => current.saturating_sub(1),
                    Vertical::Down => (current + 1).min(count - 1),
                };
                if self.selected < rows.scrolled() {
                    self.scroll.scroll_to_item(self.selected);
                }
            }
        }
        cx.notify();
    }

    /// Enter: pick the selected row, if there is one. Nothing while the read is loading, the New and Quick launch rows
    /// included; a template whatever the read's state. In the template list, the chosen template by name, even when
    /// no row lists it any more, so that the pick is refused in New rather than given to another template.
    fn pick_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        match self.mode {
            Mode::Templates => {
                if let Some(name) = self.chosen(&rows) {
                    self.pick_template(&name, window, cx);
                }
                return;
            }
            Mode::Sessions => {}
        }
        match rows.selected(self.selected) {
            Some(Selected::Session(index)) => self.pick(index, window, cx),
            Some(Selected::New) => self.pick_new(window, cx),
            Some(Selected::QuickLaunch) => self.pick_quick_launch(window, cx),
            Some(Selected::Template(position)) => {
                if let Some(Ok(templates)) = rows.templates
                    && let Some(row) = templates.into_iter().nth(position)
                {
                    self.pick_template(&row.name, window, cx);
                }
            }
            None => {}
        }
    }

    /// Pick the template named `name`: close the switcher, then, in the templates section, open New with it applied, or
    /// in the template list quick launch it (the module docs say why in that order). A pick that opened New or started
    /// a launch hides the Hosts panel, as the New row's does (a launch through the dispatcher, which hides it itself); a
    /// refused one (New unavailable) opens nothing and leaves the panel as it was.
    fn pick_template(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.mode {
            Mode::Sessions => self.hand_off(window, cx, |window, cx| {
                template_rows::pick_from_switcher(name, window, cx)
            }),
            Mode::Templates => self.hand_off(window, cx, |window, cx| {
                template_rows::launch_from_list(name, window, cx)
            }),
        }
    }

    /// Pick the Quick launch row: close the switcher, then take the ⚡ chord's path (the module docs say why in that
    /// order). A launch it started or a New it opened hides the Hosts panel (the dispatcher's doing); Templates, opened to
    /// mark one, does not.
    fn pick_quick_launch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hand_off(window, cx, template_rows::quick_launch_from_switcher);
    }

    /// Close the switcher, then run `pick`, one of the templates area's switcher entry points, and when it showed New,
    /// return the list from the Hosts panel through its own hide path (SPEC.md: "All or a host avatar returns to
    /// Sessions, as does an accepted quick-switcher pick or New handoff").
    ///
    /// The entry points report nothing back, so New is read from what they leave ([`entry::open_launcher`]). A launch
    /// started out of sight needs nothing here: the dispatcher returns the list from the panel itself
    /// ([`super::show_sessions`]). A pick that did neither (refused, or Templates opened to mark a template) leaves the
    /// panel as it was.
    fn hand_off(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        pick: impl FnOnce(&mut Window, &mut App),
    ) {
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        pick(window, cx);
        if entry::open_launcher(window, cx).is_some() {
            let _ = self.list.update(cx, |list, cx| list.hide_hosts(cx));
        }
    }

    /// Open the session at `index` in the switcher's listing as the user's choice, after closing the switcher (the
    /// module docs say why in that order), then have the session list show it. The session is opened under its row's
    /// served id, which is the key the store files it under. An index the listing does not hold picks nothing. The
    /// listing never changes once loaded, so an index taken when a row was drawn still names that row.
    fn pick(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Read::Loaded(listing) = &self.read else {
            return;
        };
        let Some(row) = listing.sessions.get(index) else {
            return;
        };
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        selection::choose(&row.info.id, cx);
        let _ = self
            .list
            .update(cx, |list, cx| list.picked_from_switcher(row, cx));
    }

    /// Pick the New row: close the switcher, then open New named with the trimmed query (the module docs say why in
    /// that order, and where focus goes). Only reached while the New row shows ([`shows_new_row`]).
    fn pick_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.input.text(cx);
        let name = query.trim();
        debug_assert!(
            shows_new_row(&query),
            "the New row is picked only while it shows"
        );
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        super::open_new(&self.list, Some(name), window, cx);
    }

    /// Close: close the switcher, picking nothing.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
    }
}

impl Render for QuickSwitcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let viewport = window.viewport_size();
        let width = CARD_MAX_WIDTH.min(viewport.width - CARD_MARGIN * 2.);
        let query = self.input.text(cx);
        let listed = self.rows(cx);
        // The template list latches what it is about to draw selected, so a choice worked out this frame (an exact name
        // a push brought, or the first row after an edit that listed nothing) stays chosen when a later push takes it
        // away or puts a row ahead of it, rather than passing to whatever is then exact or first.
        match self.mode {
            Mode::Templates => self.choice = self.chosen(&listed),
            Mode::Sessions => {}
        }
        let selected = self.selection(&listed);
        let matches = &listed.matches;

        let pulses = ticker::is_active(window, cx)
            && match &self.read {
                Read::Loaded(list) => matches
                    .iter()
                    .any(|&index| status::pulses(&list.sessions[index])),
                Read::Loading | Read::Failed(_) => false,
            };
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let step = self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx));

        let weak = cx.weak_entity();
        let (name, help) = match self.mode {
            Mode::Sessions => (
                "Search sessions or templates",
                "Search sessions across every host; tl: searches templates",
            ),
            Mode::Templates => (
                "Search templates",
                "Search templates by name; Enter starts the highlighted one",
            ),
        };
        let field = {
            let weak = weak.clone();
            div()
                // The selector is for tests, which find the field by where it is drawn.
                .debug_selector(|| "quick-switcher-field".into())
                // Only the results list gives up height when the card is capped.
                .flex_none()
                .child(
                    TextInput::new(
                        "quick-switcher-search",
                        &self.input,
                        name,
                        HoverHelp::tooltip(help),
                    )
                    .on_vertical(move |direction, _, cx| {
                        let _ = weak.update(cx, |switcher, cx| switcher.step(direction, cx));
                    }),
                )
        };
        let in_templates = listed.templates.is_some();
        let heading = quiet(
            if in_templates {
                "Templates"
            } else if query.trim().is_empty() {
                "Recent sessions"
            } else {
                "Sessions"
            },
            palette,
        );
        let columns = Columns::for_card(width);
        // The state line and the scrolling rows, decided together. The templates section's states stand in for the
        // session read's: that read goes on underneath, unseen.
        let (state, result_rows): (Option<AnyElement>, Vec<AnyElement>) =
            match (&listed.templates, &self.read) {
                (Some(Err(reason)), _) => (
                    Some(alert("quick-switcher-templates-failure", reason.clone())),
                    Vec::new(),
                ),
                (Some(Ok(templates)), _) => {
                    let empty = templates.is_empty().then(|| {
                        announced(
                            "quick-switcher-templates-empty",
                            Role::Status,
                            "No templates match",
                        )
                    });
                    let rows = templates
                        .iter()
                        .enumerate()
                        .map(|(position, row)| {
                            let weak = weak.clone();
                            let name = row.name.clone();
                            let selected = selected == Some(Selected::Template(position));
                            template_row(row, selected, self.mode, palette)
                                .on_click(move |_, window, cx| {
                                    let _ = weak.update(cx, |switcher, cx| {
                                        switcher.pick_template(&name, window, cx)
                                    });
                                })
                                .into_any_element()
                        })
                        .collect();
                    (empty, rows)
                }
                (None, Read::Loading) => (
                    Some(announced(
                        "quick-switcher-loading",
                        Role::Status,
                        "Loading sessions…",
                    )),
                    Vec::new(),
                ),
                (None, Read::Failed(reason)) => (
                    Some(alert(
                        "quick-switcher-failure",
                        HostSentence::new("quick-switcher-failure-text")
                            .text("Couldn't load sessions: ")
                            .host(reason),
                    )),
                    Vec::new(),
                ),
                (None, Read::Loaded(list)) => {
                    let empty = matches.is_empty().then(|| {
                        announced("quick-switcher-empty", Role::Status, "No sessions match")
                    });
                    // The host marks come from the registry as the store holds it when the frame is drawn, as the
                    // templates do, not from the switcher's read, which names each host only by id.
                    let store = session::current_store(cx);
                    let collections = store.as_ref().map(|store| store.read(cx).collections());
                    let rows = matches
                        .iter()
                        .map(|&index| {
                            let row = &list.sessions[index];
                            let weak = weak.clone();
                            let selected = selected == Some(Selected::Session(index));
                            // The sidebar row's own decision, so the two never disagree about where a session
                            // runs or how its host looks. The selected row's mark takes the fill's ink in place of
                            // the host's tint, as the old sheet's one `!important` rule had it for both rows.
                            let ink = selected.then_some(palette.on_fill);
                            let mark = collections
                                .and_then(|collections| HostMark::of(row, collections, ink));
                            result_row(row, query.trim(), selected, mark, step, columns, palette)
                                .on_click(move |_, window, cx| {
                                    let _ = weak.update(cx, |switcher, cx| {
                                        switcher.pick(index, window, cx)
                                    });
                                })
                                .into_any_element()
                        })
                        .collect();
                    (empty, rows)
                }
            };
        let results = div()
            .id("quick-switcher-results")
            .track_scroll(&self.scroll)
            .flex()
            .flex_col()
            .flex_shrink_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .children(result_rows);
        let new_row = listed.new.then(|| {
            let weak = weak.clone();
            new_row(
                query.trim(),
                selected == Some(Selected::New),
                palette,
                move |_, window, cx| {
                    let _ = weak.update(cx, |switcher, cx| switcher.pick_new(window, cx));
                },
            )
        });
        let quick_row = listed.quick.then(|| {
            let weak = weak.clone();
            quick_row(
                template_rows::quick_launch_row_label(cx),
                selected == Some(Selected::QuickLaunch),
                columns,
                palette,
            )
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |switcher, cx| switcher.pick_quick_launch(window, cx));
            })
        });
        // One list for assistive technology, the old `.quick-switcher-options` listbox: the scrolling matches (or
        // templates), then the New or Quick launch row pinned below them, which keeps its place while the matches
        // scroll. Only the matches give up height when the card is capped.
        let options = div()
            .id("quick-switcher-options")
            .role(Role::ListBox)
            .aria_label(if in_templates {
                "Templates"
            } else {
                "Sessions and new session"
            })
            .flex()
            .flex_col()
            .flex_shrink_1()
            .min_h(px(0.))
            .child(results)
            .children(new_row)
            .children(quick_row);
        let cap = match &self.read {
            Read::Loaded(list) if list.truncated && !in_templates => {
                Some(quiet(CAP_NOTICE, palette))
            }
            Read::Loaded(_) | Read::Loading | Read::Failed(_) => None,
        };
        let hint = templates_hint(in_templates, self.mode, palette);
        let footer = div()
            .flex()
            .flex_none()
            .flex_row()
            .justify_between()
            .items_center()
            .pt(CARD_GAP)
            .border_t_1()
            .border_color(theme::color(palette.border_dialog))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_1))
            .child("↑↓ choose · Enter pick · Escape close")
            .child(
                Button::new(
                    "quick-switcher-close",
                    "Close",
                    HoverHelp::tooltip("Close the quick switcher"),
                    Role::Button,
                )
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |switcher, cx| switcher.close(window, cx));
                }),
            );

        div()
            .id("quick-switcher")
            .debug_selector(|| "quick-switcher".into())
            .role(Role::Dialog)
            .aria_label(match self.mode {
                Mode::Sessions => "Quick switcher",
                Mode::Templates => "Template launcher",
            })
            .flex()
            .flex_col()
            .gap(CARD_GAP)
            .p(CARD_PAD)
            .w(width)
            // The layer manager caps the top-placed dialog at 75% of the window's height; shrinking to that cap is
            // this card's part, so only the results list, which scrolls, gives up the height.
            .flex_1()
            .min_h(px(0.))
            .bg(theme::color(palette.bg_2))
            .text_color(theme::color(palette.fg_0))
            .border_1()
            .border_color(theme::color(palette.border_dialog))
            .rounded(metrics::RADIUS_MD)
            .shadow(dialog_shadow(palette))
            .child(field)
            .child(heading)
            .children(state)
            .child(options)
            .children(cap)
            .child(hint)
            .child(footer)
    }
}

/// The switcher's rows for one query, in arrow-key order: the matches, then the New row or the Quick launch row when
/// one shows; or, for a `tl:` query and in the template list, the templates alone.
struct Rows {
    /// The matches, as indices into the listing, in the order they are listed. None for a `tl:` query.
    matches: Vec<usize>,
    /// Whether the New row shows ([`shows_new_row`]), whatever the read's state.
    new: bool,
    /// Whether the Quick launch row shows: nothing typed, whatever the read's state. Never with the New row.
    quick: bool,
    /// Whether the read is still loading, when no session row can be selected, the New and Quick launch rows included.
    /// Never set for templates, whose rows do not wait on the read.
    loading: bool,
    /// In the template list, the position of the template named exactly the trimmed query, if one is listed; `None`
    /// otherwise, the `tl:` section included, which has no such rule.
    exact: Option<usize>,
    /// For a `tl:` query or the template list, its templates in the helm's order, or why they cannot be listed; `None`
    /// otherwise.
    templates: Option<Result<Vec<TemplateRow>, HostSentence>>,
}

/// Whether `query` offers the New row: its trimmed text is not empty and is not a template query
/// ([`template_rows::template_query`], a leading `tl:` in any case; the old switcher's `show_new`, which left the row out
/// in template mode).
fn shows_new_row(query: &str) -> bool {
    !query.trim().is_empty() && template_rows::template_query(query).is_none()
}

/// A row the selection can rest on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Selected {
    /// The session at this index in the listing.
    Session(usize),
    /// The New row.
    New,
    /// The Quick launch row.
    QuickLaunch,
    /// The template at this position among the templates listed (a `tl:` query's, or the template list's).
    Template(usize),
}

impl Rows {
    /// How many rows the arrows walk: every template for a `tl:` query; none while the read is loading (the old
    /// switcher counted none until its read answered, so Enter could not mistake a list still on its way for one with
    /// no matches); else every match and the New or Quick launch row.
    fn reachable(&self) -> usize {
        match &self.templates {
            Some(templates) => templates.as_ref().map_or(0, Vec::len),
            None if self.loading => 0,
            None => self.matches.len() + usize::from(self.new) + usize::from(self.quick),
        }
    }

    /// How many of the rows are in the scrolling list, above the pinned row if one shows.
    fn scrolled(&self) -> usize {
        match &self.templates {
            Some(templates) => templates.as_ref().map_or(0, Vec::len),
            None => self.matches.len(),
        }
    }

    /// The row at position `selected`, kept within the reachable rows (so the last one when the rows got fewer); `None`
    /// when no row is reachable.
    fn selected(&self, selected: usize) -> Option<Selected> {
        let count = self.reachable();
        if count == 0 {
            return None;
        }
        let position = selected.min(count - 1);
        if self.templates.is_some() {
            return Some(Selected::Template(position));
        }
        Some(match self.matches.get(position) {
            Some(&index) => Selected::Session(index),
            None if self.new => Selected::New,
            None => Selected::QuickLaunch,
        })
    }
}

/// The New row for the trimmed query `name` (the old `.quick-switcher-new`): `New session named "name"`, with the name
/// escaped as host text since it may hold characters that draw as nothing (the old `display_peer`), in a row with the
/// result rows' box and a top edge in the dialog's border ink that sets it apart from the matches above. On the
/// selected row the accent edge goes all the way round instead, as in the old sheet, where the selected rule outranked
/// the top edge's. A click on it does `on_click`.
fn new_row(
    name: &str,
    selected: bool,
    palette: &Palette,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let sentence = HostSentence::new("quick-switcher-new-text")
        .text("New session named \"")
        .host(&HostText::from_host(name))
        .text("\"");
    let row = OptionRow::new(
        NEW_ROW_ID,
        sentence.accessible_name(),
        HoverHelp::tooltip(NEW_ROW_HELP),
        Role::ListBoxOption,
    )
    .selected(selected)
    .on_click(on_click)
    .child(div().min_w_0().truncate().child(sentence));
    // GPUI draws one edge color for every side, so the old `border-top-color` is a one-pixel line laid over the row's
    // transparent top edge, drawn after the row so that its hover fill does not cover it.
    div()
        .flex_none()
        .relative()
        .child(row)
        .when(!selected, |row| {
            row.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(1.))
                    .bg(theme::color(palette.border_dialog)),
            )
        })
        .into_any_element()
}

/// The 12-pixel quiet line the old heading, footer and cap notice shared.
fn quiet(text: &'static str, palette: &Palette) -> AnyElement {
    div()
        .flex_none()
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(text)
        .into_any_element()
}

/// One of the switcher's one-line states, announced to assistive technology under `role` as the old live regions
/// were.
fn announced(id: &'static str, role: Role, text: &'static str) -> AnyElement {
    div()
        .id(id)
        .flex_none()
        .role(role)
        .aria_label(text)
        .child(text)
        .into_any_element()
}

/// A failed read's reason, announced as an alert as the old `role: "alert"` lines were: the session read's, or the
/// template collection's ([`template_rows::rows`]' "Couldn't load templates:" sentence). The reason quotes a host, so it
/// is a host sentence throughout.
fn alert(id: &'static str, sentence: HostSentence) -> AnyElement {
    div()
        .id(id)
        .flex_none()
        .role(Role::Alert)
        .aria_label(sentence.accessible_name())
        .child(sentence)
        .into_any_element()
}

// ---- The templates section and the Quick launch row: the old `.quick-switcher-template*` rules -----------------------

/// `.quick-switcher-templates-hint p`: the space between the hint's title and its sentence.
const HINT_GAP: Pixels = px(4.);

/// One template's row in the templates section (the old `.quick-switcher-template`, one column): its name at weight
/// 600 over its summary in the quiet 12-pixel ink, both host text where the template's author wrote them. The summary
/// is drawn as the Templates list draws it ([`summary::line`]) and may shrink to the row, so a long one is cut at its
/// edge. On the selected row the summary takes the dimmer inverse ink, as the old sheet's selected rule had it. The
/// name names the row for assistive technology and its id, which tests find it by.
///
/// In the template list ([`Mode::Templates`]) the row's help says a pick starts the template, and a third line holds
/// its shortcuts as key chips (the old `.quick-switcher-template-keys`): the ⚡ chord when it holds ⚡, then its number
/// key's chord, in this platform's form, as the old list showed only there (SPEC.md: "Rows show the template name,
/// summary and its ⚡ or number shortcut when assigned"). A template holding neither has no third line.
fn template_row(row: &TemplateRow, selected: bool, mode: Mode, palette: &Palette) -> OptionRow {
    let name = HostText::from_host(row.name.as_str());
    let id: SharedString = format!("{TEMPLATE_ROW_ID}{}", row.name).into();
    let label = HostSentence::new(id.clone()).host(&name).accessible_name();
    let quiet = if selected {
        palette.on_accent_dim
    } else {
        palette.fg_1
    };
    let (help, chords) = match mode {
        Mode::Sessions => (TEMPLATE_ROW_HELP, Vec::new()),
        Mode::Templates => {
            let mark = row.marked.then(|| QUICK_LAUNCH_SHORTCUT_NAME.to_owned());
            let key = row.key.map(templates::number_key_name);
            (
                TEMPLATE_LIST_ROW_HELP,
                mark.into_iter().chain(key).collect(),
            )
        }
    };
    let option = OptionRow::new(id, label, HoverHelp::tooltip(help), Role::ListBoxOption)
        .selected(selected)
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .child(host_text::element(&name)),
        )
        .child(
            summary::line(&row.summary)
                .min_w_0()
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(quiet)),
        );
    if chords.is_empty() {
        return option;
    }
    option.child(
        div().flex().flex_wrap().gap(KEY_CHIP_GAP).children(
            chords
                .into_iter()
                .map(|chord| shortcuts::chip(chord, quiet, palette)),
        ),
    )
}

/// The Quick launch row (the old `.quick-switcher-quick-launch`): the ⚡ in the status column, `label`
/// ([`template_rows::quick_launch_row_label`]) from the title's, and the ⚡ chord after it, on the session rows' grid as
/// the old row was. Unlike the New row it has no top edge; the old sheet gave it none. The chord keeps the old row's
/// dimmest ink, or the dimmer inverse one on the selected row's fill.
///
/// Neither part is held to its old track. The old label sat in the title's track and the chord in the agent's, both cut
/// with an ellipsis, which in this app's fonts left "Quick launch: Mark a templ…" and "Ctrl+Shif…" on Linux. So the
/// label takes at least the title's width and may run on over the host's, and the chord follows it, never cut.
fn quick_row(
    label: HostSentence,
    selected: bool,
    columns: Columns,
    palette: &Palette,
) -> OptionRow {
    let chord = if selected {
        palette.on_accent_dim
    } else {
        palette.fg_2
    };
    let line = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(COLUMN_GAP)
        .min_w_0()
        .child(div().w(STATUS_COLUMN).flex_none().child("⚡"))
        .child(
            div()
                .flex_none()
                .min_w(columns.title)
                .max_w(columns.title + COLUMN_GAP + columns.host)
                .truncate()
                .child(label.clone()),
        )
        .child(
            div()
                .min_w(AGENT_COLUMN)
                .flex_none()
                .whitespace_nowrap()
                .text_size(px(summary::CHORD_TEXT))
                .text_color(theme::color(chord))
                .child(QUICK_LAUNCH_SHORTCUT_NAME),
        );
    OptionRow::new(
        QUICK_ROW_ID,
        label.accessible_name(),
        HoverHelp::tooltip(QUICK_ROW_HELP),
        Role::ListBoxOption,
    )
    .selected(selected)
    .child(line)
}

/// The templates hint (the old `.quick-switcher-templates-hint`), a quiet section with an edge above it: "Templates"
/// over the `tl:` sentence normally, only the templates section's own sentence while a `tl:` query shows it, and the
/// template list's own sentence there.
fn templates_hint(in_templates: bool, mode: Mode, palette: &Palette) -> AnyElement {
    let hint = div()
        .id("quick-switcher-templates-hint")
        .flex()
        .flex_none()
        .flex_col()
        .gap(HINT_GAP)
        .pt(CARD_GAP)
        .border_t_1()
        .border_color(theme::color(palette.border_dialog))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1));
    match mode {
        Mode::Templates => hint.child(
            "Enter starts the highlighted template. A partial template opens New to complete it.",
        ),
        Mode::Sessions if in_templates => {
            hint.child("Picking a template opens New with it applied; nothing launches yet.")
        }
        Mode::Sessions => hint
            .child("Templates")
            .child("Type tl: to search templates. Picking one opens New with it applied."),
    }
    .into_any_element()
}

/// The widths of a result row's flexible columns in a card `card` wide: what the fixed columns, the gaps and the
/// padding leave, shared two to one to two between title, host and directory, as the old grid's `fr` tracks did.
/// Computed rather than left to flex, because a flexible item with text makes layout shape it on every sizing probe,
/// which shows at the listing cap (SPEC_impl.md, "GPUI cookbook: traps", "Layout costs show only at scale").
#[derive(Clone, Copy)]
struct Columns {
    title: Pixels,
    host: Pixels,
    directory: Pixels,
}

impl Columns {
    fn for_card(card: Pixels) -> Columns {
        let inner = card - CARD_PAD * 2. - px(2.) - ROW_CHROME;
        let shared = (inner - STATUS_COLUMN - AGENT_COLUMN - COLUMN_GAP * 4.).max(px(0.));
        Columns {
            title: shared * 0.4,
            host: shared * 0.2,
            directory: shared * 0.4,
        }
    }
}

/// One session's result row: its status mark, title, agent, host (its `host` mark, if any, before its name) and working
/// directory on one line, and an ended status's words on a second line under the title, since they do not fit the
/// mark's column. Everything the host wrote is host text, each value its own element, with the characters `query`
/// (already trimmed) matches in it marked. On the selected row the title and marks take the inverse ink and the quieter
/// columns the dimmer one, as the old sheet's selected row did.
fn result_row(
    row: &SessionRow,
    query: &str,
    selected: bool,
    host: Option<HostMark>,
    step: u64,
    columns: Columns,
    palette: &Palette,
) -> OptionRow {
    let key = &row.info.id;
    let id: SharedString = format!("quick-switcher-row-{key}").into();
    let title = HostText::from_host(row.info.title.as_str());
    let label = HostSentence::new(id.clone()).host(&title).accessible_name();
    let badge = status::badge(row, format!("quick-switcher-status-{key}").into());
    // The sidebar row's form: the status word joins the help, which is also the row's accessible description, so a
    // live status shown only as a mark still has words for hover and assistive technology.
    let help = match Badge::word(badge.as_ref()) {
        Some(word) => format!("{word} — open this session"),
        None => "Open this session".to_owned(),
    };
    let (live, ended) = match badge {
        Some(Badge::Live(live)) => (Some(live), None),
        Some(Badge::Ended { kind, line }) => (None, Some((kind, line))),
        None => (None, None),
    };
    let quiet = if selected {
        palette.on_accent_dim
    } else {
        palette.fg_1
    };
    let small = |content: AnyElement, width: Pixels| {
        div()
            .w(width)
            .flex_none()
            .min_w_0()
            .overflow_hidden()
            .truncate()
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(quiet))
            .child(content)
    };
    let agent = match menu::agent_name(row) {
        AgentName::Known(agent) => div().child(agent).into_any_element(),
        AgentName::Program(program) => host_text::element(&program).into_any_element(),
    };
    // The old `mark`: the accent at weight 600, or the fill's ink on the selected row's accent fill.
    let style = HighlightStyle {
        color: Some(theme::color(if selected {
            palette.on_fill
        } else {
            palette.accent
        })),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    let marked = |raw: &str, value: &HostText| {
        host_text::marked(value, |shown| mark_positions(raw, shown, query), style)
    };
    let host_name = row.host_name.as_str();
    // `.quick-switcher-host`: the mark at the row slot's size, which never shrinks, then the name, which the column
    // cuts with an ellipsis.
    let mark_id = format!("{HOST_MARK_ID_PREFIX}{key}");
    let host = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(HOST_MARK_GAP)
        .min_w_0()
        .children(host.map(|mark| {
            div()
                .flex_none()
                .flex()
                .items_center()
                .child(row_marks::host_mark(
                    &mark_id,
                    mark,
                    palette,
                    HelpPlace::Ordinary,
                ))
        }))
        .child(
            marked(host_name, &HostText::from_host(host_name))
                .min_w_0()
                .truncate(),
        )
        .into_any_element();
    let cwd = row.info.cwd.as_str();
    let directory = marked(cwd, &HostText::from_host(cwd))
        .font_family(assets::TERMINAL_FONT)
        .into_any_element();
    let line = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(COLUMN_GAP)
        .min_w_0()
        .child(
            div()
                .w(STATUS_COLUMN)
                .flex_none()
                .flex()
                .items_center()
                .children(live.map(|live| live.mark(palette, selected, row.stale, step))),
        )
        .child(
            marked(row.info.title.as_str(), &title)
                .w(columns.title)
                .flex_none()
                .min_w_0()
                .truncate(),
        )
        .child(small(agent, AGENT_COLUMN))
        .child(small(host, columns.host))
        .child(small(directory, columns.directory));
    let mut option = OptionRow::new(id, label, HoverHelp::tooltip(help), Role::ListBoxOption)
        .selected(selected)
        .child(line);
    if let Some((kind, line)) = ended {
        option = option.child(
            div()
                .pl(STATUS_COLUMN + COLUMN_GAP)
                .child(status::ended_line(kind, line, palette, selected)),
        );
    }
    option
}
