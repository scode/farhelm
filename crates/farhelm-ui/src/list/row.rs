//! One session row and its lifecycle, metadata, and menu controls.
//!
//! The row keeps the floating-panel hook state with the component; only the
//! geometry decisions are delegated to the pure menu-panel helpers.

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;

use crate::Session;
#[cfg(test)]
use crate::SessionStatus;
use crate::archive::confirmation as archive_confirmation;
use crate::icons::{LocalHostIcon, RemoteHostIcon};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::profiles::{existence_word, source_profile_label};
use crate::rename::RenameForm;
use crate::status::{StatusBadgeView, confirm_consequence, replace_consequence, status_badge};

use super::shared::{DeleteTarget, HostLocality, RowState};
use crate::menu_panel::{
    self, MenuFocusQueue, MenuOpenIntent, PanelPlacement, cancel_menu_focus, clamp_title,
    closed_toggle_key_intent, focus_menu_toggle, forget_menu_focus, handle_menu_key,
    measurement_outcome, menu_panel_placement_style, remember_menu_item, should_measure_on_mount,
};

/// Which ordinary row controls exist for the current retention state.
///
/// Archive removes terminal lifecycle actions, not metadata management: an
/// archived row can still be opened, renamed, or deleted, but cannot be
/// stopped or archived a second time. Clone and Replace carry NO field here
/// at all — unlike these four, both are offered unconditionally on every
/// retention state (see `MENU_ACTIONS`/`session_menu_order`), because
/// neither is a lifecycle action or a metadata edit on the row at all: each
/// only reads the row to seed a brand-new create (opening a form for
/// clone, or acting at once for replace), which needs nothing about this
/// row to be live or mutable — an archived session has no running process
/// to act on, but its host, directory, title, and launch profile (or raw
/// invocation) are all still on this `Session`. Clone turns that history
/// back into a running agent without un-archiving the original; replace
/// discards the archived record entirely and puts the running agent in its
/// place instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowControlVisibility {
    rename: bool,
    stop: bool,
    archive: bool,
    delete: bool,
    /// Whether the mark-read/mark-unread item is offered — the ONE field
    /// here whose condition is not a pure function of `archived`. It needs
    /// both the row's LIVE
    /// status (running, waiting, or idle — an ended session has no dot and
    /// no meaningful unseen state to toggle) and whether the helm sent
    /// `seen_activity_at` at all (an old helm offers no toggle it cannot
    /// answer PUT requests for), so the caller computes it from the whole
    /// `Session` rather than this function deriving it from `archived`
    /// alone.
    mark_seen: bool,
}

fn row_control_visibility(archived: bool, mark_seen: bool) -> RowControlVisibility {
    RowControlVisibility {
        rename: true,
        stop: !archived,
        archive: !archived,
        delete: true,
        mark_seen,
    }
}

// ===== The menu's items, and how they are addressed ==================
//
// Everything below identifies a menu item by WHAT IT DOES rather than by
// where it currently sits. That distinction is the fix for a real bug:
// the item set is not fixed for the life of an open menu. Archiving a
// session withdraws Stop and Archive, and the surviving Delete keeps its
// DOM node rather than remounting, so a scheme that filed handles under
// "index 3" left Delete's handle at an index the shorter list no longer
// reaches while navigation had already moved on to the new numbering.
// Positions are derived from `MenuOrder` at the moment a key is pressed;
// nothing durable is ever keyed by one.

/// One command in a session row's actions menu.
///
/// The identity a mounted handle is filed under, and the vocabulary
/// `MenuOrder` speaks — see this section's own note for why position is
/// never that identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MenuAction {
    Rename,
    /// Toggle the row's seen state — "mark read" or "mark unread" depending
    /// on the CURRENT predicate, never a fixed label. Inserted right after
    /// `Rename`: both are metadata edits on the row rather than lifecycle
    /// actions, so grouping them ahead of Stop/Archive/Delete keeps that
    /// distinction visible in the menu's own order.
    MarkSeen,
    Clone,
    Replace,
    Stop,
    Archive,
    Delete,
}

/// Every action the menu can offer, in the order it offers them.
///
/// The canonical order lives here rather than in the rsx so that
/// `MenuOrder` and the rendered list cannot disagree about what "the
/// first item" or "the last item" means — the two places arrow keys and
/// the open-intent both resolve against.
///
/// `Replace` sits directly after `Clone`: the two are the row's only two
/// "make a new session from this one" actions, and putting them beside each
/// other is what lets a user compare "keep both" against "swap this one
/// out" without hunting across the menu.
const MENU_ACTIONS: [MenuAction; 7] = [
    MenuAction::Rename,
    MenuAction::MarkSeen,
    MenuAction::Clone,
    MenuAction::Replace,
    MenuAction::Stop,
    MenuAction::Archive,
    MenuAction::Delete,
];

/// One render's item list — the session row's own instantiation of the
/// shared, generic `menu_panel::MenuOrder` (see that type's own doc for
/// the packing rule and for why the mechanics live there rather than
/// being copied per row). The const generic argument is `MENU_ACTIONS`'s
/// own length rather than a restated literal, so the array stays the
/// single source of truth for this menu's capacity and cannot drift from
/// it by one entry the way a hand-copied number could.
type MenuOrder = menu_panel::MenuOrder<MenuAction, { MENU_ACTIONS.len() }>;

/// Builds this render's item list from the retention state — the bridge
/// between `RowControlVisibility`'s named fields and the shared
/// `MenuOrder::pack`'s generic `(action) -> bool` predicate.
///
/// `Clone` and `Replace` both answer `true` unconditionally rather than
/// reading a `RowControlVisibility` field: neither has one, because both
/// are offered on every retention state (see that struct's own doc for why
/// the omission is deliberate rather than a gap this match should be
/// filling). Replace needs no running process any more than clone does —
/// an archived source has no agent to kill, only a record to delete before
/// the fresh session takes its place.
fn session_menu_order(controls: RowControlVisibility) -> MenuOrder {
    MenuOrder::pack(MENU_ACTIONS, |action| match action {
        MenuAction::Rename => controls.rename,
        MenuAction::MarkSeen => controls.mark_seen,
        MenuAction::Clone => true,
        MenuAction::Replace => true,
        MenuAction::Stop => controls.stop,
        MenuAction::Archive => controls.archive,
        MenuAction::Delete => controls.delete,
    })
}

/// Handles for the menu items currently mounted, keyed by the action each
/// one performs — the session row's instantiation of the shared, generic
/// `menu_panel::MenuItemHandles` (see that type's own doc for the
/// lifecycle rule: cleared on every fresh open and on every close, since
/// a retained handle retains a detached DOM node with it on the web
/// renderer).
type MenuItemHandles = menu_panel::MenuItemHandles<MenuAction>;

/// This row's menu wiring, bound to its own action enum and its own
/// identity type (a session id is a `String`) — see
/// `menu_panel::MenuWiring`'s own doc for what it bundles and why both of
/// those vary per row while the mechanics built on it do not.
type MenuWiring = menu_panel::MenuWiring<MenuAction, String, { MENU_ACTIONS.len() }>;

/// The session row's class list for its three independent visual states.
///
/// A stale row is DIMMED and badged, never hidden or disabled: SPEC.md
/// requires such sessions to stay listed and be clearly marked, and their
/// lifecycle controls stay live because the helm's refusal (which names
/// the host's state) is a better answer than a dead button. `selected`
/// composes with staleness rather than replacing it — the stale dimming
/// lives on `.session-row-open`'s opacity while the selection highlight is
/// the ROW's background, so a selected stale row shows both truthfully.
///
/// `menu_open` is the third, and it is not decoration: the panel hangs
/// below-left of its toggle and covers the rows under it (see
/// `menu_panel::menu_panel_style`'s anchor doc), so "which of the several
/// visible ⋯ is the open one" has to be answerable from the ROW, not just
/// from the small toggle glyph — the toggles themselves stay uncovered
/// and clickable, which is exactly why several of them are visible at
/// once with one menu up. It composes with the other two the same
/// way — app.css keeps the selected row's own accent fill when both are
/// on, rather than letting the neutral menu tint erase the selection
/// SPEC.md requires to stay readable at a glance.
///
/// Static strings per combination, matching the prior two-state shape,
/// rather than a formatted class string.
fn row_class(stale: bool, selected: bool, menu_open: bool) -> &'static str {
    match (stale, selected, menu_open) {
        (true, true, true) => "session-row stale selected menu-open",
        (true, true, false) => "session-row stale selected",
        (true, false, true) => "session-row stale menu-open",
        (true, false, false) => "session-row stale",
        (false, true, true) => "session-row selected menu-open",
        (false, true, false) => "session-row selected",
        (false, false, true) => "session-row menu-open",
        (false, false, false) => "session-row",
    }
}

/// The row menu toggle's accessible name: the session's identity, clamped.
///
/// Every row renders an identical "⋯" toggle, so the NAME is the only
/// thing assistive technology can distinguish the buttons by — an
/// unnamed toggle invites renaming or deleting the wrong session. The
/// clamp exists because a title has no length bound (tens of KB is
/// legal) and an accessible name is read aloud in full; 64 characters
/// is plenty to tell sessions apart, and the ellipsis says something
/// was cut. Char-based, not byte-based, so a multi-byte title can never
/// split a codepoint.
fn menu_label(title: &str) -> String {
    format!("session actions for {}", clamp_title(title))
}

// ===== The open menu's focus and keyboard behavior =====================
//
// The impure half of the keyboard support lives in `menu_panel`
// (`handle_menu_key`, `remember_menu_item`, `focus_menu_toggle`, imported
// above — `handle_menu_key` calls `focus_menu_item` internally there,
// which is why this row imports neither it nor `next_menu_focus`) now
// that the session row and the host row share one generic implementation
// of it — see that module's own doc for what is shared and why. This
// row's own job is narrower: build the `MenuWiring` those functions take,
// and supply the two things only a row itself can know —
// `data-session-id` as the DOM marker `focus_menu_toggle` searches for,
// and `.session-row-menu` as this row's own toggle class.

// ===== Compacting the row's two long fields ==========================
//
// Both helpers below shorten what the row DISPLAYS and nothing else. The
// untouched original always rides along in a `title` attribute (see the
// rsx), because every abbreviation here is lossy in a way a user may need
// to undo: `~` hides which account's home a path is under, and the compact
// invocation hides every argument that is not one of the markers.

/// Fold a leading home directory into `~`, or return the path unchanged.
///
/// The rule is deliberately narrow: `/home/<user>` and `/Users/<user>`,
/// followed by at least one more segment, become `~` plus that remainder.
/// `/home/alice/src/api` reads `~/src/api`; `/home/`, `/home//x`, and
/// `/homework/x` are left alone because none of them names an account, and
/// three further shapes are excluded below.
///
/// ## What this does NOT do, and why
///
/// It does not know the session's real home. `SessionInfo` carries no home
/// directory (the supervisor expands `~` at create time and stores the
/// result), so the only thing available on this side of the wire is the
/// SHAPE of the path — hence a pattern match rather than a lookup. That
/// makes the `~` a claim about the path's SHAPE, not a verified claim
/// about whose home it is: a session running as `bob` with a cwd under
/// `/home/alice` still renders `~`, and the `title` attribute — always the
/// untouched original — is where the truth stays one hover away. A future
/// wire change that put a per-host home directory on `SessionInfo` would
/// let this become an exact lookup instead of a shape guess; nothing about
/// today's rendering depends on that never happening.
///
/// `/root` is deliberately not folded. It is already shorter than most
/// path segments, so abbreviating it would buy four characters in exchange
/// for erasing the one home directory whose owner the path actually names.
///
/// No case folding: macOS volumes are usually case-insensitive, but the
/// cwd is whatever the supervisor recorded, and a lowercase `/users/bob`
/// is rare enough not to be worth guessing about.
///
/// ## Three shapes that look right but are not, excluded on purpose
///
/// - **A dot segment anywhere after the prefix.** `/home/../etc` does not
///   resolve under any home at all, and `/home/alice/../bob` does not
///   resolve under alice's — folding either would assert something the
///   path itself contradicts.
/// - **`/Users/Shared`.** macOS's shared folder is not a personal home, so
///   `~` there would falsely claim the path belongs to whichever account
///   happens to be reading the row.
/// - **No segment after the account.** `/home/alice` bare (or with only a
///   trailing slash) has nothing for `~` to stand in for — folding it would
///   erase the one thing the path says rather than shorten it, so it stays
///   absolute.
fn abbreviate_home(cwd: &str) -> String {
    for prefix in ["/home/", "/Users/"] {
        let Some(rest) = cwd.strip_prefix(prefix) else {
            continue;
        };
        if rest
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        {
            continue;
        }
        let Some(slash) = rest.find('/') else {
            continue;
        };
        let (user, remainder) = rest.split_at(slash);
        // A zero-length user segment means `/home/` or `/home//…`: no
        // account is named, so there is no home to fold away. A
        // one-byte remainder is just the trailing `/` with nothing past
        // it — the bare-account case above, spelled with a slash.
        if user.is_empty() || remainder.len() <= 1 {
            continue;
        }
        if prefix == "/Users/" && user == "Shared" {
            continue;
        }
        return format!("~{remainder}");
    }
    cwd.to_string()
}

