//! Structured launch intent shared by the helm and supervisor.
//!
//! The helm fills the permission and workspace-trust choices for a new
//! launch. An omitted model or effort lets the harness choose its default.
//! Plain shells have neither agent options nor an agent permission mode.
//! The supervisor stores the resolved choices beside argv so plain Restart
//! and Replace can reuse the command. A saved agent launch with no recorded
//! permission is treated as YOLO because its safety mode is unknown.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// A resolved one-time start, separate from the session's durable launch.
///
/// The helm fills an existing resume command or composes a named start for a
/// harness without conversation reporting. The supervisor runs this argv for
/// this request only; captured identity still comes solely from agent reports.
/// Keeping the selector beside argv binds both into create retry identity and
/// lets admission validate it before any process can be stopped or started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedConversationStart {
    pub conversation: String,
    pub argv: Vec<String>,
}

crate::enum_with_all! {
    /// The command-line harness selected for a structured launch.
    ///
    /// This is distinct from [`crate::AgentKind`]. `AgentKind` describes the
    /// supervisor features available after a process starts; this enum preserves
    /// the harness the user selected. Cursor, OpenCode and the plain shells
    /// share the generic runtime. The launch type still carries their
    /// distinct choices and permissions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum LaunchHarness {
        /// Cursor launches with the Generic runtime: no conversation tracking or Resume.
        Cursor,
        Codex,
        Claude,
        Muse,
        /// Goose's OpenRouter-backed terminal session command.
        Goose,
        /// Pi's OpenRouter-backed terminal command.
        Pi,
        /// OMP's OpenRouter-backed terminal command — the Pi fork's launch
        /// surface, compiled beside Pi's with OMP's own approval-mode and
        /// thinking flags. Like Pi, an OMP launch is a report-only integration:
        /// the structured choice records intent, while conversation capture and
        /// resume stay the supervisor's `AgentKind::Omp` behavior.
        Omp,
        /// OpenCode's terminal UI, intentionally kept on the generic runtime
        /// integration because Farhelm does not capture or resume its sessions.
        OpenCode,
        /// Grok's native terminal UI. Tracked launches require `--no-leader`.
        Grok,
        /// Antigravity CLI. Launch choices work with or without its manual hooks.
        Antigravity,
        /// An interactive Bash shell on the host's PATH.
        Bash,
        /// An interactive Zsh shell on the host's PATH.
        Zsh,
        /// A user-provided integration whose catalog ID travels beside the harness.
        Custom,
    }
}

