//! One session row and its lifecycle, metadata, and menu controls.
//!
//! The row keeps the floating-panel hook state with the component; only the
//! geometry decisions are delegated to the pure menu-panel helpers.

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;

use crate::hosts::gui_host_name;
use crate::icons::{
    EndedGlyph, EndedStatusIcon, HarnessGlyph, HarnessIcon, HostMark, LocalHostIcon,
    ManagedCheckoutMark, PermissionGlyph, PermissionIcon, QualifierGlyph, QualifierIcon,
};
use crate::launch_composer::{selection_explicit_before_permissions, selection_permission_value};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::status::{StatusBadgeView, confirm_consequence, replace_consequence, status_badge};
use crate::{LaunchHarness, Session, SessionStatus};

use super::bell::{BellSlot, NotificationBell};
use super::shared::{DeleteTarget, HostLocality, RowState};
use crate::menu_panel::{
    self, MenuFocusQueue, MenuOpenIntent, PanelPlacement, cancel_menu_focus, clamp_title,
    closed_toggle_key_intent, focus_menu_toggle, forget_menu_focus, handle_menu_key,
    measurement_outcome, remember_menu_item, session_menu_placement_style,
    session_menu_pointer_style, should_measure_on_mount,
};

/// Which optional row controls exist for the current session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowControlVisibility {
    rename: bool,
    stop: bool,
    delete: bool,
    /// Whether the mark-read/mark-unread item is offered — the ONE field
    /// here whose condition depends on the row's live
    /// status (running, waiting, or idle — an ended session has no dot and
    /// no meaningful unseen state to toggle) and whether the helm sent
    /// `seen_activity_at` at all (an old helm offers no toggle it cannot
    /// answer PUT requests for), so the caller computes it from the whole
    /// `Session` rather than this function deriving it from one field.
    mark_seen: bool,
}

/// Keep ordinary actions available for ended sessions too; only the seen
/// toggle depends on live status and the helm's seen-state capability.
fn row_control_visibility(mark_seen: bool) -> RowControlVisibility {
    RowControlVisibility {
        rename: true,
        stop: true,
        delete: true,
        mark_seen,
    }
}

// ===== The menu's items, and how they are addressed ==================
//
// Everything below identifies a menu item by WHAT IT DOES rather than by
// where it currently sits. That distinction is the fix for a real bug:
// the item set is not fixed for the life of an open menu. Seen-state
// availability can change while the surviving actions keep their DOM nodes,
// so a scheme that filed handles under an index could leave a handle at a
// position the shorter list no longer reaches.
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
    /// actions, so grouping them ahead of Stop/Delete keeps that
    /// distinction visible in the menu's own order.
    MarkSeen,
    Clone,
    /// "Replace with": clone's editable form, seeded from this row, with
    /// replace's create-then-delete-the-source action on launch — the third
    /// "make a new session from this one" action, and the reason it is not
    /// a fourth code path is the whole point of this feature.
    /// `CreateSessionForm` (`list::create_form`) has exactly one submit
    /// path; ReplaceWith, Clone, and the plain New dialog share it byte for
    /// byte, differing only in what pre-fills the form (see
    /// `create_form::CreatePrefill`'s `replace_source`) and in what a
    /// successful submit does afterward (create; create then delete the
    /// source). Any behavior that diverges between them belongs on that
    /// shared path as a bug, never as a special case bolted onto this menu
    /// item.
    ReplaceWith,
    Replace,
    Stop,
    Delete,
}

/// Every action the menu can offer, in the order it offers them.
///
/// The canonical order lives here rather than in the rsx so that
/// `MenuOrder` and the rendered list cannot disagree about what "the
/// first item" or "the last item" means — the two places arrow keys and
/// the open-intent both resolve against.
///
/// `Clone`, `ReplaceWith`, and `Replace` sit together, in that order: all
/// three are "make a new session from this one" actions, escalating from
/// "keep both" (Clone) through "keep both, but let me edit the settings
/// first" (ReplaceWith) to "swap this one out, unedited" (Replace) —
/// putting them beside each other is what lets a user compare the three
/// without hunting across the menu.
const MENU_ACTIONS: [MenuAction; 7] = [
    MenuAction::Rename,
    MenuAction::MarkSeen,
    MenuAction::Clone,
    MenuAction::ReplaceWith,
    MenuAction::Replace,
    MenuAction::Stop,
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

/// Builds this render's item list from available controls — the bridge
/// between `RowControlVisibility`'s named fields and the shared
/// `MenuOrder::pack`'s generic `(action) -> bool` predicate.
///
/// `Clone`, `ReplaceWith`, and `Replace` all answer `true` unconditionally
/// rather than reading a `RowControlVisibility` field. They seed a new
/// session from metadata and do not require the source's process to be alive.
fn session_menu_order(controls: RowControlVisibility) -> MenuOrder {
    MenuOrder::pack(MENU_ACTIONS, |action| match action {
        MenuAction::Rename => controls.rename,
        MenuAction::MarkSeen => controls.mark_seen,
        MenuAction::Clone => true,
        MenuAction::ReplaceWith => true,
        MenuAction::Replace => true,
        MenuAction::Stop => controls.stop,
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
/// `menu_open` is the third: the session panel sits beside the sidebar,
/// so the row tint identifies which visible toggle owns it. It composes
/// with the other two the same
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

/// Split the header's saved launch facts into peer and app-authored runs.
///
/// The row receives all dynamic values from the helm, so each one stays raw
/// until `PeerLine` applies escaping and direction isolation at render time.
/// Separators and the concise state word are this UI's own wording and remain
/// ordinary text runs, which prevents a relayed value from reordering them.
fn menu_header_summary_parts(session: &Session, state: Option<&str>) -> Vec<DetailPart> {
    let values = if let Some(launch) = session.agent_selection() {
        let mut values =
            vec![crate::launch_composer::harness_label(launch.harness).to_ascii_lowercase()];
        let permission = selection_permission_value(launch);
        if permission != "default" {
            values.push(permission.to_string());
        }
        values.extend(selection_explicit_before_permissions(launch));
        values
    } else {
        vec![command_basename(&session.invocation)]
    };

    let mut parts = Vec::with_capacity(values.len() * 2 + usize::from(state.is_some()));
    for (index, value) in values.into_iter().enumerate() {
        if index != 0 {
            parts.push(DetailPart::text(" · "));
        }
        parts.push(DetailPart::peer(value));
    }
    if let Some(state) = state {
        parts.push(DetailPart::text(" · "));
        parts.push(DetailPart::text(state));
    }
    parts
}

/// Build the header's hover tooltip from already separated display runs.
///
/// The visible line uses one isolated element per peer value; the tooltip is
/// necessarily one attribute string, so every peer run is escaped before the
/// app-authored separators and state word are joined into its display form.
fn menu_header_summary_tooltip(parts: &[DetailPart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            DetailPart::Text(text) => text.clone(),
            DetailPart::Peer(value) => display_peer(value),
        })
        .collect()
}

/// Decorative line art for commands; the button text supplies the name.
///
/// These paths come from design C's small action glyphs. Keeping them in
/// one component makes every command use the same stroke and box size.
#[component]
fn MenuActionIcon(action: MenuAction) -> Element {
    rsx! {
        svg {
            class: "session-row-menu-icon",
            view_box: "0 0 14 14",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.2",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            match action {
                MenuAction::Rename => rsx! { path { d: "M2.5 11.5 L2.5 9.5 L9 3 L11 5 L4.5 11.5 Z" } },
                MenuAction::MarkSeen => rsx! {
                    path { d: "M1.5 7 C3.5 3.5 10.5 3.5 12.5 7 C10.5 10.5 3.5 10.5 1.5 7 Z" }
                    circle { cx: "7", cy: "7", r: "1.6", fill: "currentColor", stroke: "none" }
                },
                MenuAction::Clone => rsx! {
                    rect { x: "2", y: "4.5", width: "7", height: "7", rx: "1" }
                    path { d: "M5 4.5 V2.5 H11.5 V9 H9" }
                },
                MenuAction::ReplaceWith => rsx! {
                    path { d: "M2 4 H12 M2 10 H12" }
                    circle { cx: "5", cy: "4", r: "1.6", fill: "var(--bg-2)" }
                    circle { cx: "9", cy: "10", r: "1.6", fill: "var(--bg-2)" }
                },
                MenuAction::Replace => rsx! {
                    path { d: "M11.5 5.5 A4.6 4.6 0 0 0 3 4.5 M2.5 8.5 A4.6 4.6 0 0 0 11 9.5 M11.8 2.5 V5.8 H8.6 M2.2 11.5 V8.2 H5.4" }
                },
                MenuAction::Stop => rsx! { rect { x: "3.5", y: "3.5", width: "7", height: "7", rx: "1" } },
                MenuAction::Delete => rsx! { path { d: "M2.5 4 H11.5 M5.5 4 V2.5 H8.5 V4 M3.8 4 L4.5 12 H9.5 L10.2 4" } },
            }
        }
    }
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

/// The complete, accessible account of the compact agent badge.
///
/// The sidebar pairs a harness glyph with a permission mark, but the reduced
/// picture must never become the only place the invocation or permission
/// meaning exists. This value keeps the visible classification and the
/// tooltip/screen-reader wording together so a later glyph change cannot
/// accidentally make the accessible account contradict it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentBadge {
    harness: HarnessGlyph,
    permission: PermissionGlyph,
    description: String,
}

/// Hover text for a row's stale mark, in both densities.
///
/// The visible mark is the bare word (or, compact, a glyph), so the tooltip
/// says what it means rather than repeating it: the row is the helm's
/// last-known view of a session whose host is not connected right now
/// (`Session::stale`; SPEC.md keeps such sessions listed, marked stale).
const STALE_TOOLTIP: &str = "stale: the host is not connected, so this is its last-known state";

/// Keep the specific approval mode audible even when several modes share a shield.
fn permission_description(permission: PermissionGlyph) -> &'static str {
    match permission {
        PermissionGlyph::Yolo => "YOLO permission bypass",
        PermissionGlyph::Default => "default permission mode",
        PermissionGlyph::Unknown => {
            "unclassified — a command from before launch kinds, which Farhelm never classified"
        }
        PermissionGlyph::AssertedYolo => {
            "YOLO, as asserted when the command was launched, by the user or by an agent — not checked by Farhelm"
        }
        PermissionGlyph::AssertedNotYolo => {
            "not YOLO, as asserted when the command was launched, by the user or by an agent — not checked by Farhelm"
        }
        PermissionGlyph::Approve => "approve permission mode",
        PermissionGlyph::SmartApprove => "smart approve permission mode",
        PermissionGlyph::Chat => "chat permission mode",
    }
}

/// Describe a session's agent and permission from its stored launch, never
/// from its command line (SPEC.md: Farhelm does not parse a command to
/// classify it).
///
/// An agent launch shows its agent type and effective permission; a command
/// launch its declared agent type (or the generic terminal mark) and the
/// YOLO assertion it was launched with; a legacy session its stored agent
/// kind and the unclassified mark.
fn agent_badge(session: &Session) -> AgentBadge {
    if let Some(launch) = session.agent_selection() {
        let harness = harness_glyph(launch.harness);
        // The shared classifier decides YOLO; the effective permission helper
        // also handles older omitted or unsupported choices for mode display.
        let permission = if farhelm_proto::yolo::selection_is_yolo(launch) {
            PermissionGlyph::Yolo
        } else {
            match launch.harness.effective_permission(launch.permissions) {
                Some(crate::LaunchPermission::Yolo) => {
                    unreachable!("the shared classifier handled YOLO")
                }
                Some(crate::LaunchPermission::Approve) => PermissionGlyph::Approve,
                Some(crate::LaunchPermission::SmartApprove) => PermissionGlyph::SmartApprove,
                Some(crate::LaunchPermission::Chat) => PermissionGlyph::Chat,
                None => PermissionGlyph::Default,
            }
        };
        let mut description = harness_name(launch.harness).to_string();
        description.push_str(" — ");
        description.push_str(permission_description(permission));
        description.push_str(" — ");
        description.push_str(&session.invocation);
        return AgentBadge {
            harness,
            permission,
            description,
        };
    }

    let (harness, permission, mut description) = match &session.launch {
        Some(crate::SessionLaunch::Command(command)) => (
            command.agent.map_or(HarnessGlyph::Terminal, harness_glyph),
            if command.yolo {
                PermissionGlyph::AssertedYolo
            } else {
                PermissionGlyph::AssertedNotYolo
            },
            command
                .agent
                .map(harness_name)
                .map_or_else(|| "command".to_string(), str::to_string),
        ),
        Some(crate::SessionLaunch::Legacy { .. })
        | Some(crate::SessionLaunch::Agent { .. })
        | None => (
            kind_glyph(session.agent_kind),
            PermissionGlyph::Unknown,
            "command".to_string(),
        ),
    };
    description.push_str(" — ");
    description.push_str(permission_description(permission));
    description.push_str(" — ");
    description.push_str(&session.invocation);
    AgentBadge {
        harness,
        permission,
        description,
    }
}

/// The glyph for an agent type.
fn harness_glyph(harness: LaunchHarness) -> HarnessGlyph {
    match harness {
        LaunchHarness::Codex => HarnessGlyph::Codex,
        LaunchHarness::Claude => HarnessGlyph::Claude,
        LaunchHarness::Muse => HarnessGlyph::Muse,
        LaunchHarness::Cursor => HarnessGlyph::Cursor,
        LaunchHarness::Goose => HarnessGlyph::Goose,
        LaunchHarness::Pi => HarnessGlyph::Pi,
        LaunchHarness::Omp => HarnessGlyph::Omp,
        LaunchHarness::Grok => HarnessGlyph::Grok,
        LaunchHarness::OpenCode => HarnessGlyph::OpenCode,
    }
}

/// An agent type's name as the badge's description says it.
fn harness_name(harness: LaunchHarness) -> &'static str {
    match harness {
        LaunchHarness::Codex => "Codex",
        LaunchHarness::Claude => "Claude Code",
        LaunchHarness::Muse => "Muse Code",
        LaunchHarness::Cursor => "Cursor",
        LaunchHarness::Goose => "Goose",
        LaunchHarness::Pi => "Pi",
        LaunchHarness::Omp => "OMP",
        LaunchHarness::Grok => "Grok",
        LaunchHarness::OpenCode => "OpenCode",
    }
}

