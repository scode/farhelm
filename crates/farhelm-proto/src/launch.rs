//! Explicit, user-selected launch intent shared by the helm and supervisor.
//!
//! This module intentionally records choices, not inferred runtime facts. A
//! missing model, effort, permission, or workspace-trust choice means that the
//! selected harness receives no corresponding argument and decides its own
//! default. Even OpenCode may use its configured default; an explicit
//! OpenCode model still has to use its supported Zen provider. The supervisor
//! later stores the same value beside the resolved argv so clone
//! and history can describe what the user chose without attempting to parse a
//! command line back into a structured launch.

use serde::{Deserialize, Serialize};

/// The command-line harness selected for a structured launch.
///
/// This is distinct from [`crate::AgentKind`]. `AgentKind` describes the
/// supervisor features available after a process starts; this enum preserves
/// the harness the user selected. In particular, Muse is a first-class launch
/// choice even though it currently maps to the supervisor's generic runtime
/// integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}

impl LaunchHarness {
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
            LaunchHarness::Muse | LaunchHarness::Cursor | LaunchHarness::OpenCode => {
                AgentKind::Generic
            }
            LaunchHarness::Goose => AgentKind::Goose,
            LaunchHarness::Pi => AgentKind::Pi,
            LaunchHarness::Omp => AgentKind::Omp,
            LaunchHarness::Grok => AgentKind::Grok,
        }
    }

    /// Whether this harness accepts a per-run workspace-trust choice
    /// ([`LaunchSelection::workspace_trust`]). The helm refuses the choice
    /// for any other harness and remembers it only for these, and the browser
    /// offers it only for these.
    pub const fn offers_workspace_trust(self) -> bool {
        matches!(
            self,
            LaunchHarness::Codex | LaunchHarness::Muse | LaunchHarness::Pi
        )
    }

    /// Whether this harness offers `permission` as an explicit choice
    /// ([`LaunchSelection::permissions`]).
    ///
    /// The one statement of the per-harness permission sets: the helm refuses
    /// any other explicit permission, and the browser only offers, keeps, and
    /// considers compatible the ones this allows. Before it existed each of
    /// those kept its own table, and adding OMP's Approve had to touch every
    /// one. Exhaustive over harnesses so a new one has to decide here rather
    /// than inheriting a catch-all.
    ///
    /// The harness default (`None`) is not a question for this function: every
    /// harness accepts it. Pi's rule that an omitted permission MEANS YOLO is
    /// also separate (the helm rewrites it before validating, and the browser
    /// displays it that way); here Pi simply offers YOLO and nothing else.
    pub const fn offers_permission(self, permission: LaunchPermission) -> bool {
        match self {
            LaunchHarness::Goose => true,
            LaunchHarness::Omp => {
                matches!(
                    permission,
                    LaunchPermission::Yolo | LaunchPermission::Approve
                )
            }
            LaunchHarness::Codex
            | LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Cursor
            | LaunchHarness::Pi
            | LaunchHarness::OpenCode
            | LaunchHarness::Grok => matches!(permission, LaunchPermission::Yolo),
        }
    }
}

/// An explicit reasoning-effort value requested from a structured harness.
///
/// The release-owned catalog decides which values each harness and known model
/// support. Keeping the wire vocabulary closed means a misspelled effort is
/// rejected at the HTTP boundary instead of becoming an unreviewed CLI flag.
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

/// A permission choice that adds a deliberate harness flag.
///
/// `None` on [`LaunchSelection::permissions`] is the harness default. There
/// is no "safe" or effective-permission value here because this snapshot must
/// not claim to know what a changing vendor configuration will do at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPermission {
    Yolo,
    Approve,
    SmartApprove,
    Chat,
}

impl LaunchPermission {
    /// Every permission, for places that name the whole vocabulary to a
    /// person, like the helm's refusal of an
    /// unknown remembered-permissions word, so the list they print grows with
    /// the enum instead of being retyped.
    pub const ALL: [LaunchPermission; 4] = [
        LaunchPermission::Yolo,
        LaunchPermission::Approve,
        LaunchPermission::SmartApprove,
        LaunchPermission::Chat,
    ];

    /// The wire spelling (the serde `snake_case` name), for places that store
    /// or compare the choice as text, like the helm's remembered-permissions
    /// preference.
    pub fn wire_word(self) -> &'static str {
        match self {
            LaunchPermission::Yolo => "yolo",
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
    pub model: Option<String>,
    pub effort: Option<LaunchEffort>,
    pub permissions: Option<LaunchPermission>,
    /// An explicit per-run choice about project-local settings and extensions.
    ///
    /// This is separate from tool approval and from Farhelm's hook-trust
    /// bypass. Older saved selections lack the field and retain the vendor's
    /// ordinary startup behavior. An unset value stays absent in serialized
    /// JSON because that encoding also serves as a durable launch fingerprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_trust: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection};

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
                LaunchPermission::Yolo
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

    /// Spec: Goose offers every explicit permission, OMP offers YOLO and
    /// Approve, and every other harness offers only YOLO (SPEC.md's
    /// structured-launch permission rules).
    ///
    /// Why: this is the table both the helm's validation and the browser's
    /// buttons and normalization read, so a slip here changes what users can
    /// pick and what the helm accepts at once. The matrix is spelled out
    /// rather than derived so the test states the rule independently, and an
    /// exhaustive match makes a new harness a compile error here until it has
    /// a row.
    #[test]
    fn each_harness_offers_exactly_its_permissions() {
        use LaunchPermission::{Approve, Chat, SmartApprove, Yolo};
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
            | LaunchHarness::Grok => {}
        }
        let all = [Yolo, Approve, SmartApprove, Chat];
        for (harness, offered) in [
            (
                LaunchHarness::Goose,
                &[Yolo, Approve, SmartApprove, Chat][..],
            ),
            (LaunchHarness::Omp, &[Yolo, Approve][..]),
            (LaunchHarness::Codex, &[Yolo][..]),
            (LaunchHarness::Claude, &[Yolo][..]),
            (LaunchHarness::Muse, &[Yolo][..]),
            (LaunchHarness::Cursor, &[Yolo][..]),
            (LaunchHarness::Pi, &[Yolo][..]),
            (LaunchHarness::OpenCode, &[Yolo][..]),
            (LaunchHarness::Grok, &[Yolo][..]),
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
            harness: LaunchHarness::OpenCode,
            model: None,
            effort: None,
            permissions: None,
            workspace_trust: None,
        };

        let json = serde_json::to_value(&selection).expect("serialize launch selection");
        assert_eq!(json["harness"], "open_code");
        assert!(json["model"].is_null());
        assert!(json["effort"].is_null());
        assert!(json["permissions"].is_null());
        assert!(json.get("workspace_trust").is_none());
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
    }

    /// Explicit consent and refusal must remain distinct in saved intent and
    /// keyed-create fingerprints even though an unset choice is omitted.
    #[test]
    fn explicit_workspace_trust_values_serialize() {
        let mut selection = LaunchSelection {
            harness: LaunchHarness::Muse,
            model: None,
            effort: None,
            permissions: None,
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