#[warn(clippy::wildcard_enum_match_arm)]
impl LaunchHarness {
    /// Picker lists that only show release-supplied choices omit the custom catalog marker.
    pub const BUILT_INS: &'static [Self] = match Self::ALL.split_last() {
        Some((Self::Custom, rest)) => rest,
        _ => panic!("the custom catalog marker must follow the built-in harnesses"),
    };

    /// The product name for an agent type in listings and launch descriptions. The custom
    /// catalog marker names the type only. A registered entry supplies its own display name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor",
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::Muse => "Muse Code",
            Self::Goose => "Goose",
            Self::Pi => "Pi",
            Self::Omp => "OMP",
            Self::OpenCode => "OpenCode",
            Self::Grok => "Grok",
            Self::Antigravity => "Antigravity",
            Self::Bash => "Bash",
            Self::Zsh => "Zsh",
            Self::Custom => "Custom agent",
        }
    }

    /// Custom launches resolve an ID in the helm's user catalog and use that entry's separate literal command.
    pub const fn uses_custom_catalog(self) -> bool {
        match self {
            Self::Custom => true,
            Self::Cursor
            | Self::Codex
            | Self::Claude
            | Self::Muse
            | Self::Goose
            | Self::Pi
            | Self::Omp
            | Self::OpenCode
            | Self::Grok
            | Self::Antigravity
            | Self::Bash
            | Self::Zsh => false,
        }
    }

    /// Plain shells use the same structured launch path as agents, but have
    /// no agent permission mode or conversation to resume.
    pub const fn is_shell(self) -> bool {
        match self {
            Self::Bash | Self::Zsh => true,
            Self::Cursor
            | Self::Codex
            | Self::Claude
            | Self::Muse
            | Self::Goose
            | Self::Pi
            | Self::Omp
            | Self::OpenCode
            | Self::Grok
            | Self::Antigravity
            | Self::Custom => false,
        }
    }

    /// Whether a command-line prompt leaves this harness interactive.
    /// Muse accepts one on a fresh start, but its resume subcommand accepts
    /// only a session reference. Goose's interactive session has no prompt
    /// argument. Pi treats a leading `@` as a file attachment even after `--`,
    /// so it cannot preserve arbitrary literal messages. These are launch-harness
    /// facts, independent of reporting kind.
    pub const fn supports_initial_prompt(self, named: bool) -> bool {
        match self {
            LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => false,
            LaunchHarness::Muse => !named,
            LaunchHarness::Cursor
            | LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Omp
            | LaunchHarness::OpenCode
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity => true,
        }
    }

    /// Literal arguments appended after Farhelm expands its own placeholders.
    /// Positional prompts follow an option terminator, so leading hyphens
    /// cannot become agent options. OpenCode and Antigravity attach the
    /// value to a dedicated flag, so even a prompt of `--help` stays data.
    /// This must never be added to the session's reusable launch or resume argv.
    pub fn initial_prompt_arguments(self, prompt: &str) -> Result<Vec<String>, String> {
        match self {
            LaunchHarness::Goose => Err("Goose does not accept an initial prompt".into()),
            LaunchHarness::Pi => Err("Pi cannot accept an arbitrary literal initial prompt".into()),
            LaunchHarness::Custom => Err("a custom agent does not accept an initial prompt".into()),
            LaunchHarness::Bash | LaunchHarness::Zsh => {
                Err("a shell session does not accept an initial prompt".into())
            }
            LaunchHarness::OpenCode => Ok(vec![format!("--prompt={prompt}")]),
            LaunchHarness::Antigravity => Ok(vec![format!("--prompt-interactive={prompt}")]),
            LaunchHarness::Cursor
            | LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Omp
            | LaunchHarness::Grok => Ok(vec!["--".into(), prompt.into()]),
        }
    }

    /// Whether `order` lists every harness exactly once: the check behind a
    /// UI display order that differs from declaration order.
    ///
    /// A `const fn` so a display order can assert it at compile time
    /// (`const _: () = assert!(LaunchHarness::is_ordering_of_all(&ORDER));`).
    /// That turns "a new harness is missing from this picker" into a build
    /// failure at the list that needs a decision about where it goes.
    pub const fn is_ordering_of_all(order: &[LaunchHarness]) -> bool {
        Self::is_ordering(order, Self::ALL)
    }

    /// The fixed part of a picker is exhaustive over built-in harnesses. The custom choices are data.
    pub const fn is_ordering_of_built_ins(order: &[LaunchHarness]) -> bool {
        Self::is_ordering(order, Self::BUILT_INS)
    }

    const fn is_ordering(order: &[LaunchHarness], expected: &[LaunchHarness]) -> bool {
        if order.len() != expected.len() {
            return false;
        }
        let mut i = 0;
        while i < expected.len() {
            let mut seen = 0;
            let mut j = 0;
            while j < order.len() {
                if order[j] as usize == expected[i] as usize {
                    seen += 1;
                }
                j += 1;
            }
            if seen != 1 {
                return false;
            }
            i += 1;
        }
        true
    }

    /// The supervisor integration a structured launch of this harness runs
    /// under.
    ///
    /// The one place the pairing is stated: the helm compiles a launch with
    /// it, the supervisor refuses a create whose harness and kind disagree,
    /// and its store refuses a row that recorded a mismatched pair, so a copy
    /// that drifted would make a valid create look invalid on one side only.
    /// Exhaustive so a new harness has to choose its integration here.
    pub fn agent_kind(self) -> crate::AgentKind {
        use crate::AgentKind;
        match self {
            LaunchHarness::Codex => AgentKind::Codex,
            LaunchHarness::Claude => AgentKind::Claude,
            LaunchHarness::Muse => AgentKind::Muse,
            LaunchHarness::Cursor
            | LaunchHarness::OpenCode
            | LaunchHarness::Bash
            | LaunchHarness::Zsh => AgentKind::Generic,
            LaunchHarness::Goose => AgentKind::Goose,
            LaunchHarness::Pi => AgentKind::Pi,
            LaunchHarness::Omp => AgentKind::Omp,
            LaunchHarness::Grok => AgentKind::Grok,
            LaunchHarness::Antigravity => AgentKind::Antigravity,
            LaunchHarness::Custom => AgentKind::Custom,
        }
    }

    /// The kind a structured row stored before launch kinds existed.
    ///
    /// Conversion validates historical data against its original vocabulary:
    /// Muse was Generic then. Using today's mapping would discard a valid saved
    /// Muse selection when it gains an integration. This does not backfill a
    /// resume command or change the kind derived from a current launch.
    pub(crate) fn pre_launch_agent_kind(self) -> crate::AgentKind {
        use crate::AgentKind;
        match self {
            LaunchHarness::Muse | LaunchHarness::Cursor | LaunchHarness::OpenCode => {
                AgentKind::Generic
            }
            LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => self.agent_kind(),
        }
    }

    /// Whether a structured launch can carry a workspace-trust choice.
    /// Settings supplies it for new compositions. A saved launch carries
    /// the choice used by plain Restart and Replace.
    ///
    /// Exhaustive, like every per-harness capability here, so a new harness
    /// is a compile error until it answers.
    pub const fn offers_workspace_trust(self) -> bool {
        match self {
            LaunchHarness::Claude
            | LaunchHarness::Codex
            | LaunchHarness::Muse
            | LaunchHarness::Pi
            | LaunchHarness::Antigravity => true,
            LaunchHarness::Cursor
            | LaunchHarness::Goose
            | LaunchHarness::Omp
            | LaunchHarness::OpenCode
            | LaunchHarness::Grok
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => false,
        }
    }

    /// Whether this harness offers a model choice
    /// ([`LaunchSelection::model`]).
    ///
    /// A harness without one (Grok, whose CLI model contract this release
    /// does not carry) takes neither a model nor an effort: the launcher hides
    /// its model control, lists no models for it, clears both fields when the
    /// user switches to it, and treats a selection carrying either as
    /// incompatible. The helm validates the final request on its own.
    pub const fn offers_model(self) -> bool {
        match self {
            LaunchHarness::Cursor
            | LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::OpenCode
            | LaunchHarness::Antigravity
            | LaunchHarness::Custom => true,
            LaunchHarness::Grok | LaunchHarness::Bash | LaunchHarness::Zsh => false,
        }
    }

    /// The release catalog's spelling of a model typed for this harness: the
    /// form every catalog lookup (ownership, effort offering) must compare,
    /// while the selection keeps the text the user typed.
    ///
    /// OpenCode accepts a bare Zen model name as well as its `opencode/`
    /// spelling (SPEC.md), but its catalog entries are listed only under the
    /// provider-qualified form. Several bare Zen names (`gpt-6-luna`, for
    /// one) are also catalog entries of another harness, so comparing the
    /// typed text read an OpenCode launch of `gpt-6-luna` as a Codex model:
    /// the helm refused it and the browser switched the harness to Codex. A
    /// value that already names a provider is returned unchanged; whether
    /// that provider is allowed is the helm's argv compiler's decision, not
    /// this spelling's. Every other harness's catalog ids are exactly what
    /// the user types.
    pub fn catalog_model_id(self, model: &str) -> Cow<'_, str> {
        match self {
            LaunchHarness::OpenCode if !model.contains('/') => {
                Cow::Owned(format!("opencode/{model}"))
            }
            LaunchHarness::OpenCode
            | LaunchHarness::Cursor
            | LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => Cow::Borrowed(model),
        }
    }

    /// Whether this harness has an effort vocabulary
    /// ([`LaunchSelection::effort`]), which decides whether the launcher shows
    /// an effort control at all.
    ///
    /// Which efforts a harness accepts is release catalog data the helm owns
    /// and serves; this only says whether there is any such choice, which the
    /// launcher needs before (and regardless of whether) a catalog arrived.
    pub const fn offers_effort(self) -> bool {
        match self {
            LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::Antigravity
            | LaunchHarness::Custom => true,
            LaunchHarness::Cursor
            | LaunchHarness::OpenCode
            | LaunchHarness::Grok
            | LaunchHarness::Bash
            | LaunchHarness::Zsh => false,
        }
    }

    /// The permission selected for a new request that omits its permission.
    ///
    /// Harnesses that offer Guarded start there. Cursor, Pi, OpenCode and
    /// Goose default to YOLO, and shells have no permission mode. Use
    /// [`Self::stored_permission`] to classify argv already saved in a session.
    pub const fn omitted_permission(self) -> Option<LaunchPermission> {
        match self {
            LaunchHarness::Cursor
            | LaunchHarness::Pi
            | LaunchHarness::OpenCode
            | LaunchHarness::Custom
            | LaunchHarness::Goose => Some(LaunchPermission::Yolo),
            LaunchHarness::Bash | LaunchHarness::Zsh => None,
            LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Omp
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity => Some(LaunchPermission::Guarded),
        }
    }

    /// Whether YOLO is the only approval mode this harness offers.
    ///
    /// This is derived from the exhaustive permission table rather than kept
    /// as another per-harness list, so the composer and confirmation cannot
    /// drift when a mode is added or removed.
    pub const fn offers_only_yolo(self) -> bool {
        if !matches!(self.omitted_permission(), Some(LaunchPermission::Yolo)) {
            return false;
        }
        let mut index = 0;
        while index < LaunchPermission::ALL.len() {
            let permission = LaunchPermission::ALL[index];
            if !matches!(permission, LaunchPermission::Yolo) && self.offers_permission(permission) {
                return false;
            }
            index += 1;
        }
        self.offers_permission(LaunchPermission::Yolo)
    }

    /// Whether a different choice can keep a permission check or limit what
    /// the agent does. Custom remains available on some YOLO-only harnesses,
    /// but its flags cannot establish any safety guarantee for Farhelm.
    pub fn offers_non_yolo_mode(self) -> bool {
        LaunchPermission::ALL.into_iter().any(|permission| {
            !matches!(
                permission,
                LaunchPermission::Yolo | LaunchPermission::Custom
            ) && self.offers_permission(permission)
        })
    }

    /// Resolve a draft request against the harness's current offered modes.
    ///
    /// Unsupported older values fall back to the omitted mode. The helm uses
    /// stricter admission: it fills omissions but rejects unsupported explicit
    /// values instead of silently accepting this display fallback.
    pub const fn request_permission(
        self,
        permission: Option<LaunchPermission>,
    ) -> Option<LaunchPermission> {
        match permission {
            Some(value) if self.offers_permission(value) => Some(value),
            _ => self.omitted_permission(),
        }
    }

    /// Classify a stored launch using the permission it recorded.
    ///
    /// A stored command with no supported permission is treated as YOLO.
    /// Restart, Replace and agent copies can reuse the original argv, so the
    /// current default for a newly composed launch cannot establish its safety
    /// mode. Shells always have no permission mode.
    pub const fn stored_permission(
        self,
        permission: Option<LaunchPermission>,
    ) -> Option<LaunchPermission> {
        match permission {
            Some(value) if self.offers_permission(value) => Some(value),
            _ if self.is_shell() => None,
            _ => Some(LaunchPermission::Yolo),
        }
    }

    /// The guarded argv shared by launch compilation and pre-launch help.
    ///
    /// These flags select the vendor's automatic guarded mode over a bypass
    /// default. They do not override allow rules, plugins or later mode
    /// switches. Empty slices name harnesses without this offered mode; callers
    /// must still validate the requested permission before compiling it.
    pub const fn guarded_flags(self) -> &'static [&'static str] {
        match self {
            LaunchHarness::Claude | LaunchHarness::Grok => &["--permission-mode", "auto"],
            LaunchHarness::Antigravity => &["--mode", "auto"],
            LaunchHarness::Codex => &["-a", "on-request", "-s", "workspace-write"],
            LaunchHarness::Muse => &["--permission-profile", ":auto-review"],
            LaunchHarness::Omp => &["--approval-mode", "write"],
            LaunchHarness::Cursor
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::OpenCode
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => &[],
        }
    }

    /// Whether the launcher can start this harness in a conversation the user names.
    ///
    /// This says nothing about later Restart: Cursor and OpenCode accept a
    /// one-time selector but still have no conversation reporting or stored
    /// resume command. (Muse has both; see [`Self::captures_named_conversation`].)
    /// Keep the match exhaustive when adding a harness.
    pub const fn offers_named_conversation(self) -> bool {
        match self {
            LaunchHarness::Claude
            | LaunchHarness::Codex
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::Muse
            | LaunchHarness::Cursor
            | LaunchHarness::OpenCode
            | LaunchHarness::Custom => true,
            LaunchHarness::Bash | LaunchHarness::Zsh => false,
        }
    }

    /// Whether the named selector is a file interpreted in the session's folder.
    ///
    /// File selectors keep the strict argument grammar and accept relative
    /// paths, but must refuse tilde because no shell expands this argument.
    pub const fn named_conversation_is_file(self) -> bool {
        match self {
            LaunchHarness::Pi | LaunchHarness::Omp => true,
            LaunchHarness::Claude
            | LaunchHarness::Codex
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity
            | LaunchHarness::Goose
            | LaunchHarness::Muse
            | LaunchHarness::Cursor
            | LaunchHarness::OpenCode
            | LaunchHarness::Custom
            | LaunchHarness::Bash
            | LaunchHarness::Zsh => false,
        }
    }

    /// Whether a named start can replace the session's captured conversation.
    ///
    /// Goose's reporter is attached only to fresh starts. Keeping its old
    /// capture across a named restart would leave Restart aimed at a different
    /// conversation indefinitely, so that restart must clear it transactionally.
    pub const fn captures_named_conversation(self) -> bool {
        match self {
            LaunchHarness::Claude
            | LaunchHarness::Codex
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::Muse
            | LaunchHarness::Custom => true,
            LaunchHarness::Goose
            | LaunchHarness::Cursor
            | LaunchHarness::OpenCode
            | LaunchHarness::Bash
            | LaunchHarness::Zsh => false,
        }
    }

    /// Whether this harness offers `permission` as an explicit choice
    /// ([`LaunchSelection::permissions`]).
    ///
    /// The one statement of the per-harness permission sets: the helm refuses
    /// any other explicit permission, and the launcher only offers, keeps, and
    /// considers compatible the ones this allows. Before it existed each of
    /// those kept its own table, and adding OMP's Approve had to touch every
    /// one. Exhaustive over harnesses so a new one has to decide here rather
    /// than inheriting a catch-all.
    ///
    /// The harness default (`None`) is not a question for this function: every
    /// harness accepts it. Harnesses whose omitted mode means YOLO are listed
    /// separately by [`Self::omitted_permission`]; this table only describes
    /// explicit permission buttons.
    pub const fn offers_permission(self, permission: LaunchPermission) -> bool {
        match self {
            LaunchHarness::Goose => matches!(
                permission,
                LaunchPermission::Yolo
                    | LaunchPermission::Approve
                    | LaunchPermission::SmartApprove
                    | LaunchPermission::Chat
            ),
            LaunchHarness::Omp => {
                matches!(
                    permission,
                    LaunchPermission::Yolo
                        | LaunchPermission::Approve
                        | LaunchPermission::Guarded
                        | LaunchPermission::Custom
                )
            }
            LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity => matches!(
                permission,
                LaunchPermission::Yolo | LaunchPermission::Guarded | LaunchPermission::Custom
            ),
            LaunchHarness::Cursor => matches!(
                permission,
                LaunchPermission::Yolo | LaunchPermission::Custom
            ),
            LaunchHarness::Pi | LaunchHarness::OpenCode | LaunchHarness::Custom => {
                matches!(permission, LaunchPermission::Yolo)
            }
            LaunchHarness::Bash | LaunchHarness::Zsh => false,
        }
    }
}

