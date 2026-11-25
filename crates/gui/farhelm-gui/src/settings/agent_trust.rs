//! Each agent's Settings page holds its opt-in for answering workspace-trust questions on installed and persistent
//! hosts. Disposable session hosts grant supported trust during launch composition. Agents with a discovery hint
//! have a separate switch that restores or dismisses it.
//!
//! SPEC.md, "Managed checkouts" (the Agent trust paragraphs). Ported from the old app's `ClaudeTrustSettings`
//! (`claude_trust.rs`) and `WorkspaceTrustSettings` (`workspace_trust.rs`) at reference commit `24bc0580`, with their
//! labels, help, Claude's caution box and their failure lines. Here's the TLDR of how the section talks to the helm:
//!
//! - **The trust switches are helm configuration, not preferences.** Each lives behind its own route
//!   (`/api/checkout-claude-trust` for Claude, the route the old app used, and `/api/checkout-agent-trust/<agent>` for
//!   the others), which the helm does not push. Each page reads its own choice when it is first shown in an
//!   opening of the dialog (the pattern the migration's orchestrator settled for reads that are not pushed), and a
//!   reply reaches only the opening it was sent for, since it holds that dialog weakly. A switch whose read has not
//!   answered, or failed, is unavailable rather than shown off: SPEC.md, "Failed settings reads never count as an off
//!   switch".
//! - **A trust save goes through the operation store** under [`crate::operations::OpKey::preferences`], the preference
//!   row's key, although trust is helm configuration, not a preference: the store admits one request per key, and
//!   helm configuration has no key of its own. Sharing the key means a trust save is refused while any other Settings
//!   save is in flight (and the other way round), as the module docs of [`super`] describe for the switches. This
//!   area accepted that on 2026-11-14, under its goal's instruction not to request a key for helm configuration
//!   before it is needed: the embedded helm answers these saves in milliseconds, so they rarely meet. While it is in
//!   flight the switch shows the value being saved; a done save becomes the value the section
//!   holds (the helm acknowledged it), and a refusal shows the value read before with the helm's reason under the
//!   switch. Nothing is retried or read back.
//! - **The hint switches are preferences.** Whether a hint was dismissed is a field of the pushed preference row, which
//!   the hint band reads too, so those switches are ordinary Settings switches ([`super::choices::Choice`]) drawn here.
//!   The trust routes also answer with the dismissal; the section ignores it and reads the store, which stays current.
//! - **The session view's trust hint band reads its per-agent facts here too**: which agent a session's trust question
//!   belongs to ([`TrustAgent::asking`]) and the band's words ([`TrustAgent::band_words`]).

use farhelm_helm_client::{ApiPath, PathError};
use farhelm_proto::{LaunchHarness, SessionInfo};
use gpui::{
    AnyElement, App, Div, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Role, Styled as _, Window, div, px,
};

use super::choices::{self, Choice};
use super::configured::{self, Configured};
use super::{Refused, card};
use crate::components::{Checkbox, HoverHelp};
use crate::host_text::HostText;
use crate::operations::Command;
use crate::theme::{self, metrics};

/// `.checkout-claude-trust`'s gap between its rows.
const BLOCK_GAP: f32 = 8.;
/// `.claude-trust-caution`: its padding, 12-pixel words at `line-height: 1.5`, and its paragraphs 8 pixels apart.
const CAUTION_PAD: f32 = 12.;
const CAUTION_LINE_HEIGHT: f32 = 18.;
const CAUTION_GAP: f32 = 8.;

pub(super) const CAUTION_ID: &str = "settings-claude-trust-caution";
pub(super) const CAUTION_TITLE: &str = "How this works, and why it's off by default";
const CLAUDE_TRUST_RISK: &str = "The repository's Claude hooks and commands can run, as when you answer Yes. In a folder you choose, \
     the trust entry stays after the session ends. When Farhelm adds trust on the first launch in a new managed checkout, \
     it tries to remove the entry when it archives the checkout.";
