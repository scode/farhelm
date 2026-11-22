//! Follow an existing host through Resume and a fresh connection before the launcher submits its create.
//!
//! Resume and the connection arrive through different parts of the UI channel. The reply governs whether this
//! launch can carry on. The pushed host row proves whether the destination can receive the create.

use std::time::{Duration, Instant};

use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::HostStateView;

pub(super) const CONNECTION_BUDGET: Duration = Duration::from_secs(60);
pub(super) const BUSY_RETRY: Duration = Duration::from_secs(1);

/// Which evidence the wake still needs. Ready retains the destination hold through the hidden launcher’s
/// folder-default or preview wait.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Asking,
    Busy,
    Resuming,
    Ready,
}

/// The observation that matters to the wake. A provider hold by itself is compatible with the original suspension
/// being published again.
pub(super) enum Observation {
    Connected,
    Changing { resuming: bool },
    Waiting,
    SuspendedAgain,
    Failed,
    Unavailable,
}

/// Keep the starting suspension and provider diagnosis separately from the fact that the host has moved on.
/// A connector can publish Connecting and advance the timestamps before surfacing a new provider failure.
struct Watch {
    start: HostStateView,
    connection_seen: bool,
}

impl Watch {
    fn new(state: &HostStateView) -> Self {
        Self {
            start: state.clone(),
            connection_seen: false,
        }
    }

    /// Prefer a newly reported diagnosis to a generic repeat suspension, even after connection activity.
    fn observe(&mut self, state: &HostStateView) -> Observation {
        match state {
            HostStateView::Connected { .. } => {
                self.connection_seen = true;
                Observation::Connected
            }
            HostStateView::TensorlakeChanging { direction, .. } => Observation::Changing {
                resuming: *direction == LifecycleDirection::Resuming,
            },
            HostStateView::Suspended {
                observed_at,
                suspended_by_farhelm_at,
                resume_failed,
            } => match &self.start {
                HostStateView::Suspended {
                    resume_failed: initial_failure,
                    ..
                } if resume_failed.is_some() && resume_failed != initial_failure => {
                    Observation::Failed
                }
                HostStateView::Suspended {
                    observed_at: initial_observed,
                    suspended_by_farhelm_at: initial_by_farhelm,
                    ..
                } if !self.connection_seen
                    && observed_at == initial_observed
                    && suspended_by_farhelm_at == initial_by_farhelm =>
                {
                    Observation::Waiting
                }
                _ => Observation::SuspendedAgain,
            },
            HostStateView::Stopped {
                waiting_for_place: false,
                start_failed,
            } => match &self.start {
                HostStateView::Stopped {
                    start_failed: initial_failure,
                    ..
                } if start_failed.is_some() && start_failed != initial_failure => {
                    Observation::Failed
                }
                HostStateView::Stopped { .. } if !self.connection_seen => Observation::Waiting,
                _ => Observation::SuspendedAgain,
            },
            HostStateView::Connecting { .. } | HostStateView::Unreachable { .. } => {
                self.connection_seen = true;
                Observation::Waiting
            }
            HostStateView::Stopped {
                waiting_for_place: true,
                ..
            }
            | HostStateView::Deleting { .. }
            | HostStateView::Retired { .. }
            | HostStateView::SignInNeeded
            | HostStateView::VersionSkew { .. }
            | HostStateView::IdentityMismatch { .. }
            | HostStateView::OwnedElsewhere { .. }
            | HostStateView::IdentityUnverified { .. }
            | HostStateView::Duplicate { .. } => Observation::Unavailable,
        }
    }
}

/// A shared minute for busy and connection waiting. A request is measured provisionally until its reply identifies
/// whether the provider accepted it. A busy reply spends its time. Acceptance refunds it, as does a subsequent
/// Resuming phase while the provider finishes its bounded work.
pub(super) struct Budget {
    deadline: Instant,
    paused_at: Option<Instant>,
}