/// An explicit reasoning-effort value requested from a structured harness.
///
/// The release-owned catalog and built-in controls use this closed vocabulary.
/// Wire and stored selections accept custom values through [`LaunchEffortValue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchEffort {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    Ultra,
}

impl LaunchEffort {
    /// Every effort, in the order the launcher offers them, for places that
    /// name the whole vocabulary to a person (the CLI's refusal of an
    /// unknown `--effort`).
    pub const ALL: [LaunchEffort; 8] = [
        LaunchEffort::Off,
        LaunchEffort::Minimal,
        LaunchEffort::Low,
        LaunchEffort::Medium,
        LaunchEffort::High,
        LaunchEffort::Xhigh,
        LaunchEffort::Max,
        LaunchEffort::Ultra,
    ];

    /// Return the spelling accepted by the harness command-line interfaces.
    ///
    /// Serde uses snake case for wire stability, while the CLI spelling is a
    /// stable literal argument. These happen to agree today; spelling it here
    /// keeps command generation from depending on serde's representation.
    pub const fn as_cli_arg(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
            Self::Ultra => "ultra",
        }
    }
}

/// The launch and template wire shape is one string for both built-in and custom efforts.
///
/// Match the known vocabulary first so existing selections round-trip in their old spelling.
/// A built-in harness still needs a known, compatible effort before argv can be composed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LaunchEffortValue {
    Known(LaunchEffort),
    Custom(String),
}

