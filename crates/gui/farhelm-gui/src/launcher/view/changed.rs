//! Restart with's "● Changed" markers: a line under each setting the user has edited away from the session's stored
//! launch, so the launcher "shows the current settings and marks edited fields" (SPEC.md, "Restart with").
//!
//! Ported from the old launcher's `changed_marker` (`launch_controls.rs`) and the command tab's markers
//! (`list/create_form.rs`) at reference commit `24bc0580`, with its `.launch-composer-changed-marker` look: 12 pixels in
//! `--info`, under the control. An agent choice's marker says what it was ("● Changed · was high"), in the old app's
//! lowercase value words, the model id as the host wrote it; the command tab's say only "● Changed", as the old ones
//! did, since the old command line can be long.
//!
//! A marker compares the draft with the baseline Restart with opened on, the launch the request edits; outside Restart
//! with there is nothing to compare with and no marker is drawn.

use gpui::{AnyElement, IntoElement, ParentElement as _, Styled as _, div};

use super::Launcher;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::{Mode, RestartBaseline};
use crate::permission::words::{effort_word, permission_display};
use crate::theme::{self, metrics};
use farhelm_proto::LaunchPermission;

/// A setting Restart with marks when it differs from the stored launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Changed {
    Model,
    Effort,
    Permissions,
    Command,
    Yolo,
    Resume,
}

impl Changed {
    /// The marker's element id: `launcher-changed-<field>`.
    fn id(self) -> &'static str {
        match self {
            Changed::Model => "launcher-changed-model",
            Changed::Effort => "launcher-changed-effort",
            Changed::Permissions => "launcher-changed-permissions",
            Changed::Command => "launcher-changed-command",
            Changed::Yolo => "launcher-changed-yolo",
            Changed::Resume => "launcher-changed-resume",
        }
    }
}

impl Launcher {
    /// The marker under `field`, when Restart with's draft differs there from the stored launch; `None` otherwise.
    pub(super) fn changed_marker(
        &self,
        field: Changed,
        palette: &theme::Palette,
    ) -> Option<AnyElement> {
        let Mode::RestartWith(restart) = &self.draft.mode else {
            return None;
        };
        let words = HostSentence::new(field.id()).text("● Changed");
        let words = match (&restart.baseline, field) {
            (RestartBaseline::Agent(stored), Changed::Model) => {
                (self.draft.agent.model != stored.model).then(|| {
                    let words = words.text(" · was ");
                    match &stored.model {
                        Some(model) => words.host(&HostText::from_host(model.as_str())),
                        None => words.text("default"),
                    }
                })?
            }
            (RestartBaseline::Agent(stored), Changed::Effort) => (self.draft.agent.effort
                != stored.effort)
                .then(|| words.text(format!(" · was {}", effort_word(stored.effort))))?,
            (RestartBaseline::Agent(stored), Changed::Permissions) => {
                (self.draft.agent.permissions != stored.permissions).then(|| {
                    words.text(format!(" · was {}", permission_word(stored.permissions)))
                })?
            }
            (RestartBaseline::Command(stored), Changed::Command) => {
                (self.draft.command.command != stored.command).then_some(words)?
            }
            (RestartBaseline::Command(stored), Changed::Yolo) => {
                (self.draft.command.yolo != Some(stored.yolo)).then_some(words)?
            }
            (RestartBaseline::Command(stored), Changed::Resume) => {
                let resume = self
                    .draft
                    .command
                    .resume_on
                    .then_some(self.draft.command.resume.as_str());
                (resume != stored.resume.as_deref()).then_some(words)?
            }
            _ => return None,
        };
        Some(
            // The sentence is the one labelled node, under the field's id; this box only places and colors it.
            div()
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.info))
                .child(words)
                .into_any_element(),
        )
    }
}

/// The old marker's permission word: YOLO in capitals, the other modes in their lowercase phrase
/// (`permission_value`), and "default" for an unset choice.
fn permission_word(permissions: Option<LaunchPermission>) -> String {
    match permissions {
        Some(LaunchPermission::Yolo) => "YOLO".to_owned(),
        Some(_) => permission_display(permissions).to_lowercase(),
        None => "default".to_owned(),
    }
}
