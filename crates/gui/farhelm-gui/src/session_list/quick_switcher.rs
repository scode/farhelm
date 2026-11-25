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
//! - **Matching.** Which sessions match is the old matcher's rule ([`matching_positions`]): the trimmed query as a
//!   case-insensitive subsequence (lowercase mapping, not full case folding) of the title, the row's `host_name` or
//!   its raw working directory. The order ([`ranked`]) puts title matches first, then matches only in the host or the
//!   directory, and within each group ranks a row by how well it matched ([`Tier`]): the query at a word start, then
//!   the query as an unbroken substring, then scattered. In the second group the better of the host and the directory
//!   counts. The helm's activity order breaks ties. The tiers exist because activity order alone lists a scattered
//!   title match from a recently active session above a title holding exactly what the user typed, which is usually
//!   the one they are looking for. Matching reads the raw values, so how a value is escaped on screen never changes
//!   which sessions match or how they rank.
//! - **Marks.** A value whose raw form matched has its matched characters drawn in the accent at weight 600 (the old
//!   `mark`; the fill's ink on the selected row): the occurrence that earned the value its tier, found again on the
//!   escaped spelling the user sees ([`mark_positions`], [`host_text::marked`]). Each drawn value (title, host,
//!   directory) is marked at its own best occurrence, not only the value that decided the row's place. A value that
//!   did not match is never marked; one that matched only through a character shown as an escape marks less or
//!   nothing, since no escape is drawn partly marked.
//! - **Session rows.** The harness mark leads two lines, the title and the mono `host:path`. The right-hand slots hold
//!   the host mark, status and age, keeping long paths away from the marks. The host mark comes from the registry as
//!   drawn because the session read carries only the host's id and name. An unplaced host keeps its name and leaves the
//!   mark slot empty. Hover help carries the agent's name, ended details and full activity stamp, keeping the rows a
//!   uniform height.
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
//!   sessions, and no New or Quick launch row. Each row draws the mark, name and summary. No match says
//!   "No templates match", and a template collection
//!   the helm cannot read says "Couldn't load templates:" and the helm's words in place of the rows. The templates are
//!   pushed state, read from the store on every frame, so they need no read of their own: the section works while
//!   the session read is loading or after it failed (SPEC.md: "After a failed session read, New and template search
//!   remain usable"). The arrows and Enter walk the rows as they walk sessions, and a pick closes the switcher, then
//!   hands the name to [`template_rows::pick_from_switcher`], which opens New with the template applied and never
//!   launches; it does nothing while a dialog holds the keyboard, hence the order. A pick that opened New returns the
//!   list from the Hosts panel, as the New row's does, since both are New handoffs.
//! - **Quick launch.** With nothing typed, a row pinned below the matches offers "Quick launch: " and the ⚡ template's
//!   name, or "Mark a template" ([`template_rows::quick_launch_row_target`]), with the ⚡ chord beside it. It is placed
//!   and reached as the New row is (the two never show together): after the last match in arrow-key order, so the
//!   first recent session stays selected, unreachable while the read is loading, and selected after a failed one. A
//!   pick closes the switcher, then runs [`template_rows::quick_launch_from_switcher`], the ⚡ chord's own path:
//!   it launches unseen, opens New for a template that needs more choices, or opens Templates to mark one. A launch
//!   it started or a New it opened returns the list from the Hosts panel, as the old switcher's did (its quick launch
//!   went through the list's `open_new`, which hid the panel); Templates opened to mark one leaves the panel shown.
//! - **Hints.** The empty field shows examples for the mode's commands. The footer keeps navigation, Enter and Escape
//!   in view while the results scroll.
//! - **The template list (Cmd+J).** The same dialog in its templates-only mode ([`Mode::Templates`]; SPEC.md, the
//!   Cmd+J paragraph), as the old app's `SwitcherMode::Templates` was: opened by its own chord under the same refusals
//!   and one more, any launcher held out of sight ([`open`] says which), with the same focus, Escape and outside-press
//!   rules, and named "Template launcher". It sends no session read and shows no session, New or Quick launch row: the
//!   whole query, with no prefix, filters templates as `tl:` does. Rows add the template's ⚡ mark and number-key chip
//!   when assigned, and an "opens New" tag when the launch kind still needs a choice. A template named exactly the
//!   trimmed query (case-sensitively) is selected over an
//!   earlier match until the arrows choose another row, so typing a whole name and pressing Enter starts that template
//!   even when another name contains it. The selection is kept as a template's name, not a row's place, because the rows
//!   follow the templates the helm pushes while the list is open and a pick launches unseen. What is drawn selected is
//!   latched on every frame, so a push that adds or removes a row never moves the choice to another template, and a
//!   chosen template that goes stays chosen, drawn on no row ([`QuickSwitcher::chosen`]). A pick closes the list, then
//!   quick launches the chosen name through [`template_rows::launch_from_list`]: it starts unseen, or opens New with the
//!   template applied when it needs more choices, or with a refusal when no template has that name any more (SPEC.md:
//!   "a name missing from the current catalog shows a preparation error in New"), even when the list then says "No
//!   templates match". With nothing chosen, Enter does nothing.
//!
//! The shared frame supplies the field and Close in its control header and the key strip in its footer. The top
//! placement caps the card once, and the result scroll absorbs the cap while the field and pinned row stay reachable.

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
    div, px,
};