impl LaunchEffortValue {
    /// A release-catalog value, if the string belongs to the built-in effort vocabulary.
    pub fn known(&self) -> Option<LaunchEffort> {
        match self {
            Self::Known(effort) => Some(*effort),
            Self::Custom(_) => None,
        }
    }

    /// The exact spelling carried through the wire and supplied to a custom wrapper.
    pub fn as_cli_arg(&self) -> &str {
        match self {
            Self::Known(effort) => effort.as_cli_arg(),
            Self::Custom(effort) => effort,
        }
    }
}

impl From<LaunchEffort> for LaunchEffortValue {
    fn from(effort: LaunchEffort) -> Self {
        Self::Known(effort)
    }
}

impl From<String> for LaunchEffortValue {
    /// Keep the canonical stored representation when a free-text value has a built-in spelling.
    fn from(effort: String) -> Self {
        LaunchEffort::ALL
            .into_iter()
            .find(|known| known.as_cli_arg() == effort)
            .map_or_else(|| Self::Custom(effort), Self::Known)
    }
}

/// A permission choice that adds a deliberate harness flag.
///
/// `Guarded` records explicit flags, rather than inferring safety from an
/// omitted field. A stored launch with omitted permissions counts as YOLO
/// because Farhelm has no explicit mode to rely on. No value promises that vendor
/// rules or a later in-session mode switch cannot loosen the launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPermission {
    Guarded,
    Yolo,
    /// User-supplied words replace Farhelm's permission flags. Their meaning is
    /// deliberately unchecked, so every custom launch is classified as YOLO.
    Custom,
    Approve,
    SmartApprove,
    Chat,
}

impl LaunchPermission {
    /// Every permission, for places that name the whole vocabulary to a
    /// person, like the helm's refusal of an
    /// unknown remembered-permissions word, so the list they print grows with
    /// the enum instead of being retyped.
    pub const ALL: [LaunchPermission; 6] = [
        LaunchPermission::Guarded,
        LaunchPermission::Yolo,
        LaunchPermission::Custom,
        LaunchPermission::Approve,
        LaunchPermission::SmartApprove,
        LaunchPermission::Chat,
    ];

    /// The wire spelling (the serde `snake_case` name), for places that store
    /// or compare the choice as text, like the helm's remembered-permissions
    /// preference.
    pub fn wire_word(self) -> &'static str {
        match self {
            LaunchPermission::Guarded => "guarded",
            LaunchPermission::Yolo => "yolo",
            LaunchPermission::Custom => "custom",
            LaunchPermission::Approve => "approve",
            LaunchPermission::SmartApprove => "smart_approve",
            LaunchPermission::Chat => "chat",
        }
    }
}

/// The full structured intent for one launch, before catalog compilation.
///
/// `model` remains a literal identifier so a release catalog can recognize
/// supported IDs while still allowing the explicit custom-model escape hatch.
/// It is never a shell fragment or a sequence of command-line arguments.
/// The wire shape permits an absent model for every harness default. Provider
/// qualification belongs only to compiled argv, so persisted intent retains
/// the identifier the user entered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchSelection {
    pub harness: LaunchHarness,
    /// A required catalog ID for custom agent launches. Omission preserves existing launch fingerprints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_agent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<LaunchEffortValue>,
    pub permissions: Option<LaunchPermission>,
    /// Literal permission arguments, present exactly for a Custom choice.
    /// Stored as words so clones and resumes preserve argument boundaries;
    /// quoting style and shell expansion have no part in the persisted intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_flags: Option<Vec<String>>,
    /// Raw shell words prepended to the composed start and resume commands.
    /// Quotes and escapes remain exactly as typed; joining with spaces recovers the
    /// wrapper source that Bash interprets before Farhelm's quoted agent arguments.
    /// Absence preserves old launch fingerprints. A wrapper is the user's assertion
    /// that the prefixed command still runs this agent; Farhelm does not inspect or
    /// trust the wrapper's own behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapper_prefix: Option<Vec<String>>,
    /// Decode-only choice from retired supervisor-outside containers. Presence
    /// identifies a Delete-only session; it must never start a new launch.
    /// Keep its bytes for stored fingerprints and old session decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerConfig>,
    /// The launch's resolved trust for project-local settings and extensions.
    ///
    /// This is separate from tool approval and from Farhelm's hook-trust
    /// bypass. New requests carry no authority in this field: the helm
    /// overwrites it from Settings. Stored
    /// values, including false, remain meaningful for exact Restart/Replace
    /// and compiler reproduction. Older selections lack the field and retain the vendor's
    /// ordinary startup behavior. An unset value stays absent in serialized
    /// JSON because that encoding also serves as a durable launch fingerprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_trust: Option<bool>,
}

/// The supervisor-outside container model remains readable for deletion only.
/// New recipe-backed environments use an ordinary supervisor launch instead.
pub const RETIRED_CONTAINER_REASON: &str = "Containerized sessions are retired and support Delete only. Create a new container session from a recipe.";

/// The engine spelling retained by historical launch records. Unknown values
/// still fail decoding rather than changing the meaning of stored work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerEngine {
    Podman,
}

/// Decode-only configuration for retired supervisor-outside containers.
/// Preserve old rows and fingerprints for confirmed deletion; new environments
/// carry their container identity in the helm host registry instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "ContainerConfigWire")]
pub struct ContainerConfig {
    pub engine: ContainerEngine,
    pub image: String,
    pub api_proxy_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_token_file: Option<String>,
}

/// Read retired archive choices without interpreting or retaining their paths.
/// All stored surfaces use this decoder. An explicit current image wins if both
/// keys occur; old-only data retains the retired choice without recreating its
/// archive path. Other unknown fields still refuse.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainerConfigWire {
    engine: ContainerEngine,
    #[serde(default)]
    image: String,
    #[serde(default, rename = "image_file")]
    _retired_image_file: Option<serde::de::IgnoredAny>,
    api_proxy_port: u16,
    #[serde(default)]
    github_token_file: Option<String>,
}

impl From<ContainerConfigWire> for ContainerConfig {
    fn from(wire: ContainerConfigWire) -> Self {
        Self {
            engine: wire.engine,
            image: wire.image,
            api_proxy_port: wire.api_proxy_port,
            github_token_file: wire.github_token_file,
        }
    }
}

impl ContainerConfig {
    /// Validate historical shape without authorizing a new container launch.
    /// Stored compiler fingerprints still need this structural check; callers
    /// admitting work must separately refuse any present legacy choice.
    pub fn validate(&self) -> Result<(), String> {
        if self.image.starts_with('-')
            || self
                .image
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err("container image must have no whitespace or control characters and cannot start with '-'".into());
        }
        // Podman also accepts sources outside its local storage. Refuse those
        // explicitly, including transports whose details look like a valid tag.
        if [
            "containers-storage:",
            "dir:",
            "docker-archive:",
            "docker-daemon:",
            "oci:",
            "oci-archive:",
            "ostree:",
            "sif:",
        ]
        .iter()
        .any(|prefix| self.image.starts_with(prefix))
            || self.image.contains("://")
        {
            return Err("container image must name an image in Podman's local storage, without a transport prefix".into());
        }
        if let Some(path) = self.github_token_file.as_deref()
            && (!path.starts_with('/') || path.contains('\0'))
        {
            return Err("container GitHub token file must be an absolute host path".into());
        }
        if self.api_proxy_port == 0 {
            return Err("container API proxy port must be between 1 and 65535".into());
        }
        Ok(())
    }
}

