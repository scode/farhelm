//! Page input distinguishes a launch the user is still awaiting from one they left.
//!
//! Capture publishes through an ordinary hidden input before the original
//! event reaches Rust, so an answer snapshots its own press synchronously.
//! Completion reads the window counter again; an eval failure is unknown,
//! never evidence that the user stayed. Modal and request lifecycle changes
//! do not participate in this counter.

use dioxus::prelude::*;

/// The most recently observed physical press, shared without render subscriptions.
///
/// Consumers use `snapshot` only inside a user handler. Awaiting an eval there
/// could include later input in the baseline and incorrectly permit navigation.
/// The signal exists only to cross the ordered event transport, not to trigger
/// completion or presentation effects when the user types.
#[derive(Clone, Copy)]
pub(crate) struct LaunchActivity(pub(crate) Signal<Option<u64>>);

impl LaunchActivity {
    /// Capture the already-published count for this accepted user answer.
    /// Missing initialization stays unknown; zero is a legitimate page count.
    pub(crate) fn snapshot(self) -> Option<u64> {
        *self.0.peek()
    }

    /// Record the capture listener's preceding input event without trusting a
    /// malformed relay value to preserve older navigation authority.
    pub(crate) fn receive(mut self, value: &str) {
        self.0.set(serde_json::from_str(value).ok().flatten());
    }

    /// Read current page input at completion, including after modal admission.
    /// A broken or missing bridge cannot establish that no press intervened.
    pub(crate) async fn current(self) -> Option<u64> {
        document::eval(
            "const count = window.__farhelmLaunchActivity?.();\n\
             return Number.isSafeInteger(count) && count >= 0 ? count : null;",
        )
        .join::<Option<u64>>()
        .await
        .ok()
        .flatten()
    }
}

/// Install before global shortcuts can turn their initiating key into a click.
/// The asset retains page identity across authenticated-tree remounts; its
/// live relay lookup attaches the new Rust signal when that tree returns.
pub(crate) fn install() {
    document::eval(include_str!("../assets/launch-activity.js"));
}

/// A matching mounted owner and unchanged known count permit New/Clone navigation.
/// Focus on body cannot establish this: WebKit also leaves it there after an
/// intentional click. Replace uses its existing source-selection rule instead.
pub(crate) fn launch_is_current(
    mounted: Option<u64>,
    opening: u64,
    baseline: Option<u64>,
    current: Option<u64>,
) -> bool {
    mounted == Some(opening) && baseline.is_some() && baseline == current
}

#[cfg(test)]
mod tests {
    use super::launch_is_current;

    /// Retired openings and missing input evidence cannot authorize a late
    /// navigation, even when the count otherwise resembles the old answer.
    #[test]
    fn navigation_needs_the_same_owner_and_known_unchanged_input() {
        assert!(launch_is_current(Some(3), 3, Some(0), Some(0)));
        assert!(!launch_is_current(Some(3), 3, Some(0), Some(1)));
        assert!(!launch_is_current(Some(4), 3, Some(0), Some(0)));
        assert!(!launch_is_current(None, 3, Some(0), Some(0)));
        assert!(!launch_is_current(Some(3), 3, None, None));
        assert!(!launch_is_current(Some(3), 3, Some(0), None));
        assert!(!launch_is_current(Some(3), 3, None, Some(0)));
    }
}