use super::menu::{self, AgentName};
use super::row_marks::{self, HelpPlace, HostMark};
use super::selection;
use super::status::{self, Badge};
use crate::assets::Icon;
use crate::components::{
    DialogFrame, HoverHelp, OptionRow, Role, TextInput, TextInputEvent, TextInputState, Vertical,
    chips_and_words, command_field, command_glyph, command_heading_with_count, command_icon_tile,
    command_row_content, key_chip,
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
use crate::{actions, assets, operations, ticker};

// ---- Layout ----------------------------------------------------------------------------------------------------------

/// Align the body with the frame's heading and footer. The body owns padding so only the results scroll.
const BODY_PAD: Pixels = px(20.);
const BODY_GAP: Pixels = px(6.);

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

/// The template list's help for a row whose saved choices can start a session. Partial templates use New's help.
pub(crate) const TEMPLATE_LIST_ROW_HELP: &str = "Start a session from this template";

// ---- Matching -----------------------------------------------------------------------------------------------------------

/// Where `query` occurs in `text` as a case-insensitive subsequence, as the original characters' indices; `None` when
/// it does not. This is the old matcher unchanged. It decides which values match at all, and it is the
/// [`Tier::Scattered`] alignment that marks a value no better tier found.
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

/// How well a query matched one value, best first. This is the order rows take within a group ([`ranked`]), so the
/// derived `Ord` is load-bearing: `WordStart < Substring < Scattered`.
///
/// Every value a query matches at all is in one of these tiers, because an unbroken occurrence is also a subsequence.
/// The tiers only reorder what [`matching_positions`] already found; they never add or drop a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Tier {
    /// The query occurs unbroken starting at a word start: the start of the value, or right after a character that is
    /// not a letter or digit (`char::is_alphanumeric`). There are no camelCase boundaries, so `factor` does not start
    /// a word in `refactorTool` and `tool` does not either. The rule is taken literally for characters that are
    /// neither: a combining accent written as its own character (`e` followed by U+0301) is not a letter, so `bar`
    /// starts a word in such a `cafébar`, and a query starting with punctuation (`/api`) matches `/work/api` only as a
    /// substring, because the `k` before its `/` is a letter.
    WordStart,
    /// The query occurs unbroken somewhere, but never at a word start.
    Substring,
    /// The query's characters occur in order with gaps, and nowhere unbroken.
    Scattered,
}

impl Tier {
    /// The tiers in the order a value is tried against them, best first.
    const BEST_FIRST: [Tier; 3] = [Tier::WordStart, Tier::Substring, Tier::Scattered];
}