const FLAG_TRUST_RISK: &str = "As when you approve it yourself, repository hooks, settings and commands are then trusted. This \
     is why the switch is off by default.";
pub(super) const CAUTION: [&str; 3] = [
    "Claude has no supported way to do this. Farhelm edits the settings file selected by the login shell \
     (normally ~/.claude.json on the session's host), which also holds sign-in and per-folder state. \
     Farhelm replaces the file in one step, changing only the folder's trust key. A wrapper can start Claude \
     with a different file, in which case Claude may still ask.",
    "A Claude session exiting at the same instant can rarely undo Farhelm's edit or lose one of its \
     own updates. Any Claude update can change the file or its locking without notice: the edit may \
     stop working, so Claude asks again, or collide with Claude's writes. Farhelm cannot detect every \
     such change. This is best effort; Farhelm cannot promise it stays safe across Claude versions.",
    CLAUDE_TRUST_RISK,
];

/// An agent with a workspace trust switch: the GUI's per-agent trust vocabulary. Everything the section and the
/// session view's trust hint band do differently per agent, which agent a session's trust question belongs to
/// included, is an exhaustive method here, so a new trust agent needs a decision at each one (AGENTS.md, "Code rules";
/// the per-harness map in `crates/farhelm-supervisor/src/agent_kind/mod.rs` names this enum).
///
/// The discriminants are each agent's place in [`TrustAgent::ALL`], where the dialog keeps its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrustAgent {
    Claude = 0,
    Codex = 1,
    Muse = 2,
    Pi = 3,
    Antigravity = 4,
}

impl TrustAgent {
    /// Every trust agent, in the order the dialog stores their switches.
    pub(super) const ALL: [TrustAgent; 5] = [
        TrustAgent::Claude,
        TrustAgent::Codex,
        TrustAgent::Muse,
        TrustAgent::Pi,
        TrustAgent::Antigravity,
    ];

    /// Its place in [`TrustAgent::ALL`], where the dialog keeps its state.
    pub(super) fn index(self) -> usize {
        self as usize
    }

    /// A trust card is identified by the same agent mark as its session rows.
    pub(super) const fn harness(self) -> LaunchHarness {
        match self {
            Self::Claude => LaunchHarness::Claude,
            Self::Codex => LaunchHarness::Codex,
            Self::Muse => LaunchHarness::Muse,
            Self::Pi => LaunchHarness::Pi,
            Self::Antigravity => LaunchHarness::Antigravity,
        }
    }

    /// Which pages have a workspace trust switch. Settings has a page for every agent even if it has no trust control.
    #[warn(clippy::wildcard_enum_match_arm)]
    pub(super) const fn for_harness(harness: LaunchHarness) -> Option<Self> {
        match harness {
            LaunchHarness::Claude => Some(Self::Claude),
            LaunchHarness::Codex => Some(Self::Codex),
            LaunchHarness::Muse => Some(Self::Muse),
            LaunchHarness::Pi => Some(Self::Pi),
            LaunchHarness::Antigravity => Some(Self::Antigravity),
            LaunchHarness::Cursor
            | LaunchHarness::Goose
            | LaunchHarness::Omp
            | LaunchHarness::OpenCode
            | LaunchHarness::Grok
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => None,
        }
    }

