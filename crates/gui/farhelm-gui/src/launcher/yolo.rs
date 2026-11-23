//! The YOLO question's words: why a refused launch counts as YOLO, and whether it may offer to start guarded instead.
//!
//! SPEC.md, "Creation", the YOLO paragraph: a YOLO launch on a host that asks first is refused by the helm, and the GUI
//! answers with a confirmation that names the host and explains YOLO using
//! the launch's permission choices or the command's assertion. The
//! GUI supplies the explanation because the helm's refusal serves CLI users.
//!
//! The question itself (its answers, what each does) is the launcher view's (`view::yolo_question`), built on the
//! prompt foundation's inline question; nothing here touches GPUI.

use farhelm_proto::{LaunchHarness, LaunchPermission, LaunchSelection};

use super::draft::harness_label;

/// Why the launch being confirmed counts as YOLO, as far as the launcher can tell from what it sent. The helm has
/// already decided the launch needs the gate. This picks the explanation the
/// GUI can support from the recorded launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum YoloReason {
    /// Custom permission flags: Farhelm never infers safety from words the user supplied.
    CustomFlags,
    /// YOLO chosen on an agent type that offers other modes.
    ChosenPermission,
    /// An agent type whose offered permissions all count as YOLO.
    OnlyMode(LaunchHarness),
    /// A command launch asserted to be YOLO; Farhelm never reads the command to check.
    Asserted,
}

impl YoloReason {
    /// The reason for a launch whose agent choices are `selection`, or `None` for a command launch, which reaches the
    /// question only when it was asserted to be YOLO. Read from the agent type's capabilities in `farhelm_proto`, not
    /// from a list of agent types here, so a new single-mode agent type gets the right sentence by itself.
    pub(crate) fn of_launch(selection: Option<&LaunchSelection>) -> Self {
        match selection {
            None => YoloReason::Asserted,
            Some(selection) if selection.permissions == Some(LaunchPermission::Custom) => {
                YoloReason::CustomFlags
            }
            Some(selection) if !selection.harness.offers_non_yolo_mode() => {
                YoloReason::OnlyMode(selection.harness)
            }
            Some(_) => YoloReason::ChosenPermission,
        }
    }

    /// Whether the question may offer starting the same launch guarded instead: only for an explicit YOLO choice on an
    /// agent type that offers guarded mode (SPEC.md: "an explicit YOLO choice on an agent that offers guarded mode also
    /// offers starting the same launch guarded instead"). This is for the
    /// composer, where the permission is an explicit choice in the draft.
    pub(crate) fn offers_guarded(self, selection: Option<&LaunchSelection>) -> bool {
        self == YoloReason::ChosenPermission
            && selection.is_some_and(|selection| {
                selection.permissions == Some(LaunchPermission::Yolo)
                    && selection
                        .harness
                        .offers_permission(LaunchPermission::Guarded)
            })
    }

    /// The confirmation's explanation of why this launch counts as YOLO.
    pub(crate) fn sentence(self) -> String {
        match self {
            YoloReason::CustomFlags => {
                "This launch uses custom permission flags. Farhelm does not check what they do, \
                                        so it treats the session as YOLO."
                    .to_owned()
            }
            YoloReason::ChosenPermission => {
                "Farhelm treats this launch as YOLO. A guarded or approval mode is available for a new launch."
                    .to_owned()
            }
            YoloReason::OnlyMode(harness) => {
                let name = harness_label(harness);
                format!(
                    "Farhelm treats every {name} permission choice as YOLO."
                )
            }
            YoloReason::Asserted => {
                "This command was asserted to bypass its safety checks.".to_owned()
            }
        }
    }
}