/// The glyph for a legacy session's stored integration kind: the agent it
/// was recorded as running, or the generic terminal mark when it was
/// recorded as none (or the helm sent a kind this page does not know).
fn kind_glyph(kind: crate::SessionAgentKind) -> HarnessGlyph {
    match kind {
        crate::SessionAgentKind::Claude => HarnessGlyph::Claude,
        crate::SessionAgentKind::Codex => HarnessGlyph::Codex,
        crate::SessionAgentKind::Goose => HarnessGlyph::Goose,
        crate::SessionAgentKind::Pi => HarnessGlyph::Pi,
        crate::SessionAgentKind::Omp => HarnessGlyph::Omp,
        crate::SessionAgentKind::Grok => HarnessGlyph::Grok,
        crate::SessionAgentKind::Generic | crate::SessionAgentKind::Unrecognized => {
            HarnessGlyph::Terminal
        }
    }
}

/// A command line's program basename, for the row menu's one-line summary:
/// display only, never a classification. A string that does not split, or
/// names no program, shows trimmed as it is, and a program token ending in
/// `/` keeps the whole token.
fn command_basename(invocation: &str) -> String {
    let trimmed = invocation.trim();
    let Ok(argv) = shell_words::split(invocation) else {
        return trimmed.to_string();
    };
    let Some(program) = argv.first() else {
        return trimmed.to_string();
    };
    match program.rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => program.clone(),
    }
}

