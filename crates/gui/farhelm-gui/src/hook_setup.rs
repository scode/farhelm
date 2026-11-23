//! The two agents whose settings allow Farhelm to install persistent conversation hooks. The settings page and
//! launch prompt describe the same write and the same effect on sessions outside Farhelm.

use farhelm_helm_client::{ApiPath, PathError};
use farhelm_proto::ui_channel::Preferences;
use farhelm_proto::{AgentKind, LaunchHarness};

/// The GUI wording and setting identity for a supported vendor. The protocol owns the capability predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum HookAgent {
    Grok,
    Antigravity,
}

impl HookAgent {
    pub(crate) const ALL: [Self; 2] = [Self::Grok, Self::Antigravity];

    pub(crate) const fn index(self) -> usize {
        self as usize
    }

    /// Find a settings page from a launch's declared kind or the supervisor's session fact.
    #[warn(clippy::wildcard_enum_match_arm)]
    pub(crate) const fn for_kind(kind: AgentKind) -> Option<Self> {
        match kind {
            AgentKind::Grok => Some(Self::Grok),
            AgentKind::Antigravity => Some(Self::Antigravity),
            AgentKind::Claude
            | AgentKind::Codex
            | AgentKind::Goose
            | AgentKind::Pi
            | AgentKind::Omp
            | AgentKind::Muse
            | AgentKind::Custom
            | AgentKind::Generic => None,
        }
    }

    pub(crate) fn for_harness(harness: LaunchHarness) -> Option<Self> {
        Self::for_kind(harness.agent_kind())
    }

    pub(crate) const fn harness(self) -> LaunchHarness {
        match self {
            Self::Grok => LaunchHarness::Grok,
            Self::Antigravity => LaunchHarness::Antigravity,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        crate::harness::name(self.harness())
    }

    pub(crate) fn route(self) -> Result<ApiPath, PathError> {
        let kind = match self {
            Self::Grok => "grok",
            Self::Antigravity => "antigravity",
        };
        ApiPath::api(&["agent-hook-setup", kind])
    }

    /// This setting's focus target is independent of the agent's trust and hint switches.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Grok => "settings-grok-hook-setup",
            Self::Antigravity => "settings-antigravity-hook-setup",
        }
    }

    pub(crate) const fn error_id(self) -> &'static str {
        match self {
            Self::Grok => "settings-grok-hook-setup-error",
            Self::Antigravity => "settings-antigravity-hook-setup-error",
        }
    }

    pub(crate) const fn dismissal_id(self) -> &'static str {
        match self {
            Self::Grok => "settings-grok-hook-setup-hint",
            Self::Antigravity => "settings-antigravity-hook-setup-hint",
        }
    }

    pub(crate) const fn dismissal_field(self) -> &'static str {
        match self {
            Self::Grok => "grok_hook_hint_dismissed",
            Self::Antigravity => "antigravity_hook_hint_dismissed",
        }
    }

    /// The launcher and session band read the same pushed dismissal as the settings switch.
    pub(crate) const fn dismissal(self, prefs: &Preferences) -> Option<bool> {
        match self {
            Self::Grok => prefs.grok_hook_hint_dismissed,
            Self::Antigravity => prefs.antigravity_hook_hint_dismissed,
        }
    }

    /// Explain lasting consent in every surface that can enable it.
    pub(crate) fn consent(self) -> String {
        let agent = self.name();
        format!(
            "Farhelm changes {agent}'s own configuration on every machine where it starts {agent}. The change stays on each machine. The hooks also run in {agent} sessions outside Farhelm, where they only answer the agent."
        )
    }

    /// Say which only file or directory to remove if the user wants to undo prior installations after turning it off.
    pub(crate) const fn removal(self) -> &'static str {
        match self {
            Self::Grok => {
                "Turning this off leaves the hooks in place. To remove them, delete ~/.grok/hooks/farhelm.json (or $GROK_HOME/hooks/farhelm.json if the supervisor has GROK_HOME set)."
            }
            Self::Antigravity => {
                "Turning this off leaves the hooks in place. To remove them, delete ~/.gemini/config/plugins/farhelm/."
            }
        }
    }
}