/// Vendor-recognized executables and the unattended-mode flags worth naming
/// for each, most consequential first within a program's own list.
///
/// Keyed by the program's BASENAME rather than a flat flag table: `--yolo`
/// belongs to Codex, and matching it against ANY program's argv would badge
/// `echo --yolo` or a future tool that happens to share a flag spelling with
/// a vendor it has nothing to do with. A basename absent from this table
/// earns no marker no matter what its arguments look like — the row does
/// not guess at a command it does not recognize.
///
/// These flags share one property: they change what the agent is ALLOWED to
/// do without asking, which is the one fact about a command line worth four
/// characters of a sidebar row. Everything else — model pins, prompts,
/// working-directory overrides — is argument noise at this size and stays
/// in the `title` attribute with the rest of the line. Within one program's
/// list, order is precedence: an invocation carrying two of that vendor's
/// flags renders the first one listed.
const INVOCATION_MARKERS: &[(&str, &[(&str, &str)])] = &[
    (
        "claude",
        &[
            // Claude Code: skips every permission prompt.
            ("--dangerously-skip-permissions", "skip-perms"),
        ],
    ),
    (
        "codex",
        &[
            // The unabbreviated flag: bypasses approvals AND the sandbox.
            ("--dangerously-bypass-approvals-and-sandbox", "no-sandbox"),
            // `--yolo` is Codex's own alias for the flag directly above —
            // same bypass of approvals AND sandbox, shorter to type. It is
            // NOT the sandboxed auto mode; see `--full-auto` below for that
            // one, and do not conflate the two in future edits here.
            ("--yolo", "yolo"),
            // Full auto-approval, but SANDBOXED: prompts are skipped, the
            // sandbox stays enforced. Strictly less permissive than the two
            // flags above, which is exactly why it earns a marker of its
            // own rather than collapsing into "yolo".
            ("--full-auto", "full-auto"),
        ],
    ),
];

/// The row's parsed view of a launch command: the program's basename to
/// show, plus an optional marker for a recognized unattended-mode flag.
///
/// Two fields rather than one formatted string, because the row renders
/// each into its own bidi-isolated span (see `SessionRow`'s rsx and
/// `crate::peer` for why): joining them here and handing the row one
/// interpolated string would let a directional override inside the
/// basename — itself text from the invocation, not this UI's own words —
/// reorder the marker that follows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompactInvocation {
    pub(super) basename: String,
    pub(super) marker: Option<&'static str>,
}

/// The row's one-glance parse of a launch command: the program's basename,
/// plus a marker for a recognized unattended-mode flag when its vendor's
/// program is the one being run.
///
/// `claude --dangerously-skip-permissions --model opus` parses to
/// `claude` + `skip-perms`; `/usr/bin/codex --yolo` to `codex` + `yolo`;
/// `sleep 300` to `sleep` + no marker. See [`INVOCATION_MARKERS`] for the
/// flags that earn a marker, which programs they are tied to, and why the
/// rest are dropped.
///
/// Argv is real shell-word splitting (`shell_words::split`), the same
/// parser `farhelm-supervisor` uses to turn a profile's invocation string
/// into the argv it execs — this is a DISPLAY helper reusing that logic,
/// not a second, divergent implementation of it. Scanning for a marker
/// STOPS at a bare `--`: everything after it is positional argument data by
/// shell convention, not a flag this program is reading, so `codex --
/// --yolo` shows no marker even though the literal text is present.
///
/// ## Fallback
///
/// A string `shell_words` cannot parse at all (an unbalanced quote, a
/// trailing unescaped backslash) — which the supervisor will not create,
/// but a route stub or a future wire change could deliver — renders as the
/// trimmed input with no marker, exactly as an invocation with no
/// non-whitespace characters at all does. Guessing further than the parser
/// itself could resolve would be inventing structure for text that has
/// none; the neutral fallback says only what was actually sent.
///
/// A program token that ends in `/` (`/usr/bin/`, which is not a program)
/// has no basename to take, so the whole token stands.
fn compact_invocation(invocation: &str) -> CompactInvocation {
    let trimmed = invocation.trim();
    let fallback = || CompactInvocation {
        basename: trimmed.to_string(),
        marker: None,
    };
    let Ok(argv) = shell_words::split(invocation) else {
        return fallback();
    };
    let Some(program) = argv.first() else {
        return fallback();
    };
    let basename = match program.rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => program.clone(),
    };
    // Only the arguments BEFORE a bare `--` are candidates: past it, shell
    // convention says everything is positional data, not a flag this
    // program parses as its own.
    let leading_args: Vec<&str> = argv[1..]
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .map(String::as_str)
        .collect();
    let marker = INVOCATION_MARKERS
        .iter()
        .find(|(vendor, _)| *vendor == basename)
        .and_then(|(_, flags)| {
            flags
                .iter()
                .find_map(|(flag, marker)| leading_args.contains(flag).then_some(*marker))
        });
    CompactInvocation { basename, marker }
}