/// Where `query` occurs in `text` the way `tier` asks, as the original characters' indices; `None` when it does not.
///
/// The unbroken tiers take the first (leftmost) occurrence that qualifies. For [`Tier::Substring`] any unbroken
/// occurrence qualifies, a word-start one included, since a caller asks for it only when it wants "the first
/// substring occurrence" of a value no word start was found in, or on a spelling ([`mark_positions`]) that may differ
/// from the one the tier was earned on. [`Tier::Scattered`] is [`matching_positions`]. Comparison is the same
/// lowercase mapping as there, over the same sequence of lowered characters: an occurrence may begin or end partway
/// through a character that lowercases to several, and its positions are the original characters it touches, each
/// listed once. An empty query matches at no positions in every tier.
fn find(text: &str, query: &str, tier: Tier) -> Option<Vec<usize>> {
    if tier == Tier::Scattered {
        return matching_positions(text, query);
    }
    let wanted: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if wanted.is_empty() {
        // `windows(0)` would panic, and the row-drawing code does ask with an empty query.
        return Some(Vec::new());
    }
    let chars: Vec<char> = text.chars().collect();
    let folded: Vec<(usize, char)> = chars
        .iter()
        .enumerate()
        .flat_map(|(index, ch)| ch.to_lowercase().map(move |ch| (index, ch)))
        .collect();
    // A word start is judged on the original characters, not the lowered ones: in `İx` the character before `x` is
    // `İ`, a letter, even though the last lowered character is a combining dot, which is not. An occurrence that
    // begins partway through a character's lowering starts no word, since its first character is not one the user
    // can see on its own.
    let starts_word = |start: usize| {
        let index = folded[start].0;
        let begins_its_character = start == 0 || folded[start - 1].0 != index;
        begins_its_character && (index == 0 || !chars[index - 1].is_alphanumeric())
    };
    folded
        .windows(wanted.len())
        .enumerate()
        .find(|(start, window)| {
            window.iter().map(|(_, ch)| *ch).eq(wanted.iter().copied())
                && (tier == Tier::Substring || starts_word(*start))
        })
        .map(|(_, window)| {
            let mut positions: Vec<usize> = window.iter().map(|(index, _)| *index).collect();
            positions.dedup();
            positions
        })
}

/// The best tier `query` reaches in `text`, with the occurrence that earned it: the first word-start occurrence,
/// otherwise the first unbroken one, otherwise the leftmost scattered alignment. `None` when `query` does not match
/// `text` at all.
fn best_match(text: &str, query: &str) -> Option<(Tier, Vec<usize>)> {
    Tier::BEST_FIRST
        .into_iter()
        .find_map(|tier| find(text, query, tier).map(|positions| (tier, positions)))
}

/// The characters to mark in `shown`, the escaped spelling the user sees of the value `raw`: none unless `query` matches
/// `raw` itself (the value took part in the match), and then the occurrence in `shown` of the tier `raw` earned, which
/// may be nowhere. A value is matched on its raw form ([`ranked`]) but marked on what is shown, so a title found by
/// typing an invisible character, which shows as its `<U+…>` escape, is listed with nothing marked, and a value that
/// did not match is not marked because its escapes happen to spell the query, as in the old switcher.
///
/// The tier comes from `raw` rather than from the best match in `shown` because an escape is itself text with word
/// starts in it: `v2` followed by U+200B shows as `v2<U+200B>`, and the best match of `2` there is the word start
/// inside the escape, which is never drawn marked, while the `2` that earned the title its substring tier is index 1.
/// What remains is a smaller miss the old switcher had too: an earlier occurrence of the same tier inside an escape
/// still marks nothing.
pub(crate) fn mark_positions(raw: &str, shown: &str, query: &str) -> Vec<usize> {
    let Some((tier, _)) = best_match(raw, query) else {
        return Vec::new();
    };
    find(shown, query, tier).unwrap_or_default()
}

/// Keep the chosen match in the full path. The abbreviated home has no characters of its own to highlight, but the
/// characters in the retained suffix still explain the match, including when a scattered match began inside the home.
fn abbreviated_path_marks(raw: &str, shown: &str, query: &str) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    if !shown.starts_with('~') || raw.starts_with('~') {
        return mark_positions(raw, shown, query);
    }
    let full = farhelm_proto::text::escaped(raw);
    let removed = full.chars().count() - (shown.chars().count() - 1);
    mark_positions(raw, &full, query)
        .into_iter()
        .filter_map(|index| index.checked_sub(removed).map(|index| index + 1))
        .collect()
}