/// Split editable wrapper source without removing its quotes or escapes.
///
/// Only word boundaries are interpreted here. Each stored word retains its original
/// spelling so joining with spaces gives Bash the user's shell syntax at execution and
/// gives editors the same spelling on redisplay. Interword space/tab/newline and comments
/// are discarded; quoted whitespace and physical continuations within a word survive.
/// Literal shape validation still uses `shell_words`, including its acceptance of a
/// trailing unquoted backslash. That spelling acquires Bash meaning when later joined
/// to the agent command; it is not converted back into a literal backslash argument.
pub fn split_wrapper_prefix(text: &str) -> Result<Vec<String>, String> {
    validate_wrapper_source(text)?;
    let words = raw_wrapper_words(text);
    check_wrapper_prefix(Some(&words))?;
    Ok(words)
}

/// Check the persisted wrapper vector's shape without reinterpreting its word spelling.
///
/// This is also a stored-data boundary: words saved with literal argv semantics may contain
/// unmatched quote bytes. Their joined source now has Bash meaning at execution, but they
/// must remain readable. Syntax and dequoted-marker checks belong to the editor splitter;
/// API vectors retain this same shape contract. Only exact raw placeholder elements can
/// participate in Farhelm's later composed-argument filling, and those remain forbidden.
pub fn check_wrapper_prefix(words: Option<&[String]>) -> Result<(), String> {
    let Some(words) = words else { return Ok(()) };
    if words.first().is_none_or(String::is_empty) {
        return Err("wrapper prefix needs a program".into());
    }
    if words.iter().any(|word| word.contains('\0')) {
        return Err("wrapper prefix cannot contain NUL".into());
    }
    if words
        .iter()
        .any(|word| crate::session_launch::shell_placeholders().any(|marker| marker == word))
    {
        return Err("wrapper prefix cannot contain Farhelm placeholders".into());
    }
    Ok(())
}

/// Keep the editor's literal shape oracle separate from raw spelling recovery.
///
/// NUL must be checked before parsing: a comment can remove it from the literal words.
/// Decoding also catches quoted/escaped spellings of reserved markers without teaching
/// the span scan about them. Direct API and stored vectors keep their shape-only check.
fn validate_wrapper_source(text: &str) -> Result<(), String> {
    if text.contains('\0') {
        return Err("wrapper prefix cannot contain NUL".into());
    }
    let words =
        shell_words::split(text).map_err(|error| format!("invalid wrapper prefix: {error}"))?;
    check_wrapper_prefix(Some(&words))
}

/// Recover original word slices after the independent literal parser accepted the source.
///
/// This deliberately has only the quote, escape and comment states used by `shell_words`.
/// Operators, substitutions and expansions remain source bytes for Bash, not syntax for
/// this splitter. A continuation before a word is whitespace; within a word it belongs
/// to that word's raw spelling. Byte indices keep non-ASCII text intact.
fn raw_wrapper_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut start = None;
    let mut quote = None;
    let mut chars = text.char_indices();
    while let Some((at, ch)) = chars.next() {
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                }
            }
            Some('"') => match ch {
                '"' => quote = None,
                '\\' => {
                    chars.next();
                }
                _ => {}
            },
            Some(_) => unreachable!("only single and double quotes establish quote state"),
            None => match ch {
                ' ' | '\t' | '\n' => {
                    if let Some(begin) = start.take() {
                        words.push(text[begin..at].to_owned());
                    }
                }
                '#' if start.is_none() => {
                    for (_, comment_ch) in chars.by_ref() {
                        if comment_ch == '\n' {
                            break;
                        }
                    }
                }
                '\\' => {
                    // shell_words does not start an empty word for a bare continuation.
                    // Keeping it outside the slice also avoids manufacturing a stored word.
                    let next = chars.next();
                    if start.is_some() || !matches!(next, Some((_, '\n'))) {
                        start.get_or_insert(at);
                    }
                }
                '\'' | '"' => {
                    start.get_or_insert(at);
                    quote = Some(ch);
                }
                _ => {
                    start.get_or_insert(at);
                }
            },
        }
    }
    if let Some(begin) = start {
        words.push(text[begin..].to_owned());
    }
    words
}

/// Split editable permission text without evaluating expansions or commands.
/// Shell-style quotes and escapes only determine word boundaries; a `#` at
/// the start of a word begins a comment. Empty/comment-only input is refused
/// so every custom permission choice supplies literal flags.
pub fn split_permission_flags(text: &str) -> Result<Vec<String>, String> {
    let words =
        shell_words::split(text).map_err(|error| format!("invalid permission flags: {error}"))?;
    if words.is_empty() {
        return Err("custom permission flags must contain at least one word".into());
    }
    if words.iter().any(|word| word.contains('\0')) {
        return Err("permission flags cannot contain NUL".into());
    }
    Ok(words)
}

/// Custom permission flags as their editor shows them: the literal words quoted back into one line that
/// [`split_permission_flags`] reads as the same words. The launcher seeds its flags editor with this when it opens on a
/// stored launch (Clone, Replace with, Restart with), so an untouched editor sends back exactly the stored words.
pub fn join_permission_flags(words: &[String]) -> String {
    shell_words::join(words)
}

