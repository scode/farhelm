//! A switch over helm configuration: a choice of the helm's behind a route of its own that answers and takes
//! `{enabled}`, read once per opening and saved at once. Agent trust's switches ([`super::agent_trust`]) and Checkouts'
//! spare switch ([`super::checkouts`]) are this kind of control; what they share lives here.
//!
//! The section reads the switch's route when it is first shown in an opening, and the switch is unavailable rather
//! than shown off until that read answers (SPEC.md, "Failed settings reads never count as an off switch"). A flip saves
//! at once through the operation store; while it is in flight the switch shows the value being saved, a done save
//! becomes the value the section holds ([`Configured::saved`]), and a refusal shows the value read before with the
//! helm's reason under the switch ([`failure`]). Nothing is retried or read back.

use farhelm_helm_client::{ApiPath, Method, PathError};
use gpui::{
    App, Div, InteractiveElement as _, ParentElement as _, Role, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::choices::{self, HELP_LINE_HEIGHT};
use crate::drafts::Draft;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, Work};
use crate::theme::{self, metrics};

/// `PUT` of `path` with `{enabled}`, the save of a [`Configured`] switch. A path that does not build is the caller's
/// failed save, as for any command.
pub(super) fn enabled_command(
    path: Result<ApiPath, PathError>,
    enabled: bool,
) -> Result<Command, PathError> {
    let path = path?;
    let shown = format!("PUT {}", path.as_str());
    Ok(Command::new(
        Work::Save,
        Method::Put,
        path,
        Some(serde_json::json!({ "enabled": enabled })),
        shown,
    ))
}

/// Read a [`Configured`] switch's route; `on_read` gets whether it is on, or why the helm did not say, under
/// [`choices::read_route`]'s contract (once, on its own, after the calling update has returned). The trust routes'
/// answers also carry the hint's dismissal, which Agent trust ignores (see [`super::agent_trust`]'s module docs).
pub(super) fn read_enabled(
    path: Result<ApiPath, PathError>,
    cx: &mut App,
    on_read: impl FnOnce(Result<bool, HostText>, &mut App) + 'static,
) {
    let on_read = |reply: Result<EnabledReply, HostText>, cx: &mut App| {
        on_read(reply.map(|reply| reply.enabled), cx)
    };
    match path {
        Ok(path) => {
            let shown = format!("GET {}", path.as_str());
            choices::read_route(Ok(path), shown, cx, on_read);
        }
        // `read_route` reports the failed path; the words it would have sent are never used.
        Err(error) => choices::read_route(Err(error), "", cx, on_read),
    }
}

/// The part of a switch route's answer the section uses.
#[derive(serde::Deserialize)]
struct EnabledReply {
    enabled: bool,
}

/// What the section knows of a configuration switch's choice (an agent's trust, the spare switch).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Known {
    /// The read has not answered.
    Reading,
    /// The helm's choice: read when the section opened, or saved since.
    Read(bool),
    /// The read failed, with the helm's reason.
    Unreadable(HostText),
}

/// A switch over helm configuration in the open dialog: what the section knows of the choice, and its save.
pub(super) struct Configured {
    pub(super) known: Known,
    /// The value being saved and the last refusal; its loaded value never reaches the screen.
    pub(super) draft: Draft<bool>,
}

impl Configured {
    /// A switch whose read has not answered.
    pub(super) fn new() -> Self {
        Configured {
            known: Known::Reading,
            draft: Draft::load(false),
        }
    }

    /// The read of the switch's route answered.
    pub(super) fn read_answered(&mut self, answer: Result<bool, HostText>) {
        self.known = match answer {
            Ok(enabled) => Known::Read(enabled),
            Err(reason) => Known::Unreadable(reason),
        };
    }

    /// The helm has done a save of `on`: that is now the choice the section holds.
    pub(super) fn saved(&mut self, on: bool) {
        self.known = Known::Read(on);
    }

    /// The value the switch shows: the one being saved while its save is in flight, the known choice otherwise, and
    /// off (with the switch unavailable) while nothing is known.
    pub(super) fn shown(&self) -> bool {
        if self.draft.is_saving() {
            *self.draft.value()
        } else {
            self.known == Known::Read(true)
        }
    }

    /// Whether the switch takes a toggle: only once the choice is known.
    pub(super) fn available(&self) -> bool {
        matches!(self.known, Known::Read(_))
    }
}

/// The failure line under a [`Configured`] switch, drawn as an alert under `id`: `prefix` and the helm's words, after a
/// refused save or, failing that, a failed read. The old lines said "Could not save or read ..." whichever of the two
/// failed; a refused save is the newer news, so it wins. `None` when neither failed.
pub(super) fn failure(
    id: &'static str,
    prefix: String,
    state: &Configured,
    cx: &App,
) -> Option<Stateful<Div>> {
    let reason = state.draft.refusal().or(match &state.known {
        Known::Unreadable(reason) => Some(reason),
        Known::Reading | Known::Read(_) => None,
    })?;
    let palette = theme::palette(cx);
    let sentence = HostSentence::new(id).text(prefix).host(reason);
    Some(
        div()
            .id(id)
            .debug_selector(|| id.to_owned())
            .role(Role::Alert)
            .aria_label(sentence.accessible_name())
            .text_size(metrics::TEXT_SM)
            .line_height(px(HELP_LINE_HEIGHT))
            .text_color(theme::color(palette.danger))
            .child(sentence),
    )
}
