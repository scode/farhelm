//! A slow launch remains a pending request, not a failed or cancellable job.
//!
//! Callers own the request lifetime and capture its original host and title.
//! This component owns only a local display clock; it never polls the helm,
//! retries a request, or infers progress from the page-wide operation lock.

use dioxus::prelude::*;
use web_time::Instant;

use crate::peer::{DetailPart, PeerLine};

/// The operation being awaited, independent of its retry guarantees.
/// Restart has no retry key; a timeout must never promise a harmless retry.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaunchAction {
    Launch,
    Restart,
    Replace,
}

impl LaunchAction {
    /// Keep the same operation wording across the footer, pill and buttons.
    fn present(self) -> &'static str {
        match self {
            Self::Launch => "Launching",
            Self::Restart => "Restarting",
            Self::Replace => "Replacing",
        }
    }
}

/// The immutable presentation snapshot of one admitted launch API call.
/// Its timestamp starts after launcher preparation and key minting; the
/// transport can still wait for earlier configuration saves. Another
/// operation's busy state cannot look like this call's progress.
#[derive(Clone, PartialEq)]
pub(crate) struct PendingLaunch {
    /// Existing provisioning progress can name a preparation phase. It never
    /// changes this request's lifetime or its uncertain-outcome semantics.
    pub(crate) stage: Option<String>,
    /// Preparation's first-use/24-hour explanation remains visible while
    /// its ordinary host step runs, including the floating quick-launch pill.
    pub(crate) stage_detail: Option<String>,
    /// Provisioning publishes no phase timestamps. This local display clock
    /// starts at the first observation of a changed phase name.
    stage_started: Option<Instant>,
    pub(crate) started: Instant,
    pub(crate) action: LaunchAction,
    pub(crate) host: String,
    pub(crate) title: Option<String>,
}

impl PendingLaunch {
    /// Capture the values the request actually names before awaiting its reply.
    /// Later form edits or host refreshes must not relabel an in-flight request.
    pub(crate) fn new(action: LaunchAction, host: String, title: Option<String>) -> Self {
        Self {
            stage: None,
            stage_detail: None,
            stage_started: None,
            started: Instant::now(),
            action,
            host,
            title: title.filter(|title| !title.is_empty()),
        }
    }

    /// Update advisory host progress without restarting its clock on each poll.
    /// The request's total clock still controls the initial visibility delay.
    pub(crate) fn observe_stage(&mut self, name: String, detail: Option<String>) {
        if self.stage.as_ref() != Some(&name) {
            self.stage_started = Some(Instant::now());
        }
        self.stage = Some(name);
        self.stage_detail = detail;
    }

    /// Explain a deadline without claiming the helm refused the operation.
    /// `retry_safe` is supplied by the caller's retained retry identity, not
    /// inferred from the timeout. Restart never makes that promise.
    pub(crate) fn unknown_outcome(&self, retry_safe: bool) -> String {
        let outcome = match self.action {
            LaunchAction::Restart => {
                "The restart may still finish. Check the session before restarting again."
            }
            LaunchAction::Launch if retry_safe => {
                "The session may still start; it will appear in the sidebar if it does. Launching again with the same settings will not start a second copy."
            }
            LaunchAction::Launch => {
                "The session may still start; it will appear in the sidebar if it does. Check the sidebar before launching again."
            }
            LaunchAction::Replace if retry_safe => {
                "The replacement may still finish. Check the sidebar for the new session and whether the old one remains. Retrying the same replacement without cancelling it, changing its settings or starting another Replace will not start a second copy."
            }
            LaunchAction::Replace => {
                "The replacement may still finish. Check the sidebar for the new session and whether the old one remains before replacing again."
            }
        };
        format!("Still waiting on {}. {outcome}", self.host)
    }

    /// Keep relayed names isolated from the UI's own operation and punctuation.
    /// The same immutable words serve the visible line and its one announcement.
    fn label_parts(&self) -> Vec<DetailPart> {
        if let Some(stage) = &self.stage {
            return vec![DetailPart::peer(stage.clone())];
        }
        let mut parts = vec![DetailPart::text(self.action.present())];
        if let Some(title) = &self.title {
            parts.extend([
                DetailPart::text(" “"),
                DetailPart::peer(title.clone()),
                DetailPart::text("”"),
            ]);
        }
        parts.extend([
            DetailPart::text(" on "),
            DetailPart::peer(self.host.clone()),
            DetailPart::text("…"),
        ]);
        parts
    }
}

/// Show a request only after two seconds, with one polite announcement.
/// The ticking line is hidden from assistive technology so every elapsed
/// second does not interrupt the user. Unmounting cancels this display clock;
/// the caller still owns the request and clears the snapshot on every reply.
/// When two surfaces show the same request, only the selected session's status
/// announces it; its sidebar copy passes `announce: false`.
#[component]
pub(crate) fn LaunchProgress(
    pending: PendingLaunch,
    #[props(default = true)] announce: bool,
) -> Element {
    let mut now = use_signal(Instant::now);
    use_future(move || async move {
        loop {
            crate::reader::sleep_ms(1_000).await;
            now.set(Instant::now());
        }
    });
    let now = now();
    if now.saturating_duration_since(pending.started).as_secs() < 2 {
        return rsx! {};
    }
    let elapsed = now
        .saturating_duration_since(pending.stage_started.unwrap_or(pending.started))
        .as_secs();
    let label = pending.label_parts();
    rsx! {
        div { class: "launch-progress delete-progress",
            if announce {
                div { class: "visually-hidden", role: "status",
                    PeerLine { class: "launch-progress-announcement".to_string(), parts: label.clone() }
                }
            }
            div { class: "launch-progress-visible", aria_hidden: "true",
                span { class: "delete-spinner" }
                PeerLine { class: "launch-progress-label".to_string(), parts: label }
                if let Some(detail) = &pending.stage_detail {
                    PeerLine { class: "launch-progress-detail".to_string(), parts: vec![DetailPart::peer(detail.clone())] }
                }
                span { class: "launch-progress-elapsed", " {elapsed} s" }
            }
        }
    }
}
