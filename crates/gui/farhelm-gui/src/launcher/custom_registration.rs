//! The launcher's view of one saved custom agent choice. Only a successful list can establish deletion.

use farhelm_helm_client::collections::Collections;
use farhelm_proto::custom_agents::CustomAgent;
use farhelm_proto::ui_channel::{Availability, Collection};

use crate::host_text::HostText;

/// An absent registration cannot supply a launch yet. A retained entry still supplies its real capabilities while
/// a refresh fails. Reporting and resumption are independent capabilities of an entry that can be used.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CustomRegistration {
    Present {
        supports_resumption: bool,
        supports_reporting: bool,
    },
    #[default]
    Loading,
    Unavailable(HostText),
    Deleted,
}

impl CustomRegistration {
    pub(super) fn of(agent: &CustomAgent) -> Self {
        Self::Present {
            supports_resumption: agent.supports_resumption,
            supports_reporting: agent.supports_reporting,
        }
    }

    /// Consult the retained entries before the result of the latest read. The collection already owns retention.
    pub(super) fn read(collections: &Collections, id: &str) -> Self {
        if let Some(agent) = collections.get::<CustomAgent>(id) {
            return Self::of(agent);
        }
        match collections
            .meta(Collection::CustomAgents)
            .map(|meta| &meta.availability)
        {
            None => Self::Loading,
            Some(Availability::Unavailable { error }) => {
                Self::Unavailable(HostText::from_host(error.as_str()))
            }
            Some(Availability::Available) => Self::Deleted,
        }
    }

    /// A read in progress or a failed read leaves the entry's capabilities unknown.
    pub(super) fn is_unread(&self) -> bool {
        matches!(self, Self::Loading | Self::Unavailable(_))
    }

    /// Common launch wording. Deletion has different recovery actions on the Agent and Command tabs.
    pub(super) fn refusal(&self, deleted: &'static str) -> Option<&'static str> {
        match self {
            Self::Present { .. } => None,
            Self::Loading => Some("Loading custom agent registrations"),
            Self::Unavailable(_) => Some("Could not read custom agent registrations"),
            Self::Deleted => Some(deleted),
        }
    }

    /// Restart with recompiles its launch using the current entry. The saved resume command permits opening the
    /// editor while the current entry is unknown, but launching still waits for a usable registration.
    pub(super) fn restart_refusal(&self) -> Option<&'static str> {
        match self {
            Self::Present {
                supports_resumption: false,
                ..
            } => Some(
                "This custom agent no longer supports resumption. Enable Supports resumption in Settings to restart it.",
            ),
            _ => self.refusal(
                "This custom agent is no longer registered. Register it again to restart it.",
            ),
        }
    }
}