    /// Describe what the page's trust switch approves. Pi approves project resources at launch.
    pub(super) const fn page_description(self) -> &'static str {
        match self {
            Self::Pi => "Approving project settings and extensions",
            Self::Claude | Self::Codex | Self::Muse | Self::Antigravity => {
                "Answering the agent's folder-trust questions"
            }
        }
    }

    /// The agent's name in the failure lines.
    fn name(self) -> &'static str {
        match self {
            TrustAgent::Claude => "Claude",
            TrustAgent::Codex => "Codex",
            TrustAgent::Muse => "Muse",
            TrustAgent::Pi => "Pi",
            TrustAgent::Antigravity => "Antigravity",
        }
    }

    /// The trust route: Claude's own, which the old app used, or the shared one with the agent's word. Both answer
    /// `{enabled, hint_dismissed}` and take `{enabled}`.
    fn path(self) -> Result<ApiPath, PathError> {
        match self {
            TrustAgent::Claude => ApiPath::api(&["checkout-claude-trust"]),
            TrustAgent::Codex => ApiPath::api(&["checkout-agent-trust", "codex"]),
            TrustAgent::Muse => ApiPath::api(&["checkout-agent-trust", "muse"]),
            TrustAgent::Pi => ApiPath::api(&["checkout-agent-trust", "pi"]),
            TrustAgent::Antigravity => ApiPath::api(&["checkout-agent-trust", "antigravity"]),
        }
    }

    /// The trust switch's element id, which tests and the test bridge find it by.
    pub(super) fn id(self) -> &'static str {
        match self {
            TrustAgent::Claude => "settings-claude-trust",
            TrustAgent::Codex => "settings-codex-trust",
            TrustAgent::Muse => "settings-muse-trust",
            TrustAgent::Pi => "settings-pi-trust",
            TrustAgent::Antigravity => "settings-antigravity-trust",
        }
    }

    /// The trust failure line's element id: the switch's, with `-error`.
    pub(super) fn error_id(self) -> &'static str {
        match self {
            TrustAgent::Claude => "settings-claude-trust-error",
            TrustAgent::Codex => "settings-codex-trust-error",
            TrustAgent::Muse => "settings-muse-trust-error",
            TrustAgent::Pi => "settings-pi-trust-error",
            TrustAgent::Antigravity => "settings-antigravity-trust-error",
        }
    }

    /// The switch's words, which name each agent's own mechanism rather than claiming every agent asks the same
    /// question (the old `choice_label`).
    pub(super) fn label(self) -> &'static str {
        match self {
            TrustAgent::Claude => "Answer Claude's \"trust this folder?\"",
            TrustAgent::Codex => "Answer Codex's folder-trust question",
            TrustAgent::Muse => "Answer Muse's workspace-trust question",
            TrustAgent::Pi => "Approve Pi project resources",
            TrustAgent::Antigravity => "Skip Antigravity's workspace trust question",
        }
    }

    /// The switch's hover help, the old row's `data-tooltip`: Claude's own, and for the others their help's first part.
    pub(super) fn tooltip(self) -> &'static str {
        match self {
            TrustAgent::Claude => {
                "Mark the folder trusted in Claude's own settings file before the agent starts. See the caveats below."
            }
            TrustAgent::Codex | TrustAgent::Muse | TrustAgent::Pi | TrustAgent::Antigravity => {
                self.mechanism()
            }
        }
    }

    /// Keep the warning with the focused switch. The longer paragraphs also remain readable on the page.
    fn switch_description(self) -> String {
        let risk = if self.has_caution() {
            CLAUDE_TRUST_RISK
        } else {
            FLAG_TRUST_RISK
        };
        [Some(self.tooltip()), Some(risk), self.caveat()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// What Farhelm does for eligible launches, the old `help` for the flag-based agents and the old help
    /// paragraph for Claude.
    fn mechanism(self) -> &'static str {
        match self {
            TrustAgent::Claude => {
                "When you choose Claude Code as the agent, Farhelm marks the folder trusted in Claude's settings file \
                 before it starts. This also applies when you start Claude Code in a new managed checkout. Farhelm \
                 skips your home directory and its parents, including /. Claude's trust covers everything inside \
                 those directories, so Claude may still ask there. On installed hosts, New, Clone, Restart with and \
                 Replace with use this setting. Sandbox and container sessions apply workspace trust automatically. \
                 Restart and Replace keep the session's saved launch."
            }
            TrustAgent::Codex => {
                "In folders you choose and in managed checkouts, Farhelm passes a trust option to Codex for that \
                 launch. Codex keeps its saved configuration."
            }
            TrustAgent::Muse => {
                "In folders you choose and in managed checkouts, Farhelm passes Muse's trust option for that launch. \
                 Muse keeps its saved configuration."
            }
            TrustAgent::Pi => {
                "In folders you choose and in managed checkouts, Farhelm tells Pi to approve project settings and \
                 extensions for that launch. Pi keeps its saved configuration."
            }
            TrustAgent::Antigravity => {
                "For plain folders and managed checkouts, Farhelm passes Antigravity's --settings option for that \
                 launch. Its saved configuration stays as it is."
            }
        }
    }

    /// The paragraphs under the switch: Claude's help alone (its risks are in the caution box), and for the others
    /// their mechanism with which launches use it, then the risk (SPEC.md: "Their help states that the repository's
    /// hooks, settings and commands may then run as if the user had approved them").
    pub(super) fn help(self) -> Vec<String> {
        match self {
            TrustAgent::Claude => vec![self.mechanism().to_owned()],
            TrustAgent::Codex | TrustAgent::Muse | TrustAgent::Pi | TrustAgent::Antigravity => {
                let mut paragraphs = vec![
                    format!(
                        "{} On installed hosts, New, Clone, Restart with and Replace with use this setting. Sandbox \
                         and container sessions apply workspace trust automatically. Restart and Replace keep the \
                         session's saved launch.",
                        self.mechanism()
                    ),
                    FLAG_TRUST_RISK.to_owned(),
                ];
                paragraphs.extend(self.caveat().map(str::to_owned));
                paragraphs
            }
        }
    }

    /// A last help paragraph for an agent whose option has a consequence the mechanism sentence does not cover:
    /// Antigravity's option replaces its saved trust list for the launch, and the user's own Custom `--settings`
    /// wins over it (SPEC.md: "Custom permission options supplying `--settings` or `--settings=...` take precedence").
    fn caveat(self) -> Option<&'static str> {
        match self {
            TrustAgent::Antigravity => Some(
                "For this launch, the option replaces Antigravity's saved list of trusted folders. Custom permission \
                 flags that include --settings or --settings=… take precedence.",
            ),
            TrustAgent::Claude | TrustAgent::Codex | TrustAgent::Muse | TrustAgent::Pi => None,
        }
    }

    /// Whether the block carries the caution box under its help: only Claude's, whose trust is an edit of Claude's own
    /// settings file, with the risks that brings (SPEC.md: "The caution explains ..."). The others pass a command-line
    /// option, whose risk their help states in one sentence.
    pub(super) fn has_caution(self) -> bool {
        match self {
            TrustAgent::Claude => true,
            TrustAgent::Codex | TrustAgent::Muse | TrustAgent::Pi | TrustAgent::Antigravity => {
                false
            }
        }
    }

    /// The switch that shows or hides the agent's discovery hint; Pi has no hint (SPEC.md: "Pi has a switch but no
    /// hint band").
    pub(super) fn hint(self) -> Option<Choice> {
        match self {
            TrustAgent::Claude => Some(Choice::ShowClaudeTrustHint),
            TrustAgent::Codex => Some(Choice::ShowCodexTrustHint),
            TrustAgent::Muse => Some(Choice::ShowMuseTrustHint),
            TrustAgent::Antigravity => Some(Choice::ShowAntigravityTrustHint),
            TrustAgent::Pi => None,
        }
    }

    /// The agent whose current question can be answered by a Farhelm trust setting, for the terminal hint.
    ///
    /// Claude's reader supplies its own flag. The band points at a setting that applies only to structured Claude
    /// launches, including launches into new managed checkouts.
    ///
    /// Codex, Muse and Antigravity share the neutral question flag. The structured launch names the setting they use.
    /// Pi's project approval has no recognized question.
    pub(crate) fn asking(info: &SessionInfo) -> Option<TrustAgent> {
        let agent = info
            .launch
            .agent_selection()
            .and_then(|selection| Self::for_harness(selection.harness));
        if info.claude_trust_prompt && agent == Some(Self::Claude) {
            return Some(TrustAgent::Claude);
        }
        if !info.workspace_trust_prompt {
            return None;
        }
        let agent = agent?;
        match agent {
            Self::Codex | Self::Muse | Self::Antigravity => Some(agent),
            // Claude uses its own flag above. Pi offers a switch without a hint.
            Self::Claude | Self::Pi => None,
        }
    }

    /// The trust hint band's words for the agent, `None` for an agent without a hint (Pi). Claude's are the old
    /// `ClaudeTrustBand`'s (`claude_trust.rs`), which point at the caveats of editing Claude's own file; Codex's and
    /// Muse's are the old `AgentTrustBand`'s (`workspace_trust.rs`), whose mechanism is a command-line flag, both at
    /// reference commit `24bc0580`.
    pub(crate) fn band_words(self) -> Option<BandWords> {
        // The flag-based agents' explanation after their lead.
        const FLAG: &str = "Farhelm can answer it through a command-line flag. It's off by \
                            default because repository hooks, settings and commands are then trusted.";
        match self {
            TrustAgent::Claude => Some(BandWords {
                lead: "Claude asks this for every new folder. ",
                rest: "Farhelm can answer it for you, but it's off by default: there's no supported way to do it, \
                       so Farhelm would have to edit Claude's own settings file, which comes with caveats.",
                settings: "Settings and caveats",
                settings_help: "Open Claude's folder-trust setting and its caveats",
                dismiss_help: "Stop showing this note for every session; Settings can turn it back on",
            }),
            TrustAgent::Codex => Some(BandWords {
                lead: "Codex asks about trust for this folder. ",
                rest: FLAG,
                settings: "Settings",
                settings_help: "Open Settings at the Codex folder-trust switch",
                dismiss_help: "Stop showing this Codex note for every session; Settings can turn it back on",
            }),
            TrustAgent::Muse => Some(BandWords {
                lead: "Muse asks about trust for this folder. ",
                rest: FLAG,
                settings: "Settings",
                settings_help: "Open Settings at the Muse workspace-trust switch",
                dismiss_help: "Stop showing this Muse note for every session; Settings can turn it back on",
            }),
            TrustAgent::Antigravity => Some(BandWords {
                lead: "Antigravity asks whether to trust this folder. ",
                rest: "Farhelm can skip this question for plain folders and managed checkouts. It's off by default because repository hooks, settings and commands are then trusted.",
                settings: "Settings",
                settings_help: "Open Settings at the Antigravity workspace trust switch",
                dismiss_help: "Stop showing this Antigravity note for every session. Settings can turn it back on.",
            }),
            TrustAgent::Pi => None,
        }
    }

    /// `PUT` of the trust route with `{enabled}`.
    pub(super) fn command(self, enabled: bool) -> Result<Command, PathError> {
        configured::enabled_command(self.path(), enabled)
    }

    /// Read the trust route; `on_read` gets whether the agent's trust is on, or why the helm did not say
    /// ([`configured::read_enabled`]). The caller is the dialog's own update, or the trust hint band's
    /// (`crate::session_pane::trust_band`), and `on_read` updates that view, which [`super::choices::read_route`]'s
    /// contract makes safe.
    pub(crate) fn read(
        self,
        cx: &mut App,
        on_read: impl FnOnce(Result<bool, HostText>, &mut App) + 'static,
    ) {
        configured::read_enabled(self.path(), cx, on_read);
    }
}