/// Enforce the stored permission/argument relationship at every admission path.
/// The same splitter used by editors validates a joined wire vector, preserving
/// even quoted spaces and literal expansion syntax. Semantic flag meanings and
/// supervisor placeholders remain the compiler's responsibility.
pub fn check_permission_flags(
    permission: Option<LaunchPermission>,
    flags: Option<&[String]>,
) -> Result<(), String> {
    match (permission, flags) {
        (Some(LaunchPermission::Custom), Some(words)) => {
            split_permission_flags(&shell_words::join(words)).map(|_| ())
        }
        (Some(LaunchPermission::Custom), None) => {
            Err("custom permissions need permission flags".into())
        }
        (_, Some(_)) => Err("permission flags are only allowed with custom permissions".into()),
        (_, None) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    /// Literal messages must remain positional data, even when they look like
    /// options or Farhelm placeholders. None of these argv shapes opts into
    /// a harness's noninteractive print/exec mode.
    #[test]
    fn initial_prompt_arguments_preserve_literal_text_for_each_harness() {
        for harness in super::LaunchHarness::ALL {
            for text in [
                "-p",
                "@notes.txt",
                "{farhelm_args}",
                "{cwd}",
                "resume",
                "first\n\tsecond",
            ] {
                let args = harness.initial_prompt_arguments(text);
                if !harness.supports_initial_prompt(false) {
                    assert!(args.is_err());
                    continue;
                }
                let args = args.unwrap();
                match harness {
                    super::LaunchHarness::OpenCode => {
                        assert_eq!(args, [format!("--prompt={text}")])
                    }
                    super::LaunchHarness::Antigravity => {
                        assert_eq!(args, [format!("--prompt-interactive={text}")])
                    }
                    super::LaunchHarness::Goose
                    | super::LaunchHarness::Pi
                    | super::LaunchHarness::Bash
                    | super::LaunchHarness::Zsh
                    | super::LaunchHarness::Custom => unreachable!(),
                    super::LaunchHarness::Cursor
                    | super::LaunchHarness::Codex
                    | super::LaunchHarness::Claude
                    | super::LaunchHarness::Muse
                    | super::LaunchHarness::Omp
                    | super::LaunchHarness::Grok => assert_eq!(args, ["--", text]),
                }
            }
        }
        assert!(!super::LaunchHarness::Muse.supports_initial_prompt(true));
        assert!(super::LaunchHarness::Codex.supports_initial_prompt(true));
        assert!(super::LaunchHarness::Antigravity.supports_initial_prompt(true));
        assert_eq!(
            super::LaunchHarness::Antigravity
                .initial_prompt_arguments("--help")
                .unwrap(),
            ["--prompt-interactive=--help"]
        );
    }

    use super::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection};

    /// Quoting chooses argv boundaries, never execution. Comments and malformed
    /// quotes cannot turn an empty custom draft into an accepted launch.
    #[test]
    fn custom_flags_preserve_literal_words_and_require_a_complete_choice() {
        let words =
            super::split_permission_flags(r#"--mode 'ask me' "$HOME" '$(touch x)' # ignored"#)
                .unwrap();
        assert_eq!(words, ["--mode", "ask me", "$HOME", "$(touch x)"]);
        assert_eq!(
            super::split_permission_flags(&shell_words::join(&words)).unwrap(),
            words
        );
        for text in ["", "  ", "# comment", "--mode '", "--mode \0"] {
            assert!(super::split_permission_flags(text).is_err(), "{text:?}");
        }
        assert!(
            super::check_permission_flags(Some(LaunchPermission::Custom), Some(&words)).is_ok()
        );
        for (mode, flags) in [
            (Some(LaunchPermission::Custom), None),
            (Some(LaunchPermission::Custom), Some(&[][..])),
            (Some(LaunchPermission::Guarded), Some(words.as_slice())),
            (None, Some(words.as_slice())),
        ] {
            assert!(super::check_permission_flags(mode, flags).is_err());
        }
    }

    /// New request defaults must not rewrite the meaning of an older stored
    /// launch. Its restart, replace and inherited copies still carry the old
    /// argv, so only a new compilation may establish guarded intent.
    #[test]
    fn stored_omissions_remain_distinct_from_request_defaults() {
        for harness in [
            LaunchHarness::Claude,
            LaunchHarness::Codex,
            LaunchHarness::Muse,
            LaunchHarness::Grok,
            LaunchHarness::Antigravity,
        ] {
            assert_eq!(
                harness.request_permission(None),
                Some(LaunchPermission::Guarded)
            );
            assert_eq!(
                harness.stored_permission(None),
                Some(LaunchPermission::Yolo)
            );
            assert_eq!(
                harness.stored_permission(Some(LaunchPermission::Guarded)),
                Some(LaunchPermission::Guarded)
            );
        }
        assert_eq!(
            LaunchHarness::Cursor.request_permission(None),
            Some(LaunchPermission::Yolo)
        );
        assert_eq!(
            LaunchHarness::Cursor.stored_permission(None),
            Some(LaunchPermission::Yolo)
        );
        assert_eq!(
            LaunchHarness::Omp.request_permission(None),
            Some(LaunchPermission::Guarded)
        );
        assert_eq!(
            LaunchHarness::Omp.stored_permission(None),
            Some(LaunchPermission::Yolo)
        );
        assert!(!LaunchHarness::Goose.offers_permission(LaunchPermission::Guarded));
    }

    /// Adding Guarded to an agent must also make it the new-request default.
    /// Stored launches are deliberately outside this rule: their argv already
    /// records what ran, and a new default must not reclassify an old session.
    #[test]
    fn every_harness_offering_guarded_defaults_to_guarded() {
        for harness in LaunchHarness::ALL {
            if harness.offers_permission(LaunchPermission::Guarded) {
                assert_eq!(
                    harness.omitted_permission(),
                    Some(LaunchPermission::Guarded)
                );
            }
        }
    }

    /// Spec: `LaunchPermission::ALL` lists every variant exactly once.
    ///
    /// Why: callers print it as "the words this helm accepts"; a variant
    /// missing from it would be accepted but never mentioned. The exhaustive
    /// match below fails to compile when a variant is added, which is the
    /// reminder to extend `ALL` as well; it cannot check that on its own.
    #[test]
    fn all_permissions_lists_every_variant_once() {
        for permission in LaunchPermission::ALL {
            match permission {
                LaunchPermission::Guarded
                | LaunchPermission::Yolo
                | LaunchPermission::Custom
                | LaunchPermission::Approve
                | LaunchPermission::SmartApprove
                | LaunchPermission::Chat => {}
            }
            assert_eq!(
                LaunchPermission::ALL
                    .iter()
                    .filter(|other| **other == permission)
                    .count(),
                1,
                "{permission:?}"
            );
        }
    }

    /// Spec: `LaunchHarness::ALL` lists every harness once, in declaration
    /// order, and `is_ordering_of_all` accepts exactly the permutations of it.
    ///
    /// Why: UI display orders assert `is_ordering_of_all` at compile time as
    /// their only guard against leaving a new harness out of a picker, so a
    /// check that accepted a duplicate in place of a missing harness, or a
    /// short list, would let exactly that drift through.
    #[test]
    fn all_harnesses_and_ordering_check() {
        assert_eq!(
            LaunchHarness::ALL
                .iter()
                .map(|harness| *harness as usize)
                .collect::<Vec<_>>(),
            (0..LaunchHarness::ALL.len()).collect::<Vec<_>>()
        );
        assert!(LaunchHarness::is_ordering_of_all(LaunchHarness::ALL));
        let mut reversed = LaunchHarness::ALL.to_vec();
        reversed.reverse();
        assert!(LaunchHarness::is_ordering_of_all(&reversed));
        assert!(!LaunchHarness::is_ordering_of_all(&LaunchHarness::ALL[1..]));
        let mut duplicated = LaunchHarness::ALL.to_vec();
        duplicated[0] = duplicated[1];
        assert!(!LaunchHarness::is_ordering_of_all(&duplicated));
        let mut extended = LaunchHarness::ALL.to_vec();
        extended.push(LaunchHarness::Codex);
        assert!(!LaunchHarness::is_ordering_of_all(&extended));
    }

    /// Spec: the launcher hides unsupported choices, including the model,
    /// effort and permission fields of both shells.
    ///
    /// Why: these predicates replaced per-harness checks scattered through
    /// the browser and the helm, so a slip here changes what the composer
    /// shows, what it clears on a harness switch, and what the helm fills in,
    /// all at once. The matrix is spelled out rather than derived so the test
    /// states the rule independently of the implementation.
    #[test]
    fn each_harness_offers_exactly_its_launch_choices() {
        use LaunchHarness::*;
        // (harness, offers model, offers effort, omitted permission, only YOLO)
        let expected = [
            (Cursor, true, false, Some(LaunchPermission::Yolo), false),
            (Codex, true, true, Some(LaunchPermission::Guarded), false),
            (Claude, true, true, Some(LaunchPermission::Guarded), false),
            (Muse, true, true, Some(LaunchPermission::Guarded), false),
            (Goose, true, true, Some(LaunchPermission::Yolo), false),
            (Pi, true, true, Some(LaunchPermission::Yolo), true),
            (Omp, true, true, Some(LaunchPermission::Guarded), false),
            (OpenCode, true, false, Some(LaunchPermission::Yolo), true),
            (Grok, false, false, Some(LaunchPermission::Guarded), false),
            (
                Antigravity,
                true,
                true,
                Some(LaunchPermission::Guarded),
                false,
            ),
            (Bash, false, false, None, false),
            (Zsh, false, false, None, false),
            (Custom, true, true, Some(LaunchPermission::Yolo), true),
        ];
        assert_eq!(expected.len(), LaunchHarness::ALL.len());
        for (harness, model, effort, omitted, only_yolo) in expected {
            assert_eq!(harness.offers_model(), model, "{harness:?} model");
            assert_eq!(harness.offers_effort(), effort, "{harness:?} effort");
            assert_eq!(harness.omitted_permission(), omitted, "{harness:?} default");
            assert_eq!(
                harness.offers_only_yolo(),
                only_yolo,
                "{harness:?} only YOLO"
            );
        }
    }

    /// Spec: each agent offers its own explicit permission modes, and
    /// Bash and Zsh offer none.
    ///
    /// Why: this is the table both the helm's validation and the launcher's
    /// buttons and normalization read, so a slip here changes what users can
    /// pick and what the helm accepts at once. The matrix is spelled out
    /// rather than derived so the test states the rule independently, and an
    /// exhaustive match makes a new harness a compile error here until it has
    /// a row.
    #[test]
    fn each_harness_offers_exactly_its_permissions() {
        use LaunchPermission::{Approve, Chat, Custom, Guarded, SmartApprove, Yolo};
        // Exhaustive on purpose: add the new harness to the matrix below.
        match LaunchHarness::Goose {
            LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Cursor
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::OpenCode
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => {}
        }
        let all = [Guarded, Yolo, Custom, Approve, SmartApprove, Chat];
        for (harness, offered) in [
            (
                LaunchHarness::Goose,
                &[Yolo, Approve, SmartApprove, Chat][..],
            ),
            (LaunchHarness::Omp, &[Guarded, Yolo, Custom, Approve][..]),
            (LaunchHarness::Codex, &[Guarded, Yolo, Custom][..]),
            (LaunchHarness::Claude, &[Guarded, Yolo, Custom][..]),
            (LaunchHarness::Muse, &[Guarded, Yolo, Custom][..]),
            (LaunchHarness::Cursor, &[Yolo, Custom][..]),
            (LaunchHarness::Pi, &[Yolo][..]),
            (LaunchHarness::OpenCode, &[Yolo][..]),
            (LaunchHarness::Grok, &[Guarded, Yolo, Custom][..]),
            (LaunchHarness::Antigravity, &[Guarded, Yolo, Custom][..]),
            (LaunchHarness::Bash, &[][..]),
            (LaunchHarness::Zsh, &[][..]),
            (LaunchHarness::Custom, &[Yolo][..]),
        ] {
            for permission in all {
                assert_eq!(
                    harness.offers_permission(permission),
                    offered.contains(&permission),
                    "{harness:?} / {permission:?}"
                );
            }
        }
    }

    /// The persisted JSON preserves absence as absence, so later catalog
    /// changes cannot turn a harness default into an invented explicit choice.
    #[test]
    fn omitted_optional_choices_round_trip_as_none() {
        let selection = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::OpenCode,
            model: None,
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };

        let json = serde_json::to_value(&selection).expect("serialize launch selection");
        assert_eq!(json["harness"], "open_code");
        assert!(json["model"].is_null());
        assert!(json["effort"].is_null());
        assert!(json["permissions"].is_null());
        assert!(json.get("workspace_trust").is_none());
        assert!(json.get("wrapper_prefix").is_none());
        assert!(json.get("container").is_none());
        assert_eq!(
            serde_json::from_value::<LaunchSelection>(json).expect("deserialize launch selection"),
            selection
        );
    }

    /// Existing launch history predates workspace trust; decoding it must
    /// retain the harness default rather than inventing consent.
    #[test]
    fn older_selection_without_workspace_trust_decodes_as_unset() {
        let selection: LaunchSelection = serde_json::from_value(serde_json::json!({
            "harness": "muse",
            "model": null,
            "effort": null,
            "permissions": null
        }))
        .expect("decode an older stored selection");
        assert_eq!(selection.workspace_trust, None);
        assert_eq!(selection.wrapper_prefix, None);
    }

    /// Container intent must fail closed before it reaches a host: engine and
    /// field typos cannot silently remove isolation, and images/ports must retain
    /// their host-local meaning without consulting the helm's filesystem.
    #[test]
    fn container_configuration_rejects_unknown_authority_and_invalid_inputs() {
        let valid = serde_json::json!({
            "engine": "podman", "image": "localhost/agent:latest", "api_proxy_port": 65535
        });
        let config: super::ContainerConfig = serde_json::from_value(valid.clone()).unwrap();
        config.validate().unwrap();
        for (field, value) in [
            ("engine", serde_json::json!("other")),
            ("mounts", serde_json::json!(["/host"])),
            ("api_proxy_port", serde_json::json!(65536)),
            ("api_proxy_port", serde_json::json!(-1)),
        ] {
            let mut input = valid.clone();
            input[field] = value;
            assert!(serde_json::from_value::<super::ContainerConfig>(input).is_err());
        }
        for (field, value) in [
            ("image", serde_json::json!("agent with spaces")),
            ("image", serde_json::json!("agent\0")),
            ("image", serde_json::json!("-option")),
            ("image", serde_json::json!("docker://example.invalid/agent")),
            ("image", serde_json::json!("oci-archive:agent.tar")),
            ("github_token_file", serde_json::json!("relative-token")),
            ("github_token_file", serde_json::json!("")),
            ("api_proxy_port", serde_json::json!(0)),
        ] {
            let mut input = valid.clone();
            input[field] = value;
            let config: super::ContainerConfig = serde_json::from_value(input).unwrap();
            assert!(config.validate().is_err());
        }
    }

    /// Retired archive data stays readable on every shared stored surface without
    /// silently turning isolation off or preserving a path as an image name.
    #[test]
    fn container_archive_settings_decode_as_an_empty_local_image() {
        let mut old = serde_json::json!({
            "engine": "podman", "image_file": "/images/old.oci", "api_proxy_port": 42000
        });
        let config: super::ContainerConfig = serde_json::from_value(old.clone()).unwrap();
        config.validate().unwrap();
        assert!(config.image.is_empty());
        let encoded = serde_json::to_value(&config).unwrap();
        assert_eq!(encoded["image"], "");
        assert!(encoded.get("image_file").is_none());
        old["image"] = serde_json::json!("localhost/agent:latest");
        let config: super::ContainerConfig = serde_json::from_value(old).unwrap();
        assert_eq!(config.image, "localhost/agent:latest");
        for image in [
            "",
            "agent",
            "docker:latest",
            "localhost:5000/team/agent:latest",
            "sha256:abc123",
        ] {
            let mut config = config.clone();
            config.image = image.into();
            config.validate().unwrap();
        }
    }

    /// Wrapper storage must preserve shell spelling rather than literal argv. Redisplay
    /// and launch both join these raw words, so quoting them again changes their meaning.
    #[test]
    fn wrapper_prefix_preserves_raw_spelling_and_join_roundtrips() {
        assert_eq!(
            super::split_wrapper_prefix("with-endpoint --profile 'work space' --").unwrap(),
            vec!["with-endpoint", "--profile", "'work space'", "--"]
        );
        for (source, expected) in [
            (
                r#"wrap "$HOME" '$()' work\ space pre'quoted'post ''"#,
                vec![
                    "wrap",
                    r#""$HOME""#,
                    "'$()'",
                    r"work\ space",
                    "pre'quoted'post",
                    "''",
                ],
            ),
            (
                r#"wrap "say \"hi\"" "\$HOME" "\q""#,
                vec!["wrap", r#""say \"hi\"""#, r#""\$HOME""#, r#""\q""#],
            ),
            (
                "écho\u{a0}name \t\"line\nbreak\" \\#literal word#tail # ignored\n next",
                vec![
                    "écho\u{a0}name",
                    "\"line\nbreak\"",
                    "\\#literal",
                    "word#tail",
                    "next",
                ],
            ),
            (
                "\\\nwrap arg\\\nument \\\n tail",
                vec!["wrap", "arg\\\nument", "tail"],
            ),
            ("wrap\\", vec!["wrap\\"]),
        ] {
            let raw = super::split_wrapper_prefix(source).unwrap();
            assert_eq!(raw, expected, "{source:?}");
            let joined = raw.join(" ");
            assert_eq!(
                super::split_wrapper_prefix(&joined).unwrap(),
                raw,
                "{source:?}"
            );
            assert_eq!(
                shell_words::split(&joined).unwrap(),
                shell_words::split(source).unwrap(),
                "{source:?}"
            );
            super::check_wrapper_prefix(Some(&raw)).unwrap();
        }
    }

    /// Editors refuse syntax and dequoted markers, while API/stored vectors retain their
    /// original shape boundary. Re-parsing stored literal quote bytes would make old rows
    /// unreadable; Bash still interprets their raw joined source without a compatibility path.
    #[test]
    fn wrapper_prefix_refuses_invalid_shape_nul_and_dequoted_placeholders() {
        for text in [
            "",
            "# comment only",
            "'' arg",
            "'unfinished",
            "\"unfinished\\",
            "{cwd}",
            "run {conversation}",
            "run {farhelm_args}",
            "run {codex:trusted-cwd}",
            "run {codex:untrusted-cwd}",
            "run '{cwd}'",
            "run \\{cwd\\}",
            "run '{c'wd'}'",
            "run # hidden\0",
        ] {
            assert!(super::split_wrapper_prefix(text).is_err(), "{text:?}");
        }
        for marker in crate::session_launch::shell_placeholders() {
            for spelling in [
                marker.to_string(),
                format!("'{marker}'"),
                format!("\"{marker}\""),
                format!("\\{marker}"),
            ] {
                assert!(super::split_wrapper_prefix(&format!("run {spelling}")).is_err());
            }
            assert!(
                super::check_wrapper_prefix(Some(&["run".into(), marker.to_string()])).is_err()
            );
        }
        assert!(super::check_wrapper_prefix(Some(&["run\0bad".into()])).is_err());
        assert!(super::check_wrapper_prefix(Some(&["run".into(), "# hidden\0".into()])).is_err());
        assert!(super::check_wrapper_prefix(Some(&["".into(), "run".into()])).is_err());
        assert!(super::check_wrapper_prefix(Some(&[])).is_err());
        assert!(super::check_wrapper_prefix(None).is_ok());
        assert!(super::check_wrapper_prefix(Some(&["''".into()])).is_ok());
        for word in ["a'b", "'{cwd}'", "\\{cwd\\}", "''"] {
            super::check_wrapper_prefix(Some(&["run".into(), word.into()])).unwrap();
        }
    }

    /// Explicit consent and refusal must remain distinct in saved intent and
    /// keyed-create fingerprints even though an unset choice is omitted.
    #[test]
    fn explicit_workspace_trust_values_serialize() {
        let mut selection = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Muse,
            model: None,
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        for choice in [true, false] {
            selection.workspace_trust = Some(choice);
            let json = serde_json::to_value(&selection).expect("serialize explicit trust choice");
            assert_eq!(json["workspace_trust"], choice);
            assert_eq!(
                serde_json::from_value::<LaunchSelection>(json)
                    .expect("decode explicit trust choice"),
                selection
            );
        }
    }

    /// A structured choice must reject unknown keys rather than silently
    /// dropping a misspelled command-affecting field.
    #[test]
    fn structured_selection_rejects_unknown_fields() {
        let error = serde_json::from_value::<LaunchSelection>(serde_json::json!({
            "harness": "codex",
            "model": "gpt-5.6-terra",
            "effrot": "high",
        }))
        .expect_err("misspelled effort must not be ignored");

        assert!(error.to_string().contains("effrot"));
    }

    /// CLI spellings are independent of serde names because argv generation
    /// is an execution boundary, not a JSON presentation detail.
    #[test]
    fn effort_cli_spellings_are_stable() {
        assert_eq!(LaunchEffort::Xhigh.as_cli_arg(), "xhigh");
        assert_eq!(LaunchEffort::Ultra.as_cli_arg(), "ultra");
        assert_eq!(LaunchEffort::Minimal.as_cli_arg(), "minimal");
        assert_eq!(
            serde_json::to_value(LaunchPermission::Yolo).expect("serialize permission"),
            "yolo"
        );
        assert_eq!(
            serde_json::to_value(LaunchPermission::SmartApprove)
                .expect("serialize Goose permission"),
            "smart_approve"
        );
    }

    /// Grok's durable launch tag must remain a closed, stable wire value.
    #[test]
    fn grok_harness_serializes_with_its_exact_wire_name() {
        assert_eq!(
            serde_json::to_value(LaunchHarness::Grok).expect("serialize Grok harness"),
            serde_json::json!("grok")
        );
    }

    /// Why this matters: `wire_word` stands in for serde's spelling wherever
    /// the choice is stored as text; if they differed, a remembered choice
    /// would no longer match what a launch records.
    ///
    /// Specification: every permission's `wire_word` equals its serialized
    /// JSON string.
    #[farhelm_testtrace::test]
    fn permission_wire_words_match_serde() {
        for permission in [
            LaunchPermission::Yolo,
            LaunchPermission::Approve,
            LaunchPermission::SmartApprove,
            LaunchPermission::Chat,
        ] {
            assert_eq!(
                serde_json::to_value(permission).unwrap(),
                serde_json::json!(permission.wire_word())
            );
        }
    }
}
