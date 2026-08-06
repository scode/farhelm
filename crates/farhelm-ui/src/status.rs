//! What a session's status SAYS: the badge every surface renders it as, and
//! the consequence sentence a delete confirmation opens with.
//!
//! Both are pure `SessionStatus` -> text mappings with no renderer and no
//! I/O, which is what lets them be pinned by unit tests here rather than only
//! through the browser. The status itself, and the predicates that ask about
//! it (`SessionStatus::is_live` / `has_ended`), live in `lib.rs` next to the
//! wire type — this module is about wording, never about deciding what a
//! status means.
//!
//! ## Why the wording is not left to each call site
//!
//! Two surfaces render a session's status: the list's rows, and the session
//! view's own header — which shows the same badge behind a stale session's
//! notice (SPEC.md's "title, directory, last-known status"). A second copy of
//! the mapping would let one surface describe a session differently from the
//! other, and the difference would be invisible until a user compared the two
//! screens for the same session.
//!
//! The confirmation wording carries more than consistency: it is the one
//! sentence standing between a click and an irreversible delete, and SPEC.md's
//! no-guessing rule constrains it directly — `Unknown` must admit uncertainty
//! rather than borrow a live status's claim, and `Interrupted`/`Error` must not
//! promise to kill something that is already gone. Those are the assertions
//! the tests at the bottom exist for.

use crate::SessionStatus;

/// Map a status — and, for an ended session, its annotation — to the
/// badge's CSS modifier class and display text, or `None` for a status
/// that must not be badged at all. Kept as one function so
/// every case stays next to its siblings instead of drifting apart across
/// separate match arms in the render tree.
///
/// ## `Unknown` renders NO BADGE
///
/// The one status with no wording (PLAN_M6_75.md item 3), and the `Option`
/// exists for it alone. "Nothing has classified this session yet" is not
/// information a user can act on, and a badge saying `unknown` reads as a
/// verdict about the agent rather than an admission about the system — the
/// gap it describes is a freshly created session that nothing has
/// classified yet, and it lasts until the first classified status arrives.
/// No bound is promised on that, and none is needed: showing nothing for
/// however long it lasts is honest, and showing a word is not.
///
/// Callers must therefore render no badge ELEMENT rather than an empty one:
/// an empty `.status-badge` span still paints its own box and reads as a
/// blank verdict, which is the same mistake in CSS.
///
/// The restart case never reaches here at all — the helm's merge rule
/// refuses to let an `Unknown` overwrite a status it already knows
/// definitely, so a restarting session keeps showing its previous status.
/// That the two cases are covered by two different mechanisms is why both
/// deserve their own test at the level that can see them.
///
/// The annotation is a QUALIFIER on the exited status, never a
/// replacement for it: SPEC.md is explicit that "stopped" is not a
/// distinct status, so a user-stopped session reads "exited — stopped by
/// user (code 0)". An earlier version rendered the annotation alone, which
/// read as a fourth status word and quietly dropped the one fact every
/// row's badge is supposed to state. The annotation is ignored for every
/// other status — it describes how a run ENDED, and a live session has
/// not.
///
/// `pub(crate)`, not private: the session view renders the same badge behind
/// a stale session's notice (SPEC.md's "title, directory, last-known
/// status"), and two copies of this mapping would let one surface describe a
/// session differently from the other.
pub(crate) fn status_badge(
    status: &SessionStatus,
    annotation: Option<&str>,
) -> Option<(&'static str, String)> {
    Some(match status {
        // The three live statuses get three words and three CSS modifiers,
        // because the whole point of the split is that a user can tell them
        // apart at a glance across a list. The class names match the status
        // names deliberately: one vocabulary from the wire to the
        // stylesheet, so a new status is a rename nobody has to translate.
        SessionStatus::Running => ("running", "running".to_string()),
        SessionStatus::Waiting => ("waiting", "waiting".to_string()),
        SessionStatus::Idle => ("idle", "idle".to_string()),
        SessionStatus::Exited { exit_code } => {
            let mut text = "exited".to_string();
            if let Some(annotation) = annotation {
                text.push_str(" — ");
                text.push_str(annotation);
            }
            if let Some(code) = exit_code {
                text.push_str(&format!(" (code {code})"));
            }
            ("exited", text)
        }
        SessionStatus::Interrupted => ("interrupted", "interrupted".to_string()),
        // The shim's exec-failure sentinel (PLAN_M3.md item 3): the agent
        // never ran at all, which is a different claim from `Exited`'s
        // "it ran and finished" — so it gets its own word and its own
        // red-family color (`app.css`'s `.status-badge.error`), the one
        // case in this match that IS reporting a failure. `detail` (the
        // shim's own errno/argv0 report) rides straight into the badge
        // text rather than being tucked behind a tooltip or a separate
        // element: it is usually short, and it is the one piece of
        // information that actually explains why the row needs attention.
        SessionStatus::Error { detail } => ("error", format!("error — {detail}")),
        // Deliberately the one early return: see this function's own docs.
        SessionStatus::Unknown => return None,
    })
}