/// A session title as the sidebar shows it: escaped with `display_peer` and
/// direction-isolated in a `.peer-value` span with `dir="ltr"`.
///
/// Titles are peer text (agents may rename any session). The row,
/// confirmations and rename dialog show them the same safe way: escaping keeps
/// invisible and override characters from making two titles look alike,
/// and isolation keeps a title's own strong-RTL text from reordering the
/// sentence around it (see `peer.rs`). One component for these surfaces
/// means none can drift. The attributes are formatted values on purpose, so
/// they reach the DOM as dynamic attributes a headless test can observe.
#[component]
pub(crate) fn PeerTitle(
    class: &'static str,
    title: String,
    quoted: bool,
    tooltip: bool,
) -> Element {
    let shown = display_peer(&title);
    let text = if quoted {
        format!("\"{shown}\"")
    } else {
        shown.clone()
    };
    let isolated = "ltr";
    rsx! {
        span {
            class: "{class} peer-value",
            dir: "{isolated}",
            "data-tooltip": tooltip.then(|| shown.clone()),
            "{text}"
        }
    }
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
/// toggle beside it. Everything else — rename, clone, stop,
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
/// panel's controls follow the toggle (rename → clone → stop →
/// delete, as visible per `RowControlVisibility`); confirming, the panel
/// holds consequence text plus confirm → cancel (with initial FOCUS on
/// cancel — see "Focus-on-open" below). Rename instead opens ListView's
/// independent dialog and immediately closes this anchored panel.
///
/// ## Host and staleness (PLAN_M6.md item 6)
///
/// Compact rows use one visual line. Outside compact mode, an ended status or
/// stale qualifier adds a full-width detail line between identity and
/// host metadata. The identity line reserves fixed status and locality slots,
/// then carries title, glyph-only agent identity, and activity age. Compact
/// ended states use the status slot for an icon, while their complete wording
/// stays accessible; an unknown locality leaves its reserved slot blank rather
/// than claiming an identity. The host line retains its full values in
/// tooltips.
///
/// The second line restores the host/directory pairing so rows from several
/// machines can be scanned without mixing identity into the already dense
/// first line. Compact mode is the explicit escape hatch for a fleet where
/// that repeated context is less useful. The glyph and permission mark come
/// from the launch: an agent launch's type and permission, a command
/// launch's declared type and YOLO answer, and a legacy session's stored
/// kind with an unclassified mark, never from reading the command. The
/// full cwd remains available on the open button in compact mode, and the
/// agent tooltip retains the full invocation and permission meaning. See
/// `abbreviate_home` and `command_basename` for the display constraints.
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
/// The session identity prompt uses two separate elements, not one combined sentence:
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
/// A managed checkout adds its lifetime consequence separately: ordinary
/// borrowers need the same warning as the originating session, and the
/// wording must remain conditional because only the supervisor can decide
/// whether this Delete removes the final durable reference.
///
/// ## Rename ownership
///
/// `renaming` still disables this row's open button, but no longer swaps the
/// menu contents. ListView owns the dialog outside keyed rows, preserving the
/// textarea through refreshes that reorder or temporarily remove this row.
/// This row only supplies the source identity and title that seed that editor.
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
///   automatic dismissal that took the menu away from a focused item —
///   except the two transfers, Rename and a clone/replace-with composer
///   acceptance, which hand focus to their own newly mounted dialog
///   instead; see the dismissal effect in the body for why that teardown
///   is centralized rather than written per key.
///
/// The decisions are pure functions in `menu_panel` (`menu_key_action`,
/// `next_menu_focus`, `closed_toggle_key_intent`); `menu_panel`'s
/// `handle_menu_key` is the one place they meet a real event, shared with
/// the host row's menu (see that module's own doc for why).
///
/// Confirmation sub-states deliberately bind NOTHING. Their
/// contents are not menu items — a two-button prompt — and arrow keys do
/// not describe commands while it is visible. Escape is left unbound there
/// too, and that one is a decision
/// rather than an omission: closing the panel does not clear
/// `ListView`'s confirming flag (see `menu_panel_placement_style` for why
/// that state outlives the panel), so an Escape that dismissed
/// the prompt without answering it would leave the row primed to reopen
/// straight back into the same prompt.
///
/// A confirmation autofocuses its cancel button, because the risk is a stray
/// Enter landing on a destructive action. The separate rename dialog owns its
/// own focus, Escape, and cancellation lifecycle.
#[component]
#[allow(clippy::too_many_arguments)]
pub(super) fn SessionRow(
    session: Session,
    state: RowState,
    /// Whether the shared sidebar preference hides the host/directory line.
    /// The identity row never changes, so compactness cannot hide status,
    /// locality, or the menu target a keyboard user needs to reach.
    compact: bool,
    on_open: EventHandler<Session>,
    /// The "clone" menu item's click: hands the row's own `Session` up so
    /// `ListView` can seed a fresh create form from it
    /// (`create_form::CreatePrefill`). Nothing is mutated or restarted
    /// here — this row's only job is to say WHICH session was cloned.
    on_clone: EventHandler<Session>,
    /// The "replace with" menu item's click: hands the row's own `Session`
    /// up so `ListView` can seed the SAME create form clone opens, marked
    /// with this row as its replace-with source
    /// (`create_form::CreatePrefill::replace_source`). Like `on_clone`, and
    /// unlike `on_replace` below, nothing is mutated or restarted by this
    /// click alone — the create form's own submit is the only place either
    /// verb ever reaches the API, and replace-with's submit both creates
    /// and deletes the source in one request
    /// (`api::replace_session_with`). Guarded by the same
    /// `clone_is_refused` predicate `on_clone`/`on_replace` use, since a
    /// row mid-decision is exactly as unstable a replace-with source as it
    /// is a clone or replace one.
    on_replace_with: EventHandler<Session>,
    /// The "replace" menu item's click: opens `confirming_replace`, hands
    /// the row's own `Session` up. Unlike `on_clone` this DOES eventually
    /// mutate the fleet — `ListView`'s handler calls `api::replace_session`
    /// — but not from this click alone; see `on_confirm_replace` for the
    /// step that actually acts.
    on_replace: EventHandler<Session>,
    /// The replace prompt's confirm: hands up the `Session` this render drew
    /// the prompt's consequence text from, so the replace acts on what the
    /// user read (see `ListView`'s `confirm_replace`).
    on_confirm_replace: EventHandler<Session>,
    on_cancel_replace: EventHandler<String>,
    on_stop: EventHandler<String>,
    on_delete: EventHandler<DeleteTarget>,
    on_confirm_delete: EventHandler<(String, crate::DeleteGuard)>,
    on_cancel_delete: EventHandler<String>,
    /// The read/unread toggle's click, from either the menu item or the
    /// row's own dot (SPEC.md, Status): the session id and
    /// the target `seen_activity_at` to PUT — `Some(effective_activity)` to
    /// mark read, `None` to mark unread. A tuple like `on_rename_start`
    /// rather than the whole `Session` like `on_clone`: the
    /// caller needs nothing else about the row, and computing the target
    /// value here (where the current unseen predicate is already in scope)
    /// keeps `ListView` from having to re-derive it.
    on_mark_seen: EventHandler<(String, Option<i64>)>,
    on_rename_start: EventHandler<(String, String)>,
    on_menu_toggle: EventHandler<String>,
    /// The notification bell's click (SPEC.md, Status): the session id, its
    /// current read mark, which `ListView` keeps as the open list's "new
    /// since last time" boundary, and the newest entry the list will show,
    /// which closing it marks read.
    on_bell_toggle: EventHandler<(String, u64, u64)>,
    /// Escape inside the bell's list: close it.
    on_bell_close: EventHandler<String>,
    /// The list's clear button: the session id and the newest sequence
    /// number the list showed.
    on_bell_clear: EventHandler<(String, u64)>,
) -> Element {
    let RowState {
        error,
        busy,
        confirming,
        confirming_replace,
        renaming,
        nav_disabled,
        menu_open,
        composer_transfer_open,
        selected,
        locality,
        host_icon,
        host_color,
        activity,
        deleting,
        bell_open,
    } = state;
    #[cfg(test)]
    SESSION_ROW_RENDERS.with(|renders| renders.set(renders.get() + 1));
    // The two abbreviations the dense row runs on, computed once per
    // render beside the full strings they stand in for. Both `title`
    // attributes carry the original: an abbreviation the user cannot undo
    // would make the sidebar's own claim about a session unverifiable.
    let cwd_shown = abbreviate_home(&session.cwd);
    // An absent folder has always left the line blank; only a supplied path
    // needs a visible spelling for hidden or directional characters.
    let cwd_shown = if cwd_shown.is_empty() {
        cwd_shown
    } else {
        display_peer(&cwd_shown)
    };
    // Glyph and permission mark from the session's launch, never from its
    // command text (see `agent_badge`).
    let agent = agent_badge(&session);
    let agent_tooltip = agent.description.clone();
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
    let menu_state = badge
        .as_ref()
        .map(|badge| badge.class.split_whitespace().next().unwrap_or(badge.class));
    let menu_summary_parts = menu_header_summary_parts(&session, menu_state);
    // Live statuses occupy the fixed leading slot as dots. In compact mode an
    // ended status uses that same slot for an icon; noncompact rows put its
    // complete wording on the full-width detail line below the identity.
    let (status_slot_badge, mut ended_badge) = match badge {
        Some(badge) if badge.visible => (None, Some(badge)),
        badge => (badge, None),
    };
    if let (
        Some(detail),
        SessionStatus::Exited {
            exit_code: None, ..
        },
    ) = (&mut ended_badge, &session.status)
    {
        // `status_badge` is shared with the header, where its established
        // concise wording stays intact. The sidebar's full detail contract
        // needs this otherwise absent datum to remain explicit.
        detail.text = detail.text.replacen("exited", "exited (code unknown)", 1);
    }
    let ended_glyph = match (&session.status, session.annotation.as_deref()) {
        (SessionStatus::Exited { .. }, Some("stopped by user")) => Some(EndedGlyph::Stopped),
        (SessionStatus::Exited { .. }, _) => Some(EndedGlyph::Exited),
        (SessionStatus::Interrupted, _) => Some(EndedGlyph::Interrupted),
        (SessionStatus::Error { .. }, _) => Some(EndedGlyph::Error),
        _ => None,
    };
    // A noncompact detail line is meaningful when it carries either the
    // complete ended message or a qualifier whose compact presentation is a
    // glyph. Live and unknown rows otherwise keep their existing two-line
    // height rather than acquiring an empty layout row. The list's
    // menu-placement check counts these lines through the same function
    // (`rows::menu_row_reordered`), so it must stay the one definition.
    let has_detail = crate::rows::has_detail_line(&session);
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
    // Only a registry-confirmed local row earns the GUI's local wording.
    // Legacy rows and rows waiting for the hosts read keep their supplied name.
    let shown_host_name = session
        .host_name
        .as_deref()
        .map(|name| gui_host_name(name, locality == HostLocality::Local));
    let open_session = session.clone();
    // A row shows its bell while any notification is left uncleared; the
    // helm has already dropped the cleared ones.
    let has_bell = !session.notifications.is_empty();
    let bell_toggle_target = (
        session.id.clone(),
        session.notifications_read_through,
        session.newest_notification_seq().unwrap_or(0),
    );
    let bell_close_id = session.id.clone();
    let bell_clear_id = session.id.clone();
    let stop_id = session.id.clone();
    let delete_target = DeleteTarget::for_session(&session);
    let clone_target = session.clone();
    let replace_with_target = session.clone();
    let replace_target = session.clone();
    let confirm_id = session.id.clone();
    let cancel_id = session.id.clone();
    let confirm_replace_source = session.clone();
    let cancel_replace_id = session.id.clone();
    let rename_start = (session.id.clone(), session.title.clone());
    // The toggle is offered on a LIVE row (running, waiting, idle — SPEC.md;
    // an ended session has no dot and no meaningful unseen state) whose helm
    // answered the seen-state question at all (`unseen.is_some()`); staleness
    // is deliberately NOT part of this predicate — a session on an
    // unreachable host still has a last-known dot to toggle, and the route
    // that serves this write is itself helm-local with nothing to refuse for
    // an unreachable host (SPEC_impl.md's `session_seen` paragraph).
    let offers_mark_seen = session.status.is_live() && unseen.is_some();
    let controls = row_control_visibility(offers_mark_seen);
    // "mark read" when the row currently has unseen output, "mark unread"
    // otherwise — the label follows the CURRENT predicate every render,
    // never a value captured once.
    let mark_seen_label = if unseen == Some(true) {
        "mark read"
    } else {
        "mark unread"
    };
    // The menu item has no description line, so its hover text says what
    // the toggle changes: the dot's new-output color, in every client.
    let mark_seen_tooltip = if unseen == Some(true) {
        "mark read: treat this session's latest output as seen"
    } else {
        "mark unread: show this session as having new output again"
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
    // This render's item list is also the source of every focus position,
    // so the rendered and navigable menus cannot disagree.
    let menu_order = session_menu_order(controls);
    // Whether the panel is currently showing its item list rather than a
    // confirmation prompt. Rename immediately closes this panel and mounts
    // ListView's stable dialog, so it is not a panel sub-state.
    let showing_menu_items = !(confirming || confirming_replace);
    // The accessible name for the panel's prompt states. Only read when
    // one of them is showing; the menu state names its inner list
    // instead. Same clamp as the toggle's own name, for the same reason
    // (see `clamp_title`).
    let prompt_label = if confirming {
        format!("delete confirmation for {}", clamp_title(&session.title))
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
    let replace_with_key_id = session.id.clone();
    let replace_key_id = session.id.clone();
    let stop_key_id = session.id.clone();
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
    // cleared this by the time the teardown runs. An item UNMOUNTING,
    // by contrast, leaves this populated: the teardown reclaims the
    // removed item's element identity in the same pass, and Dioxus
    // dispatches events only through surviving element records, so the
    // item's own `onfocusout` never runs to clear this. The browser may
    // still fire the removal `focusout` observably at document level —
    // Chromium does — but that event is not what clears this signal;
    // only the item's handler does, and teardown has removed it. A
    // scroll or resize dismissal therefore correctly keeps its position
    // and gets the handback.
    //
    // That same unmount gap is why the TOGGLE clears this too (its
    // `onfocusin` below, through `forget_menu_focus`): a confirmation can
    // replace the whole item list without closing the panel, so the item
    // holding focus can vanish silently and leave this signal — and
    // `menu_requested` with it — naming a position focus has long since left.
    let mut menu_focus = use_signal(|| None::<usize>);
    // The last position a keyboard step asked focus to move TO — see
    // `MenuWiring::requested`'s own doc for why this has to exist
    // separately from `menu_focus` (F5/COR-FOCUS-BURST follow-up: an older
    // in-flight focus request's `onfocusin` can land after a newer press
    // already moved `menu_focus` on, and only a signal DOM events never
    // touch survives that). Cleared alongside `menu_focus` wherever the
    // menu opens or closes, below, and wherever the toggle takes focus
    // (`forget_menu_focus`). An item-set change remaps a surviving request
    // by action: withdrawing a middle item can leave its old position in
    // bounds but naming a different action. Keep the remapped request so a
    // late focus event still cannot roll a keyboard burst back; clear only
    // a request for an action that has disappeared.
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
    // Ending a session withdraws mark-seen from an open menu. Losing the
    // seen-state field would do the same; the predicate does not assume it
    // stays present, even though an old-helm transition is not reachable here.
    // Rename and
    // delete keep their DOM nodes across that change (Dioxus diffs them
    // in place), so nothing re-registers them and the withdrawn items'
    // handles would otherwise sit in the map retaining detached nodes,
    // while a `menu_focus` recorded against the longer list would point
    // past the end of the shorter one. Rebuilt here rather than in the
    // click path because no click is involved — the listing simply
    // reports a different session.
    //
    // `use_reactive` because `offers_mark_seen` is a plain
    // prop-derived value: an effect body that merely closed over it would
    // run once with the first render's answer and never again.
    //
    // Stale FOCUS is reconciled by ACTION identity rather than by comparing
    // the stored position against the new list's length — see
    // `menu_panel::reconcile_menu_focus`'s own doc for why a length check
    // misses a withdrawal from the middle of the list. Removing MarkSeen
    // shifts the surviving actions even when focus stays within the new bounds.
    let withdrawal_close_id = session.id.clone();
    use_effect(use_reactive(&offers_mark_seen, move |offers_mark_seen| {
        let order = session_menu_order(row_control_visibility(offers_mark_seen));
        let mut focused_position = *menu_focus.peek();
        if order != *previous_menu_order.peek() {
            // Preserve the latest keyboard target across a withdrawal and
            // any older focus event still in flight. A withdrawn target
            // has no new slot, so the reconciled focus becomes the fallback.
            // Publish the origin even if its slot stays unchanged; a
            // withdrawn target still supplies focus-return bookkeeping to
            // the existing dismissal path (see `carry_menu_request`).
            let (origin, requested) = menu_panel::carry_menu_request(
                *previous_menu_order.peek(),
                order,
                *menu_requested.peek(),
                focused_position,
            );
            focused_position = origin;
            menu_focus.set(focused_position);
            menu_requested.set(requested);
        }
        item_handles
            .write()
            .retain(|action, _| order.position(*action).is_some());
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
    }));
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
    //
    // `composer_transfer_open` is deliberately NOT an effect dependency:
    // the teardown still runs exactly when the menu closes, and the
    // closure it runs with is the closing render's own — which already
    // carries the composer's open state as of that same pass. Depending
    // on it would only add teardown passes on composer open and close,
    // when there is no menu transition to resolve.
    use_effect(use_reactive(
        (&menu_open, &renaming),
        move |(menu_open, renaming)| {
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
            // Rename transfers focus to its independently mounted dialog.
            // The ordinary menu teardown must not reclaim it for the toggle.
            //
            // A clone/replace-with acceptance is the same transfer through
            // the composer. Removing the activated item can leave the
            // row's inside-focus bookkeeping populated: the teardown
            // reclaims the removed item's element identity in the same
            // pass, and Dioxus dispatches events only through surviving
            // element records, so the item's own `onfocusout` never runs
            // to clear `menu_focus`. That is a framework-teardown fact,
            // not a claim about the browser: Chromium fires the removal
            // `focusout` observably at document level, and still the
            // bookkeeping stays set — what clears this signal is the
            // item's handler, and teardown has removed it. Without this
            // exception the dismissal reads a stale "inside" and its
            // toggle eval lands AFTER the composer's own search focus,
            // stealing it back. Retiring the return here removes the
            // losing claimant instead of racing it; the composer's mount
            // focus is then the only operation left to win. A refused
            // clone never reaches this flag (its handler returns before
            // opening anything), so the menu keeps its focus there.
            if was_inside && !renaming && !composer_transfer_open {
                focus_menu_toggle("data-session-id", &dismiss_id, ".session-row-menu");
            }
        },
    ));
    // `deleting` is layered on rather than folded into `row_class`'s table:
    // it is transient and independent of the other three, and doubling that
    // exhaustive match for it would say nothing the suffix does not.
    let row_class = match (row_class(session.stale, selected, menu_open), deleting) {
        (class, true) => format!("{class} deleting"),
        (class, false) => class.to_string(),
    };
    let delete_progress = crate::status::delete_progress_label(&session.status, session.tabs.len());
    // Clipped rather than removed while deleting; see the title line below.
    let title_class: &'static str = if deleting {
        "session-title visually-hidden"
    } else {
        "session-title"
    };

    rsx! {
        div {
            class: row_class,
            "data-session-id": "{session.id}",
            "data-session-stale": "{session.stale}",
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
                    // Keep the full directory discoverable when compact mode
                    // removes the metadata line and its own tooltip.
                    // A tooltip is one attribute string and cannot carry
                    // DOM direction isolation, so every peer-derived hover
                    // text goes through `display_peer`, like the title's own
                    // tooltip.
                    // Outside compact mode the row's own parts (title,
                    // marks, age, directory) each carry hover text, and the
                    // button's tooltip only names what a click does for the
                    // space between them.
                    "data-tooltip": if compact {
                        format!("{} — click to open", display_peer(&session.cwd))
                    } else {
                        "click to open this session".to_string()
                    },
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
                    // confirmation or standalone rename editor being open — the
                    // simplest way to satisfy "cancel is the only way back
                    // to normal" (see the component doc above) is to make
                    // the open button inert for the whole time a prompt is
                    // showing, rather than giving it a second, competing
                    // meaning as an implicit cancel.
                    disabled: nav_disabled || confirming || confirming_replace
                        || renaming,
                    onclick: move |_| on_open.call(open_session.clone()),
                    // STACKED lines rather than one squeezed flex row: the
                    // sidebar column (BUGS_BURNDOWN.md issue 5, interviewed
                    // row contents) is far too narrow for the old
                    // everything-on-one-line layout, whose min-width floors
                    // produced the MT-8 overflow class the moment space ran
                    // short. Fixed status/locality tracks lead the title and
                    // its qualifiers; agent and activity occupy the trailing
                    // tracks. The host/directory pair follows on its own line,
                    // with compact mode removing that line as one unit. `span` wrappers,
                    // not `div`: this all sits inside the
                    // native `.session-row-open` <button>, whose content
                    // model only permits phrasing content — a flow-content
                    // div inside a button is invalid HTML that engines and
                    // accessibility tooling may interpret inconsistently.
                    // The stacked-line layout comes from the class's CSS,
                    // not the element kind.
                    span { class: "session-row-line session-row-identity",
                        span { class: "session-status-slot",
                        if let Some(badge) = status_slot_badge {
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
                        } else if compact {
                            if let Some(glyph) = ended_glyph {
                                if let Some(badge) = &ended_badge {
                                    // The compact row button's tooltip names
                                    // its cwd, so this wrapper keeps ended
                                    // detail available to pointer users too.
                                    span { class: "compact-ended-status", "data-tooltip": "{display_peer(&badge.text)}",
                                        EndedStatusIcon { glyph }
                                        span { class: "visually-hidden", "{badge.text}" }
                                    }
                                }
                            }
                        }
                        }
                        // The locality slot (2026-09-03): a fixed leading
                        // track immediately after status, so every title
                        // starts at the same position whether this row is
                        // local, remote, or still unknown. See
                        // `.session-locality-slot` and `.host-kind-icon` in
                        // app.css for the geometry.
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
                        span { class: "session-locality-slot",
                        match locality {
                            HostLocality::Local => rsx! {
                                LocalHostIcon {}
                                span { class: "visually-hidden", "local" }
                            },
                            HostLocality::Remote => rsx! {
                                HostMark { icon: host_icon, color: host_color }
                                span { class: "visually-hidden", "remote" }
                            },
                            HostLocality::Unknown => rsx! {},
                        }
                        }
                        // One grid child owns the title and compact-only
                        // qualifiers. Noncompact qualifier words move into
                        // the full-width detail line so they stay readable
                        // without competing with the agent and activity.
                        span { class: if compact { "session-identity-copy compact" } else { "session-identity-copy" },
                            // A tooltip is one attribute string and cannot
                            // carry DOM direction isolation. Escape invisible
                            // directional controls in this display surface
                            // like other peer text.
                            // Titles are peer text (agents may rename any
                            // session), so the row shows them escaped and
                            // direction-isolated, not just in the tooltip.
                            // A committed delete takes over the title line
                            // until its reply lands: the one thing worth
                            // saying about a row on its way out is that it
                            // is going, and the title line is where the eye
                            // lands. The title element stays, clipped rather
                            // than removed: assistive technology still names
                            // the row by it, and so does everything that
                            // finds a row by its `.session-title`.
                            if deleting {
                                span { class: "delete-progress", role: "status",
                                    span { class: "delete-spinner", "aria-hidden": "true" }
                                    "{delete_progress}"
                                }
                            }
                            PeerTitle {
                                class: title_class,
                                title: session.title.clone(),
                                quoted: false,
                                tooltip: true,
                            }
                            if compact {
                                // Compact rows omit the directory line. Keep
                                // checkout lifetime visible beside the name.
                                if let Some(checkout) = &session.working_copy {
                                    ManagedCheckoutMark { checkout: checkout.clone(), cwd: session.cwd.clone() }
                                }
                                if session.stale {
                                    span { class: "compact-qualifier", "data-tooltip": "{STALE_TOOLTIP}",
                                        QualifierIcon { glyph: QualifierGlyph::Stale }
                                        span { class: "visually-hidden", "stale" }
                                    }
                                }
                            }
                        }
                        // The agent track is a bounded visual classifier.
                        span {
                            class: "session-agent",
                            "data-tooltip": "{display_peer(&agent_tooltip)}",
                            // Give each glyph its own hover target. The
                            // parent still exposes the argv when the pointer
                            // is between the two marks.
                            span { "data-tooltip": "{display_peer(&agent_tooltip)}",
                                HarnessIcon { glyph: agent.harness }
                            }
                            span { "data-tooltip": "{permission_description(agent.permission)}",
                                PermissionIcon { glyph: agent.permission }
                            }
                            span { class: "visually-hidden", "{display_peer(&agent.description)}" }
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
                        span { class: "session-activity-column",
                        // The bell's reserved space, which the overlay
                        // after this button draws the bell over (see
                        // `bell`'s module doc for why it is not here).
                        if has_bell {
                            BellSlot {}
                        }
                        if let Some(activity) = &activity {
                            span {
                                class: "status-time",
                                "data-tooltip": "{activity.absolute}",
                                "{activity.age}"
                            }
                        }
                        }
                    }
                    if !compact && has_detail {
                    // A full-width sibling of the identity grid, rather than
                    // a wrapped child of its title track: status detail and
                    // qualifiers may use the space beneath the agent and age
                    // while the row's menu gutter remains fixed.
                    span { class: "session-row-line session-row-detail",
                        span { class: "session-detail-copy",
                            if let Some(badge) = ended_badge {
                                StatusBadgeView { badge, dot_onclick: move |_| {}, dot_title: None }
                            }
                            if session.stale {
                                span { class: "stale-badge", "data-tooltip": "{STALE_TOOLTIP}", "stale" }
                            }
                        }
                    }
                    }
                    if !compact {
                    // The optional second line names the actual host beside
                    // the working directory. A missing legacy host name
                    // stays missing: locality can establish an icon, never
                    // an invented machine name.
                    span { class: "session-row-line session-row-meta",
                        if let Some(host_name) = &shown_host_name {
                            span {
                                class: "session-host peer-value",
                                dir: "ltr",
                                "data-tooltip": "{display_peer(host_name)}",
                                "{host_name}"
                            }
                            span {
                                class: "session-host-separator",
                                "data-separator": if session.working_copy.is_some() { "dot" } else { "colon" },
                                if session.working_copy.is_some() { "·" } else { ":" }
                            }
                        }
                        // Current checkout membership names the repository;
                        // the shared mark keeps its lifetime and actual cwd
                        // reachable through one hover target.
                        if let Some(checkout) = &session.working_copy {
                            ManagedCheckoutMark { checkout: checkout.clone(), cwd: session.cwd.clone(), show_repository: true }
                        } else {
                            // Two spans, not one: `.session-cwd` is the rtl
                            // clipping container that puts the ellipsis on the
                            // LEFT, and the inner `dir="ltr"` child is the bidi
                            // isolate that keeps the path's characters in
                            // logical order under it — rtl applied directly to
                            // the text would move a leading "/" to the visual
                            // right (see `.session-cwd` in app.css). The
                            // tooltip carries the UNABBREVIATED path, which is
                            // what makes the `~` safe: see `abbreviate_home`
                            // for whose home it does and does not know about.
                            span { class: "session-cwd", "data-tooltip": "{display_peer(&session.cwd)}",
                                span { class: "session-cwd-text", dir: "ltr", "{cwd_shown}" }
                            }
                        }
                    }
                    }
                    // Compact mode drops the metadata line, so the directory
                    // reaches the pointer through the open button's tooltip
                    // and assistive technology through this clipped copy,
                    // which joins the button's accessible name. The native
                    // `title` the tooltip replaced was the button's
                    // accessible description instead; the tooltip is not in
                    // the accessibility tree at all.
                    if compact {
                        span { class: "visually-hidden", "{display_peer(&session.cwd)}" }
                    }
                }
                // The notification bell, a sibling of the open button laid
                // over it rather than nested inside it: see `bell`'s module
                // doc. Only on rows whose session has notifications left.
                if has_bell {
                    NotificationBell {
                        session_id: session.id.clone(),
                        title: session.title.clone(),
                        notifications: session.notifications.clone(),
                        unread: session.unread_notifications(),
                        activity_age: activity.as_ref().map(|activity| activity.age.clone()),
                        open: bell_open,
                        on_toggle: move |_| on_bell_toggle.call(bell_toggle_target.clone()),
                        on_close: move |_| on_bell_close.call(bell_close_id.clone()),
                        on_clear: move |through| on_bell_clear.call((bell_clear_id.clone(), through)),
                    }
                }
                // The actions menu: one small toggle beside the open
                // button, everything else in a floating panel it anchors
                // (BUGS_BURNDOWN.md issue 5's interviewed design — the row
                // itself carries no action buttons). The panel is also
                // where a destructive action CONFIRMS: clicking delete or
                // delete swaps the panel's contents for the consequence
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
                    "data-tooltip": "session actions: rename, clone, replace, stop, delete and more",
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
                    // with a confirm prompt showing, the toggle binds
                    // nothing, or Shift+Tab back to it would offer an Escape
                    // that dismisses the panel while leaving the prompt
                    // itself unanswered. Rename is absent here: it closes
                    // this panel before its dialog mounts.
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
                        class: "session-row-menu-flyout",
                        style: session_menu_placement_style(placement()),
                        if showing_menu_items {
                            if let Some(pointer_style) = session_menu_pointer_style(placement()) {
                                span { class: "session-row-menu-pointer", style: pointer_style, "aria-hidden": "true" }
                            }
                        }
                        div {
                            // The panel is the positioned box inside the
                            // non-scrolling flyout wrapper. The wrapper owns
                            // the pointer so its clamp follows the panel's
                            // actual rendered height even when the panel
                            // scrolls. That holds only while this panel is
                            // the wrapper's ONLY in-flow child: anything else
                            // rendered beside it (the refusal line once was)
                            // adds to the height the pointer clamps against
                            // and can leave the pointer beside that sibling,
                            // off the panel. What the panel currently IS
                            // lives one level in (the item list's own
                            // `role="menu"`) or on the panel only while it is
                            // a prompt.
                            // The class carries the sub-state because the
                            // geometry differs — a list of inset commands
                            // versus a padded prompt — and it is derived from
                            // the same `showing_menu_items` value that picks
                            // the markup, in the same expression, so the two
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
                        // The confirm sub-states ARE a small named exchange:
                        // one consequence sentence and its two answers,
                        // with focus deliberately placed on the
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
                        if confirming {
                            // Two elements, consequence first: an
                            // untruncatable consequence and a separately
                            // truncatable title, in THIS order, so a long
                            // title can never clip the safety-critical
                            // half (see the component doc above).
                            span {
                                class: "confirm-consequence",
                                "{confirm_consequence(&session.status, session.tabs.len())}"
                            }
                            // The last check before an irreversible action:
                            // an override or invisible character in a title
                            // must not make this quote name another session.
                            PeerTitle {
                                class: "confirm-title",
                                title: session.title.clone(),
                                quoted: true,
                                tooltip: false,
                            }
                            if session.working_copy.is_some() {
                                // Membership includes ordinary borrowers. Do not
                                // promise a move from a stale client-side count;
                                // Delete decides from durable references.
                                span {
                                    class: "confirm-consequence confirm-checkout-consequence",
                                    "The managed checkout stays while another session uses it. When its last session is deleted, its folder moves to the trash; no files are deleted."
                                }
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-danger confirm-delete",
                                "data-tooltip": "delete this session and its state now",
                                // Disabled while the shared token is held:
                                // the handler refuses then anyway (keeping
                                // the prompt), and the attribute is that
                                // refusal made visible. Cancel stays
                                // enabled — backing out is always safe.
                                disabled: busy,
                                // The guard comes from THIS render's
                                // snapshot, the one the consequence text
                                // above was drawn from, so a prompt that
                                // drifted is answered for what it said.
                                onclick: move |_| on_confirm_delete.call((
                                    confirm_id.clone(),
                                    crate::status::delete_guard(&session.status, session.tabs.len()),
                                )),
                                "confirm delete"
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-neutral confirm-cancel",
                                "data-tooltip": "cancel: keep this session",
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
                        } else if confirming_replace {
                            // Same two-element, consequence-first shape as
                            // the delete prompt above — see
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
                                "{replace_consequence(&session.status, session.tabs.len())}"
                            }
                            // The last check before an irreversible action:
                            // an override or invisible character in a title
                            // must not make this quote name another session.
                            PeerTitle {
                                class: "confirm-title",
                                title: session.title.clone(),
                                quoted: true,
                                tooltip: false,
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-danger confirm-replace",
                                "data-tooltip": "replace this session now, as described",
                                // See confirm-delete: refusal made visible.
                                disabled: busy,
                                onclick: move |_| on_confirm_replace.call(confirm_replace_source.clone()),
                                "confirm replace"
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-neutral replace-cancel",
                                "data-tooltip": "cancel: keep this session as it is",
                                autofocus: true,
                                onclick: move |_| on_cancel_replace.call(cancel_replace_id.clone()),
                                "cancel"
                            }
                        } else {
                            // The header is outside the menu role: it names
                            // the target and its saved launch state, but is
                            // not another command in the roving tab order.
                            div {
                                class: "session-row-menu-header",
                                div {
                                    class: "session-row-menu-title",
                                    "data-tooltip": "{display_peer(&session.title)}",
                                    span {
                                        class: "peer-value",
                                        dir: "ltr",
                                        "{display_peer(&session.title)}"
                                    }
                                }
                                div {
                                    class: "session-row-menu-summary",
                                    "data-tooltip": "{menu_header_summary_tooltip(&menu_summary_parts)}",
                                    PeerLine {
                                        class: "session-row-menu-summary-runs".to_string(),
                                        parts: menu_summary_parts.clone(),
                                        peer_tooltips: true,
                                    }
                                }
                            }
                            // The menu proper: ONLY the actionable rows,
                            // in their own element. The panel around it
                            // also holds the session header and, as a
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
                                // `.session-row-menu-item` is the session
                                // look, while the per-action
                                // class beside it stays exactly what it
                                // was, since the browser suite keys off
                                // those.
                                if controls.rename {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item session-row-rename",
                                        role: "menuitem",
                                        aria_label: "rename",
                                        "data-tooltip": "rename: give this session a new title",
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
                                        MenuActionIcon { action: MenuAction::Rename }
                                        span { class: "session-row-menu-label", "rename" }
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
                                        aria_label: "{mark_seen_label}",
                                        "data-tooltip": "{mark_seen_tooltip}",
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
                                        MenuActionIcon { action: MenuAction::MarkSeen }
                                        span { class: "session-row-menu-label", "{mark_seen_label}" }
                                    }
                                }
                                if controls.rename || controls.mark_seen {
                                    div { class: "session-row-menu-separator", role: "separator" }
                                }
                                // Offered on every row, unconditionally —
                                // see `RowControlVisibility`'s own doc for
                                // why clone has no visibility field to gate
                                // it at all. Rather than acting on this
                                // row's process at all, it reads this row's
                                // host, directory, title, and launch (its
                                // structured choices or raw invocation) to seed a
                                // brand-new, independent create — the click
                                // only OPENS that form pre-filled
                                // (`create_form::CreatePrefill`); nothing
                                // here mutates or restarts anything itself.
                                button {
                                    r#type: "button",
                                    class: "btn session-row-menu-item session-row-clone",
                                    role: "menuitem",
                                    aria_label: "clone",
                                    aria_describedby: "session-menu-clone-description",
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
                                    MenuActionIcon { action: MenuAction::Clone }
                                    span { class: "session-row-menu-copy",
                                        span { class: "session-row-menu-label", "clone" }
                                        span { id: "session-menu-clone-description", class: "session-row-menu-description", "new session, keep this one" }
                                    }
                                }
                                // Between clone and replace: clone's exact
                                // editable form, pre-filled the same way,
                                // but marked with THIS row as its
                                // replace-with source
                                // (`create_form::CreatePrefill::replace_source`)
                                // so the form's own submit both creates the
                                // edited session and deletes this row's own
                                // — replace's action, reached through
                                // clone's form instead of replace's
                                // unconditional inline confirmation. Like
                                // clone, the click only OPENS the form;
                                // nothing here mutates or restarts anything
                                // by itself.
                                button {
                                    r#type: "button",
                                    class: "btn session-row-menu-item session-row-replace-with",
                                    role: "menuitem",
                                    aria_label: "replace with",
                                    aria_describedby: "session-menu-replace-with-description",
                                    aria_disabled: if busy { "true" },
                                    tabindex: if menu_tab_stop == Some(MenuAction::ReplaceWith) { "0" } else { "-1" },
                                    onmounted: move |element| {
                                        remember_menu_item(menu_wiring, MenuAction::ReplaceWith, element.data())
                                    },
                                    onfocusin: move |_| {
                                        menu_focus.set(menu_order.position(MenuAction::ReplaceWith));
                                    },
                                    onfocusout: move |_| menu_focus.set(None),
                                    onkeydown: move |evt| {
                                        handle_menu_key(
                                            &evt,
                                            menu_order.position(MenuAction::ReplaceWith),
                                            menu_wiring,
                                            &replace_with_key_id,
                                        );
                                    },
                                    onclick: move |_| {
                                        if busy {
                                            return;
                                        }
                                        on_replace_with.call(replace_with_target.clone());
                                    },
                                    MenuActionIcon { action: MenuAction::ReplaceWith }
                                    span { class: "session-row-menu-copy",
                                        span { class: "session-row-menu-label", "replace with…" }
                                        span { id: "session-menu-replace-with-description", class: "session-row-menu-description", "edit settings, then swap" }
                                    }
                                }
                                // Also unconditional, directly beside the
                                // two above — the row's LAST "make a new
                                // session from this one" action, and the
                                // one difference from replace-with is the
                                // whole point of offering both: replace-with
                                // opens an editable form first, replace acts
                                // at once, deletes this row's own session,
                                // and puts an unedited copy in its place.
                                // The click only opens `confirming_replace`;
                                // `on_confirm_replace` is what actually
                                // calls the API.
                                button {
                                    r#type: "button",
                                    class: "btn session-row-menu-item session-row-replace",
                                    role: "menuitem",
                                    aria_label: "replace",
                                    aria_describedby: "session-menu-replace-description",
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
                                    MenuActionIcon { action: MenuAction::Replace }
                                    span { class: "session-row-menu-copy",
                                        span { class: "session-row-menu-label", "replace" }
                                        span { id: "session-menu-replace-description", class: "session-row-menu-description", "fresh conversation, same settings" }
                                    }
                                }
                                if controls.stop {
                                    div { class: "session-row-menu-separator", role: "separator" }
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item session-row-stop",
                                        role: "menuitem",
                                        aria_label: "stop",
                                        aria_describedby: "session-menu-stop-description",
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
                                        MenuActionIcon { action: MenuAction::Stop }
                                        span { class: "session-row-menu-copy",
                                            span { class: "session-row-menu-label", "stop" }
                                            span { id: "session-menu-stop-description", class: "session-row-menu-description", "ends the agent and its processes" }
                                        }
                                    }
                                }
                                // Delete has its own final group. The
                                // separator describes that boundary to
                                // assistive technology but remains outside
                                // the action order and arrow navigation.
                                if delete_follows_a_separator {
                                    div { class: "session-row-menu-separator", role: "separator" }
                                }
                                if controls.delete {
                                    button {
                                        r#type: "button",
                                        class: "btn session-row-menu-item session-row-delete",
                                        role: "menuitem",
                                        aria_label: "delete",
                                        aria_describedby: "session-menu-delete-description",
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
                                        MenuActionIcon { action: MenuAction::Delete }
                                        span { class: "session-row-menu-copy",
                                            span { class: "session-row-menu-label", "delete" }
                                            span { id: "session-menu-delete-description", class: "session-row-menu-description", "removes the session and its state" }
                                        }
                                    }
                                }
                            }
                        }
                        // The refusal a panel action produced renders INSIDE
                        // the open panel, under its controls, in every
                        // sub-state: the panel floats over the row's own
                        // error line, so an error rendered only down there
                        // could sit hidden behind the very surface whose
                        // click caused it. Inside the panel, not beside it
                        // in the flyout: the pointer's clamp reads the
                        // flyout's height as the panel's, and a long refusal
                        // must scroll with the panel rather than hang past
                        // the viewport below it.
                        if let Some(err) = error.clone() {
                            PeerLine {
                                class: "action-error".to_string(),
                                parts: vec![DetailPart::Peer(err)],
                            }
                        }
                        // End of `.session-row-menu-panel`.
                        }
                    // End of `.session-row-menu-flyout`.
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
        canonical_cwd: None,
        invocation: "agent".to_string(),
        agent_kind: crate::SessionAgentKind::Unrecognized,
        launch: None,
        status: SessionStatus::Exited { exit_code: Some(0) },
        annotation: None,
        restart_offer: crate::RestartOffer::NotCaptured,
        created_at: 0,
        last_activity_at: 0,
        tabs: Vec::new(),
        host: None,
        host_identity: None,
        host_name: None,
        stale: false,
        github_repo: None,
        working_copy: None,
        // Old-helm default: most existing row tests predate this field and
        // must keep seeing no toggle and the pre-plan colours unless a test
        // overrides it via `..row_specimen(id)`.
        seen_activity_at: None,
        notifications: Vec::new(),
        notifications_read_through: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::detail_text;

    /// The header must report stored structured choices, not infer them from
    /// an invocation that can disagree with the saved launch selection.
    /// A legacy row uses the existing bounded executable label and omits an
    /// unknown state rather than inventing a classification.
    #[farhelm_testtrace::test]
    fn menu_header_uses_saved_launch_facts_and_existing_state_words() {
        let structured = Session {
            launch: agent_launch(crate::LaunchSelection {
                harness: LaunchHarness::Claude,
                model: None,
                effort: None,
                permissions: Some(crate::LaunchPermission::Yolo),
                workspace_trust: None,
            }),
            invocation: "unrelated-command".to_string(),
            ..row_specimen("structured")
        };
        assert_eq!(
            detail_text(&menu_header_summary_parts(&structured, Some("running"))),
            "claude · yolo · running"
        );

        let legacy = Session {
            invocation: "codex --yolo".to_string(),
            ..row_specimen("legacy")
        };
        assert_eq!(
            detail_text(&menu_header_summary_parts(&legacy, None)),
            "codex"
        );
    }

    /// MarkSeen sits right after Rename when offered and disappears entirely
    /// when the helm cannot answer the seen-state write.
    #[farhelm_testtrace::test]
    fn mark_seen_sits_right_after_rename_when_offered() {
        let offered = session_menu_order(row_control_visibility(true));
        assert_eq!(offered.len(), 7);
        assert_eq!(offered.get(0), Some(MenuAction::Rename));
        assert_eq!(offered.get(1), Some(MenuAction::MarkSeen));
        assert_eq!(offered.get(2), Some(MenuAction::Clone));
        assert_eq!(offered.get(3), Some(MenuAction::ReplaceWith));
        assert_eq!(offered.get(4), Some(MenuAction::Replace));
        assert_eq!(offered.position(MenuAction::MarkSeen), Some(1));

        let withdrawn = session_menu_order(row_control_visibility(false));
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
    #[farhelm_testtrace::test]
    fn menu_label_wraps_the_clamped_title_in_session_wording() {
        assert_eq!(menu_label("short"), "session actions for short");
    }

    /// Why this matters: agents may rename any session, and a title with an
    /// invisible character or a direction override could render identically
    /// to another session's, or make the delete and replace confirmations
    /// quote a different title than the one they act on. Spec: the row's
    /// visible title, its hover tooltip, and both confirmation quotes show
    /// the escaped form (`display_peer`) and never the raw control
    /// characters. The folder line follows the same rule after home
    /// abbreviation; an absent folder stays blank rather than gaining a
    /// placeholder that the row did not previously show.
    #[farhelm_testtrace::test]
    fn titles_and_folders_render_escaped_without_filling_an_empty_folder() {
        std::thread_local! {
            static PROMPT: std::cell::Cell<(bool, bool)> = const { std::cell::Cell::new((false, false)) };
            static CWD: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") };
        }
        const SPOOF: &str = "build\u{200B} \u{202E}lanif";

        /// Render the real row for each supplied folder and confirmation state;
        /// inspecting its text mutations keeps Rust debug escaping out of the proof.
        fn app() -> Element {
            let on_open = use_callback(|_: Session| {});
            let on_clone = use_callback(|_: Session| {});
            let on_replace_with = use_callback(|_: Session| {});
            let on_mark_seen = use_callback(|_: (String, Option<i64>)| {});
            let on_replace = use_callback(|_: Session| {});
            let on_confirm_replace = use_callback(|_: Session| {});
            let on_cancel_replace = use_callback(|_: String| {});
            let on_stop = use_callback(|_: String| {});
            let on_delete = use_callback(|_: DeleteTarget| {});
            let on_confirm_delete = use_callback(|_: (String, crate::DeleteGuard)| {});
            let on_cancel_delete = use_callback(|_: String| {});
            let on_rename_start = use_callback(|_: (String, String)| {});
            let on_menu_toggle = use_callback(|_: String| {});
            let on_bell_toggle = use_callback(|_: (String, u64, u64)| {});
            let on_bell_close = use_callback(|_: String| {});
            let on_bell_clear = use_callback(|_: (String, u64)| {});
            let (confirming, confirming_replace) = PROMPT.with(std::cell::Cell::get);
            let session = Session {
                title: SPOOF.to_string(),
                cwd: CWD.with(std::cell::Cell::get).to_string(),
                ..row_specimen("spoofed")
            };
            rsx! {
                SessionRow {
                    session,
                    compact: false,
                    state: RowState {
                        error: None,
                        busy: false,
                        confirming,
                        confirming_replace,
                        renaming: false,
                        nav_disabled: false,
                        // The confirmations live inside the actions menu.
                        menu_open: confirming || confirming_replace,
                        composer_transfer_open: false,
                        selected: false,
                        locality: HostLocality::Unknown,
                        host_icon: Default::default(),
                        host_color: Default::default(),
                        activity: None,
                        deleting: false,
                        bell_open: None,
                    },
                    on_open,
                    on_clone,
                    on_replace_with,
                    on_mark_seen,
                    on_replace,
                    on_confirm_replace,
                    on_cancel_replace,
                    on_stop,
                    on_delete,
                    on_confirm_delete,
                    on_cancel_delete,
                    on_rename_start,
                    on_menu_toggle,
                    on_bell_toggle,
                    on_bell_close,
                    on_bell_clear,
                }
            }
        }

        let escaped = display_peer(SPOOF);
        let quoted = format!("\"{escaped}\"");
        // Premise: escaping changes this title.
        assert_ne!(escaped, SPOOF);
        for (prompt, cwd, shown) in [
            ((false, false), "/home/example/project", "~/project"),
            ((false, false), "", ""),
            (
                (false, false),
                "/home/example/build\u{200B} \u{202E}lanif",
                "~/build<U+200B> <U+202E>lanif",
            ),
            (
                (true, false),
                "/home/example/build\u{200B} \u{202E}lanif",
                "~/build<U+200B> <U+202E>lanif",
            ),
            (
                (false, true),
                "/home/example/build\u{200B} \u{202E}lanif",
                "~/build<U+200B> <U+202E>lanif",
            ),
        ] {
            CWD.with(|cell| cell.set(cwd));
            PROMPT.with(|cell| cell.set(prompt));
            let mut dom = VirtualDom::new(app);
            // The actual text payloads the DOM receives, not a Debug dump
            // (which would escape the very characters under test).
            let edits = dom.rebuild_to_vec().edits;
            let texts: Vec<String> = edits
                .iter()
                .filter_map(|edit| match edit {
                    dioxus::core::Mutation::CreateTextNode { value, .. }
                    | dioxus::core::Mutation::SetText { value, .. } => Some(value.clone()),
                    _ => None,
                })
                .collect();
            if cwd.is_empty() {
                assert!(
                    texts.iter().all(|text| !text.contains("(empty)")),
                    "an absent folder must not gain a visible placeholder: {texts:?}",
                );
            } else {
                assert!(
                    texts.iter().any(|text| text == shown),
                    "the folder keeps home abbreviation and shows hidden characters: {texts:?}",
                );
            }
            // Isolation: every title surface is a `PeerTitle`, whose class and
            // `dir` reach the DOM as dynamic attributes.
            let attribute = |name: &str, wanted: &dyn Fn(&str) -> bool| {
                edits
                    .iter()
                    .filter(|edit| {
                        matches!(edit, dioxus::core::Mutation::SetAttribute {
                            name: attr,
                            value: dioxus::core::AttributeValue::Text(value),
                            ..
                        } if *attr == name && wanted(value))
                    })
                    .count()
            };
            let isolated_titles = attribute("class", &|value: &str| {
                value.split(' ').any(|class| class == "peer-value")
                    && (value.contains("session-title") || value.contains("confirm-title"))
            });
            let expected = 1 + usize::from(prompt.0 || prompt.1);
            assert_eq!(
                isolated_titles, expected,
                "{prompt:?}: each title surface is a .peer-value span"
            );
            assert!(
                attribute("dir", &|value: &str| value == "ltr") >= expected,
                "{prompt:?}: each title surface carries dir=ltr"
            );
            assert!(
                texts
                    .iter()
                    .all(|text| !text.contains('\u{202E}') && !text.contains('\u{200B}')),
                "{prompt:?}: a raw control character reached a text node: {texts:?}"
            );
            assert!(
                texts.iter().any(|text| text == &escaped),
                "{prompt:?}: the row shows the escaped title: {texts:?}"
            );
            // The title's hover text is the one copy of it that cannot be
            // direction-isolated (a tooltip is a single attribute string),
            // so escaping is its whole defense: the row's tooltip carries
            // the escaped title, and no tooltip carries the raw one.
            assert!(
                attribute("data-tooltip", &|value: &str| value == escaped) >= 1,
                "{prompt:?}: the row title's tooltip is the escaped title"
            );
            assert_eq!(
                attribute("data-tooltip", &|value: &str| value.contains('\u{202E}')
                    || value.contains('\u{200B}')),
                0,
                "{prompt:?}: a raw control character reached a tooltip"
            );
            let confirmation_mounted = texts.iter().any(|text| text == &quoted);
            assert_eq!(
                confirmation_mounted,
                prompt.0 || prompt.1,
                "{prompt:?}: the open confirmation quotes the escaped title, and only then: {texts:?}"
            );
        }
    }

    /// Repeated parent refreshes must update direct callback props in place
    /// without rerendering an otherwise unchanged session row.
    ///
    /// Dioxus gives `EventHandler` component props special ownership and
    /// memoization. Hiding them inside an ordinary props struct bypasses
    /// that path: every fleet refresh then retains another callback set and
    /// rerenders every row. This drives the real `SessionRow` through many
    /// parent renders, so either regression changes the count from one.
    #[farhelm_testtrace::test]
    fn repeated_parent_refreshes_do_not_rerender_an_unchanged_row() {
        fn app() -> Element {
            let on_open = use_callback(|_: Session| {});
            let on_clone = use_callback(|_: Session| {});
            let on_replace_with = use_callback(|_: Session| {});
            let on_mark_seen = use_callback(|_: (String, Option<i64>)| {});
            let on_replace = use_callback(|_: Session| {});
            let on_confirm_replace = use_callback(|_: Session| {});
            let on_cancel_replace = use_callback(|_: String| {});
            let on_stop = use_callback(|_: String| {});
            let on_delete = use_callback(|_: DeleteTarget| {});
            let on_confirm_delete = use_callback(|_: (String, crate::DeleteGuard)| {});
            let on_cancel_delete = use_callback(|_: String| {});
            let on_rename_start = use_callback(|_: (String, String)| {});
            let on_menu_toggle = use_callback(|_: String| {});
            let on_bell_toggle = use_callback(|_: (String, u64, u64)| {});
            let on_bell_close = use_callback(|_: String| {});
            let on_bell_clear = use_callback(|_: (String, u64)| {});
            let session = row_specimen("session-1");
            rsx! {
                SessionRow {
                    session,
                    compact: false,
                    state: RowState {
                        error: None,
                        busy: false,
                        confirming: false,
                        confirming_replace: false,
                        renaming: false,
                        nav_disabled: false,
                        menu_open: false,
                        composer_transfer_open: false,
                        selected: false,
                        locality: HostLocality::Unknown,
                        host_icon: Default::default(),
                        host_color: Default::default(),
                        activity: None,
                        deleting: false,
                        bell_open: None,
                    },
                    on_open,
                    on_clone,
                    on_replace_with,
                    on_mark_seen,
                    on_replace,
                    on_confirm_replace,
                    on_cancel_replace,
                    on_stop,
                    on_delete,
                    on_confirm_delete,
                    on_cancel_delete,
                    on_rename_start,
                    on_menu_toggle,
                    on_bell_toggle,
                    on_bell_close,
                    on_bell_clear,
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
    #[farhelm_testtrace::test]
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
            let on_open = use_callback(|_: Session| {});
            let on_clone = use_callback(|_: Session| {});
            let on_replace_with = use_callback(|_: Session| {});
            let on_mark_seen = use_callback(|_: (String, Option<i64>)| {});
            let on_replace = use_callback(|_: Session| {});
            let on_confirm_replace = use_callback(|_: Session| {});
            let on_cancel_replace = use_callback(|_: String| {});
            let on_stop = use_callback(|_: String| {});
            let on_delete = use_callback(|_: DeleteTarget| {});
            let on_confirm_delete = use_callback(|_: (String, crate::DeleteGuard)| {});
            let on_cancel_delete = use_callback(|_: String| {});
            let on_rename_start = use_callback(|_: (String, String)| {});
            let on_menu_toggle = use_callback(|_: String| {});
            let on_bell_toggle = use_callback(|_: (String, u64, u64)| {});
            let on_bell_close = use_callback(|_: String| {});
            let on_bell_clear = use_callback(|_: (String, u64)| {});
            let selected = SELECTED.with(|selected| selected.get());
            rsx! {
                for id in ["session-1", "session-2", "session-3"] {
                    SessionRow {
                        key: "{id}",
                        session: row_specimen(id),
                        compact: false,
                        state: RowState {
                            error: None,
                            busy: false,
                            confirming: false,
                            confirming_replace: false,
                            renaming: false,
                            nav_disabled: false,
                            menu_open: false,
                            composer_transfer_open: false,
                            selected: selected == id,
                            locality: HostLocality::Unknown,
                            host_icon: Default::default(),
                            host_color: Default::default(),
                            activity: None,
                            deleting: false,
                            bell_open: None,
                        },
                        on_open,
                        on_clone,
                        on_replace_with,
                        on_mark_seen,
                        on_replace,
                        on_confirm_replace,
                        on_cancel_replace,
                        on_stop,
                        on_delete,
                        on_confirm_delete,
                        on_cancel_delete,
                        on_rename_start,
                        on_menu_toggle,
                        on_bell_toggle,
                        on_bell_close,
                        on_bell_clear,
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

    /// Render one row whose session carries `notifications` with read mark
    /// `read_through`, its list open (with that read mark as the opening
    /// boundary) when `open` is set, and return the DOM edits.
    fn bell_row_edits(
        notifications: Vec<crate::SessionNotification>,
        read_through: u64,
        open: bool,
    ) -> Vec<dioxus::core::Mutation> {
        std::thread_local! {
            static CASE: std::cell::RefCell<Option<(Vec<crate::SessionNotification>, u64, bool)>> =
                const { std::cell::RefCell::new(None) };
        }
        CASE.with(|case| *case.borrow_mut() = Some((notifications, read_through, open)));
        fn app() -> Element {
            let (notifications, read_through, open) =
                CASE.with(|case| case.borrow().clone()).expect("case set");
            let session = Session {
                notifications,
                notifications_read_through: read_through,
                ..row_specimen("belled")
            };
            rsx! {
                SessionRow {
                    session,
                    compact: false,
                    state: RowState {
                        error: None,
                        busy: false,
                        confirming: false,
                        confirming_replace: false,
                        renaming: false,
                        nav_disabled: false,
                        menu_open: false,
                        composer_transfer_open: false,
                        selected: false,
                        locality: HostLocality::Unknown,
                        host_icon: Default::default(),
                        host_color: Default::default(),
                        activity: None,
                        deleting: false,
                        bell_open: open.then_some(read_through),
                    },
                    on_open: |_: Session| {},
                    on_clone: |_: Session| {},
                    on_replace_with: |_: Session| {},
                    on_mark_seen: |_: (String, Option<i64>)| {},
                    on_replace: |_: Session| {},
                    on_confirm_replace: |_: Session| {},
                    on_cancel_replace: |_: String| {},
                    on_stop: |_: String| {},
                    on_delete: |_: DeleteTarget| {},
                    on_confirm_delete: |_: (String, crate::DeleteGuard)| {},
                    on_cancel_delete: |_: String| {},
                    on_rename_start: |_: (String, String)| {},
                    on_menu_toggle: |_: String| {},
                    on_bell_toggle: |_: (String, u64, u64)| {},
                    on_bell_close: |_: String| {},
                    on_bell_clear: |_: (String, u64)| {},
                }
            }
        }
        VirtualDom::new(app).rebuild_to_vec().edits
    }

    /// Every text-valued attribute the edits set under `name`.
    fn attribute_values(edits: &[dioxus::core::Mutation], name: &str) -> Vec<String> {
        edits
            .iter()
            .filter_map(|edit| match edit {
                dioxus::core::Mutation::SetAttribute {
                    name: attr,
                    value: dioxus::core::AttributeValue::Text(value),
                    ..
                } if *attr == name => Some(value.clone()),
                _ => None,
            })
            .collect()
    }

    /// Every text node the edits create or set.
    fn text_values(edits: &[dioxus::core::Mutation]) -> Vec<String> {
        edits
            .iter()
            .filter_map(|edit| match edit {
                dioxus::core::Mutation::CreateTextNode { value, .. }
                | dioxus::core::Mutation::SetText { value, .. } => Some(value.clone()),
                _ => None,
            })
            .collect()
    }

    /// Unresolved history for row-rendering tests; individual scenarios may
    /// resolve it without changing the sequence, as the supervisor does.
    fn notification(seq: u64, text: &str) -> crate::SessionNotification {
        crate::SessionNotification {
            seq,
            at: 1_700_000_000,
            text: text.to_string(),
            resolved: false,
        }
    }

    /// The bell's three states (SPEC.md, Status): absent, taking no space,
    /// on a row with no notifications; quiet when every notification is
    /// read; loud, filled, when any is unread. The accessible name carries
    /// the unread count in every state that has a bell, because the loud
    /// colour says nothing to a screen reader.
    #[farhelm_testtrace::test]
    fn the_bell_is_absent_quiet_or_loud_with_the_unread_count_in_its_name() {
        // Static classes never appear as attribute edits, so presence is
        // read from the bell's dynamic attributes; the reserved slot's
        // geometry is the browser suite's to check.
        let none = bell_row_edits(Vec::new(), 0, false);
        let classes = attribute_values(&none, "class");
        assert!(
            !classes
                .iter()
                .any(|class| class.contains("session-row-bell")),
            "a row without notifications has no bell: {classes:?}"
        );
        assert!(
            !attribute_values(&none, "aria-label")
                .iter()
                .any(|label| label.starts_with("notifications")),
        );

        let read = bell_row_edits(vec![notification(2, "b"), notification(1, "a")], 2, false);
        let classes = attribute_values(&read, "class");
        assert!(classes.iter().any(|class| class == "btn session-row-bell"));
        assert!(
            attribute_values(&read, "data-glyph").contains(&"bell".to_string()),
            "a read bell is drawn outlined"
        );
        assert!(
            attribute_values(&read, "aria-label")
                .contains(&"notifications for stable: none unread".to_string())
        );

        let unread = bell_row_edits(vec![notification(3, "c"), notification(2, "b")], 1, false);
        assert!(
            attribute_values(&unread, "class")
                .iter()
                .any(|class| class == "btn session-row-bell loud")
        );
        assert!(attribute_values(&unread, "data-glyph").contains(&"bell-unread".to_string()));
        assert!(
            attribute_values(&unread, "aria-label")
                .contains(&"notifications for stable: 2 unread".to_string())
        );
        assert!(
            attribute_values(&unread, "data-tooltip")
                .iter()
                .any(|tooltip| tooltip.starts_with("notifications: 2 unread")),
            "the bell has hover help naming the count"
        );
        assert!(
            attribute_values(&unread, "data-row-menu-key").is_empty(),
            "a closed bell renders no list"
        );
    }

    /// The open list shows every notification newest first, marks the ones
    /// above the read mark as of opening as new, and renders the
    /// supervisor's text as escaped, isolated peer text, since a supervisor
    /// is not trusted to write markup or direction controls into the
    /// sidebar. (The visible "new" word is static template text, which
    /// these edits do not carry; the browser suite checks it.)
    #[farhelm_testtrace::test]
    fn the_open_list_marks_new_entries_and_escapes_supervisor_text() {
        const SPOOF: &str = "hook \u{202E}kcatta";
        let edits = bell_row_edits(
            vec![
                notification(3, SPOOF),
                notification(2, "older"),
                notification(1, "oldest"),
            ],
            2,
            true,
        );
        assert_eq!(
            attribute_values(&edits, "data-row-menu-key"),
            vec!["bell:belled"],
            "the open list carries the key its outside-click relay answers to"
        );
        let classes = attribute_values(&edits, "class");
        let entry_classes: Vec<&String> = classes
            .iter()
            .filter(|class| class.starts_with("session-bell-entry") && !class.contains("meta"))
            .collect();
        assert_eq!(
            entry_classes,
            vec![
                "session-bell-entry new",
                "session-bell-entry",
                "session-bell-entry"
            ],
            "newest first, and only the entry above the read mark is new"
        );
        assert_eq!(
            attribute_values(&edits, "data-notification-seq"),
            vec!["3", "2", "1"]
        );
        let texts = text_values(&edits);
        assert!(texts.iter().any(|text| *text == display_peer(SPOOF)));
        assert!(
            texts.iter().all(|text| !text.contains('\u{202E}')),
            "a raw direction control reached a text node: {texts:?}"
        );
    }

    /// Resolved history stays visible but is read even above the opening
    /// read mark. A newer recurrence must be loud and new again; otherwise
    /// recovery would either hide the history or silence a later problem.
    #[farhelm_testtrace::test]
    fn resolved_notifications_are_quiet_and_never_new() {
        let mut resolved = notification(3, "recovered");
        resolved.resolved = true;
        assert!(!resolved.is_unread_after(0));
        let quiet = bell_row_edits(vec![resolved.clone()], 0, false);
        assert!(
            attribute_values(&quiet, "aria-label")
                .contains(&"notifications for stable: none unread".into())
        );
        assert!(
            !attribute_values(&quiet, "class")
                .iter()
                .any(|class| class.contains("loud"))
        );
        let open = bell_row_edits(vec![resolved, notification(2, "still a problem")], 0, true);
        let classes = attribute_values(&open, "class");
        assert!(classes.contains(&"session-bell-entry resolved".into()));
        assert_eq!(
            classes
                .iter()
                .filter(|class| class.as_str() == "session-bell-entry new")
                .count(),
            1
        );
        let recurrence = notification(4, "came back");
        assert!(recurrence.is_unread_after(3));
        let reopened = bell_row_edits(vec![recurrence], 3, true);
        assert!(attribute_values(&reopened, "class").contains(&"session-bell-entry new".into()));
        assert!(
            attribute_values(&reopened, "aria-label")
                .contains(&"notifications for stable: 1 unread".into())
        );
    }

    /// `stale`, `selected` and `menu-open` are independent row states and
    /// every combination must say so in the class list — in particular a
    /// selected STALE row carries both, because selection must not hide
    /// the SPEC.md-required stale marking and staleness must not hide
    /// which session the main pane is on.
    ///
    /// `menu-open` identifies the row whose side flyout is visible, so
    /// the row itself has to say which "⋯" owns the
    /// open menu (see `row_class`). It must not displace either of the
    /// other two — a stale, selected row with its menu up is still stale
    /// and still the selection.
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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

    /// An agent launch carrying `selection`, as a session row holds one.
    fn agent_launch(selection: crate::LaunchSelection) -> Option<crate::SessionLaunch> {
        Some(crate::SessionLaunch::Agent {
            selection,
            start: vec!["agent".to_string(), "{farhelm_args}".to_string()],
            resume: None,
        })
    }

    /// Structured metadata wins over a contradictory retained command, so a
    /// display helper never turns a legacy-looking string into replacement
    /// launch intent or hides the declared harness a user selected.
    #[farhelm_testtrace::test]
    fn structured_launch_metadata_is_authoritative_for_the_agent_badge() {
        let session = Session {
            launch: agent_launch(crate::LaunchSelection {
                harness: LaunchHarness::Claude,
                model: None,
                effort: None,
                permissions: Some(crate::LaunchPermission::Yolo),
                workspace_trust: None,
            }),
            invocation: "unknown-command --anything".to_string(),
            ..row_specimen("structured-agent-badge")
        };
        assert_eq!(
            agent_badge(&session),
            AgentBadge {
                harness: HarnessGlyph::Claude,
                permission: PermissionGlyph::Yolo,
                description: "Claude Code — YOLO permission bypass — unknown-command --anything"
                    .to_string(),
            }
        );
    }

    /// Structured permission metadata distinguishes Goose's gated modes from
    /// YOLO, while an older omitted Pi field still describes Pi's sole
    /// effective mode.
    #[farhelm_testtrace::test]
    fn structured_goose_and_pi_badges_use_effective_permission_modes() {
        let goose = Session {
            launch: agent_launch(crate::LaunchSelection {
                harness: LaunchHarness::Goose,
                model: Some("z-ai/glm-5.3".into()),
                effort: None,
                permissions: Some(crate::LaunchPermission::SmartApprove),
                workspace_trust: None,
            }),
            invocation: "goose session".to_string(),
            ..row_specimen("structured-goose-badge")
        };
        assert_eq!(
            agent_badge(&goose).permission,
            PermissionGlyph::SmartApprove
        );
        assert!(agent_badge(&goose).description.contains("smart approve"));

        let pi = Session {
            launch: agent_launch(crate::LaunchSelection {
                harness: LaunchHarness::Pi,
                model: Some("z-ai/glm-5.3".into()),
                effort: None,
                permissions: None,
                workspace_trust: None,
            }),
            invocation: "pi --provider openrouter".to_string(),
            ..row_specimen("structured-pi-badge")
        };
        assert_eq!(agent_badge(&pi).permission, PermissionGlyph::Yolo);
        assert!(
            agent_badge(&pi)
                .description
                .contains("YOLO permission bypass")
        );
    }

    /// Every row carries exactly one permission mark, and it comes from the
    /// launch itself (SPEC.md): an agent launch's mode, a command launch's
    /// assertion, or the unclassified mark for a legacy session, never a
    /// reading of the command line.
    #[farhelm_testtrace::test]
    fn every_row_has_a_permission_mark_without_inferring_approval() {
        for (permission, expected) in [
            (None, PermissionGlyph::Default),
            (Some(crate::LaunchPermission::Yolo), PermissionGlyph::Yolo),
        ] {
            let session = Session {
                launch: agent_launch(crate::LaunchSelection {
                    harness: LaunchHarness::Codex,
                    model: None,
                    effort: None,
                    permissions: permission,
                    workspace_trust: None,
                }),
                invocation: "codex".into(),
                ..row_specimen("structured-mode")
            };
            assert_eq!(agent_badge(&session).permission, expected);
            assert!(
                agent_badge(&session)
                    .description
                    .contains(permission_description(expected))
            );
        }
        // A command launch carries its assertion and says it is one, and a
        // legacy session is unclassified whatever its command spells;
        // nothing reads the command (`codex --yolo` asserted not YOLO stays
        // a plain shield).
        for (yolo, expected) in [
            (true, PermissionGlyph::AssertedYolo),
            (false, PermissionGlyph::AssertedNotYolo),
        ] {
            let session = Session {
                launch: Some(crate::SessionLaunch::Command(crate::CommandLaunch {
                    command: "codex --yolo".to_string(),
                    yolo,
                    agent: Some(LaunchHarness::Codex),
                    resume: None,
                })),
                invocation: "codex --yolo".into(),
                ..row_specimen("asserted-mode")
            };
            let badge = agent_badge(&session);
            assert_eq!(badge.permission, expected);
            assert_eq!(
                badge.harness,
                HarnessGlyph::Codex,
                "the declared agent type"
            );
            assert!(
                badge.description.contains("asserted"),
                "{}",
                badge.description
            );
        }
        for invocation in ["codex --yolo", "sleep 300"] {
            let session = Session {
                launch: Some(crate::SessionLaunch::Legacy {
                    invocation: invocation.to_string(),
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                }),
                invocation: invocation.into(),
                ..row_specimen("unknown-mode")
            };
            let badge = agent_badge(&session);
            assert_eq!(badge.permission, PermissionGlyph::Unknown, "{invocation}");
            assert_eq!(badge.harness, HarnessGlyph::Terminal, "{invocation}");
            assert!(badge.description.contains("unclassified"));
        }
        for (permission, expected) in [
            (crate::LaunchPermission::Approve, PermissionGlyph::Approve),
            (
                crate::LaunchPermission::SmartApprove,
                PermissionGlyph::SmartApprove,
            ),
            (crate::LaunchPermission::Chat, PermissionGlyph::Chat),
        ] {
            let session = Session {
                launch: agent_launch(crate::LaunchSelection {
                    harness: LaunchHarness::Goose,
                    model: None,
                    effort: None,
                    permissions: Some(permission),
                    workspace_trust: None,
                }),
                invocation: "goose session".into(),
                ..row_specimen("approval-mode")
            };
            assert_eq!(agent_badge(&session).permission, expected);
            assert!(
                agent_badge(&session)
                    .description
                    .contains(permission_description(expected))
            );
        }
    }

    /// Real shell-word splitting (`shell_words::split`, the same parser
    /// `farhelm-supervisor` uses on a session's invocation), pinned against
    /// the specific quoting shapes a hand-rolled partial parser gets wrong.
    #[farhelm_testtrace::test]
    fn the_basename_survives_every_shell_quoting_shape() {
        // A backslash-escaped space in an otherwise unquoted path.
        assert_eq!(
            command_basename("/opt/with\\ space/bin/claude --dangerously-skip-permissions"),
            "claude"
        );
        // Adjacent quoted and unquoted fragments glue into ONE argv[0] —
        // the shape `shell_words::quote` itself produces for a path with
        // spaces when only part of it needs quoting. The space sits in the
        // segment BEFORE the basename on purpose: a naive split that broke
        // on it would still (by coincidence) recover the right basename
        // from the wrong first token, so this fixture only passes if the
        // space was actually consumed as part of the same argv[0].
        assert_eq!(
            command_basename("\"/opt/with space\"/bin/codex --yolo"),
            "codex"
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
            command_basename("\"/opt/with space/bin/farhelm\" internal fake-agent"),
            "farhelm"
        );
        assert_eq!(
            command_basename("'/opt/with space/bin/farhelm' internal fake-agent"),
            "farhelm"
        );
        // A quoted flag after the program.
        assert_eq!(command_basename("codex \"--yolo\""), "codex");
        // `#` starts a real POSIX comment at a word boundary — this parser
        // is not a hand-rolled stand-in, it is the genuine article — so
        // everything from it to the end of the string vanishes from argv
        // entirely rather than surviving as a literal trailing token.
        assert_eq!(command_basename("sleep 300 #not-a-comment"), "sleep");
        // Single quotes behave exactly like double quotes for this parser.
        assert_eq!(command_basename("'codex' --full-auto"), "codex");
    }

    /// Degenerate invocations render something rather than panicking or
    /// inventing a word.
    ///
    /// None of these can come out of the supervisor, which refuses a
    /// create it cannot split into argv — they come from route stubs in
    /// the browser suite and from whatever a future wire change allows.
    /// The contract is that the element still renders and says only what
    /// it was given.
    #[farhelm_testtrace::test]
    fn a_degenerate_invocation_falls_back_to_what_it_was_given() {
        assert_eq!(command_basename(""), "");
        assert_eq!(command_basename("   "), "");
        assert_eq!(
            command_basename("/usr/bin/"),
            "/usr/bin/",
            "a token with no basename to take stands as it is"
        );
        assert_eq!(
            command_basename("\"unbalanced --yolo"),
            "\"unbalanced --yolo",
            "an unclosed quote is something shell_words itself cannot resolve, so this falls \
             back to the trimmed raw text rather than guessing at structure that was never there"
        );
    }
}