#[cfg(test)]
std::thread_local! {
    // How often the real row component ran in the callback-memoization
    // regression below. Thread-local because each Dioxus virtual DOM is
    // single-threaded while the Rust test harness runs tests concurrently.
    pub(super) static SESSION_ROW_RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One row, in two layers. The row itself is a plain `<div>` wrapper
/// around two real `<button>`s — the open button (the session's stacked
/// identity lines, see "Host and staleness" below) and the small "⋯" actions-menu
/// toggle beside it. Everything else — rename, clone, stop, archive,
/// delete, and their confirm prompts — mounts inside the floating panel the
/// toggle anchors, and only while that panel is open. Real buttons
/// rather than a `div` with `role`/`tabindex`/a hand-rolled `onkeydown`:
/// every action gets Enter- and Space-activation, focus styling, and
/// screen-reader semantics for free (a hand-rolled div also had a latent
/// bug: Space on a focused element scrolls the page unless the handler
/// prevents default, which native button activation never triggers).
///
/// The wrapper is a `div`, not a `button`, because HTML forbids
/// interactive content nested inside a `<button>` — a whole-row button
/// could not legally host the toggle. Tab order follows the layers:
/// closed, it walks open → toggle and on to the next row; open, the
/// panel's controls follow the toggle (rename → clone → stop → archive →
/// delete, as visible per `RowControlVisibility`); confirming, the
/// panel holds consequence text plus confirm → cancel (with initial
/// FOCUS on cancel — see "Focus-on-open" below); renaming, the panel
/// holds the current title plus the field's input → save → cancel.
///
/// ## Host and staleness (PLAN_M6.md item 6)
///
/// The row is always two lines: the title line (the title, the
/// stale/archived/status badges, the locality glyph, an optional host name
/// for a remote or unknown session, and the last-activity age) and a meta
/// line carrying the abbreviated cwd beside a compact invocation badge.
/// Each field ellipsizes alone in the fixed-width sidebar; the host name
/// specifically occupies a BOUNDED slot on the title line (`.session-host`
/// in app.css, `flex: 0 1 auto` with a `max-width` cap) rather than a
/// line of its own, so a long destination cannot push the title to
/// nothing and a long title cannot hide the host entirely.
///
/// The two-line shape is a density decision (2026-08-23, the UI refresh),
/// and it reverses the interviewed row contents recorded in
/// BUGS_BURNDOWN.md's "Decisions (interviewed 2026-08-13)", which called
/// for the host and the full invocation on every row. In a fleet that is
/// mostly local, a dedicated host line said "this machine" over and over,
/// and the full invocation line was usually an absolute path plus flags
/// that ellipsized into indistinguishability — two lines of row height for
/// near-zero information. Both are still reachable, and neither ever costs
/// a line of its own again (2026-09-03 moved the host onto the title line
/// rather than reviving its dedicated one — see `SPEC_impl.md`'s
/// sidebar-row paragraph for that reversal's own reasoning): a remote or
/// unknown session's host name rides the title line's bounded slot, and
/// the full cwd and invocation are on the respective elements' `title`
/// attributes. See `abbreviate_home` and `compact_invocation` for what
/// each abbreviation costs.
///
/// A row the helm marked stale
/// — its host is in some non-connected state — is dimmed and badged rather
/// than hidden, per SPEC.md's "stay in the list … clearly marked". Its
/// stop/rename/delete controls stay ENABLED, which looks like an oversight
/// and is not: the helm refuses those operations with the host's own state
/// named in the message ("host 3 is unreachable-reprobing, so this operation
/// is refused and nothing was queued: …"), and that message is strictly more
/// useful than a greyed-out button. Nothing queues either way — SPEC.md v1
/// refuses rather than deferring — so there is no risk of a click being
/// quietly banked.
///
/// `data-session-id` stays on the outer wrapper for Playwright to key off
/// of, joined by `data-session-stale` and `data-session-selected` (the
/// latter mirrors the `.selected` highlight — see `RowState::selected` —
/// so e2e can assert selection without matching CSS classes). No
/// `stop_propagation` on the
/// stop/delete buttons: the wrapper
/// `<div>` has no click handler of its own for a click to bubble into
/// (the open action lives on its own sibling button), so there is nothing
/// here for propagation to trigger by accident.
///
/// `error` is this row's own entry from `ListView`'s per-session error map
/// (`None` when its last action succeeded or none has run yet); `busy` is
/// whether a stop/delete for THIS session is currently in flight, which
/// disables both action buttons — both are `ListView`'s per-session state
/// (see its own docs for why a single shared error/in-flight slot would be
/// wrong), just narrowed to this one row by the caller before it gets here.
///
/// `nav_disabled`, unlike `error`/`busy`, is NOT this row's own state — it
/// is `ListView`'s single global "something is in flight somewhere" flag
/// (see `nav_locked` in `ListView`), applied identically to every row's
/// open button. Opening ANY row swaps which session the keyed
/// `SessionView` shows, tearing the previous one down mid-operation, and
/// repaints this list under whatever operation is still in flight — the
/// point of disabling it is to keep the selection still until every
/// in-flight create/stop/delete has delivered its result.
///
/// ## Inline delete confirmation
///
/// `confirming` (also `ListView`'s per-session state, same discipline as
/// `error`/`busy`) swaps the stop/delete pair out for a prompt plus
/// **confirm delete**/**cancel** in place — there used to be a
/// `window.confirm()` call here instead; wry ships no native JS dialogs on
/// macOS's WKWebView (observed directly running the desktop build), which
/// made delete-on-a-live-session silently do nothing on that target. An
/// in-page prompt has no such platform dependency, so it replaces the
/// eval-based one everywhere, not just on desktop. While it is open, tab
/// order walks confirm delete → cancel; initial FOCUS lands directly on
/// cancel regardless of tab order (see below).
///
/// The `.session-row-open` button (title/cwd/invocation/badge) stays
/// VISIBLE while a prompt is pending, merely `disabled`: the prompt lives
/// in the floating actions panel now, which overlays the rows below
/// instead of competing with the button for the row's own space, so the
/// MT-8 overflow that once forced the button out of layout entirely
/// (`display: none` behind a `prompting` class) cannot recur by
/// construction. Disabling it is still required — cancel must be the only
/// way back to normal, never an implicit click on open (see `confirming`
/// in `ListView`).
///
/// The prompt itself is TWO separate elements, not one combined sentence:
/// `.confirm-consequence` (from `confirm_consequence`, fixed wording with
/// no title in it at all) and `.confirm-title` (the title alone, quoted).
/// Both are ordinary Dioxus text interpolation, never `document::eval` —
/// a title containing quotes or JS-source-looking text (plausible, since a
/// supervisor over `--ssh` may be a different, possibly untrusted host)
/// renders as inert DOM text, never something parsed as script, which is
/// what makes the injection concern the old eval path needed
/// `serde_json`-encoding to guard against moot on this path. The SPLIT
/// exists for a different reason: a legal title can be tens of KB with no
/// whitespace at all, and app.css lets `.confirm-title` (only) shrink and
/// ellipsize under space pressure while `.confirm-consequence` never
/// shrinks — so the safety-critical "will be killed" half can never be
/// the one that gets clipped, which a single combined, single-ellipsized
/// string could not promise once the title ran long enough. Rendering the
/// consequence element FIRST is what makes "read the risk before the
/// title" the actual reading order, not just an incidental visual
/// side effect that a later DOM-order change could quietly undo.
///
/// ## Inline rename (PLAN_M5.md item 6)
///
/// `renaming` swaps the actions panel's rename/clone/stop/archive/delete
/// set for the session's own title plus `rename::RenameForm`, disabling (not
/// hiding) the open button exactly as the confirm prompt does. The two
/// states are mutually exclusive by construction — `ListView` refuses to
/// open either while the other is showing — so the branches below can be a
/// plain if/else chain rather than a composition of overlays.
///
/// Repeating the title inside the panel is load-bearing, not decoration:
/// a REFUSED rename must show the rejected draft NEXT TO the name that
/// still stands (SPEC.md requires the old title to stay while the
/// supervisor's refusal is shown), and the row's own title line may be
/// ellipsized past recognition in the narrow sidebar.
///
/// The draft itself is `ListView`'s (`rename_draft`), seeded when the
/// field opens; everything the submitted string then goes through is
/// `ListView`'s too. This component neither validates it nor decides what
/// a refusal means.
///
/// Focus-on-open uses the plain HTML `autofocus` attribute on the cancel
/// button (below), not Dioxus's `onmounted`/`set_focus` API: `set_focus`
/// returns a `Result` future that can fail (`MountedError`, e.g. on a
/// renderer that does not support it), and since focus-on-cancel is a
/// safety default — landing keyboard focus on the SAFE action before a
/// stray Enter/Space can reach anything — silently discarding that
/// `Result` would let the safety behavior vanish with nothing to show for
/// it. `autofocus` cannot fail the same way: it is applied by the browser
/// itself at parse/insert time as a plain attribute, with no fallible
/// async call in the UI's own control to get wrong or ignore. It fires
/// whenever the cancel button is freshly created inside the `if
/// confirming` branch — which, since the prompt lives in the on-demand
/// panel, can be MORE than once per confirmation: the confirming flag
/// survives the panel closing (see `confirming` in `ListView`), so
/// closing and reopening the panel remounts the prompt and lands focus
/// on cancel again. That repeat is the safe direction — every fresh
/// appearance of the prompt starts with the escape hatch focused.
///
/// ## Keyboard
///
/// The item list is a real `role="menu"` of `role="menuitem"` buttons,
/// and it behaves like one:
///
/// - Opening it enters it. Pointer, Enter, Space, and ArrowDown all land
///   focus on the FIRST command; ArrowUp on a closed toggle opens onto
///   the last. The intent is recorded at open time and honoured by the
///   item that matches it as it mounts (`MenuOpenIntent`,
///   `remember_menu_item`).
/// - ArrowDown/ArrowUp step (wrapping), Home/End jump to the ends. The
///   toggle answers the same keys while the menu is open, which is how a
///   user who stepped back out to it gets in again.
/// - The whole menu is ONE tab stop: a roving `tabindex` gives only the
///   focused item (or, before focus lands, the first) `tabindex="0"`, and
///   Tab/Shift+Tab dismiss the menu and put focus back on the toggle it
///   stands in for — from which the next Tab continues out of the row
///   natively. Walking every command with Tab is what `role="menu"`
///   promises not to make anyone do. See `menu_panel::MenuKeyAction::Exit`
///   for why the browser's own focus move is suppressed rather than ridden.
/// - Escape closes and hands focus back to the "⋯". So does every
///   automatic dismissal that took the menu away from a focused item; see
///   the dismissal effect in the body for why that teardown is
///   centralized rather than written per key.
///
/// The decisions are pure functions in `menu_panel` (`menu_key_action`,
/// `next_menu_focus`, `closed_toggle_key_intent`); `menu_panel`'s
/// `handle_menu_key` is the one place they meet a real event, shared with
/// the host row's menu (see that module's own doc for why).
///
/// The confirm and rename sub-states deliberately bind NOTHING. Their
/// contents are not menu items — a text field and a two-button prompt —
/// and arrow keys inside a rename field belong to the caret, not to a
/// menu. Escape is left unbound there too, and that one is a decision
/// rather than an omission: closing the panel does not clear
/// `ListView`'s confirming/renaming flag (see `menu_panel_placement_style`
/// for why that state outlives the panel), so an Escape that dismissed
/// the prompt without answering it would leave the row primed to reopen
/// straight back into the same prompt.
///
/// What each of those states focuses on arrival differs, and the
/// difference is deliberate rather than an inconsistency to iron out. A
/// CONFIRMATION autofocuses its cancel button, because the risk is a
/// stray Enter landing on a destructive action; cancel is one keystroke
/// away from the moment the prompt appears. RENAME autofocuses its text
/// area (`rename::RenameForm`), because the whole point of opening it is
/// to type, and its cancel sits after Save in tab order — so backing out
/// of a rename is Shift+Tab away, not Escape and not one Tab.
#[component]
#[allow(clippy::too_many_arguments)]
pub(super) fn SessionRow(
    session: Session,
    state: RowState,
    rename_draft: Signal<String>,
    on_open: EventHandler<Session>,
    /// The "clone" menu item's click: hands the row's own `Session` up so
    /// `ListView` can seed a fresh create form from it
    /// (`create_form::CreatePrefill`). Nothing is mutated or restarted
    /// here — this row's only job is to say WHICH session was cloned.
    on_clone: EventHandler<Session>,
    /// The "replace" menu item's click: opens `confirming_replace`, hands
    /// the row's own `Session` up. Unlike `on_clone` this DOES eventually
    /// mutate the fleet — `ListView`'s handler calls `api::replace_session`
    /// — but not from this click alone; see `on_confirm_replace` for the
    /// step that actually acts.
    on_replace: EventHandler<Session>,
    on_confirm_replace: EventHandler<String>,
    on_cancel_replace: EventHandler<String>,
    on_stop: EventHandler<String>,
    on_delete: EventHandler<DeleteTarget>,
    on_confirm_delete: EventHandler<String>,
    on_cancel_delete: EventHandler<String>,
    on_archive: EventHandler<Session>,
    on_confirm_archive: EventHandler<String>,
    on_cancel_archive: EventHandler<String>,
    /// The read/unread toggle's click, from either the menu item or the
    /// row's own dot (SPEC.md, Status): the session id and
    /// the target `seen_activity_at` to PUT — `Some(effective_activity)` to
    /// mark read, `None` to mark unread. A tuple like `on_rename_start`
    /// rather than the whole `Session` like `on_clone`/`on_archive`: the
    /// caller needs nothing else about the row, and computing the target
    /// value here (where the current unseen predicate is already in scope)
    /// keeps `ListView` from having to re-derive it.
    on_mark_seen: EventHandler<(String, Option<i64>)>,
    on_rename_start: EventHandler<(String, String)>,
    on_rename_submit: EventHandler<(String, String)>,
    on_rename_cancel: EventHandler<()>,
    on_menu_toggle: EventHandler<String>,
) -> Element {
    let RowState {
        error,
        busy,
        confirming,
        confirming_archive,
        confirming_replace,
        renaming,
        nav_disabled,
        menu_open,
        selected,
        locality,
        activity,
    } = state;
    #[cfg(test)]
    SESSION_ROW_RENDERS.with(|renders| renders.set(renders.get() + 1));
    // The two abbreviations the dense row runs on, computed once per
    // render beside the full strings they stand in for. Both `title`
    // attributes carry the original: an abbreviation the user cannot undo
    // would make the sidebar's own claim about a session unverifiable.
    let cwd_shown = abbreviate_home(&session.cwd);
    // Only for a session NOT created from a profile: the branch below that
    // renders `source_profile`'s snapshotted name never reads this value at
    // all, so parsing argv and scanning for a marker on every render of a
    // profile-backed row would be pure waste.
    let invocation_compact = session
        .source_profile
        .is_none()
        .then(|| compact_invocation(&session.invocation));
    // `None` for a status nothing has classified yet, and the row then
    // renders no badge ELEMENT at all rather than an empty one — see
    // `status_badge`'s own docs for why an empty badge box would be the
    // same mistake in CSS.
    //
    // Computed once and shared with `controls` below: the badge's colour and
    // the menu's mark-read/mark-unread offer are two independent consumers
    // that must describe the SAME verdict about this row's unseen output.
    // Calling `has_unseen_output` once and handing both consumers the
    // result is what makes that agreement structural rather than a
    // convention two call sites have to maintain by hand.
    let unseen = session.has_unseen_output();
    let badge = status_badge(&session.status, session.annotation.as_deref(), unseen);
    // The browser suite's stable wire token for locality, the same role
    // `data-host-kind` plays in the host panel: a plain string rather than
    // `Debug`'s derived spelling, so a rename of the enum's variants (their
    // Rust-side naming is free to change) does not silently rewrite what
    // every existing selector matches on.
    let locality_attribute = match locality {
        HostLocality::Local => "local",
        HostLocality::Remote => "remote",
        HostLocality::Unknown => "unknown",
    };
    let open_session = session.clone();
    let stop_id = session.id.clone();
    let delete_target = DeleteTarget {
        id: session.id.clone(),
        status: session.status.clone(),
    };
    let archive_target = session.clone();
    let clone_target = session.clone();
    let replace_target = session.clone();
    let confirm_id = session.id.clone();
    let cancel_id = session.id.clone();
    let confirm_archive_id = session.id.clone();
    let cancel_archive_id = session.id.clone();
    let confirm_replace_id = session.id.clone();
    let cancel_replace_id = session.id.clone();
    let rename_start = (session.id.clone(), session.title.clone());
    let rename_submit_id = session.id.clone();
    // The toggle is offered on a LIVE row (running, waiting, idle — SPEC.md;
    // an ended session has no dot and no meaningful unseen state) whose helm
    // answered the seen-state question at all (`unseen.is_some()`); staleness
    // is deliberately NOT part of this predicate — a session on an
    // unreachable host still has a last-known dot to toggle, and the route
    // that serves this write is itself helm-local with nothing to refuse for
    // an unreachable host (SPEC_impl.md's `session_seen` paragraph).
    let offers_mark_seen = session.status.is_live() && unseen.is_some();
    let controls = row_control_visibility(session.archived, offers_mark_seen);
    // "mark read" when the row currently has unseen output, "mark unread"
    // otherwise — the label follows the CURRENT predicate every render,
    // never a value captured once.
    let mark_seen_label = if unseen == Some(true) {
        "mark read"
    } else {
        "mark unread"
    };
    // The value this row's toggle click sends: clearing the seen stamp
    // (`None`) when marking unread, or the row's current effective activity
    // (`Some`) when marking read — `api::mark_seen`'s own contract.
    let mark_seen_target = (
        session.id.clone(),
        (unseen == Some(true)).then(|| session.effective_activity()),
    );
    // A second clone for the dot's own click closure below: the menu
    // item's closure and the dot's are two independent `move` closures
    // in the same render, and each needs to own a copy of the target
    // rather than fight the other for the one original.
    let dot_mark_seen_target = mark_seen_target.clone();
    let menu_id = session.id.clone();
    // This render's item list, derived from the same visibility answer
    // that decides whether each item renders at all — an archived row's
    // menu is Rename, Clone, Replace, Delete, in that order, with Stop and
    // Archive withdrawn, so Delete sits wherever THIS shorter list puts it
    // rather than at whatever position a fixed numbering across every
    // retention state would give it. Every focus position below is read
    // out of this one value (see `MenuOrder`), so the rendered list and the
    // navigable list cannot disagree.
    let menu_order = session_menu_order(controls);
    // Whether the panel is currently showing its ITEM list, as opposed to
    // a confirm prompt or the rename field. Only the item list is a menu
    // — see the "Keyboard" section above for why the other two states
    // carry neither the ARIA role nor any key binding.
    let showing_menu_items = !(confirming || confirming_archive || confirming_replace || renaming);
    // The accessible name for the panel's prompt states. Only read when
    // one of them is showing; the menu state names its inner list
    // instead. Same clamp as the toggle's own name, for the same reason
    // (see `clamp_title`).
    let prompt_label = if confirming {
        format!("delete confirmation for {}", clamp_title(&session.title))
    } else if confirming_archive {
        format!("archive confirmation for {}", clamp_title(&session.title))
    } else if confirming_replace {
        format!("replace confirmation for {}", clamp_title(&session.title))
    } else {
        format!("rename {}", clamp_title(&session.title))
    };
    // The separator earns its place only when something actually
    // precedes delete. Rename is unconditional today, so this is always
    // true in practice — but a separator as the list's FIRST child would
    // be a rule under nothing, and deriving the answer costs one lookup.
    let delete_follows_a_separator = menu_order
        .position(MenuAction::Delete)
        .is_some_and(|position| position > 0);
    // The row's identity, once per key handler: Escape closes the menu
    // through the same `on_menu_toggle` a click uses, and each closure
    // owns what it captures — the same reason the click handlers above
    // each hold their own clone.
    let toggle_key_id = session.id.clone();
    let rename_key_id = session.id.clone();
    let mark_seen_key_id = session.id.clone();
    let clone_key_id = session.id.clone();
    let replace_key_id = session.id.clone();
    let stop_key_id = session.id.clone();
    let archive_key_id = session.id.clone();
    let delete_key_id = session.id.clone();
    // The toggle's own `MountedData`, captured once via `onmounted` below,
    // and where the panel believes its own screen position currently is
    // (`PanelPlacement` — its own doc has the state machine) — both
    // row-local (unlike `menu_open`, which `ListView` owns so only one
    // row's menu can ever be open). `ListView` has no business knowing
    // this row's screen geometry, and this row has no business deciding
    // WHETHER its menu is open — the split mirrors that division exactly.
    let mut toggle_handle = use_signal(|| None::<Rc<MountedData>>);
    let placement = use_signal(|| PanelPlacement::Unmeasured);
    // The mounted menu items, for the arrow keys to move focus between —
    // see `MenuItemHandles`. Cleared on every fresh open (the toggle's
    // `onclick`) AND on every close (the dismissal effect below) rather
    // than trusted to be overwritten: the panel unmounts when the menu
    // closes, so every handle in here is detached from that moment on,
    // and each one is a strong `Rc` keeping a dead DOM node alive with
    // it.
    let mut item_handles: MenuItemHandles = use_signal(HashMap::new);
    // Which item currently holds keyboard focus, as a position in this
    // render's `MenuOrder`, or `None` when focus is not on an item at
    // all. Two things read it and each needs it to mean exactly that: the
    // roving `tabindex` (which item is the menu's single tab stop) and the
    // dismissal teardown (was focus INSIDE the menu when it closed, and
    // therefore ours to hand back). The arrow keys used to read it too, as
    // their sense of where they are stepping from, but no longer do — see
    // `menu_requested`, just below, for why that moved.
    //
    // Maintained from both directions on purpose. Our own focus moves
    // write it synchronously, ahead of the asynchronous request, so the
    // roving `tabindex` reflects the intended destination immediately. The
    // items' `onfocusin`/`onfocusout` then keep it honest about focus this
    // component did not move — a pointer click straight onto an item, and,
    // load-bearingly, focus LEAVING the menu. That last case is what lets
    // the teardown below tell "the menu was taken away from a focused
    // item" (hand focus back to the toggle) from "the user went somewhere
    // else and the menu closed behind them" (leave their focus alone):
    // clicking the hosts toggle or the create form moves
    // focus first and closes the menu second, so `focusout` has already
    // cleared this by the time the teardown runs. An item UNMOUNTING
    // cannot fire `focusout` here — a removed node's events never reach
    // the delegated listener — so a scroll or resize dismissal correctly
    // keeps its position and gets the handback.
    //
    // That same unmount blindness is why the TOGGLE clears this too (its
    // `onfocusin` below, through `forget_menu_focus`): this panel swaps its
    // whole item list out for the rename field or a confirm prompt without
    // ever closing, so the item holding focus can vanish silently and leave
    // this signal — and `menu_requested` with it — naming a position focus
    // has long since left.
    let mut menu_focus = use_signal(|| None::<usize>);
    // The last position a keyboard step asked focus to move TO — see
    // `MenuWiring::requested`'s own doc for why this has to exist
    // separately from `menu_focus` (F5/COR-FOCUS-BURST follow-up: an older
    // in-flight focus request's `onfocusin` can land after a newer press
    // already moved `menu_focus` on, and only a signal DOM events never
    // touch survives that). Cleared alongside `menu_focus` wherever the
    // menu opens or closes, below, and wherever the toggle takes focus
    // (`forget_menu_focus`), and NOT reconciled against a mid-open
    // item-set change the way `menu_focus` is: `next_menu_focus`'s existing
    // out-of-range handling already treats a stale index as "not on an
    // item" and re-enters at an end, the same tolerance a stale
    // `event_origin` already relies on, so a request left pointing at a
    // withdrawn action's old slot degrades no worse than that.
    let mut menu_requested = use_signal(|| None::<usize>);
    // The order `menu_focus`'s stored position was last recorded against —
    // seeded from this render's own list, so the first run of the
    // item-set-change effect below (on mount) compares a list against
    // itself and correctly finds nothing to reconcile. Bookkeeping for that
    // one effect alone; nothing else in this row should read it.
    let mut previous_menu_order = use_signal(|| menu_order);
    // Where focus should land as the panel mounts, recorded by whatever
    // opened the menu and consumed by the first item that matches it (see
    // `remember_menu_item`). Row-local, and always `None` while the menu
    // is closed.
    let mut open_intent = use_signal(|| None::<MenuOpenIntent>);
    // The row's serialized focus pipeline — see `MenuFocusQueue` for the
    // interleaving it exists to prevent.
    let focus_queue = MenuFocusQueue {
        target: use_signal(|| None::<Rc<MountedData>>),
        draining: use_signal(|| false),
    };
    // Bumped on every FRESH open (the toggle's `onclick`, opening branch
    // only — never on close, and never by the `onmounted` heal path,
    // which reuses whatever generation is already current rather than
    // starting a new one). A still-in-flight measurement from an OLDER
    // open captures its own generation before awaiting, and discards its
    // result if this counter has since moved on — see `spawn_measurement`
    // below. This closes a race a plain "reset placement, then measure"
    // scheme leaves open: two opens of the SAME toggle close enough
    // together (a fast close-then-reopen, or reopen after the row itself
    // moved) can have their measurements resolve out of order, and
    // without this counter the OLDER one finishing LAST would silently
    // overwrite the newer, correct measurement with a stale one.
    let open_generation = use_signal(|| 0_u64);
    // Shared by the toggle's `onclick` (every fresh open) and its
    // `onmounted` (healing a remount that lands with the menu ALREADY
    // open — see that handler's own doc for why that happens and why
    // re-measuring there is what fixes it). Takes no arguments: both call
    // sites close over this row's own `toggle_handle`/`placement`/
    // `open_generation` directly, and both want the exact same
    // capture-generation → query → classify-if-still-current sequence, so
    // this is the one place that sequence is written.
    let spawn_measurement = move || {
        let handle = toggle_handle;
        let mut placement = placement;
        // Captured SYNCHRONOUSLY, before the `await` below yields — a
        // generation bump landing after this line but before the read
        // would otherwise be invisible to this task, defeating the guard
        // it exists to provide.
        let generation = open_generation();
        spawn(async move {
            let measured = match handle.peek().clone() {
                Some(handle) => handle.get_client_rect().await.ok(),
                None => None,
            };
            // `measurement_outcome` (in `menu_panel`) is the apply-if-current
            // decision itself, pinned by its own unit test; this call is
            // just where the real async race it guards against actually
            // happens. See `open_generation`'s own doc for the concrete
            // interleaving a newer open superseding this one guards
            // against.
            if let Some(outcome) =
                measurement_outcome(generation, *open_generation.peek(), measured)
            {
                placement.set(outcome);
            }
        });
    };
    // Everything a FRESH open has to reset, in one place, because two
    // paths open this menu: the toggle's `onclick` (pointer, and the
    // native activation Enter and Space produce) and its `onkeydown` for
    // the closed-state arrows. Both want the identical sequence, and an
    // open that skipped any part of it would carry the previous open's
    // state into the new one.
    //
    // The captured signals are shadowed as local `mut` copies rather than
    // mutated through the closure's own captures, which keeps this an
    // `Fn` (and therefore `Copy`) and lets both event closures capture it
    // — `Signal` is a `Copy` handle into shared storage, so a local copy
    // writes exactly the same cell.
    let begin_open = move |intent: MenuOpenIntent| {
        let mut open_generation = open_generation;
        let mut placement = placement;
        let mut item_handles = item_handles;
        let mut menu_focus = menu_focus;
        let mut menu_requested = menu_requested;
        let mut open_intent = open_intent;
        // A fresh measurement every open: the toggle can move BETWEEN
        // opens (a row above it changing height, the window resizing), so
        // a rect measured for a previous open is not safe to reuse —
        // hence resetting to `Unmeasured` rather than leaving whatever
        // `placement` last held. The generation bump BEFORE
        // `spawn_measurement` is what keeps a measurement still in flight
        // for a PRIOR open of this SAME toggle from landing after this
        // reset and clobbering it — see `open_generation`'s own doc for
        // the exact race.
        open_generation += 1;
        placement.set(PanelPlacement::Unmeasured);
        // Every handle in here belongs to the previous open's
        // now-unmounted panel — see `item_handles`' own doc.
        item_handles.write().clear();
        // Cancel BEFORE recording the new intent, never after: the
        // intent's own focus request goes out as the items mount, and a
        // cancel written below it would throw that request away (see the
        // dismissal effect, where getting this order wrong once already
        // cost the toggle its focus).
        cancel_menu_focus(focus_queue);
        // Focus is on the toggle at this instant, not on an item; the
        // intent is what moves it, as each item mounts.
        menu_focus.set(None);
        menu_requested.set(None);
        open_intent.set(Some(intent));
        spawn_measurement();
    };
    // Which item is the menu's single tab stop. A `role="menu"` is one
    // stop in the document's tab order by contract — Tab enters it or
    // leaves it, arrows move within it — so exactly one item may carry
    // `tabindex="0"` and the rest `-1`. The focused item is that stop
    // while there is one; before focus has landed anywhere (the instant
    // between mount and the open-intent's focus call, or after focus has
    // left the menu without closing it) the FIRST item stands in, so the
    // menu is never a hole in the tab order.
    let menu_tab_stop = menu_focus()
        .and_then(|position| menu_order.get(position))
        .or_else(|| menu_order.get(0));
    // The bundle every menu closure below reaches through, assembled once
    // from this render's own item list — see `MenuWiring`.
    let menu_wiring = MenuWiring {
        order: menu_order,
        handles: item_handles,
        focus: focus_queue,
        focused: menu_focus,
        requested: menu_requested,
        open_intent,
        close_menu: on_menu_toggle,
    };
    // The item set can change UNDER an open menu: archiving a session
    // withdraws stop and archive while the panel stays up, and a session
    // ending (or, in principle, an old-helm connection losing the seen-state
    // field mid-session — not reachable in practice, but the predicate does
    // not assume otherwise) withdraws mark-seen the same way. Rename and
    // delete keep their DOM nodes across that change (Dioxus diffs them
    // in place), so nothing re-registers them and the withdrawn items'
    // handles would otherwise sit in the map retaining detached nodes,
    // while a `menu_focus` recorded against the longer list would point
    // past the end of the shorter one. Rebuilt here rather than in the
    // click path because no click is involved — the listing simply
    // reports a different session.
    //
    // `use_reactive` because `session.archived`/`offers_mark_seen` are plain
    // prop-derived values: an effect body that merely closed over them would
    // run once with the first render's answer and never again.
    //
    // Stale FOCUS is reconciled by ACTION identity rather than by comparing
    // the stored position against the new list's length — see
    // `menu_panel::reconcile_menu_focus`'s own doc for why a length check
    // misses a withdrawal from the MIDDLE of the list (this row's own
    // shorter list happens not to reorder around Stop/Archive today, but
    // the host row's identical effect hits the case directly, and this row
    // shares the mechanics rather than a second, narrower copy of them).
    let archived = session.archived;
    let withdrawal_close_id = session.id.clone();
    use_effect(use_reactive(
        (&archived, &offers_mark_seen),
        move |(archived, offers_mark_seen)| {
            let order = session_menu_order(row_control_visibility(archived, offers_mark_seen));
            item_handles
                .write()
                .retain(|action, _| order.position(*action).is_some());
            let focused_position = *menu_focus.peek();
            // `menu_open` is this render's own belief about whether THIS row's
            // menu is the open one — passed through so `reconcile_menu_focus`
            // can gate `Withdrawn` on it (F4/COR-SESSION-WITHDRAWAL-REOPEN):
            // `on_menu_toggle` below is an ordinary click TOGGLE, not an
            // idempotent close, and calling it when some OTHER dismissal (a
            // layout closer, a newer host-menu choice) has already closed this
            // row's menu since this prop was computed would reopen it instead.
            match menu_panel::reconcile_menu_focus(
                *previous_menu_order.peek(),
                order,
                focused_position,
                menu_open,
            ) {
                menu_panel::MenuFocusReconciliation::Unchanged => {}
                menu_panel::MenuFocusReconciliation::Moved(position) => {
                    menu_focus.set(Some(position));
                }
                // No surviving item to aim focus at. Left as-is rather than
                // cleared here: closing through `on_menu_toggle` is what the
                // dismissal effect below keys its focus-return on
                // (`was_inside`), and clearing `menu_focus` first would make
                // that check see nothing to return focus FROM. Only ever
                // reached while `menu_open` is true (see the call above), so
                // this toggle call is always a genuine close of THIS row's own
                // open menu, never a reopen.
                menu_panel::MenuFocusReconciliation::Withdrawn => {
                    on_menu_toggle.call(withdrawal_close_id.clone());
                }
            }
            previous_menu_order.set(order);
        },
    ));
    // Every close funnels through here, whichever path caused it —
    // Escape, Tab, a click on the toggle, or one of `ListView`'s
    // automatic dismissals (a sidebar scroll or resize, the hosts panel
    // or create form opening, the row reordering under a
    // refresh). Those last ones are the reason this cannot live in the
    // key handler: `ListView` owns `menu_open` and closes it without
    // consulting this row at all, so a teardown written per key press
    // would leave every automatic dismissal dropping keyboard focus onto
    // the document body.
    //
    // The row's own id, owned by this effect: the teardown runs after the
    // render whose `session` prop it would otherwise have to borrow.
    let dismiss_id = session.id.clone();
    // Focus goes back to the toggle only when it was still INSIDE the
    // menu at the moment it closed (`menu_focus`, whose own doc explains
    // how `focusout` makes that distinction reliable): reclaiming it
    // unconditionally would yank focus away from whatever control the
    // user had just moved to, which is exactly what dismissed the menu in
    // the hosts-panel and filter-bar cases.
    use_effect(use_reactive((&menu_open,), move |(menu_open,)| {
        if menu_open {
            return;
        }
        // ORDER MATTERS, and it is the opposite of the obvious one:
        // discard the pending request FIRST — it names an item of the
        // panel that just went away — and only then ask for the toggle.
        // Cancelling after requesting would clear the very target this
        // teardown just set, which is a silent way to lose focus
        // entirely.
        cancel_menu_focus(focus_queue);
        let was_inside = menu_focus.peek().is_some();
        menu_focus.set(None);
        menu_requested.set(None);
        open_intent.set(None);
        // Detached the instant the panel unmounted; see `item_handles`.
        item_handles.write().clear();
        if was_inside {
            focus_menu_toggle("data-session-id", &dismiss_id, ".session-row-menu");
        }
    }));
    let row_class = row_class(session.stale, selected, menu_open);

    rsx! {
        div {
            class: row_class,
            "data-session-id": "{session.id}",
            "data-session-stale": "{session.stale}",
            "data-session-archived": "{session.archived}",
            "data-session-selected": "{selected}",
            // The browser suite's hook for the locality glyph, the same
            // role `data-host-kind` plays on a host panel row: a stable
            // string the markup carries independent of which icon (if any)
            // actually rendered, so a test can assert the verdict without
            // depending on SVG internals.
            "data-host-locality": "{locality_attribute}",
            // Two stacked rows, not one: the buttons need a plain flex
            // ROW (see `.session-row-main` in app.css), but a per-session
            // error line needs its own full-width row underneath rather
            // than squeezing in as a fourth flex item next to the
            // buttons — hence the extra wrapper rather than putting
            // everything directly under `.session-row`.
            div { class: "session-row-main",
                button {
                    r#type: "button",
                    class: "session-row-open",
                    // The accessible counterpart of the visual highlight:
                    // the sidebar is a navigation-shaped list of open
                    // buttons, and `aria-current` is the native way to say
                    // "this one is where you are" without inventing a
                    // listbox role for a list that is not one. Absent
                    // entirely on unselected rows (a conditional
                    // attribute), not `"false"`.
                    aria_current: if selected { "true" },
                    // Disabled by ANY of the three locks: the global nav
                    // lock (any in-flight op anywhere), or this row's own
                    // confirmation or rename field being open — the
                    // simplest way to satisfy "cancel is the only way back
                    // to normal" (see the component doc above) is to make
                    // the open button inert for the whole time a prompt is
                    // showing, rather than giving it a second, competing
                    // meaning as an implicit cancel.
                    disabled: nav_disabled || confirming || confirming_archive || confirming_replace
                        || renaming,
                    onclick: move |_| on_open.call(open_session.clone()),
                    // STACKED lines rather than one squeezed flex row: the
                    // sidebar column (BUGS_BURNDOWN.md issue 5, interviewed
                    // row contents) is far too narrow for the old
                    // everything-on-one-line layout, whose min-width floors
                    // produced the MT-8 overflow class the moment space ran
                    // short. The title line leads with identity, its
                    // qualifiers, and (2026-09-03) locality — the glyph and,
                    // for a remote or unknown host, its name, sharing the
                    // line with the title rather than spending a second one
                    // on it (see the locality slot below and
                    // `SPEC_impl.md`'s sidebar-row paragraph for why); the
                    // directory/invocation pair follows on a line of its
                    // own, each field ellipsizing alone. `span` wrappers,
                    // not `div`: this all sits inside the
                    // native `.session-row-open` <button>, whose content
                    // model only permits phrasing content — a flow-content
                    // div inside a button is invalid HTML that engines and
                    // accessibility tooling may interpret inconsistently.
                    // The stacked-line layout comes from the class's CSS,
                    // not the element kind.
                    span { class: "session-row-line",
                        span { class: "session-title", "{session.title}" }
                        if session.stale {
                            span { class: "stale-badge", "stale" }
                        }
                        if session.archived {
                            span { class: "archived-badge", "archived" }
                        }
                        if let Some(badge) = badge {
                            // The row's own dot doubles as the mark-read/
                            // mark-unread MOUSE shortcut (SPEC.md, Status)
                            // — the keyboard-operable path is the `…` menu
                            // item above, which shares `mark_seen_target`/
                            // `mark_seen_label`. `dot_title` alone decides
                            // whether the dot LOOKS clickable, so both must
                            // stay `None`/no-op together for a row that does
                            // not offer the toggle.
                            StatusBadgeView {
                                badge,
                                dot_onclick: move |_| {
                                    if !offers_mark_seen || busy {
                                        return;
                                    }
                                    on_mark_seen.call(dot_mark_seen_target.clone());
                                },
                                dot_title: offers_mark_seen.then(|| mark_seen_label.to_string()),
                            }
                        }
                        // The locality slot (2026-09-03): between the
                        // badges and the age, so the title
                        // ellipsizes first under pressure and the host
                        // keeps a bounded share of the line rather than
                        // being pushed off it entirely — see `.session-host`
                        // and `.host-kind-icon` in app.css for the width
                        // split.
                        //
                        // Every row draws a glyph once its locality is
                        // KNOWN — local is a positive signal on its own row,
                        // not an absence to notice elsewhere — and an
                        // `Unknown` row draws none, because a local claim
                        // this row cannot back is exactly what
                        // `shared::session_locality` was designed never to
                        // assert. The glyph carries its own accessible word
                        // in a `.visually-hidden` span beside it (the same
                        // clip-not-remove pattern `status::StatusBadgeView`
                        // uses for a status word next to a color-only dot),
                        // since the icon itself is `aria-hidden`.
                        match locality {
                            HostLocality::Local => rsx! {
                                LocalHostIcon {}
                                span { class: "visually-hidden", "local" }
                            },
                            HostLocality::Remote => rsx! {
                                RemoteHostIcon {}
                                span { class: "visually-hidden", "remote" }
                            },
                            HostLocality::Unknown => rsx! {},
                        }
                        // The host NAME rides the same line, for a remote or
                        // unknown row only — a local row already said so
                        // with the glyph above and never repeats "this
                        // machine" in words (see `session_locality`'s doc:
                        // the row still never prints the helm's own
                        // rendering of its local host). The name is the
                        // helm's own rendering (`host_name`), denormalized
                        // onto the row so the list needs no second request
                        // — a row from a helm that sends none simply shows
                        // nothing here rather than inventing a label.
                        // Escaped and direction-isolated like every other
                        // rendering of a destination: this one names the
                        // machine a row's stop and delete will reach, so a
                        // name able to reorder the line around it could
                        // make one host's session read as another's.
                        //
                        // `title` carries the same escaped value, not the
                        // raw one — SPEC.md's promise is the FULL value on
                        // the row, and the title-line slot ellipsizes at
                        // 40% width (`.session-host` in app.css), so a long
                        // destination needs the same tooltip recovery path
                        // the directory line already has.
                        if let Some(host_name) =
                            session.host_name.as_ref().filter(|_| locality != HostLocality::Local)
                        {
                            span {
                                class: "session-host peer-value",
                                dir: "ltr",
                                title: "{display_peer(host_name)}",
                                "{display_peer(host_name)}"
                            }
                        }
                        // Beside the badge, and about something ELSE. The
                        // badge is the session's current (or, on a stale
                        // row, last-known) status; this is how long ago the
                        // supervisor last saw the agent's pane change.
                        // Neither this stamp nor `created_at` records when
                        // the status was classified, so the two are
                        // independent facts sitting next to each other, not
                        // a verdict and its freshness.
                        //
                        // Its OWN element rather than text inside the badge,
                        // and that is load-bearing in two directions. The
                        // badge's text content stays exactly the status word
                        // — which is what the browser suite asserts against
                        // and what a screen reader announces for the status
                        // — and the age stays legible as its own quiet field
                        // instead of inheriting a status color that would
                        // make "2m" look like a verdict.
                        if let Some(activity) = &activity {
                            span {
                                class: "status-time",
                                title: "{activity.absolute}",
                                "{activity.age}"
                            }
                        }
                    }
                    // Directory and invocation SHARE the last line: both are
                    // compact enough now to fit beside each other (a
                    // tilde-folded path and a one-word invocation badge),
                    // and pairing them is what gets the row to roughly half
                    // its old height. The cwd flexes and the badge does not
                    // (see `.session-row-meta` in app.css), so pressure
                    // eats the path's leading segments rather than the
                    // shorter, denser field.
                    span { class: "session-row-line session-row-meta",
                        // Two spans, not one: `.session-cwd` is the rtl
                        // clipping container that puts the ellipsis on the
                        // LEFT, and the inner `dir="ltr"` child is the bidi
                        // isolate that keeps the path's characters in
                        // logical order under it — rtl applied directly to
                        // the text would move a leading "/" to the visual
                        // right (see `.session-cwd` in app.css). The
                        // `title` carries the UNABBREVIATED path, which is
                        // what makes the `~` safe: see `abbreviate_home`
                        // for whose home it does and does not know about.
                        span { class: "session-cwd", title: "{session.cwd}",
                            span { class: "session-cwd-text", dir: "ltr", "{cwd_shown}" }
                        }
                        // The invocation slot answers "what is this?" in as
                        // few characters as the question allows. A session
                        // created FROM a profile answers with the profile's
                        // snapshotted name, which is what the user actually
                        // chose; everything else answers with the derived
                        // badge (`compact_invocation`). Either way the full
                        // command line is on the `title`.
                        //
                        // The profile case keeps the same existence data as
                        // the panel chip. Renames and unknown states use it
                        // for their qualifier and warning color; deletion is
                        // deliberately plain because a historical snapshot is
                        // not evidence that the session itself is unhealthy.
                        if let Some(source) = &session.source_profile {
                            span {
                                class: "session-invocation peer-value",
                                dir: "ltr",
                                "data-profile-existence": "{existence_word(source.existence)}",
                                title: "{source_profile_label(source)} — {session.invocation}",
                                "{display_peer(&source.name)}"
                            }
                        } else if let Some(compact) = &invocation_compact {
                            // Basename and marker are TWO isolated spans,
                            // not one interpolated string: the basename is
                            // raw text from the invocation, so a
                            // directional override inside it must not be
                            // able to reorder the marker that follows (see
                            // `CompactInvocation`'s own doc, and
                            // `crate::peer` for why `dir="ltr"` plus
                            // `unicode-bidi: isolate` together bound
                            // resolution to one element). The marker itself
                            // is trusted — it comes only from this build's
                            // own `INVOCATION_MARKERS` table — but still
                            // gets its own isolated span so the basename's
                            // bidi context can never reach past its edge.
                            span {
                                class: "session-invocation",
                                title: "{session.invocation}",
                                span {
                                    class: "peer-value",
                                    dir: "ltr",
                                    "{display_peer(&compact.basename)}"
                                }
                                if let Some(marker) = compact.marker {
                                    " · "
                                    span { class: "peer-value", dir: "ltr", "{marker}" }
                                }
                            }
                        }
                    }
                }
                // The actions menu: one small toggle beside the open
                // button, everything else in a floating panel it anchors
                // (BUGS_BURNDOWN.md issue 5's interviewed design — the row
                // itself carries no action buttons). The panel is also
                // where a destructive action CONFIRMS: clicking delete or
                // archive swaps the panel's contents for the consequence
                // line and confirm/cancel pair, keeping the whole exchange
                // on one small surface instead of bouncing the user
                // somewhere else. Rename lives here too — the ONLY
                // rename surface, by decision, which is what retires the
                // old dual-optimistic-overlay disagreement with the
                // titlebar (whose affordance the redesign removed).
                // Deliberately NOT disabled under the nav lock: opening a
                // panel mutates nothing — every action inside it carries
                // its own disabled state — and locking the toggle would
                // hide the very buttons whose disabled state tells the
                // user WHY the page is briefly inert.
                button {
                    r#type: "button",
                    class: "btn session-row-menu",
                    // The session's identity is part of the accessible
                    // name: a list renders one of these per row, and
                    // "session actions" alone leaves a screen-reader user
                    // no way to tell which session a toggle controls
                    // before activating it. Clamped, because a title can
                    // legally run to tens of KB and an accessible name is
                    // read aloud in full.
                    aria_label: menu_label(&session.title),
                    aria_expanded: menu_open,
                    // What this button opens, in the vocabulary the ARIA
                    // menu-button pattern uses — the counterpart of the
                    // item list's own `role="menu"` below, and the reason
                    // a screen reader announces "menu button" rather than
                    // leaving the "⋯" to speak for itself. It tracks the
                    // sub-state rather than claiming "menu" unconditionally:
                    // a row mid-confirmation or mid-rename opens onto a
                    // prompt, and that prompt carries `role="dialog"`, so
                    // saying "menu" there would promise a list of commands
                    // that is not what this button is about to show.
                    aria_haspopup: if showing_menu_items { "menu" } else { "dialog" },
                    // The toggle answers keys in both of its states, and
                    // they are different sets. CLOSED, it answers only the
                    // two arrows, by opening at the end each one names
                    // (`closed_toggle_key_intent`) — Escape must not
                    // become a second way to open, and Enter/Space already
                    // reach `onclick` as native button activation. OPEN,
                    // it is the way back INTO a menu whose focus has
                    // stepped out to it, so the full navigation set
                    // applies. The sub-state guard is the one exception:
                    // with a confirm prompt or the rename field showing,
                    // the toggle binds nothing, or Shift+Tab back to it
                    // would offer an Escape that dismisses the panel while
                    // leaving the prompt itself unanswered.
                    onkeydown: move |evt| {
                        if !menu_open {
                            let Some(intent) = closed_toggle_key_intent(&evt.key()) else {
                                return;
                            };
                            // Both arrows scroll by default, and this one
                            // is opening a panel measured against the
                            // toggle's current position.
                            evt.prevent_default();
                            on_menu_toggle.call(toggle_key_id.clone());
                            begin_open(intent);
                            return;
                        }
                        if !showing_menu_items {
                            return;
                        }
                        handle_menu_key(&evt, None, menu_wiring, &toggle_key_id);
                    },
                    // Focus arriving HERE means it is no longer on an item,
                    // and the panel's own item `onfocusout` cannot always
                    // say so — see `forget_menu_focus` for the unmount case
                    // that leaves both signals stale and the arrow-key
                    // misstep it caused.
                    onfocusin: move |_| forget_menu_focus(menu_wiring),
                    // Fires once this button is actually in the DOM, giving
                    // us a handle `get_client_rect()` can be called on
                    // later. Also covers a rarer case: a FAILED listing
                    // read unmounts every row (`ListView`'s render swaps to
                    // an error banner) without clearing `menu_open` — that
                    // flag is `ListView`'s, and a transient read failure is
                    // not evidence the user changed their mind — so a row
                    // that remounts on recovery can land here with
                    // `menu_open` already true and `placement` back at its
                    // freshly-initialized `Unmeasured` (a brand new
                    // row-local signal). Nothing else would ever start a
                    // measurement for that panel — the click that opened it
                    // is long gone — so it would stay invisible forever
                    // without this: healing it right here, the instant the
                    // handle FIRST becomes usable, is what a plain "measure
                    // once on open" scheme misses on this one path.
                    onmounted: move |element| {
                        toggle_handle.set(Some(element.data()));
                        if should_measure_on_mount(menu_open, *placement.peek()) {
                            spawn_measurement();
                        }
                    },
                    onclick: move |_| {
                        // `on_menu_toggle` fires FIRST and synchronously,
                        // before any `await` — this is what keeps
                        // `aria_expanded` truthful the instant the click
                        // lands and keeps a fast double-click (or two
                        // clicks landing close together on different rows)
                        // from racing an async measurement into a stray
                        // extra toggle. The panel mounts hidden
                        // (`PanelPlacement::Unmeasured`) the same render —
                        // see that variant's own doc — so opening no longer
                        // needs to WAIT for a measurement, only closing
                        // still needs none at all.
                        let opening = !menu_open;
                        on_menu_toggle.call(menu_id.clone());
                        if !opening {
                            return;
                        }
                        // A pointer open lands on the first command, the
                        // same as Enter, Space, and ArrowDown — all four
                        // arrive here or at `closed_toggle_key_intent`,
                        // and a menu button that opens its menu without
                        // entering it makes every keyboard user press one
                        // extra arrow for the list they just asked for.
                        begin_open(MenuOpenIntent::First);
                    },
                    "⋯"
                }
                if menu_open {
                    div {
                        // The panel is the POSITIONED box and nothing
                        // more; what it currently IS lives one level in
                        // (the item list's own `role="menu"`) or on the
                        // panel only while it is a prompt. The class
                        // carries the sub-state because the geometry
                        // differs — a full-bleed list of rows versus a
                        // padded prompt — and it is derived from the same
                        // `showing_menu_items` value that picks the
                        // markup, in the same expression, so the two
                        // cannot drift. (This used to key off the panel's
                        // own `role="menu"`; that attribute has moved
                        // inward, and a selector on a role the element no
                        // longer carries would have silently stopped
                        // matching.)
                        class: if showing_menu_items {
                            "session-row-menu-panel"
                        } else {
                            "session-row-menu-panel menu-prompt"
                        },
                        // The confirm and rename sub-states ARE a small
                        // named exchange: one consequence sentence and
                        // the two answers to it, or a field and its two
                        // answers, with focus deliberately placed on the
                        // safe one as it appears. `dialog` is the role
                        // that says so, and the toggle's own
                        // `aria-haspopup` above tracks it. Non-modal by
                        // omission (`aria-modal` defaults to false) —
                        // nothing behind the panel is hidden, and the
                        // row's open button is disabled rather than
                        // trapped. It deliberately does NOT bind Escape,
                        // which a dialog conventionally would: see the
                        // "Keyboard" section in this component's doc for
                        // why dismissing a prompt without answering it
                        // would leave the row primed to reopen into it.
                        role: if !showing_menu_items { "dialog" },
                        aria_label: if !showing_menu_items { prompt_label.clone() },
                        // The presentation half of `PanelPlacement`'s state
                        // machine — see that type's own doc for what each
                        // variant means and why the panel needs three
                        // states rather than a plain measured/unmeasured
                        // flag.
                        style: menu_panel_placement_style(placement()),
                        if confirming {
                            // Two elements, consequence first: an
                            // untruncatable consequence and a separately
                            // truncatable title, in THIS order, so a long
                            // title can never clip the safety-critical
                            // half (see the component doc above).
                            span {
                                class: "confirm-consequence",
                                "{confirm_consequence(&session.status)}"
                            }
                            span { class: "confirm-title", "\"{session.title}\"" }
                            button {
                                r#type: "button",
                                class: "btn confirm-delete",
                                // Disabled while the shared token is held:
                                // the handler refuses then anyway (keeping
                                // the prompt), and the attribute is that
                                // refusal made visible. Cancel stays
                                // enabled — backing out is always safe.
                                disabled: busy,
                                onclick: move |_| on_confirm_delete.call(confirm_id.clone()),
                                "confirm delete"
                            }
                            button {
                                r#type: "button",
                                class: "btn confirm-cancel",
                                // Safe default: land keyboard focus on
                                // cancel, not confirm, the instant this
                                // prompt appears — a stray Enter/Space
                                // right after the menu's delete click backs
                                // OUT of the destructive action instead of
                                // into it (see the component doc for why
                                // declarative `autofocus` over the async
                                // focus API).
                                autofocus: true,
                                onclick: move |_| on_cancel_delete.call(cancel_id.clone()),
                                "cancel"
                            }
                        } else if confirming_archive {
                            if let Some(consequence) = archive_confirmation(&session, session.tabs.len()) {
                                span { class: "confirm-consequence", "{consequence}:" }
                            } else {
                                span { class: "confirm-consequence", "archiving removes the terminal:" }
                            }
                            span { class: "confirm-title", "\"{session.title}\"" }
                            button {
                                r#type: "button",
                                class: "btn confirm-archive",
                                // See confirm-delete: refusal made visible.
                                disabled: busy,
                                onclick: move |_| on_confirm_archive.call(confirm_archive_id.clone()),
                                "confirm archive"
                            }
                            button {
                                r#type: "button",
                                class: "btn archive-cancel",
                                autofocus: true,
                                onclick: move |_| on_cancel_archive.call(cancel_archive_id.clone()),
                                "cancel"
                            }
                        } else if confirming_replace {
                            // Same two-element, consequence-first shape as
                            // the delete and archive prompts above — see
                            // the component doc's opening paragraphs for
                            // why the consequence never shrinks or
                            // ellipsizes while the title does. Unlike
                            // those two, `replace_consequence` has no
                            // `Option`/fallback branch to pick between: it
                            // is total over `SessionStatus`, so every
                            // status reaches here with real wording of its
                            // own (`status::replace_consequence`'s own
                            // doc).
                            span {
                                class: "confirm-consequence",
                                "{replace_consequence(&session.status)}"
                            }
                            span { class: "confirm-title", "\"{session.title}\"" }
                            button {
                                r#type: "button",
                                class: "btn confirm-replace",
                                // See confirm-delete: refusal made visible.
                                disabled: busy,
                                onclick: move |_| on_confirm_replace.call(confirm_replace_id.clone()),
                                "confirm replace"
                            }
                            button {
                                r#type: "button",
                                class: "btn replace-cancel",
                                autofocus: true,
                                onclick: move |_| on_cancel_replace.call(cancel_replace_id.clone()),
                                "cancel"
                            }
                        } else if renaming {
                            // The AUTHORITATIVE title stays beside the
                            // field: a refused rename must never leave the
                            // rejected DRAFT as the only name on this
                            // surface (SPEC.md's "the old title stays"
                            // while the refusal is shown).
                            span { class: "rename-current-title", "{session.title}" }
                            RenameForm {
                                draft: rename_draft,
                                busy,
                                on_submit: move |title| {
                                    on_rename_submit.call((rename_submit_id.clone(), title))
                                },
                                on_cancel: move |_| on_rename_cancel.call(()),
                            }
                        } else {
                            // The menu proper: ONLY the actionable rows,
                            // in their own element. The panel around it
                            // also holds the profile footer (a fact about
                            // the session, not a command) and, as a
                            // sibling below, any refusal line — neither
                            // belongs inside a `role="menu"`, where a
                            // screen reader would have to decide what a
                            // non-`menuitem` child means. Naming it here
                            // rather than on the panel is the same move:
                            // the name belongs to the list of commands,
                            // which is what the toggle's
                            // `aria-haspopup="menu"` promises.
                            div {
                                class: "session-row-menu-items",
                                role: "menu",
                                aria_label: menu_label(&session.title),
                                // Every item carries the same menu
                                // attachments beside its own action: the
                                // `menuitem` role, an `onmounted` that
                                // files its DOM handle under the action
                                // it performs (see `remember_menu_item`),
                                // the focus bookkeeping that keeps
                                // `menu_focus` honest, a roving
                                // `tabindex`, and an `onkeydown` that
                                // resolves its own position out of this
                                // render's `MenuOrder`.
                                //
                                // Two of those are worth naming. The
                                // roving `tabindex` makes the whole menu
                                // ONE tab stop, which is what a
                                // `role="menu"` promises: Tab leaves,
                                // arrows navigate, and Tab never walks
                                // every command one at a time. And busy
                                // items are `aria-disabled` with a
                                // guarded `onclick` rather than natively
                                // `disabled`, because a browser cannot
                                // focus a disabled control — a menu that
                                // went busy while the user was in it
                                // would consume every arrow key and be
                                // unable to honour any of them, and an
                                // item whose own action made the menu
                                // busy would lose focus mid-press,
                                // putting Escape out of reach.
                                // `.session-row-menu-item` is the shared
                                // LOOK — a full-width, left-aligned,
                                // borderless row — while the per-action
                                // class beside it stays exactly what it
                                // was, since the browser suite keys off
                                // those.
                                if controls.rename {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item session-row-rename",
                                        role: "menuitem",
                                        aria_disabled: if busy { "true" },
                                        tabindex: if menu_tab_stop == Some(MenuAction::Rename) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(
                                                menu_wiring,
                                                MenuAction::Rename,
                                                element.data(),
                                            )
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(MenuAction::Rename));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(MenuAction::Rename),
                                                menu_wiring,
                                                &rename_key_id,
                                            );
                                        },
                                        onclick: move |_| {
                                            if busy {
                                                return;
                                            }
                                            on_rename_start.call(rename_start.clone());
                                        },
                                        "rename"
                                    }
                                }
                                // Offered whenever `offers_mark_seen` says so
                                // (live row, helm answers the seen-state
                                // question) — see `RowControlVisibility`'s own
                                // doc. The LABEL follows the CURRENT unseen
                                // predicate every render, never a value
                                // captured at mount, so a session that
                                // produces output while its menu happens to
                                // be open relabels itself the moment the next
                                // listing read lands.
                                if controls.mark_seen {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item session-row-mark-seen",
                                        role: "menuitem",
                                        aria_disabled: if busy { "true" },
                                        tabindex: if menu_tab_stop == Some(MenuAction::MarkSeen) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(
                                                menu_wiring,
                                                MenuAction::MarkSeen,
                                                element.data(),
                                            )
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(MenuAction::MarkSeen));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(MenuAction::MarkSeen),
                                                menu_wiring,
                                                &mark_seen_key_id,
                                            );
                                        },
                                        onclick: move |_| {
                                            if busy {
                                                return;
                                            }
                                            on_mark_seen.call(mark_seen_target.clone());
                                        },
                                        "{mark_seen_label}"
                                    }
                                }
                                // Offered on EVERY row, archived included,
                                // unconditionally — see
                                // `RowControlVisibility`'s own doc for why
                                // clone has no visibility field to gate it
                                // at all. Archiving withdraws only the
                                // PROCESS action, stop: there is no live
                                // agent left to stop. Rename stays
                                // reachable on the same archived row, since
                                // it edits metadata rather than a process,
                                // and restart (elsewhere in this UI, not on
                                // this menu) relaunches an archived
                                // session's OWN process and un-archives it
                                // in doing so. Clone is a third, DIFFERENT
                                // thing again: rather than acting on this
                                // row's process at all, it reads this row's
                                // host, directory, title, and launch
                                // profile (or raw invocation) to seed a
                                // brand-new, independent create — the click
                                // only OPENS that form pre-filled
                                // (`create_form::CreatePrefill`); nothing
                                // here mutates or restarts anything itself,
                                // and the archived original is left exactly
                                // as it was.
                                button {
                                    r#type: "button",
                                    class: "btn session-row-menu-item session-row-clone",
                                    role: "menuitem",
                                    aria_disabled: if busy { "true" },
                                    tabindex: if menu_tab_stop == Some(MenuAction::Clone) { "0" } else { "-1" },
                                    onmounted: move |element| {
                                        remember_menu_item(menu_wiring, MenuAction::Clone, element.data())
                                    },
                                    onfocusin: move |_| {
                                        menu_focus.set(menu_order.position(MenuAction::Clone));
                                    },
                                    onfocusout: move |_| menu_focus.set(None),
                                    onkeydown: move |evt| {
                                        handle_menu_key(
                                            &evt,
                                            menu_order.position(MenuAction::Clone),
                                            menu_wiring,
                                            &clone_key_id,
                                        );
                                    },
                                    onclick: move |_| {
                                        if busy {
                                            return;
                                        }
                                        on_clone.call(clone_target.clone());
                                    },
                                    "clone"
                                }
                                // Also unconditional, directly beside clone
                                // — the row's other "make a new session
                                // from this one" action, and the one
                                // difference between them is the whole
                                // point of offering both: clone keeps this
                                // row and opens an editable form around a
                                // SECOND session, replace acts at once,
                                // deletes this row's own session, and puts
                                // the fresh one in its place. The click
                                // only opens `confirming_replace`;
                                // `on_confirm_replace` is what actually
                                // calls the API.
                                button {
                                    r#type: "button",
                                    class: "btn session-row-menu-item session-row-replace",
                                    role: "menuitem",
                                    aria_disabled: if busy { "true" },
                                    tabindex: if menu_tab_stop == Some(MenuAction::Replace) { "0" } else { "-1" },
                                    onmounted: move |element| {
                                        remember_menu_item(menu_wiring, MenuAction::Replace, element.data())
                                    },
                                    onfocusin: move |_| {
                                        menu_focus.set(menu_order.position(MenuAction::Replace));
                                    },
                                    onfocusout: move |_| menu_focus.set(None),
                                    onkeydown: move |evt| {
                                        handle_menu_key(
                                            &evt,
                                            menu_order.position(MenuAction::Replace),
                                            menu_wiring,
                                            &replace_key_id,
                                        );
                                    },
                                    onclick: move |_| {
                                        if busy {
                                            return;
                                        }
                                        on_replace.call(replace_target.clone());
                                    },
                                    "replace"
                                }
                                if controls.stop {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item session-row-stop",
                                        role: "menuitem",
                                        aria_disabled: if busy { "true" },
                                        tabindex: if menu_tab_stop == Some(MenuAction::Stop) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(
                                                menu_wiring,
                                                MenuAction::Stop,
                                                element.data(),
                                            )
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(MenuAction::Stop));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(MenuAction::Stop),
                                                menu_wiring,
                                                &stop_key_id,
                                            );
                                        },
                                        onclick: move |_| {
                                            if busy {
                                                return;
                                            }
                                            on_stop.call(stop_id.clone());
                                        },
                                        "stop"
                                    }
                                }
                                if controls.archive {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item session-row-archive",
                                        role: "menuitem",
                                        aria_disabled: if busy { "true" },
                                        tabindex: if menu_tab_stop == Some(MenuAction::Archive) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(
                                                menu_wiring,
                                                MenuAction::Archive,
                                                element.data(),
                                            )
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(MenuAction::Archive));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(MenuAction::Archive),
                                                menu_wiring,
                                                &archive_key_id,
                                            );
                                        },
                                        onclick: move |_| {
                                            if busy {
                                                return;
                                            }
                                            on_archive.call(archive_target.clone());
                                        },
                                        "archive"
                                    }
                                }
                                // The boundary before the destructive
                                // item, as a real one: sighted users
                                // already got a rule (drawn by this
                                // element's own CSS), and without a
                                // `role="separator"` the accessibility
                                // tree showed four consecutive commands
                                // with nothing to say the last is
                                // different in kind. Not focusable and
                                // not counted — `MenuOrder` holds only
                                // actionable items, so arrow navigation
                                // steps straight past it.
                                if delete_follows_a_separator {
                                    div { class: "session-row-menu-separator", role: "separator" }
                                }
                                if controls.delete {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item session-row-delete",
                                        role: "menuitem",
                                        aria_disabled: if busy { "true" },
                                        tabindex: if menu_tab_stop == Some(MenuAction::Delete) { "0" } else { "-1" },
                                        onmounted: move |element| {
                                            remember_menu_item(
                                                menu_wiring,
                                                MenuAction::Delete,
                                                element.data(),
                                            )
                                        },
                                        onfocusin: move |_| {
                                            menu_focus.set(menu_order.position(MenuAction::Delete));
                                        },
                                        onfocusout: move |_| menu_focus.set(None),
                                        onkeydown: move |evt| {
                                            handle_menu_key(
                                                &evt,
                                                menu_order.position(MenuAction::Delete),
                                                menu_wiring,
                                                &delete_key_id,
                                            );
                                        },
                                        onclick: move |_| {
                                            if busy {
                                                return;
                                            }
                                            on_delete.call(delete_target.clone());
                                        },
                                        "delete"
                                    }
                                }
                            }
                            // The profile this session was CREATED from, as
                            // it snapshotted the name — moved here from the
                            // row proper (the interviewed row contents drop
                            // the chip) so SPEC.md's snapshot rule keeps a
                            // visible surface: the name never moves under an
                            // existing session. Renames stay qualified, while
                            // a deleted row remains a plain historical label.
                            // `data-profile-existence` remains the browser
                            // suite's handle on the derived state. A SIBLING
                            // of the menu above, never a child of it: it is a
                            // fact about the session, not a fifth thing to do.
                            if let Some(source) = &session.source_profile {
                                span {
                                    class: "session-profile peer-value",
                                    dir: "ltr",
                                    "data-profile-existence": "{existence_word(source.existence)}",
                                    "{source_profile_label(source)}"
                                }
                            }
                        }
                    }
                    // The refusal a panel action produced renders INSIDE
                    // the open panel, under its controls: the panel floats
                    // over the row's own error line, so an error rendered
                    // only down there could sit hidden behind the very
                    // surface whose click caused it.
                    if let Some(err) = error.clone() {
                        PeerLine {
                            class: "action-error".to_string(),
                            parts: vec![DetailPart::Peer(err)],
                        }
                    }
                }
            }
            // The helm's own refusal, and on a stale row it names the host's
            // state and can quote peer-supplied text — so it renders through
            // the same escaping and isolation as every other peer string.
            // Only while the panel is CLOSED: the open panel carries the
            // error itself (above), and rendering both would say one
            // refusal twice.
            if let Some(err) = error {
                if !menu_open {
                    PeerLine {
                        class: "action-error".to_string(),
                        parts: vec![DetailPart::Peer(err)],
                    }
                }
            }
        }
    }
}