/// The safety-critical half of the inline delete-confirmation prompt:
/// what deleting THIS session will actually do, worded so the risk reads
/// on its own without depending on the title. Rendered into its own
/// untruncatable DOM element (`SessionRow`'s `.confirm-consequence`, never
/// ellipsized) AHEAD of the title, which gets a separate, deliberately
/// truncatable element instead — a legal title can be tens of KB with no
/// whitespace at all, and the earlier single-string design (title
/// embedded mid-sentence, the whole thing ellipsized as one span) would
/// clip whichever half landed at the tail once a title ran long enough,
/// which for that wording was always this one: the actual consequence, is
/// still running and will be killed. Splitting the two apart, consequence
/// first, is what makes that unclippable regardless of title length.
///
/// Only ever OPENED from a LIVE or `Unknown` status (see `list::ListView`'s
/// `on_delete`, which sends every `has_ended()` status straight to the
/// delete) — but is written total over `SessionStatus` rather than partial,
/// because `confirming` is `ListView`'s own state, decoupled from any single
/// render: a session that was live when the user opened this prompt
/// can flip to `Exited` under it (stopped from another client, say)
/// before either button is clicked, and this function re-runs on every
/// render off whatever status the row's LATEST prop carries. The
/// `Exited`, `Interrupted`, AND `Error` arms are all that residual case's
/// fallback, not wordings SPEC.md's confirm-contract actually specifies —
/// and `Error` is not merely a defensive completeness case: a session
/// that was genuinely live when this prompt opened, whose agent then
/// turns out never to have execed at all (the launch shim's sentinel is
/// read only once the pane goes dead-or-absent — `service.rs`'s
/// dead-or-absent gate), can flip straight from live to `Error` under
/// an already-open prompt exactly like the `Exited` case above, just with
/// a narrower window.
///
/// The three live statuses share ONE wording, deliberately: what a delete
/// costs is the same whether the agent is working, waiting, or idle — its
/// process tree dies either way — and the confirmation is the last thing a
/// user reads before an irreversible action, which is the worst possible
/// place to introduce a cosmetic distinction they might mistake for a
/// safety one.
///
/// `Interrupted`'s wording is deliberately NOT a killing warning
/// (PLAN_M3.md item 2): the status exists only because the HOST rebooted,
/// which took the agent and every descendant of it with it, so there is
/// nothing left for a delete to kill and claiming otherwise would be the
/// mirror image of the fabricated-liveness mistake `Unknown`'s wording
/// exists to avoid. What deleting actually costs is the session itself —
/// worth saying, because an interrupted session is the one case where the
/// record outlives everything it described and is all that is left to
/// lose (and, since restart landed, the only route back into that
/// conversation).
pub(crate) fn confirm_consequence(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => {
            "still running — deleting kills the agent:"
        }
        SessionStatus::Unknown => {
            "status unknown — the agent may still be running and will be killed:"
        }
        SessionStatus::Exited { .. } => "delete anyway:",
        SessionStatus::Interrupted => {
            "interrupted by a host reboot — nothing left to kill; deleting discards the session:"
        }
        // `Error` never OPENS this prompt (see `on_delete`'s own gate),
        // but a prompt already open for a LIVE session CAN land here —
        // see this function's own docs — so this arm is reachable, not
        // merely a defensive completeness case.
        SessionStatus::Error { .. } => {
            "the agent never started — nothing to kill; deleting discards the session:"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins BOTH the badge's display text and its CSS modifier class per
    /// status — not just the text — since a class regression (e.g. an
    /// `Exited` row silently keeping a live class) would only
    /// otherwise surface as a wrong-COLORED row in the browser, which no
    /// text-only assertion here would ever catch.
    ///
    /// The three live statuses are the M6.75 split (PLAN_M6_75.md item 3),
    /// and each gets its own word AND its own class: the badge is the whole
    /// user-visible product of this milestone, so two statuses sharing a
    /// colour would defeat the point of splitting them at all.
    #[test]
    fn status_badge_matches_text_and_class_for_each_status() {
        assert_eq!(
            status_badge(&SessionStatus::Running, None),
            Some(("running", "running".to_string()))
        );
        assert_eq!(
            status_badge(&SessionStatus::Waiting, None),
            Some(("waiting", "waiting".to_string()))
        );
        assert_eq!(
            status_badge(&SessionStatus::Idle, None),
            Some(("idle", "idle".to_string()))
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: Some(7) }, None),
            Some(("exited", "exited (code 7)".to_string()))
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: None }, None),
            Some(("exited", "exited".to_string()))
        );
        assert_eq!(
            status_badge(&SessionStatus::Interrupted, None),
            Some(("interrupted", "interrupted".to_string()))
        );
        assert_eq!(
            status_badge(
                &SessionStatus::Error {
                    detail: "exec_failed argv0=/nope errno=2".to_string()
                },
                None
            ),
            Some((
                "error",
                "error — exec_failed argv0=/nope errno=2".to_string()
            )),
            "the shim's own recorded detail must reach the badge text, not just its class"
        );
    }

    /// The no-badge-until-classified rule (PLAN_M6_75.md item 3): an
    /// `Unknown` status produces NO badge — not the word "unknown", not an
    /// empty string with a class attached, nothing.
    ///
    /// Worth its own test rather than one more row above, because the
    /// failure it guards against is a regression toward the OLD behavior
    /// that looks perfectly reasonable in a diff: giving `Unknown` a word
    /// again. What makes that wrong is a product argument the code cannot
    /// state on its own — a freshly created session stays unclassified
    /// until the first classified status arrives, and painting a verdict
    /// for that window tells the user something about their agent that
    /// nobody actually knows.
    ///
    /// The annotation argument is exercised too: `Unknown` must stay
    /// badgeless even for a session carrying one, since the presence of an
    /// annotation must never be a back door to a badge.
    #[test]
    fn an_unknown_status_produces_no_badge_at_all() {
        assert_eq!(status_badge(&SessionStatus::Unknown, None), None);
        assert_eq!(
            status_badge(&SessionStatus::Unknown, Some("stopped by user")),
            None,
            "an annotation must not conjure a badge for a status that has none"
        );
    }

    /// SPEC.md: "'stopped' is not a distinct status" — a user-stopped
    /// session is an EXITED session carrying a qualifier, so the badge
    /// must still SAY exited and add the supervisor's own wording after
    /// it, with the exit code still visible when there is one. Rendering
    /// the annotation alone (an earlier shape of this) reads as a fourth
    /// status word and drops the one fact the badge exists to state. The
    /// `exited` CSS class is asserted alongside the text for the same
    /// reason: a stopped session must still LOOK like an ended one.
    ///
    /// The live-session case is the one a naive implementation gets
    /// wrong: an annotation describes how a run ENDED, so it must never
    /// leak onto a session that is running — which is exactly what a
    /// stopped-then-restarted session is.
    #[test]
    fn stop_annotation_qualifies_the_exited_badge_without_replacing_it() {
        assert_eq!(
            status_badge(
                &SessionStatus::Exited { exit_code: None },
                Some("stopped by user")
            ),
            Some(("exited", "exited — stopped by user".to_string()))
        );
        assert_eq!(
            status_badge(
                &SessionStatus::Exited { exit_code: Some(0) },
                Some("stopped by user")
            ),
            Some(("exited", "exited — stopped by user (code 0)".to_string()))
        );
        assert_eq!(
            status_badge(&SessionStatus::Running, Some("stopped by user")),
            Some(("running", "running".to_string())),
            "an annotation must never describe a session that is running"
        );
    }

    /// Pins the exact two confirm-prompt wordings SPEC.md's no-guessing
    /// rule requires to stay distinct: a LIVE status must claim the agent
    /// IS running, while `Unknown` must only ever admit uncertainty — a
    /// regression that quietly reused one string for both (or rounded
    /// `Unknown` up to a live status's wording) is exactly what this guards
    /// against. Scoped to `confirm_consequence`'s own string-building
    /// alone — it says nothing about how `SessionRow` later renders the
    /// result, nor about the SEPARATE title element sitting next to it
    /// (both exercised by the Playwright suite instead, not by anything
    /// callable from this unit test).
    #[test]
    fn confirm_consequence_wording_differs_between_live_and_unknown() {
        for live in [
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Idle,
        ] {
            assert_eq!(
                confirm_consequence(&live),
                "still running — deleting kills the agent:",
                "every live status costs the same delete, so every one must say so: {live:?}"
            );
        }
        assert_eq!(
            confirm_consequence(&SessionStatus::Unknown),
            "status unknown — the agent may still be running and will be killed:"
        );
    }

    /// An interrupted session is NOT alive (a host reboot is what made it
    /// interrupted), so its consequence line must not claim anything will
    /// be killed — the same no-fabrication rule that keeps `Unknown` from
    /// borrowing a live status's wording, applied in the opposite direction.
    /// Asserted as properties rather than as one exact string so the
    /// wording can be improved without the test having to be rewritten
    /// each time; what must not change is that it stops promising a kill
    /// and starts naming what deleting actually costs.
    #[test]
    fn interrupted_consequence_promises_no_kill() {
        let wording = confirm_consequence(&SessionStatus::Interrupted);
        assert!(
            !wording.contains("kills") && !wording.contains("will be killed"),
            "nothing survives a reboot for a delete to kill: {wording}"
        );
        assert!(
            wording.contains("reboot") && wording.contains("discard"),
            "the honest consequence is losing the session record itself: {wording}"
        );
    }

    /// Review-swarm fix batch item 22: `confirm_consequence`'s `Error` arm
    /// is reachable — not a defensive completeness case — via the exact
    /// same residual race `Interrupted`'s own test above exercises in
    /// prose: a confirm prompt opened while a session was live stays
    /// open under a LATER render whose status has since moved on, and
    /// `Error` is one of the statuses it can have moved to. The wording
    /// must match `Error`'s actual meaning (never started, not merely
    /// "finished"), not borrow `Interrupted`'s reboot-specific phrasing.
    #[test]
    fn error_consequence_promises_no_kill_and_names_no_reboot() {
        let wording = confirm_consequence(&SessionStatus::Error {
            detail: "exec_failed argv0=/nope errno=2".to_string(),
        });
        assert!(
            !wording.contains("kills") && !wording.contains("will be killed"),
            "an agent that never started leaves nothing for a delete to kill: {wording}"
        );
        assert!(
            !wording.contains("reboot"),
            "an exec failure is not a reboot; the wording must not borrow that framing: {wording}"
        );
        assert!(
            wording.contains("discard"),
            "the honest consequence is losing the session record itself: {wording}"
        );
    }
}