/// What the trust hint band says for one agent ([`TrustAgent::band_words`]): the explanation's bold lead and the rest
/// of it, the Settings button's label and hover help, and Don't show again's hover help. The ×, its help and the
/// failure line say the same for every agent, so they stay with the band.
pub(crate) struct BandWords {
    pub(crate) lead: &'static str,
    pub(crate) rest: &'static str,
    pub(crate) settings: &'static str,
    pub(crate) settings_help: &'static str,
    pub(crate) dismiss_help: &'static str,
}

/// One agent's trust switch in the open dialog.
pub(super) struct Trust {
    pub(super) agent: TrustAgent,
    pub(super) state: Configured,
    /// The switch's focus handle, which the dialog keeps so the trust hint's entry point can put focus on it.
    pub(super) focus: FocusHandle,
    /// Loading and navigating back to the page can overlap. Remember that its read is already in flight.
    pub(super) read_started: bool,
}

/// One agent's block: its trust switch, its help (and Claude's caution box), its hint switch, and the failure lines,
/// with the trust switch's line saying why a press sent nothing (`refused`, [`choices::busy_line`]) last. `on_toggle`
/// is what a toggle of the trust switch does; `hint` draws the hint switch through the dialog.
pub(super) fn block(
    trust: &Trust,
    refused: &Refused,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    hint: impl FnOnce(Choice) -> AnyElement,
    cx: &App,
) -> Div {
    let agent = trust.agent;
    let contents = div()
        .flex()
        .items_start()
        .gap(px(8.))
        .child(card::agent_mark(agent.harness(), cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(card::switch_copy(agent.label(), "", cx))
                .children(
                    agent
                        .help()
                        .into_iter()
                        .enumerate()
                        .map(|(index, text)| card::help(("trust-help", index), text, cx)),
                ),
        );
    let switch = Checkbox::new(
        agent.id(),
        agent.label(),
        HoverHelp::tooltip(agent.tooltip()),
        Role::Switch,
    )
    .switch()
    .content(contents)
    .accessible_description(agent.switch_description())
    .checked(trust.state.shown())
    .value_unknown(!trust.state.available())
    .focus(&trust.focus)
    .on_toggle(on_toggle);
    let failure = configured::failure(
        agent.error_id(),
        format!("Could not save or read {} trust settings: ", agent.name()),
        &trust.state,
        cx,
    );
    let trust_row = div()
        .flex()
        .flex_col()
        .gap(px(BLOCK_GAP))
        .min_w_0()
        .child(switch)
        .children(failure)
        .children(choices::busy_line(refused, agent.id(), cx))
        .children(agent.has_caution().then(|| caution(cx)));
    card::rows(
        std::iter::once(trust_row.into_any_element()).chain(agent.hint().map(hint)),
        cx,
    )
}

/// Claude's caution box (`.claude-trust-caution`): why the switch is off by default, in the amber of a warning, with
/// the 3-pixel amber rule down its left edge.
fn caution(cx: &App) -> impl IntoElement {
    let palette = theme::palette(cx);
    let paragraph = |(index, text)| {
        card::help(("settings-claude-trust-caution", index), text, cx)
            .line_height(px(CAUTION_LINE_HEIGHT))
            .text_color(theme::color(palette.warn_bright))
    };
    div()
        .id(CAUTION_ID)
        .flex()
        .my(px(4.))
        .border_t_1()
        .border_r_1()
        .border_b_1()
        .border_color(theme::color(palette.warn_border))
        .bg(theme::color(palette.warn_bg_dim))
        .child(div().w(px(3.)).flex_none().bg(theme::color(palette.warn)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .p(px(CAUTION_PAD))
                .flex()
                .flex_col()
                .gap(px(CAUTION_GAP))
                .text_size(metrics::TEXT_SM)
                .line_height(px(CAUTION_LINE_HEIGHT))
                .text_color(theme::color(palette.warn_bright))
                .child(div().font_weight(FontWeight::BOLD).child(CAUTION_TITLE))
                .children(CAUTION.into_iter().enumerate().map(paragraph)),
        )
}