/// One session as the row render tests below need it: real enough for
/// `SessionRow` to render, stable so that reconstructing it every
/// parent render compares equal and memoization is what is measured.
#[cfg(test)]
pub(super) fn row_specimen(id: &str) -> Session {
    Session {
        id: id.to_string(),
        title: "stable".to_string(),
        cwd: "/tmp".to_string(),
        invocation: "agent".to_string(),
        status: SessionStatus::Exited { exit_code: Some(0) },
        annotation: None,
        restart_offer: crate::RestartOffer::FreshOnly,
        created_at: 0,
        last_activity_at: 0,
        archived: false,
        tabs: Vec::new(),
        host: None,
        host_identity: None,
        host_name: None,
        stale: false,
        source_profile: None,
        // Old-helm default: most existing row tests predate this field and
        // must keep seeing no toggle and the pre-plan colours unless a test
        // overrides it via `..row_specimen(id)`.
        seen_activity_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Revealing an archived row must keep its metadata actions while
    /// withholding lifecycle controls that no longer have a terminal to act
    /// on. Clone carries no field of its own to pin here at all — see
    /// `RowControlVisibility`'s own doc for why it is offered on every
    /// retention state unconditionally; `menu_order_follows_the_retention_
    /// state_rather_than_a_fixed_numbering` below is what actually proves
    /// it survives archiving, through `session_menu_order` instead.
    #[test]
    fn archived_rows_keep_metadata_controls_without_lifecycle_controls() {
        assert_eq!(
            row_control_visibility(true, false),
            RowControlVisibility {
                rename: true,
                stop: false,
                archive: false,
                delete: true,
                mark_seen: false,
            }
        );
    }

    /// The item list a render offers, and every focus position derived
    /// from it, must follow the retention state rather than a fixed
    /// numbering — and clone and replace, in particular, must sit right
    /// after rename, in that order, in BOTH retention states, since both
    /// are offered unconditionally.
    ///
    /// This is the arithmetic behind a real bug. Archiving a session
    /// while its menu is open withdraws stop and archive, and delete's
    /// DOM node survives that change rather than remounting — so a scheme
    /// that filed handles under "position 3" left delete's handle at an
    /// index the two-item list no longer reaches, and Home/End/arrows
    /// silently did nothing. Keying handles by action and asking
    /// `MenuOrder` for the position at key-press time is the fix; this
    /// pins the half of it that can be checked without a renderer.
    ///
    /// The `last()` case earns its own assertion because ArrowUp on a
    /// closed toggle and End both resolve through it, and an archived row's
    /// last item is Delete at the END of the SHORTER four-item archived
    /// list (Rename, Clone, Replace, Delete), not wherever it would sit in
    /// the six-item active one.
    #[test]
    fn menu_order_follows_the_retention_state_rather_than_a_fixed_numbering() {
        // `mark_seen: false` throughout — this test is about the archive
        // dimension specifically; `mark_seen_sits_right_after_rename_when_offered`
        // below is where MarkSeen's own predicate and position are pinned.
        let active = session_menu_order(row_control_visibility(false, false));
        assert_eq!(active.len(), 6);
        assert_eq!(active.get(0), Some(MenuAction::Rename));
        assert_eq!(active.get(1), Some(MenuAction::Clone));
        assert_eq!(active.get(2), Some(MenuAction::Replace));
        assert_eq!(active.get(3), Some(MenuAction::Stop));
        assert_eq!(active.get(4), Some(MenuAction::Archive));
        assert_eq!(active.get(5), Some(MenuAction::Delete));
        assert_eq!(active.get(6), None);
        assert_eq!(active.last(), Some(MenuAction::Delete));
        assert_eq!(active.position(MenuAction::Clone), Some(1));
        assert_eq!(active.position(MenuAction::Replace), Some(2));
        assert_eq!(active.position(MenuAction::Delete), Some(5));

        let archived = session_menu_order(row_control_visibility(true, false));
        assert_eq!(archived.len(), 4);
        assert_eq!(archived.get(0), Some(MenuAction::Rename));
        assert_eq!(archived.get(1), Some(MenuAction::Clone));
        assert_eq!(archived.get(2), Some(MenuAction::Replace));
        assert_eq!(archived.get(3), Some(MenuAction::Delete));
        assert_eq!(archived.get(4), None);
        assert_eq!(archived.last(), Some(MenuAction::Delete));
        // The whole point: the SAME action, a different position, and no
        // durable state anywhere that remembers the old one. Replace joins
        // clone here — both stay reachable on an archived row (see
        // `session_menu_order`'s own doc for why).
        assert_eq!(archived.position(MenuAction::Clone), Some(1));
        assert_eq!(archived.position(MenuAction::Replace), Some(2));
        assert_eq!(archived.position(MenuAction::Delete), Some(3));
        // Withdrawn actions have no position at all, which is what the
        // handle map's rebuild filters on when the set shrinks under an
        // open menu.
        assert_eq!(archived.position(MenuAction::Stop), None);
        assert_eq!(archived.position(MenuAction::Archive), None);
    }

    /// MarkSeen's own dimension, independent of the archive one the test
    /// above covers: offered or not, it sits right after Rename — the
    /// plan's stated position — in EVERY retention state, and withdrawn
    /// entirely reads exactly like Stop/Archive being withdrawn (no
    /// position at all, not a disabled one).
    #[test]
    fn mark_seen_sits_right_after_rename_when_offered() {
        let offered = session_menu_order(row_control_visibility(false, true));
        assert_eq!(offered.len(), 7);
        assert_eq!(offered.get(0), Some(MenuAction::Rename));
        assert_eq!(offered.get(1), Some(MenuAction::MarkSeen));
        assert_eq!(offered.get(2), Some(MenuAction::Clone));
        assert_eq!(offered.get(3), Some(MenuAction::Replace));
        assert_eq!(offered.position(MenuAction::MarkSeen), Some(1));

        let offered_archived = session_menu_order(row_control_visibility(true, true));
        assert_eq!(offered_archived.len(), 5);
        assert_eq!(offered_archived.get(0), Some(MenuAction::Rename));
        assert_eq!(offered_archived.get(1), Some(MenuAction::MarkSeen));
        assert_eq!(offered_archived.get(2), Some(MenuAction::Clone));
        assert_eq!(offered_archived.get(3), Some(MenuAction::Replace));
        assert_eq!(offered_archived.get(4), Some(MenuAction::Delete));

        let withdrawn = session_menu_order(row_control_visibility(false, false));
        assert_eq!(
            withdrawn.position(MenuAction::MarkSeen),
            None,
            "an ended session, or one whose helm never answered the seen-state \
             question, offers no toggle at all"
        );
    }

    /// `menu_label` composes `"session actions for …"` around whatever
    /// `clamp_title` returns, and does no clamping of its own.
    ///
    /// The clamp CONTRACT itself — the character-count cut, the ellipsis,
    /// the multi-byte and escape-token boundary safety — is pinned once in
    /// `menu_panel::tests::clamp_title_cuts_long_values_with_an_ellipsis`,
    /// shared by every caller that clamps a row identity (the toggle, the
    /// menu, and this row's own two prompt states besides). Repeating that
    /// contract here would fail for the same helper regression and tell a
    /// reader nothing `menu_panel`'s own test does not already say; this
    /// test's only job is the composition around it.
    #[test]
    fn menu_label_wraps_the_clamped_title_in_session_wording() {
        assert_eq!(menu_label("short"), "session actions for short");
    }

    /// Repeated parent refreshes must update direct callback props in place
    /// without rerendering an otherwise unchanged session row.
    ///
    /// Dioxus gives `EventHandler` component props special ownership and
    /// memoization. Hiding them inside an ordinary props struct bypasses
    /// that path: every fleet refresh then retains another callback set and
    /// rerenders every row. This drives the real `SessionRow` through many
    /// parent renders, so either regression changes the count from one.
    #[test]
    fn repeated_parent_refreshes_do_not_rerender_an_unchanged_row() {
        fn app() -> Element {
            let rename_draft = use_signal(String::new);
            let on_open = use_callback(|_: Session| {});
            let on_clone = use_callback(|_: Session| {});
            let on_mark_seen = use_callback(|_: (String, Option<i64>)| {});
            let on_replace = use_callback(|_: Session| {});
            let on_confirm_replace = use_callback(|_: String| {});
            let on_cancel_replace = use_callback(|_: String| {});
            let on_stop = use_callback(|_: String| {});
            let on_delete = use_callback(|_: DeleteTarget| {});
            let on_confirm_delete = use_callback(|_: String| {});
            let on_cancel_delete = use_callback(|_: String| {});
            let on_archive = use_callback(|_: Session| {});
            let on_confirm_archive = use_callback(|_: String| {});
            let on_cancel_archive = use_callback(|_: String| {});
            let on_rename_start = use_callback(|_: (String, String)| {});
            let on_rename_submit = use_callback(|_: (String, String)| {});
            let on_rename_cancel = use_callback(|_: ()| {});
            let on_menu_toggle = use_callback(|_: String| {});
            let session = row_specimen("session-1");
            rsx! {
                SessionRow {
                    session,
                    state: RowState {
                        error: None,
                        busy: false,
                        confirming: false,
                        confirming_archive: false,
                        confirming_replace: false,
                        renaming: false,
                        nav_disabled: false,
                        menu_open: false,
                        selected: false,
                        locality: HostLocality::Unknown,
                        activity: None,
                    },
                    rename_draft,
                    on_open,
                    on_clone,
                    on_mark_seen,
                    on_replace,
                    on_confirm_replace,
                    on_cancel_replace,
                    on_stop,
                    on_delete,
                    on_confirm_delete,
                    on_cancel_delete,
                    on_archive,
                    on_confirm_archive,
                    on_cancel_archive,
                    on_rename_start,
                    on_rename_submit,
                    on_rename_cancel,
                    on_menu_toggle,
                }
            }
        }

        SESSION_ROW_RENDERS.with(|renders| renders.set(0));
        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        for _ in 0..64 {
            dom.mark_dirty(dioxus::core::ScopeId::APP);
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
        SESSION_ROW_RENDERS.with(|renders| {
            assert_eq!(
                renders.get(),
                1,
                "unchanged rows must stay memoized across fleet refreshes"
            );
        });
    }

    /// A selection switch rerenders exactly the two rows whose `selected`
    /// flag changed; every other row stays memoized.
    ///
    /// This is the cost model the selection highlight was designed to: the
    /// flag participates in prop comparison (via `RowState`'s `PartialEq`,
    /// though any compared prop position would do), so a selection change
    /// is two row renders, not a fleet-wide repaint. If selection ever
    /// stops being part of what memoization compares — or starts invalidating
    /// rows it does not describe — this count drifts from 2 and catches it.
    #[test]
    fn a_selection_change_rerenders_only_the_affected_rows() {
        // The selection lives OUTSIDE the virtual DOM (the test flips it
        // between renders, the way `AppBody`'s signal changes between
        // `ListView` renders); the app below re-derives every `RowState`
        // from it on each parent render, so memoization alone decides
        // which rows actually run.
        std::thread_local! {
            static SELECTED: std::cell::Cell<&'static str> = const { std::cell::Cell::new("session-1") };
        }

        fn app() -> Element {
            let rename_draft = use_signal(String::new);
            let on_open = use_callback(|_: Session| {});
            let on_clone = use_callback(|_: Session| {});
            let on_mark_seen = use_callback(|_: (String, Option<i64>)| {});
            let on_replace = use_callback(|_: Session| {});
            let on_confirm_replace = use_callback(|_: String| {});
            let on_cancel_replace = use_callback(|_: String| {});
            let on_stop = use_callback(|_: String| {});
            let on_delete = use_callback(|_: DeleteTarget| {});
            let on_confirm_delete = use_callback(|_: String| {});
            let on_cancel_delete = use_callback(|_: String| {});
            let on_archive = use_callback(|_: Session| {});
            let on_confirm_archive = use_callback(|_: String| {});
            let on_cancel_archive = use_callback(|_: String| {});
            let on_rename_start = use_callback(|_: (String, String)| {});
            let on_rename_submit = use_callback(|_: (String, String)| {});
            let on_rename_cancel = use_callback(|_: ()| {});
            let on_menu_toggle = use_callback(|_: String| {});
            let selected = SELECTED.with(|selected| selected.get());
            rsx! {
                for id in ["session-1", "session-2", "session-3"] {
                    SessionRow {
                        key: "{id}",
                        session: row_specimen(id),
                        state: RowState {
                            error: None,
                            busy: false,
                            confirming: false,
                            confirming_archive: false,
                            confirming_replace: false,
                            renaming: false,
                            nav_disabled: false,
                            menu_open: false,
                            selected: selected == id,
                            locality: HostLocality::Unknown,
                            activity: None,
                        },
                        rename_draft,
                        on_open,
                        on_clone,
                        on_mark_seen,
                        on_replace,
                        on_confirm_replace,
                        on_cancel_replace,
                        on_stop,
                        on_delete,
                        on_confirm_delete,
                        on_cancel_delete,
                        on_archive,
                        on_confirm_archive,
                        on_cancel_archive,
                        on_rename_start,
                        on_rename_submit,
                        on_rename_cancel,
                        on_menu_toggle,
                    }
                }
            }
        }

        SELECTED.with(|selected| selected.set("session-1"));
        SESSION_ROW_RENDERS.with(|renders| renders.set(0));
        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        SESSION_ROW_RENDERS.with(|renders| {
            assert_eq!(renders.get(), 3, "initial build renders every row once");
        });

        SELECTED.with(|selected| selected.set("session-2"));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
        SESSION_ROW_RENDERS.with(|renders| {
            assert_eq!(
                renders.get(),
                5,
                "a selection switch must rerender the deselected and newly \
                 selected rows and nothing else"
            );
        });
    }

    /// `stale`, `selected` and `menu-open` are independent row states and
    /// every combination must say so in the class list — in particular a
    /// selected STALE row carries both, because selection must not hide
    /// the SPEC.md-required stale marking and staleness must not hide
    /// which session the main pane is on.
    ///
    /// `menu-open` joined them when the actions panel became a floating
    /// surface hanging below-left of its toggle: the panel covers the
    /// rows below it, so the row itself has to say which "⋯" owns the
    /// open menu (see `row_class`). It must not displace either of the
    /// other two — a stale, selected row with its menu up is still stale
    /// and still the selection.
    #[test]
    fn row_class_composes_stale_selected_and_menu_open_independently() {
        assert_eq!(row_class(false, false, false), "session-row");
        assert_eq!(row_class(true, false, false), "session-row stale");
        assert_eq!(row_class(false, true, false), "session-row selected");
        assert_eq!(row_class(true, true, false), "session-row stale selected");
        assert_eq!(row_class(false, false, true), "session-row menu-open");
        assert_eq!(row_class(true, false, true), "session-row stale menu-open");
        assert_eq!(
            row_class(false, true, true),
            "session-row selected menu-open"
        );
        assert_eq!(
            row_class(true, true, true),
            "session-row stale selected menu-open"
        );
    }

    /// A home directory folds to `~` only where the path actually names an
    /// account, and every other path survives untouched.
    ///
    /// The negative cases are the point. This runs against a string, not
    /// against a real filesystem — no home directory is on the wire (see
    /// `abbreviate_home`) — so the only thing keeping it from mangling
    /// unrelated paths is the shape of the match, and `/homework`,
    /// `/home/` and `/home//x` are exactly the shapes a looser prefix
    /// check would eat. `/home/alice` bare is here too, not among the
    /// folded cases — see the exclusions test below for why.
    #[test]
    fn a_home_prefix_folds_only_when_it_names_an_account() {
        assert_eq!(abbreviate_home("/home/alice/src/api"), "~/src/api");
        assert_eq!(abbreviate_home("/Users/alice/src/api"), "~/src/api");

        for untouched in [
            "/home",
            "/home/",
            "/home//nested",
            "/homework/notes",
            "/var/lib/thing",
            "/root",
            "relative/path",
            "",
        ] {
            assert_eq!(
                abbreviate_home(untouched),
                untouched,
                "{untouched} names no home directory and must be left alone"
            );
        }
    }

    /// Three shapes that match the `<prefix><account>/…` pattern
    /// syntactically but must not fold anyway, each for its own reason
    /// (see `abbreviate_home`'s doc for the full argument behind each
    /// one).
    ///
    /// These are the exclusions a 2026-08-23 review added on top of the
    /// original shape match: a dot segment breaks the assumption that the
    /// string resolves under a home at all, `/Users/Shared` is not a
    /// personal home no matter whose account is reading the row, and a
    /// bare account with nothing after it has nothing for `~` to stand in
    /// for.
    #[test]
    fn a_home_prefix_declines_three_shapes_that_look_right_but_are_not() {
        for dotted in [
            "/home/../etc/passwd",
            "/home/alice/../bob/src",
            "/Users/alice/./src",
        ] {
            assert_eq!(
                abbreviate_home(dotted),
                dotted,
                "{dotted} does not resolve under the home the shape match would claim"
            );
        }
        assert_eq!(
            abbreviate_home("/Users/Shared/notes"),
            "/Users/Shared/notes"
        );
        assert_eq!(abbreviate_home("/home/alice"), "/home/alice");
        assert_eq!(abbreviate_home("/home/alice/"), "/home/alice/");
    }

    /// Build a [`CompactInvocation`] the way `compact_invocation` would, for
    /// terser assertions below.
    fn badge(basename: &str, marker: Option<&'static str>) -> CompactInvocation {
        CompactInvocation {
            basename: basename.to_string(),
            marker,
        }
    }

    /// The invocation badge is the program's basename plus, at most, one
    /// unattended-mode marker — and the marker table's ORDER decides which
    /// one when several apply.
    ///
    /// Worth pinning because the badge is all the row shows of a command
    /// line the user may have spent real thought on: a regression that
    /// dropped the marker would make an agent running with every
    /// permission prompt skipped look identical to one that asks.
    #[test]
    fn the_invocation_badge_is_a_basename_plus_at_most_one_marker() {
        assert_eq!(compact_invocation("sleep 300"), badge("sleep", None));
        assert_eq!(
            compact_invocation("/usr/bin/codex --yolo"),
            badge("codex", Some("yolo"))
        );
        assert_eq!(
            compact_invocation("claude --dangerously-skip-permissions --model opus"),
            badge("claude", Some("skip-perms"))
        );
        assert_eq!(
            compact_invocation("codex --full-auto --yolo"),
            badge("codex", Some("yolo")),
            "the table's order, not the command line's, picks the marker"
        );
        // A flag is only a marker as a whole token: substring matching
        // would badge an unrelated argument that merely contains one.
        assert_eq!(
            compact_invocation("claude --model=yolo-9"),
            badge("claude", None)
        );
    }

    /// Codex's two OTHER markers, each pinned on its own: the table test
    /// above only ever exercises `--yolo` directly, and a regression that
    /// broke either of these while leaving `--yolo` intact would pass every
    /// other test in this file.
    #[test]
    fn the_no_sandbox_flag_earns_its_own_marker() {
        assert_eq!(
            compact_invocation("codex --dangerously-bypass-approvals-and-sandbox"),
            badge("codex", Some("no-sandbox"))
        );
    }

    #[test]
    fn the_full_auto_flag_earns_its_own_marker() {
        assert_eq!(
            compact_invocation("codex --full-auto"),
            badge("codex", Some("full-auto"))
        );
    }

    /// A marker applies only to the VENDOR it belongs to, and never past a
    /// bare `--` — the table is a claim about specific recognized
    /// programs' own flags, not a scan of every argv for a string that
    /// happens to match one.
    #[test]
    fn markers_are_tied_to_the_recognized_programs_own_flags() {
        // An unrecognized program earns no marker even though the flag
        // text is right there in its argv.
        assert_eq!(compact_invocation("echo --yolo"), badge("echo", None));
        // Codex's own flag, but past a bare `--`: shell convention says
        // everything from there on is positional data, not a flag this
        // program reads as its own.
        assert_eq!(compact_invocation("codex -- --yolo"), badge("codex", None));
        // Claude Code's binary, but Codex's flag — the wrong vendor's flag
        // is not a marker.
        assert_eq!(compact_invocation("claude --yolo"), badge("claude", None));
    }

    /// A bidi override character in the PROGRAM's basename rides through
    /// `compact_invocation` raw and unmangled — the same discipline
    /// `crate::peer::DetailPart::Peer` follows for every relayed value:
    /// escaping is deferred to RENDER time (`display_peer`, covered by
    /// `peer::tests::directional_and_invisible_characters_are_escaped_
    /// rather_than_rendered`), which is what lets the row put the basename
    /// in its own bidi-isolated span rather than a pre-sanitized one that
    /// could not be told apart from ordinary text.
    ///
    /// A corrupted basename can never accidentally ACQUIRE a marker it does
    /// not legitimately have, either: [`INVOCATION_MARKERS`] matches by
    /// EXACT string equality against the basename, so `\u{202E}codex` is
    /// simply not `codex` and earns no marker — one more reason the row's
    /// two-span rendering (basename, marker) is a real structural split and
    /// not an escaping trick alone: there is no path by which an overridden
    /// basename could smuggle a false marker onto the row for the isolation
    /// to then have to defend against.
    #[test]
    fn a_bidi_override_in_the_basename_survives_unmangled_and_earns_no_false_marker() {
        assert_eq!(
            compact_invocation("/opt/bin/\u{202E}codex --yolo"),
            CompactInvocation {
                basename: "\u{202E}codex".to_string(),
                marker: None,
            },
            "the override character rides along in the basename field untouched, and the \
             corrupted name simply fails the exact match against the recognized `codex` vendor"
        );
    }

    /// Real shell-word splitting (`shell_words::split`, the same parser
    /// `farhelm-supervisor` uses on a profile's invocation), pinned against
    /// the specific quoting shapes a hand-rolled partial parser gets wrong.
    #[test]
    fn the_basename_survives_every_shell_quoting_shape() {
        // A backslash-escaped space in an otherwise unquoted path.
        assert_eq!(
            compact_invocation("/opt/with\\ space/bin/claude --dangerously-skip-permissions"),
            badge("claude", Some("skip-perms"))
        );
        // Adjacent quoted and unquoted fragments glue into ONE argv[0] —
        // the shape `shell_words::quote` itself produces for a path with
        // spaces when only part of it needs quoting. The space sits in the
        // segment BEFORE the basename on purpose: a naive split that broke
        // on it would still (by coincidence) recover the right basename
        // from the wrong first token, so this fixture only passes if the
        // space was actually consumed as part of the same argv[0].
        assert_eq!(
            compact_invocation("\"/opt/with space\"/bin/codex --yolo"),
            badge("codex", Some("yolo"))
        );
        // The whole path quoted, once with double quotes and once with
        // single — deliberately DISCRIMINATING fixtures, unlike the
        // earlier one this replaced (`"/opt/farhelm test/farhelm"`, whose
        // pre-space and post-space segments happened to share a basename,
        // so a regression to naive whitespace splitting would have passed
        // it too). Here the segment before the space ("with") differs from
        // the true basename ("farhelm"), so only a parse that actually
        // consumed the space as part of argv[0] recovers the right answer.
        assert_eq!(
            compact_invocation("\"/opt/with space/bin/farhelm\" internal fake-agent"),
            badge("farhelm", None)
        );
        assert_eq!(
            compact_invocation("'/opt/with space/bin/farhelm' internal fake-agent"),
            badge("farhelm", None)
        );
        // A quoted flag: the parser strips the quotes, and the flag still
        // matches the marker table as a whole token.
        assert_eq!(
            compact_invocation("codex \"--yolo\""),
            badge("codex", Some("yolo"))
        );
        // `#` starts a real POSIX comment at a word boundary — this parser
        // is not a hand-rolled stand-in, it is the genuine article — so
        // everything from it to the end of the string vanishes from argv
        // entirely rather than surviving as a literal trailing token.
        assert_eq!(
            compact_invocation("sleep 300 #not-a-comment"),
            badge("sleep", None)
        );
        // Single quotes behave exactly like double quotes for this parser.
        assert_eq!(
            compact_invocation("'codex' --full-auto"),
            badge("codex", Some("full-auto"))
        );
    }

    /// Degenerate invocations render something rather than panicking or
    /// inventing a word.
    ///
    /// None of these can come out of the supervisor, which refuses a
    /// create it cannot split into argv — they come from route stubs in
    /// the browser suite and from whatever a future wire change allows.
    /// The contract is that the element still renders and says only what
    /// it was given, with no marker.
    #[test]
    fn a_degenerate_invocation_falls_back_to_what_it_was_given() {
        assert_eq!(compact_invocation(""), badge("", None));
        assert_eq!(compact_invocation("   "), badge("", None));
        assert_eq!(
            compact_invocation("/usr/bin/"),
            badge("/usr/bin/", None),
            "a token with no basename to take stands as it is"
        );
        assert_eq!(
            compact_invocation("\"unbalanced --yolo"),
            badge("\"unbalanced --yolo", None),
            "an unclosed quote is something shell_words itself cannot resolve, so this falls \
             back to the trimmed raw text with no marker rather than guessing at structure that \
             was never there"
        );
    }
}
