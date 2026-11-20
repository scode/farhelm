//! What a session's launch says about permission checks: the permission shield, one classification shared by every
//! surface that shows it.
//!
//! SPEC.md, "Session list", defines the shield a session row draws; SPEC.md, "Agent-spawned sessions", has launch and
//! restart approval cards show "one Permissions row using the session list's classification", and prompt cards the
//! same chip presentation. Keeping the classification here lets those cards draw the shield a session's row draws for
//! the same launch. Ported from the old app's `list/row.rs` (`session_permission`, `observed_yolo_mode`) and `icons.rs`
//! (`PermissionIcon`) at the reference commit, where the list and the approval cards also shared one classifier.
//!
//! The sidebar-and-session-view area built this first, in the session list's row marks; it moved here unchanged so
//! that the approval cards, its second consumer, need not reach into the session list's private modules. It stays
//! that area's code. What a surface says about the shield (the row's description, a card's chip words) is that
//! surface's own; only the classification into the four drawings, and the drawings themselves, live here.
//!
//! [`words`] is the permission and effort choices' shared names and help (the launcher area's code, moved out of the
//! launcher once other surfaces read them); it sits here because the same surfaces show both.

pub(crate) mod words;

use farhelm_proto::{LaunchPermission, SessionLaunch};

use crate::assets::Icon;
use crate::harness;
use crate::theme::{Palette, Token};

/// The shield's four drawings, each with its ink (the old `data-glyph` tokens `yolo`, `shielded`, `own-setting`,
/// `unknown`). Several launch permissions share a drawing; the description is what tells them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shield {
    /// The slashed shield, in amber: an agent launch in YOLO or with custom flags (which Farhelm counts as YOLO), a
    /// command asserted YOLO, or an agent seen in a mode without checks.
    Yolo,
    /// The plain shield, in green: an agent launch in guarded or an approval mode, or a command asserted not YOLO.
    Shielded,
    /// The dashed shield, in the quiet foreground: an agent launch that forced no mode, so the agent's own setting
    /// decides. Uncertainty, not an approval guarantee.
    OwnSetting,
    /// The question mark, in amber: a command from before launch kinds, which carries no assertion.
    Unknown,
}

impl Shield {
    /// The bundled drawing.
    pub(crate) const fn icon(self) -> Icon {
        match self {
            Shield::Yolo => Icon::PermissionYolo,
            Shield::Shielded => Icon::PermissionShielded,
            Shield::OwnSetting => Icon::PermissionOwnSetting,
            Shield::Unknown => Icon::PermissionUnknown,
        }
    }

    /// The shield's own ink: `--warn` for YOLO and the unclassified question mark, `--ok` for a shield, `--fg-1` for
    /// the agent's own setting. A selected session row draws it in `--on-fill` instead.
    pub(crate) const fn ink(self, palette: &Palette) -> Token {
        match self {
            Shield::Yolo | Shield::Unknown => palette.warn,
            Shield::Shielded => palette.ok,
            Shield::OwnSetting => palette.fg_1,
        }
    }
}

/// The shield `launch` itself earns: an agent launch by its stored permission (none is the agent's own setting, and
/// `farhelm_proto::yolo::selection_is_yolo` is the one rule for what counts as YOLO, custom flags included), a command
/// launch by its assertion, a legacy launch the question mark. The caller
/// handles a shell before reaching this classification.
fn launched(launch: &SessionLaunch) -> Shield {
    match launch {
        SessionLaunch::Agent { selection, .. } => {
            match selection.harness.stored_permission(selection.permissions) {
                None => Shield::OwnSetting,
                Some(_) if farhelm_proto::yolo::selection_is_yolo(selection) => Shield::Yolo,
                Some(
                    LaunchPermission::Guarded
                    | LaunchPermission::Approve
                    | LaunchPermission::SmartApprove
                    | LaunchPermission::Chat,
                ) => Shield::Shielded,
                // `selection_is_yolo` already answered these; kept as YOLO so a change to that rule can only make
                // the shield louder here, never quieter.
                Some(LaunchPermission::Yolo | LaunchPermission::Custom) => Shield::Yolo,
            }
        }
        SessionLaunch::Command(command) if command.yolo => Shield::Yolo,
        SessionLaunch::Command(_) => Shield::Shielded,
        SessionLaunch::Legacy { .. } => Shield::Unknown,
    }
}

/// The agent's own words for the no-checks mode its status line showed, when the supervisor saw one (`screen_yolo`)
/// and the launch's agent type is one it reads a status line for ([`harness::screen_yolo_mode`]). A legacy launch has
/// no agent type, so it never has an observation, whatever an inconsistent peer sent.
pub(crate) fn observed_yolo_mode(
    launch: &SessionLaunch,
    screen_yolo: bool,
) -> Option<&'static str> {
    launch
        .agent_type()
        .and_then(harness::screen_yolo_mode)
        .filter(|_| screen_yolo)
}

/// The shield the row draws: the YOLO shield when the agent's status line was seen in a mode without checks, else
/// what the launch earns. SPEC.md, "Session list": a definite reading "changes any shield other than YOLO or
/// asserted-YOLO to the YOLO shield", and "never makes the shield safer than its launch choice", which holds by
/// construction, since an observation only ever yields the loudest shield. Display only: the stored launch is what
/// every decision reads. A shell has no shield, even if an old or inconsistent
/// peer sends a screen observation.
pub(crate) fn permission(launch: &SessionLaunch, screen_yolo: bool) -> Option<Shield> {
    if launch.is_shell() {
        return None;
    }
    Some(if observed_yolo_mode(launch, screen_yolo).is_some() {
        Shield::Yolo
    } else {
        launched(launch)
    })
}