impl Budget {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            deadline: now + CONNECTION_BUDGET,
            paused_at: None,
        }
    }

    pub(super) fn pause(&mut self, now: Instant) {
        self.paused_at.get_or_insert(now);
    }

    pub(super) fn resume(&mut self, now: Instant, refund: bool) {
        if let Some(started) = self.paused_at.take()
            && refund
        {
            self.deadline += now.saturating_duration_since(started);
        }
    }

    pub(super) fn remaining(&self, now: Instant) -> Option<Duration> {
        self.paused_at
            .is_none()
            .then(|| self.deadline.saturating_duration_since(now))
    }
}

/// When callbacks can advance the wake. Add the retry instant to Busy here so inspection of its wake plan stays
/// local to the record. The connection deadline is absolute while the budget runs.
pub(super) enum Advance {
    Wait(Option<Duration>),
    Retry,
    Ready,
    Fail(&'static str),
}

pub(super) struct ResumeWait {
    pub(super) phase: Phase,
    watch: Watch,
    budget: Budget,
    retry_at: Option<Instant>,
    /// A push can precede its reply. Retain the observed failure in case an accepted or busy reply follows.
    /// A terminal command refusal supplies its own more specific diagnosis instead.
    pending_failure: Option<ObservationFailure>,
}

/// Preserve structured failure precedence when multiple pushes precede the command reply.
#[derive(Clone, Copy)]
enum ObservationFailure {
    Provider,
    SuspendedAgain,
    Unavailable,
}

impl ResumeWait {
    pub(super) fn new(state: &HostStateView, now: Instant) -> Self {
        let mut budget = Budget::new(now);
        budget.pause(now);
        Self {
            phase: Phase::Asking,
            watch: Watch::new(state),
            budget,
            retry_at: None,
            pending_failure: None,
        }
    }

    pub(super) fn replied(&mut self, busy: bool, now: Instant) {
        self.budget.resume(now, !busy);
        self.retry_at = busy.then_some(now + BUSY_RETRY);
        self.phase = if busy { Phase::Busy } else { Phase::Resuming };
    }

    pub(super) fn again(&mut self, now: Instant) {
        self.phase = Phase::Asking;
        self.budget.pause(now);
        self.retry_at = None;
    }

    pub(super) fn observe(
        &mut self,
        state: &HostStateView,
        now: Instant,
        provider_failure: &'static str,
    ) -> Advance {
        let observation = self.watch.observe(state);
        let failure = match observation {
            Observation::SuspendedAgain => Some(ObservationFailure::SuspendedAgain),
            Observation::Failed => Some(ObservationFailure::Provider),
            Observation::Unavailable => Some(ObservationFailure::Unavailable),
            Observation::Connected | Observation::Changing { .. } | Observation::Waiting => None,
        };
        if matches!(failure, Some(ObservationFailure::Provider)) || self.pending_failure.is_none() {
            self.pending_failure = failure;
        }
        if self.phase == Phase::Asking {
            return Advance::Wait(None);
        }
        if let Some(failure) = self.pending_failure {
            return Advance::Fail(match failure {
                ObservationFailure::Provider => provider_failure,
                ObservationFailure::SuspendedAgain => {
                    "The host was suspended again while it was waking; resume it and launch again"
                }
                ObservationFailure::Unavailable => {
                    "The host is unavailable; wait for it to reconnect before launching"
                }
            });
        }
        if self.phase == Phase::Resuming
            && matches!(observation, Observation::Changing { resuming: true })
        {
            self.budget.pause(now);
            return Advance::Wait(None);
        }
        self.budget.resume(now, true);
        let remaining = self.budget.remaining(now).unwrap_or_default();
        if remaining.is_zero() {
            let reason = if self.phase == Phase::Busy {
                "The host stayed busy with another operation for 60 seconds; try again when it finishes"
            } else {
                "The host resumed but did not connect within 60 seconds; try again when it connects"
            };
            return Advance::Fail(reason);
        }
        if matches!(observation, Observation::Connected) {
            self.phase = Phase::Ready;
            return Advance::Ready;
        }
        if let Some(retry) = self.retry_at {
            let until_retry = retry.saturating_duration_since(now);
            if until_retry.is_zero() {
                return Advance::Retry;
            }
            return Advance::Wait(Some(remaining.min(until_retry)));
        }
        Advance::Wait(Some(remaining))
    }
}
