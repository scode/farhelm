//! A recipe row's prepared-snapshot line: whether the snapshot new hosts and sessions start from (the built snapshot
//! with Farhelm installed on top) is ready, being prepared, stale, failed or not made yet.
//!
//! SPEC.md, "Topology": "The recipe row shows preparation below its built-snapshot status: **Ready for new hosts and
//! sessions** with age, Farhelm version and the 24-hour policy in the positive tone; a pulsing current phase and step
//! count while preparing; the reason when stale; and the failed phase or an interrupted attempt with the
//! next-new-host-or-session retry explanation. An idle row qualifies readiness as last checked, since age display can
//! advance without a new helm read." SPEC_impl.md, "Tensorlake prepared snapshots". Ported from the old app's
//! `prepared_status` and `preparation_age` (`tensorlake/recipes.rs` at reference commit `24bc0580`), line for line.
//!
//! Preparation has no button: the first new host or session prepares, so the line only explains. Freshness is the
//! helm's to decide, on every use; the line shows only the stale reason the helm reported, and an age that the viewer's
//! clock moves on, never a staleness of its own reckoning. The age is display only, which is why the row qualifies
//! readiness as "when last checked".

use farhelm_proto::recipes::{PrepareStep, PrepareView, PreparedStaleReason, RecipeView};

use super::{Line, Tone, Words, utc_minute};

/// "1 min", "41 s", "2 h": how long ago `at` was at `now`, both in seconds since the epoch. Display only, so it
/// tolerates a viewer's clock behind the helm's (an age never goes negative), and saturating arithmetic keeps extreme
/// times from overflowing. The old `preparation_age`.
pub(in crate::settings) fn age(now: i64, at: i64) -> String {
    let seconds = now.saturating_sub(at).max(0);
    if seconds < 60 {
        format!("{seconds} s")
    } else if seconds < 3600 {
        format!("{} min", seconds / 60)
    } else {
        format!("{} h", seconds / 3600)
    }
}

/// "step 3 of 5".
fn step_of(step: PrepareStep) -> String {
    format!("step {} of {}", step.number(), PrepareStep::COUNT)
}

/// The prepared-snapshot line of `view`, with `now` the viewer's time in seconds since the epoch (`None` when the
/// viewer's clock cannot say, when ages are left out and readiness gives its UTC time instead).
///
/// A running, failed or interrupted preparation takes precedence over the prepared snapshot it may replace. Only the
/// helm's stale reason makes the line a warning: a client's clock decides nothing about reuse. A running preparation's
/// line is the one that pulses ([`Line::running`]).
pub(in crate::settings) fn prepared_status(view: &RecipeView, now: Option<i64>) -> Line {
    match &view.preparation {
        PrepareView::Preparing { step, started_at } => {
            let elapsed = now
                .map(|now| format!(" · {}", age(now, *started_at)))
                .unwrap_or_default();
            Line {
                running: true,
                ..Line::app(
                    Tone::Plain,
                    format!(
                        "Preparing for new hosts and sessions: {} ({}){elapsed}",
                        step.label(),
                        step_of(*step)
                    ),
                )
            }
        }
        PrepareView::Failed { step, message, .. } => Line {
            tone: Tone::Error,
            words: vec![
                Words::App(format!(
                    "Preparing failed while {} ({}): ",
                    step.label(),
                    step_of(*step)
                )),
                // The preparer's own words, from the sandbox.
                Words::Host(crate::host_text::HostText::from_host(message.clone())),
                Words::App(". The next new host or session tries again.".into()),
            ],
            running: false,
        },
        PrepareView::Interrupted { .. } => Line::app(
            Tone::Warning,
            "Preparation stopped when the helm stopped. The next new host or session tries again.",
        ),
        PrepareView::Idle => match &view.prepared {
            None => Line::app(
                Tone::Muted,
                "Not prepared yet; the first new host or session prepares it (about a minute).",
            ),
            Some(prepared) => match prepared.stale_reason {
                Some(reason) => {
                    let reason = match reason {
                        PreparedStaleReason::Expired => "it expired",
                        PreparedStaleReason::FarhelmUpdated => "Farhelm updated",
                        PreparedStaleReason::SnapshotRebuilt => "the snapshot was rebuilt",
                    };
                    Line::app(
                        Tone::Warning,
                        format!(
                            "Not prepared since {reason}; the next new host or session prepares it (about a minute)."
                        ),
                    )
                }
                None => {
                    let when = now
                        .map(|now| format!("{} ago", age(now, prepared.prepared_at)))
                        .unwrap_or_else(|| utc_minute(prepared.prepared_at));
                    Line::app(
                        Tone::Positive,
                        format!(
                            "Ready for new hosts and sessions when last checked · prepared {when} for Farhelm {} · \
                             expires after 24 h; checked before use",
                            prepared.farhelm_version
                        ),
                    )
                }
            },
        },
    }
}

/// Whether `view`'s prepared-snapshot line shows an age that the clock moves on (a running preparation's elapsed time,
/// or a ready snapshot's age), so the section holds the ticker's 30-second clock only while one does.
pub(in crate::settings) fn shows_age(view: &RecipeView) -> bool {
    match &view.preparation {
        PrepareView::Preparing { .. } => true,
        PrepareView::Failed { .. } | PrepareView::Interrupted { .. } => false,
        PrepareView::Idle => view
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.stale_reason.is_none()),
    }
}
