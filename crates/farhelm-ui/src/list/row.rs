//! One session row and its lifecycle, metadata, and menu controls.
//!
//! The row keeps the floating-panel hook state with the component; only the
//! geometry decisions are delegated to the pure menu-panel helpers.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::Session;
#[cfg(test)]
use crate::SessionStatus;
use crate::archive::confirmation as archive_confirmation;
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::profiles::{existence_word, source_profile_label};
use crate::rename::RenameForm;
use crate::status::{confirm_consequence, status_badge};

use super::menu_panel::{
    PanelPlacement, measurement_outcome, menu_panel_placement_style, should_measure_on_mount,
};
use super::shared::{DeleteTarget, RowState};

/// Which ordinary row controls exist for the current retention state.
///
/// Archive removes terminal lifecycle actions, not metadata management:
/// an archived row can still be opened, renamed, or deleted, but cannot be
/// stopped or archived a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowControlVisibility {
    rename: bool,
    stop: bool,
    archive: bool,
    delete: bool,
}

fn row_control_visibility(archived: bool) -> RowControlVisibility {
    RowControlVisibility {
        rename: true,
        stop: !archived,
        archive: !archived,
        delete: true,
    }
}

/// The session row's class list for its two independent visual states.
///
/// A stale row is DIMMED and badged, never hidden or disabled: SPEC.md
/// requires such sessions to stay listed and be clearly marked, and their
/// lifecycle controls stay live because the helm's refusal (which names
/// the host's state) is a better answer than a dead button. `selected`
/// composes with staleness rather than replacing it — the stale dimming
/// lives on `.session-row-open`'s opacity while the selection highlight is
/// the ROW's background, so a selected stale row shows both truthfully.
/// Static strings per combination, matching the prior two-state shape,
/// rather than a formatted class string.
fn row_class(stale: bool, selected: bool) -> &'static str {
    match (stale, selected) {
        (true, true) => "session-row stale selected",
        (true, false) => "session-row stale",
        (false, true) => "session-row selected",
        (false, false) => "session-row",
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
    const MAX_CHARS: usize = 64;
    let mut clamped: String = title.chars().take(MAX_CHARS + 1).collect();
    if clamped.chars().count() > MAX_CHARS {
        clamped.truncate(
            clamped
                .char_indices()
                .nth(MAX_CHARS)
                .map_or(clamped.len(), |(i, _)| i),
        );
        format!("session actions for {clamped}…")
    } else {
        format!("session actions for {clamped}")
    }
}

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
/// toggle beside it. Everything else — rename, stop, archive, delete,
/// and their confirm prompts — mounts inside the floating panel the
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
/// panel's controls follow the toggle (rename → stop → archive →
/// delete, as visible per `RowControlVisibility`); confirming, the
/// panel holds consequence text plus confirm → cancel (with initial
/// FOCUS on cancel — see "Focus-on-open" below); renaming, the panel
/// holds the current title plus the field's input → save → cancel.
///
/// ## Host and staleness (PLAN_M6.md item 6)
///
/// The row is at most three lines: the title (with the stale/archived/
/// status badges beside it), the host — only when the session is NOT on
/// the helm's own machine — and a meta line carrying the abbreviated cwd
/// beside a compact invocation badge. Each field ellipsizes alone in the
/// fixed-width sidebar.
///
/// The shape is a density decision (2026-08-23, the UI refresh), and it
/// reverses the interviewed row contents recorded in BUGS_BURNDOWN.md's
/// "Decisions (interviewed 2026-08-13)", which called for the host and the
/// full invocation on every row. In a fleet that is mostly local, the host
/// line said "this machine" over and over, and the full invocation line
/// was usually an absolute path plus flags that ellipsized into
/// indistinguishability — two lines of row height for near-zero
/// information. Both are still reachable: the host line returns the moment
/// a session IS remote, and the full cwd and invocation are on the
/// respective elements' `title` attributes. See `abbreviate_home` and
/// `compact_invocation` for what each abbreviation costs.
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
/// `renaming` swaps the actions panel's rename/stop/archive/delete set
/// for the session's own title plus `rename::RenameForm`, disabling (not
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
#[component]
#[allow(clippy::too_many_arguments)]
pub(super) fn SessionRow(
    session: Session,
    state: RowState,
    rename_draft: Signal<String>,
    on_open: EventHandler<Session>,
    on_stop: EventHandler<String>,
    on_delete: EventHandler<DeleteTarget>,
    on_confirm_delete: EventHandler<String>,
    on_cancel_delete: EventHandler<String>,
    on_archive: EventHandler<Session>,
    on_confirm_archive: EventHandler<String>,
    on_cancel_archive: EventHandler<String>,
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
        renaming,
        nav_disabled,
        menu_open,
        selected,
        host_is_local,
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
    let badge = status_badge(&session.status, session.annotation.as_deref());
    let open_session = session.clone();
    let stop_id = session.id.clone();
    let delete_target = DeleteTarget {
        id: session.id.clone(),
        status: session.status.clone(),
    };
    let archive_target = session.clone();
    let confirm_id = session.id.clone();
    let cancel_id = session.id.clone();
    let confirm_archive_id = session.id.clone();
    let cancel_archive_id = session.id.clone();
    let rename_start = (session.id.clone(), session.title.clone());
    let rename_submit_id = session.id.clone();
    let controls = row_control_visibility(session.archived);
    let menu_id = session.id.clone();
    // The toggle's own `MountedData`, captured once via `onmounted` below,
    // and where the panel believes its own screen position currently is
    // (`PanelPlacement` — its own doc has the state machine) — both
    // row-local (unlike `menu_open`, which `ListView` owns so only one
    // row's menu can ever be open). `ListView` has no business knowing
    // this row's screen geometry, and this row has no business deciding
    // WHETHER its menu is open — the split mirrors that division exactly.
    let mut toggle_handle = use_signal(|| None::<Rc<MountedData>>);
    let mut placement = use_signal(|| PanelPlacement::Unmeasured);
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
    let mut open_generation = use_signal(|| 0_u64);
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
    let row_class = row_class(session.stale, selected);

    rsx! {
        div {
            class: row_class,
            "data-session-id": "{session.id}",
            "data-session-stale": "{session.stale}",
            "data-session-archived": "{session.archived}",
            "data-session-selected": "{selected}",
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
                    disabled: nav_disabled || confirming || confirming_archive || renaming,
                    onclick: move |_| on_open.call(open_session.clone()),
                    // STACKED lines rather than one squeezed flex row: the
                    // sidebar column (BUGS_BURNDOWN.md issue 5, interviewed
                    // row contents) is far too narrow for the old
                    // everything-on-one-line layout, whose min-width floors
                    // produced the MT-8 overflow class the moment space ran
                    // short. The title line leads with identity and its
                    // qualifiers; the host (when it is not the helm's own
                    // machine) and the directory/invocation pair follow on
                    // lines of their own, each field ellipsizing alone.
                    // `span` wrappers, not `div`: this all sits inside the
                    // native `.session-row-open` <button>, whose content
                    // model only permits phrasing content — a flow-content
                    // div inside a button is invalid HTML that engines and
                    // accessibility tooling may interpret inconsistently.
                    // The stacked-line layout comes from the class's CSS,
                    // not the element kind.
                    span { class: "session-row-line",
                        span { class: "session-title", "{session.title}" }
                        // Beside the status badge rather than replacing it:
                        // the last-known status is still what the helm
                        // knows, and this says how old that knowledge is —
                        // two facts, not one, exactly as the stop annotation
                        // qualifies rather than replaces `exited`.
                        if session.stale {
                            span { class: "stale-badge", "stale" }
                        }
                        if session.archived {
                            span { class: "archived-badge", "archived" }
                        }
                        if let Some((badge_class, badge_text)) = badge {
                            // `title` because `.status-badge` caps at 32ch
                            // and ellipsizes (`app.css`) — the shim's own
                            // `error` detail rides straight into this text
                            // and can run long, so the tooltip is the only
                            // way back to a badge that has visibly clipped.
                            span {
                                class: "status-badge {badge_class}",
                                title: "{badge_text}",
                                "{badge_text}"
                            }
                        }
                    }
                    // The host gets a line of its own, but ONLY for a
                    // session that is not on the helm's own machine: with
                    // more than one machine in play the name is what
                    // disambiguates two otherwise identical sessions, and
                    // with only one it is the same word on every row,
                    // costing a quarter of the row's height to say nothing.
                    // `host_is_local` is `ListView`'s answer, not this
                    // row's — see `shared::session_is_local` for why the
                    // row cannot decide it and why every unknown answers
                    // "not local" (the line shows, rather than a local
                    // claim being invented).
                    //
                    // The name is the helm's own rendering (`host_name`),
                    // denormalized onto the row so the list needs no second
                    // request — a row from a helm that sends none simply
                    // shows nothing here rather than inventing a label.
                    // Escaped and direction-isolated like every other
                    // rendering of a destination: this one names the machine
                    // a row's stop and delete will reach, so a name able to
                    // reorder the line around it could make one host's
                    // session read as another's.
                    if let Some(host_name) = session.host_name.as_ref().filter(|_| !host_is_local) {
                        span { class: "session-row-line",
                            span { class: "session-host peer-value", dir: "ltr",
                                "{display_peer(host_name)}"
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
                        // The profile case keeps the qualifier that the
                        // panel's chip carries — in the tooltip and in
                        // `data-profile-existence`, which is what the
                        // color and the browser suite key off — because a
                        // bare snapshotted name on a row would read as a
                        // claim about the catalog as it stands today, and
                        // SPEC.md's snapshot rule says it is not one.
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
                        // A fresh measurement every open: the toggle can
                        // move BETWEEN opens (a row above it changing
                        // height, the window resizing), so a rect measured
                        // for a previous open is not safe to reuse — hence
                        // resetting to `Unmeasured` here rather than
                        // leaving whatever `placement` last held. The
                        // generation bump BEFORE `spawn_measurement` is
                        // what keeps a measurement still in flight for a
                        // PRIOR open of this SAME toggle from landing after
                        // this reset and clobbering it — see
                        // `open_generation`'s own doc for the exact race.
                        open_generation += 1;
                        placement.set(PanelPlacement::Unmeasured);
                        spawn_measurement();
                    },
                    "⋯"
                }
                if menu_open {
                    div {
                        class: "session-row-menu-panel",
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
                            if controls.rename {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-rename",
                                    disabled: busy,
                                    onclick: move |_| on_rename_start.call(rename_start.clone()),
                                    "rename"
                                }
                            }
                            if controls.stop {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-stop",
                                    disabled: busy,
                                    onclick: move |_| on_stop.call(stop_id.clone()),
                                    "stop"
                                }
                            }
                            if controls.archive {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-archive",
                                    disabled: busy,
                                    onclick: move |_| on_archive.call(archive_target.clone()),
                                    "archive"
                                }
                            }
                            if controls.delete {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-delete",
                                    disabled: busy,
                                    onclick: move |_| on_delete.call(delete_target.clone()),
                                    "delete"
                                }
                            }
                            // The profile this session was CREATED from, as
                            // it snapshotted the name — moved here from the
                            // row proper (the interviewed row contents drop
                            // the chip) so SPEC.md's snapshot rule keeps a
                            // visible surface: the name never moves under
                            // an existing session, and the qualifier
                            // (`profiles::source_profile_label`) keeps that
                            // from reading as a claim about today's
                            // catalog. `data-profile-existence` remains the
                            // browser suite's handle on the half that does
                            // change.
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
        archived: false,
        tabs: Vec::new(),
        host: None,
        host_identity: None,
        host_name: None,
        stale: false,
        source_profile: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Revealing an archived row must keep its metadata actions while
    /// withholding lifecycle controls that no longer have a terminal to act
    /// on.
    #[test]
    fn archived_rows_keep_metadata_controls_without_lifecycle_controls() {
        assert_eq!(
            row_control_visibility(true),
            RowControlVisibility {
                rename: true,
                stop: false,
                archive: false,
                delete: true,
            }
        );
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
                        renaming: false,
                        nav_disabled: false,
                        menu_open: false,
                        selected: false,
                        host_is_local: false,
                    },
                    rename_draft,
                    on_open,
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
                            renaming: false,
                            nav_disabled: false,
                            menu_open: false,
                            selected: selected == id,
                            host_is_local: false,
                        },
                        rename_draft,
                        on_open,
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

    /// `stale` and `selected` are independent row states and every
    /// combination must say so in the class list — in particular a
    /// selected STALE row carries both, because selection must not hide
    /// the SPEC.md-required stale marking and staleness must not hide
    /// which session the main pane is on.
    #[test]
    fn row_class_composes_stale_and_selected_independently() {
        assert_eq!(row_class(false, false), "session-row");
        assert_eq!(row_class(true, false), "session-row stale");
        assert_eq!(row_class(false, true), "session-row selected");
        assert_eq!(row_class(true, true), "session-row stale selected");
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