/// The sessions of `sessions` (the helm's activity order) that `query` finds, as indices into it, in the order the
/// switcher lists them. Every title match comes first, then every session that matches only in its host's name or its
/// working directory. Within each group rows are ordered by [`Tier`], the title's tier in the first group and the
/// better of the host's and the directory's in the second, and rows of the same tier keep the helm's order, so among
/// equally good matches the most recently active comes first. A scattered title match still comes before an exact
/// match in a folder: the group outranks the tier.
///
/// The query is trimmed first. Matching reads the listing's title, `host_name` and `cwd` before display escaping,
/// so a hidden character in a title is found by typing it, and typing its escaped spelling (`<U+200B>`) finds nothing
/// (unlike the launcher's editable fields, where a typed spelling stands for its character). A staged capture supplies
/// its display folders in that listing.
/// An empty query matches every row in one tier, which leaves the helm's order as it is.
pub(crate) fn ranked(sessions: &[SessionRow], query: &str) -> Vec<usize> {
    /// Which group a row is in, title matches first; the derived `Ord` is the group order.
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Group {
        Title,
        Elsewhere,
    }
    let query = query.trim();
    let tier = |text: &str| best_match(text, query).map(|(tier, _)| tier);
    let mut found: Vec<(Group, Tier, usize)> = sessions
        .iter()
        .enumerate()
        .filter_map(|(index, row)| match tier(&row.info.title) {
            Some(title) => Some((Group::Title, title, index)),
            None => [tier(&row.host_name), tier(&row.info.cwd)]
                .into_iter()
                .flatten()
                .min()
                .map(|elsewhere| (Group::Elsewhere, elsewhere, index)),
        })
        .collect();
    // Stable, so the helm's activity order survives within each (group, tier).
    found.sort_by_key(|(group, tier, _)| (*group, *tier));
    found.into_iter().map(|(_, _, index)| index).collect()
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
    layers::record_opened(&switcher, cx);
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
/// wording. A staged capture rewrites this window's copy before it searches or draws the rows. Each opening makes a
/// switcher of its own and the reply holds it weakly, so a reply after close cannot reach a later opening. A reset
/// retires the UI session, which drops the wait, and closes every dialog, the switcher included.
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
            Ok(mut list) => {
                crate::agent_startup::stage_session_rows(&mut list.sessions, cx);
                Read::Loaded(list)
            }
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

/// The golden pictures start at the loaded view. Opening and replying through
/// the channel are covered by the switcher's interaction tests.
#[cfg(test)]
pub(crate) fn seed_golden_listing(list: serde_json::Value, cx: &mut App) {
    let switcher = layers::last_opened::<QuickSwitcher>(cx).expect("golden switcher opened");
    let list = serde_json::from_value(list).expect("golden listing decodes");
    switcher.update(cx, |switcher, cx| switcher.answered(Read::Loaded(list), cx));
}

/// Stage a query on the open field. This uses the same text state and result calculation as an edit in the app.
#[cfg(test)]
pub(crate) fn seed_golden_query(query: &str, window: &mut Window, cx: &mut App) {
    let switcher = layers::last_opened::<QuickSwitcher>(cx).expect("golden switcher opened");
    switcher.update(cx, |switcher, cx| {
        switcher.input.set_text(query.to_owned(), window, cx);
        cx.notify();
    });
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
    /// Use the shared clock while a session result has an age to show.
    clock: Option<ticker::Clock>,
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
            Mode::Sessions => "Jump to a session, or type a name to start one…",
            Mode::Templates => "Start a template…",
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
            clock: None,
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
        let (width, title, icon, shortcut) = match self.mode {
            Mode::Sessions => (
                px(640.),
                "Quick switcher",
                Icon::Search,
                actions::QUICK_SWITCHER_SHORTCUT_NAME,
            ),
            Mode::Templates => (
                px(560.),
                "Template launcher",
                Icon::Templates,
                actions::TEMPLATE_LIST_SHORTCUT_NAME,
            ),
        };
        let area = crate::desktop::linux_title_bar::content_area(window).size;
        let width = width.min(area.width - metrics::DIALOG_SIDE_MARGIN * 2.);
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
        let shows_age = listed.templates.is_none()
            && matches!(&self.read, Read::Loaded(list) if matches.iter().any(|&index| list.sessions[index].info.effective_activity() > 0));
        if shows_age != self.clock.is_some() {
            self.clock = shows_age.then(|| ticker::clock(cx));
        }
        let row_context = RowContext::new(
            width,
            self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx)),
            cx,
        );

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
                    command_field(
                        TextInput::new(
                            "quick-switcher-search",
                            &self.input,
                            name,
                            HoverHelp::tooltip(help),
                        ),
                        command_icon_tile(icon, palette),
                        Some(key_chip(shortcut, palette.fg_1, palette)),
                    )
                    .on_vertical(move |direction, _, cx| {
                        let _ = weak.update(cx, |switcher, cx| switcher.step(direction, cx));
                    }),
                )
        };
        let in_templates = listed.templates.is_some();
        let heading = section_heading(self.mode, &listed, &self.read, &query, palette);
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
                            template_row(row, selected, self.mode, row_context.inner_width, palette)
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
                    let catalog = store.as_ref().map(|store| store.read(cx).collections());
                    let term = query.trim();
                    let rows = matches
                        .iter()
                        .map(|&index| {
                            let row = &list.sessions[index];
                            let weak = weak.clone();
                            let selected = selected == Some(Selected::Session(index));
                            result_row(row, term, selected, catalog, &row_context, palette)
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
                template_rows::quick_launch_row_target(cx),
                selected == Some(Selected::QuickLaunch),
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
            .relative()
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
            .child(layers::beside_edge("quick-switcher-help-edge", window, cx))
            .child(results)
            .children(new_row)
            .children(quick_row.map(|row| pinned_row(row, palette)));
        let cap = match &self.read {
            Read::Loaded(list) if list.truncated && !in_templates => {
                Some(quiet(CAP_NOTICE, palette))
            }
            Read::Loaded(_) | Read::Loading | Read::Failed(_) => None,
        };
        let body = div()
            .flex()
            .flex_col()
            .flex_shrink_1()
            .min_h_0()
            .gap(BODY_GAP)
            .px(BODY_PAD)
            .py(px(10.))
            .child(heading)
            .children(state)
            .child(options)
            .children(cap);
        DialogFrame::new("quick-switcher", title, body, vec![])
            .width(width)
            .split_body()
            .heading_control(
                field,
                query.is_empty().then(|| header_hints(self.mode, palette)),
                None,
            )
            .footer_lead(footer_hints(self.mode, palette))
            .close(
                "Close",
                HoverHelp::tooltip("Close the quick switcher"),
                false,
                move |_, window, cx| {
                    let _ = weak.update(cx, |switcher, cx| switcher.close(window, cx));
                },
            )
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

/// Draw the name the pick will pass to New. The name remains host text, including when the row is selected.
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
    .child(
        command_row_content(
            Some(row_marks::cell(
                "quick-switcher-new-mark".into(),
                Icon::Launch,
                row_ink(selected, palette, palette.fg_1),
            )),
            None,
            [div().truncate().child(sentence).into_any_element()],
        )
        .items_center(),
    );
    pinned_row(row, palette)
}

/// Pinned actions stay below the scrolling results. The hairline separates them from matches even at the scroll cap.
fn pinned_row(row: OptionRow, palette: &Palette) -> AnyElement {
    div()
        .flex_none()
        .mt(px(4.))
        .pt(px(4.))
        .border_t_1()
        .border_color(theme::color(palette.border_dim))
        .child(row)
        .into_any_element()
}

/// The listing cap stays outside the result scroll so it is visible whichever row the user reaches.
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

/// Report the fleet size from the unfiltered reply, even when the listing cap limits the rows we can search.
fn section_heading(
    mode: Mode,
    listed: &Rows,
    read: &Read,
    query: &str,
    palette: &Palette,
) -> AnyElement {
    let (label, count) = match (&listed.templates, mode, query.trim().is_empty(), read) {
        (Some(Ok(templates)), Mode::Templates, _, _) => {
            ("Templates", Some(templates.len().to_string()))
        }
        (Some(_), _, _, _) => ("Templates", None),
        (None, _, true, Read::Loaded(list)) => (
            "Recent sessions",
            Some(format!("{} · every host", list.total)),
        ),
        (None, _, true, _) => ("Recent sessions", None),
        (None, _, false, _) => ("Sessions", None),
    };
    command_heading_with_count("quick-switcher-heading", label, count, palette).into_any_element()
}

/// The empty field teaches the commands whose meaning would otherwise be hidden in the result rows.
fn header_hints(mode: Mode, palette: &Palette) -> AnyElement {
    let chip = |name| key_chip(name, palette.fg_1, palette).into_any_element();
    let example = |name| {
        key_chip(name, palette.accent, palette)
            .font_family(assets::TERMINAL_FONT)
            .into_any_element()
    };
    let words = |text| div().child(text).into_any_element();
    let [first, second] = match mode {
        Mode::Sessions => [
            chips_and_words([example("tl:"), words("search templates")]),
            chips_and_words([
                example("fix login bug"),
                chip("⏎"),
                words("on no match starts a session with that name"),
            ]),
        ],
        Mode::Templates => [
            chips_and_words([
                command_glyph(Icon::BoltFilled, palette.warn_bright).into_any_element(),
                words("quick launch"),
                chip(QUICK_LAUNCH_SHORTCUT_NAME),
            ]),
            chips_and_words([
                words("number keys"),
                key_chip(templates::number_key_name("1"), palette.fg_1, palette).into_any_element(),
                words("…"),
                key_chip(templates::number_key_name("0"), palette.fg_1, palette).into_any_element(),
            ]),
        ],
    };
    div()
        .id("quick-switcher-hints")
        .flex()
        .flex_wrap()
        .gap_x(px(12.))
        .gap_y(px(4.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(first)
        .child(second)
        .into_any_element()
}

/// Keep the universal navigation keys in view while the matches scroll. Enter opens a session or starts a template.
fn footer_hints(mode: Mode, palette: &Palette) -> AnyElement {
    let pair = |key, words| {
        chips_and_words([
            key_chip(key, palette.fg_1, palette).into_any_element(),
            div().child(words).into_any_element(),
        ])
    };
    let enter = match mode {
        Mode::Sessions => "open",
        Mode::Templates => "start",
    };
    // The key caps alone read as fragments to a screen reader; give the strip one sentence, as the launcher's hint has.
    let label: SharedString =
        format!("Up and Down to move, Enter to {enter}, Escape to close").into();
    div()
        .id("quick-switcher-footer-keys")
        .role(Role::Label)
        .aria_label(label)
        .flex()
        .flex_wrap()
        .gap(px(12.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(pair("↑ ↓", "move"))
        .child(pair("⏎", enter))
        .child(pair("esc", "close"))
        .into_any_element()
}

// ---- Command rows ---------------------------------------------------------------------------------------------------

/// The two template lists share the leading mark and text. Only Cmd+J tries to launch on a pick. Its clues stay
/// visible at the right edge even when a long summary needs an ellipsis.
fn template_row(
    row: &TemplateRow,
    selected: bool,
    mode: Mode,
    inner_width: Pixels,
    palette: &Palette,
) -> OptionRow {
    let name = HostText::from_host(row.name.as_str());
    let id: SharedString = format!("{TEMPLATE_ROW_ID}{}", row.name).into();
    let summary_id = format!("quick-switcher-summary-{}", row.name);
    let label = HostSentence::new(id.clone()).host(&name).accessible_name();
    let quiet = if selected {
        palette.on_accent_dim
    } else {
        palette.fg_1
    };
    let help = match mode {
        Mode::Sessions => TEMPLATE_ROW_HELP,
        Mode::Templates if row.opens_new => TEMPLATE_ROW_HELP,
        Mode::Templates => TEMPLATE_LIST_ROW_HELP,
    };
    let (icon, ink) = row.mark.appearance(selected, palette);
    // A bare tl: can show the whole catalog. There are no trailing cues to measure in that mode.
    let text_width = (mode == Mode::Sessions)
        .then_some((inner_width - px(row_marks::GLYPH_WIDTH + 7.)).max(px(0.)));
    let mut contents = command_row_content(
        Some(row_marks::cell(
            format!("quick-switcher-template-mark-{}", row.name).into(),
            icon,
            ink,
        )),
        text_width,
        [
            div()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .child(host_text::element(&name))
                .into_any_element(),
            summary::line(summary_id, &row.summary)
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(quiet))
                .into_any_element(),
        ],
    )
    .items_center();
    if mode == Mode::Templates && (row.opens_new || row.marked || row.key.is_some()) {
        contents = contents.child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(7.))
                .children(row.opens_new.then(|| opens_new_tag(selected, palette)))
                .children(row.marked.then(|| {
                    div()
                        .id(format!("quick-switcher-bolt-{}", row.name))
                        .role(Role::Image)
                        .aria_label("Quick launch")
                        .flex_none()
                        .child(command_glyph(
                            Icon::BoltFilled,
                            row_ink(selected, palette, palette.warn_bright),
                        ))
                }))
                .children(
                    row.key
                        .map(|key| key_chip(templates::number_key_name(key), quiet, palette)),
                ),
        );
    }
    OptionRow::new(id, label, HoverHelp::tooltip(help), Role::ListBoxOption)
        .selected(selected)
        .child(contents)
}

/// The preparation tag is shorter and rounder than a shortcut chip, so it reads as a cue rather than a key.
fn opens_new_tag(selected: bool, palette: &Palette) -> gpui::Div {
    let (ink, edge, ground) = if selected {
        (
            palette.on_fill,
            theme::SELECTED_TEMPLATE_TAG_BORDER,
            theme::SELECTED_TEMPLATE_TAG_GROUND,
        )
    } else {
        (palette.fg_1, palette.border_dim, palette.bg_1)
    };
    div()
        .flex_none()
        .px(px(6.))
        .py(px(2.))
        .border_1()
        .border_color(theme::color(edge))
        .bg(theme::color(ground))
        .rounded_full()
        .text_size(px(11.))
        .line_height(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::color(ink))
        .child("opens New")
}

/// Reserve the trailing width for the complete quick-launch shortcut, even when a template name truncates.
fn quick_row(target: HostSentence, selected: bool, palette: &Palette) -> OptionRow {
    let quiet = row_ink(selected, palette, palette.fg_1);
    let label = target
        .clone()
        .prepend_text("Quick launch: ")
        .accessible_name();
    let words = div()
        .flex()
        .min_w_0()
        .items_center()
        .gap(px(4.))
        .child(
            div()
                .flex_none()
                .text_color(theme::color(quiet))
                .child("Quick launch"),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .child(target),
        );
    let line = command_row_content(
        Some(row_marks::cell(
            "quick-switcher-quick-launch-mark".into(),
            Icon::BoltFilled,
            row_ink(selected, palette, palette.warn_bright),
        )),
        None,
        [words.into_any_element()],
    )
    .items_center()
    .child(key_chip(QUICK_LAUNCH_SHORTCUT_NAME, quiet, palette).flex_none());
    OptionRow::new(
        QUICK_ROW_ID,
        label,
        HoverHelp::tooltip(QUICK_ROW_HELP),
        Role::ListBoxOption,
    )
    .selected(selected)
    .child(line)
}

/// The label and timestamp use the same clock for every visible result. The card width is known before drawing rows.
struct RowContext {
    inner_width: Pixels,
    now: Option<i64>,
    zone: crate::local_time::Zone,
    pulse: u64,
}

impl RowContext {
    fn new(width: Pixels, pulse: u64, cx: &App) -> Self {
        Self {
            // The frame's border, body insets and OptionRow's own border and padding.
            inner_width: width - BODY_PAD * 2. - px(24.),
            now: super::age::seconds(ticker::wall_now(cx)),
            zone: crate::local_time::zone(cx),
            pulse,
        }
    }
}

/// Selected marks take inverse ink, so their identity and status colors never compete with the accent fill.
fn row_ink(selected: bool, palette: &Palette, ordinary: theme::Token) -> theme::Token {
    if selected { palette.on_fill } else { ordinary }
}

/// Two lines fit every session type. The age, host and status stay in their own slots even when the path truncates.
fn result_row(
    row: &SessionRow,
    query: &str,
    selected: bool,
    collections: Option<&farhelm_helm_client::Collections>,
    context: &RowContext,
    palette: &Palette,
) -> OptionRow {
    let key = &row.info.id;
    let id: SharedString = format!("quick-switcher-row-{key}").into();
    let title = HostText::from_host(row.info.title.as_str());
    let label = HostSentence::new(id.clone()).host(&title).accessible_name();
    let badge = status::badge(row, format!("quick-switcher-status-{key}").into());
    let age = super::age::of(row.info.effective_activity(), context.now, context.zone);
    let help = result_help(row, collections, badge.as_ref(), age.as_ref());

    let style = HighlightStyle {
        color: Some(theme::color(row_ink(selected, palette, palette.accent))),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    let marked = |raw: &str, value: &HostText| {
        host_text::marked(value, |shown| mark_positions(raw, shown, query), style)
    };
    let host_name = row.host_name.as_str();
    let cwd = row.info.cwd.as_str();
    let destination = div()
        .flex()
        .items_center()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(if selected {
            palette.on_accent_dim
        } else {
            palette.fg_1
        }))
        .font_family(assets::TERMINAL_FONT)
        .child(
            marked(host_name, &HostText::from_host(host_name))
                .flex_none()
                .max_w(gpui::relative(0.4))
                .truncate(),
        )
        .child(":")
        .child(
            host_text::marked(
                &HostText::from_host(super::abbreviate_home(cwd)),
                |shown| abbreviated_path_marks(cwd, shown, query),
                style,
            )
            .min_w_0()
            .truncate(),
        );

    let (agent, ink) = row_marks::agent_mark(row, selected, palette);
    let agent = row_marks::cell(
        format!("quick-switcher-agent-{key}").into(),
        crate::harness::icon(agent),
        ink,
    );
    let inverse = selected.then_some(palette.on_fill);
    let host = collections.and_then(|collections| HostMark::of(row, collections, inverse));
    let mark_id = format!("{HOST_MARK_ID_PREFIX}{key}");
    let host = host.map(|mark| row_marks::host_mark(&mark_id, mark, palette, HelpPlace::Ordinary));
    let status = status::live(row)
        .map(|live| {
            live.mark(palette, selected, row.stale, context.pulse)
                .into_any_element()
        })
        .or_else(|| {
            status::EndedGlyph::of(row).map(|ended| {
                row_marks::cell(
                    format!("quick-switcher-ended-{key}").into(),
                    ended.icon(),
                    ended.color(palette, selected),
                )
                .into_any_element()
            })
        });
    // A multi-year session can use more than four digits. Reserve the width before the text is shaped.
    let age_width = age.as_ref().map_or(px(34.), |age| {
        px(34_f32.max(age.short.len() as f32 * 8. + 2.))
    });
    let age = div()
        .w(age_width)
        .flex_none()
        .text_right()
        .text_size(metrics::TEXT_SM)
        .font_family(assets::TERMINAL_FONT)
        .text_color(theme::color(if selected {
            palette.on_accent_dim
        } else {
            palette.fg_1
        }))
        .children(age.map(|age| age.short));
    let slot = |children: Option<AnyElement>| {
        div()
            .w(metrics::ROW_SLOT)
            .flex_none()
            .flex()
            .justify_center()
            .items_center()
            .children(children)
    };
    let trailing = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(7.))
        .child(slot(host.map(IntoElement::into_any_element)))
        .child(slot(status))
        .child(age);
    let text_width =
        (context.inner_width - px(38.) - metrics::ROW_SLOT * 2. - age_width).max(px(0.));
    let contents = command_row_content(
        Some(div().mt(px(3.)).child(agent)),
        Some(text_width),
        [
            marked(row.info.title.as_str(), &title)
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .into_any_element(),
            destination.into_any_element(),
        ],
    )
    .items_center()
    .child(trailing);
    OptionRow::new(id, label, help, Role::ListBoxOption)
        .selected(selected)
        .child(contents)
}

/// The row's marks have text equivalents in hover help. Preserve host text isolation for names and failure details.
fn result_help(
    row: &SessionRow,
    collections: Option<&farhelm_helm_client::Collections>,
    badge: Option<&Badge>,
    age: Option<&super::age::Age>,
) -> HoverHelp {
    use crate::host_text::Line;
    let agent = HostSentence::new("quick-switcher-agent-help").text("Agent: ");
    let agent = match menu::agent_name(row, collections) {
        AgentName::Known(name) => agent.text(name),
        AgentName::Registered(name) | AgentName::Program(name) => agent.host(&name),
    };
    let mut details = vec![agent];
    match badge {
        Some(Badge::Live(live)) => {
            details.push(HostSentence::new("quick-switcher-status-help").text(live.word()))
        }
        Some(Badge::Ended {
            line: Line::App(words),
            ..
        }) => {
            details.push(HostSentence::new("quick-switcher-status-help").text(words.clone()));
        }
        Some(Badge::Ended {
            line: Line::Sentence(sentence),
            ..
        }) => details.push(sentence.clone()),
        None => {}
    }
    if let Some(age) = age {
        details.push(HostSentence::new("quick-switcher-age-help").text(age.full.clone()));
    }
    details.push(HostSentence::new("quick-switcher-pick-help").text("Open this session"));
    HoverHelp::paragraphs(details)
}
