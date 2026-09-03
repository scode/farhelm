//! What a session's status SAYS: the badge every surface renders it as, and
//! the consequence sentence a delete confirmation opens with.
//!
//! ## The badge is a dot for some statuses and a word for others
//!
//! A live session (running/waiting/idle) draws as a colored dot beside the
//! title, with its word kept in the DOM as screen-reader-only text; an ended
//! one (exited/interrupted/error) keeps its word visible. The split is not
//! cosmetic: what a live badge has to communicate is one of three states,
//! which a color says faster than a word and in less of a dense row's width,
//! while an ended badge carries an exit code, a "stopped by user"
//! annotation, or the shim's exec-failure detail — facts no dot can hold.
//! The relative age that now sits beside the badge (`activity`) is what took
//! over the width the live word gave up.
//!
//! The word never LEAVES the DOM, which is the load-bearing half of that
//! design. A color-only status is unreadable to a screen reader and
//! unassertable by anything that reads text, so [`StatusBadge::visible`]
//! chooses between showing the text and hiding it visually — never between
//! having it and not. The badge element's text content is therefore exactly
//! the same string it was before the dot existed, on every status.
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
//! Two surfaces render a session's status: the list's rows, and the open
//! session's consolidated header — where it sits beside the title, or, while
//! the session is stale, in the metadata band under the host-unreachable
//! notice that carries SPEC.md's "title, directory, last-known status". A
//! second copy of the mapping would let one surface describe a session
//! differently from the other, and the difference would be invisible until a
//! user compared the two screens for the same session.
//!
//! The confirmation wording carries more than consistency: it is the one
//! sentence standing between a click and an irreversible delete, and SPEC.md's
//! no-guessing rule constrains it directly — `Unknown` must admit uncertainty
//! rather than borrow a live status's claim, and `Interrupted`/`Error` must not
//! promise to kill something that is already gone. Those are the assertions
//! the tests at the bottom exist for.

use dioxus::prelude::*;

use crate::SessionStatus;

/// One rendered status badge: its CSS modifier class, the text it states,
/// and whether that text is SHOWN or only available to a screen reader.
///
/// The `visible` flag is the dot-versus-word split (see the module docs).
/// Callers must render `text` either way — a hidden word is still the badge's
/// whole textual content, and it is what assistive technology and the browser
/// suite read the status off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusBadge {
    /// Matches the status name (`running`, `exited`, …), one vocabulary from
    /// the wire to the stylesheet, and carries the badge's color.
    pub(crate) class: &'static str,
    /// What this badge states, annotation and exit code included.
    pub(crate) text: String,
    /// `false` for the live statuses, whose dot is the visible half. It is
    /// `SessionStatus::has_ended` by construction, never a per-status
    /// judgement — a badge shows its word exactly when the status has
    /// something a dot cannot carry.
    pub(crate) visible: bool,
}

/// Map a status — and, for an ended session, its annotation — to the badge to
/// render, or `None` for a status that must not be badged at all. Kept as one
/// function so every case stays next to its siblings instead of drifting
/// apart across separate match arms in the render tree.
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
/// distinct status, so a user-stopped session reads "exited (code 0) —
/// stopped by user". The code leads rather than trails the annotation
/// because the badge is capped at 32 monospace characters (`app.css`'s
/// `.status-badge`) and the older code-last ordering let a long annotation
/// push the code past that cap, hiding the one fact a badge exists to
/// state; leading with it means it survives truncation regardless of how
/// long the supervisor's own prose runs. An earlier version rendered the
/// annotation alone, which read as a fourth status word and quietly
/// dropped the code entirely. The annotation is ignored for every other
/// status — it describes how a run ENDED, and a live session has not.
///
/// `pub(crate)`, not private: the session view renders the same badge in its
/// header — and, for a stale session, in the metadata band under the
/// host-unreachable notice (SPEC.md's "title, directory, last-known status")
/// — and two copies of this mapping would let one surface describe a session
/// differently from the other.
///
/// ## `unseen`, and why only `Idle` reads it
///
/// `unseen` is `Session::has_unseen_output()` (SPEC.md, Status) — `None`
/// when the helm predates the field, `Some(bool)` otherwise. Only the
/// `Idle` arm below branches on it:
/// `Running`'s pulse and `Waiting`'s colour already say "look here" on their
/// own, so the entry that asked for this split asked for it on idle alone.
/// An `Idle` row with unseen output gets class `idle unseen` and the text
/// `idle — new output`; the text change matters as much as the class,
/// because the hidden word is what a screen reader and the browser suite
/// read, and "new output" is a fact the colour alone would not otherwise
/// carry to either.
pub(crate) fn status_badge(
    status: &SessionStatus,
    annotation: Option<&str>,
    unseen: Option<bool>,
) -> Option<StatusBadge> {
    let (class, text) = match status {
        // The three live statuses get three words and three CSS modifiers,
        // because the whole point of the split is that a user can tell them
        // apart at a glance across a list. The class names match the status
        // names deliberately: one vocabulary from the wire to the
        // stylesheet, so a new status is a rename nobody has to translate.
        SessionStatus::Running => ("running", "running".to_string()),
        SessionStatus::Waiting => ("waiting", "waiting".to_string()),
        SessionStatus::Idle if unseen == Some(true) => {
            ("idle unseen", "idle — new output".to_string())
        }
        SessionStatus::Idle => ("idle", "idle".to_string()),
        SessionStatus::Exited { exit_code } => {
            // The exit code leads the annotation, not the other way
            // around: the badge is capped at 32 monospace characters
            // (`app.css`'s `.status-badge`), and "exited — stopped by user
            // (code 0)" ran past that cap, ellipsizing away the one datum
            // — the code — this whole match arm exists to report. Putting
            // it first means it survives truncation regardless of how long
            // the supervisor's own annotation prose runs.
            let mut text = "exited".to_string();
            if let Some(code) = exit_code {
                text.push_str(&format!(" (code {code})"));
            }
            if let Some(annotation) = annotation {
                text.push_str(" — ");
                text.push_str(annotation);
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
    };
    Some(StatusBadge {
        class,
        text,
        // Derived from the status rather than spelled out per arm, because
        // it is not a per-status choice: the dot-versus-word split IS the
        // live-versus-ended split (see this module's docs), and the arms
        // above are only about wording. Restating the boolean six times
        // would invite a seventh arm to get it wrong on its own.
        visible: status.has_ended(),
    })
}

/// The badge element itself, shared by every surface that draws one: the
/// sidebar's rows, the open session's header, and the stale-metadata band
/// under a host-unreachable notice.
///
/// A component rather than three copies of the same rsx because the DOT and
/// the visually-hidden word have to agree everywhere — a surface that
/// rendered the dot without the word would be a status no screen reader can
/// read, and one that rendered the word without the dot would be a status
/// nobody can see. The badge element's own text content is the status text
/// on every path through this, hidden or not.
///
/// The dot is drawn only for a status whose word is hidden. An ended
/// session's badge already carries its color on visible text, so a dot
/// beside it would repeat in a symbol what the word says in letters.
///
/// `title` carries the full text on every badge, for two different reasons
/// that happen to want the same attribute. An ended badge is capped at 32ch
/// and ellipsizes (`app.css`'s `.status-badge`) — the shim's own `error`
/// detail rides straight into this text and can run long, so the tooltip is
/// the only way back to a badge that has visibly clipped. A live badge has
/// no visible text at all, so the tooltip is what lets a mouse user recover
/// the word its color stands in for.
///
/// `dot_onclick`/`dot_title` are the row's own mark-read/mark-unread MOUSE
/// shortcut (SPEC.md, Status), wired through this shared component rather
/// than duplicated wherever a clickable dot is wanted —
/// the whole point of having ONE component is that the dot and the word
/// cannot drift apart between surfaces, and a second, row-only copy of this
/// markup would reopen exactly that risk for the one surface that needs a
/// click.
///
/// Neither is `Option`; every caller passes both explicitly, a plain no-op
/// `dot_onclick` and `dot_title: None` for a badge with no toggle to offer —
/// the session view's header and stale-metadata band (only a row's own dot
/// is ever a control) and a row whose status is not live or whose helm never
/// answered the seen-state question. `dot_title` alone decides whether the
/// dot LOOKS clickable (the CSS hook below), so a caller cannot forget to
/// keep the two in sync by wiring a live `dot_onclick` behind a `None`
/// title, or vice versa.
///
/// The dot itself stays `aria-hidden="true"` regardless: it is a MOUSE
/// shortcut only, never a focusable control (`dot_title` sets a `title`
/// tooltip, not an accessible name) — the row's `…` menu carries the same
/// toggle as a real, keyboard-operable menu item, and is the path a screen
/// reader or keyboard user takes instead.
#[component]
pub(crate) fn StatusBadgeView(
    badge: StatusBadge,
    dot_onclick: EventHandler<MouseEvent>,
    dot_title: Option<String>,
) -> Element {
    // Computed once, ahead of the rsx below, so the `onclick` closure has
    // its own `bool` to check rather than fighting `title: dot_title` for
    // ownership of the `String` — the class selector needs the same
    // answer, so one shared value is also what keeps the two from being
    // able to drift apart.
    let toggle_offered = dot_title.is_some();
    rsx! {
        span { class: "status-badge {badge.class}", title: "{badge.text}",
            if badge.visible {
                "{badge.text}"
            } else {
                // The dot is empty and `aria-hidden`: it contributes nothing
                // to the accessible name (the word beside it is the whole of
                // that), and its color comes from the badge's own status
                // class through `currentColor` — see `.status-dot` in
                // app.css. The word follows it under `.visually-hidden`,
                // which CLIPS rather than removes: `display: none` and
                // `visibility: hidden` would take it out of the
                // accessibility tree, leaving a status that is a color and
                // nothing else.
                //
                // `status-dot-toggle` is the CSS hook for the clickable
                // cursor, present exactly when `dot_title` is — see this
                // component's own doc for why that one value governs both.
                span {
                    class: if toggle_offered { "status-dot status-dot-toggle" } else { "status-dot" },
                    "aria-hidden": "true",
                    title: dot_title,
                    onclick: move |evt| {
                        // `stop_propagation` is why a dot click does not
                        // also select the row: the row's own open control
                        // is an ANCESTOR button (`.session-row-open`), and
                        // an unstopped click would bubble straight into it.
                        // Gated on `toggle_offered` rather than called
                        // unconditionally: for a badge with nothing to
                        // toggle, stopping here regardless would turn that
                        // dot's exact pixels into a dead spot that neither
                        // opens the row nor does anything else, since a
                        // no-toggle `dot_onclick` is a no-op. Letting the
                        // click bubble instead keeps the dot's area part of
                        // the ordinary open control.
                        if toggle_offered {
                            evt.stop_propagation();
                            dot_onclick.call(evt);
                        }
                    },
                }
                span { class: "visually-hidden", "{badge.text}" }
            }
        }
    }
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

/// The row's replace confirmation, beside [`confirm_consequence`] and under
/// the same no-guessing discipline that function's own doc states: a live
/// status must claim the agent IS killed, `Unknown` may only admit
/// uncertainty,
/// and neither `Exited` nor `Interrupted` may claim a kill that cannot
/// happen. What replace adds beyond delete's wording is the OTHER half of
/// the operation — every arm ends by saying a fresh session with the same
/// settings takes the old one's place, which is the one sentence that
/// tells a reader this prompt is not delete's. Without it, a user
/// skimming a familiar-looking warning could read "kills the agent" and
/// assume the row is simply gone, missing that a running replacement is
/// what they are actually about to get.
///
/// Unlike [`confirm_consequence`], EVERY arm here can legitimately open
/// this prompt — replace has no `has_ended()`-style bypass the way delete
/// does for an already-finished session (see `list::row`'s `on_replace`),
/// because "the same settings, a moment later" is worth confirming
/// regardless of whether there was anything left to kill. That is also why
/// `Exited` gets its own wording instead of delete's terse "delete
/// anyway:" — replace has no analogous "there is nothing to reconsider"
/// shortcut to fall back on, so its own consequence is spelled out in full
/// even for a session that was already at rest.
pub(crate) fn replace_consequence(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => {
            "still running — replacing kills the agent and discards the conversation; a fresh \
             session with the same settings takes its place:"
        }
        SessionStatus::Unknown => {
            "status unknown — the agent may still be running and will be killed, and the \
             conversation is discarded either way; a fresh session with the same settings \
             takes its place:"
        }
        SessionStatus::Exited { .. } => {
            "replacing discards the conversation; a fresh session with the same settings takes \
             its place:"
        }
        SessionStatus::Interrupted => {
            "interrupted by a host reboot — nothing left to kill, but replacing still discards \
             the conversation; a fresh session with the same settings takes its place:"
        }
        SessionStatus::Error { .. } => {
            "the agent never started — nothing to kill; a fresh session with the same settings \
             takes its place:"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three properties of a badge, spelled out at each call site so the
    /// assertions below read as the specification they are rather than as
    /// struct literals.
    fn badge(class: &'static str, text: &str, visible: bool) -> Option<StatusBadge> {
        Some(StatusBadge {
            class,
            text: text.to_string(),
            visible,
        })
    }

    /// Pins BOTH the badge's text and its CSS modifier class per status —
    /// not just the text — since a class regression (e.g. an `Exited` row
    /// silently keeping a live class) would only otherwise surface as a
    /// wrong-COLORED row in the browser, which no text-only assertion here
    /// would ever catch. That matters more now than it did when the class
    /// merely tinted a word: for a live status the class IS the badge, since
    /// the color is the only part of it anyone sees.
    ///
    /// The three live statuses are the M6.75 split (PLAN_M6_75.md item 3),
    /// and each gets its own word AND its own class: the badge is the whole
    /// user-visible product of this milestone, so two statuses sharing a
    /// colour would defeat the point of splitting them at all.
    ///
    /// The `visible` column is the dot-versus-word split, and it is spelled
    /// out per status HERE precisely because the implementation now derives
    /// it from `SessionStatus::has_ended`: an assertion written the same way
    /// would restate the code instead of checking it. The two directions
    /// fail differently and both fail quietly. A live status that turned
    /// visible again puts the word back in a row that was densified to lose
    /// it — cosmetic, and obvious the moment anyone looks. An ended status
    /// that turned INVISIBLE hides an exit code, a "stopped by user"
    /// annotation, or the shim's exec-failure detail behind a dot that
    /// cannot carry any of them, and nothing on screen would say a fact went
    /// missing.
    ///
    /// What it deliberately does NOT cover is the rendered element:
    /// `StatusBadgeView` turning `visible: false` into a dot plus clipped
    /// text is a browser fact (`.visually-hidden` has to actually clip), and
    /// the terminal spec's live-dot test is what asserts it.
    #[test]
    fn status_badge_matches_text_and_class_for_each_status() {
        assert_eq!(
            status_badge(&SessionStatus::Running, None, None),
            badge("running", "running", false)
        );
        assert_eq!(
            status_badge(&SessionStatus::Waiting, None, None),
            badge("waiting", "waiting", false)
        );
        assert_eq!(
            status_badge(&SessionStatus::Idle, None, None),
            badge("idle", "idle", false)
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: Some(7) }, None, None),
            badge("exited", "exited (code 7)", true)
        );
        assert_eq!(
            status_badge(&SessionStatus::Exited { exit_code: None }, None, None),
            badge("exited", "exited", true)
        );
        assert_eq!(
            status_badge(&SessionStatus::Interrupted, None, None),
            badge("interrupted", "interrupted", true)
        );
        assert_eq!(
            status_badge(
                &SessionStatus::Error {
                    detail: "exec_failed argv0=/nope errno=2".to_string()
                },
                None,
                None
            ),
            badge("error", "error — exec_failed argv0=/nope errno=2", true),
            "the shim's own recorded detail must reach the badge text, not just its class"
        );
    }

    /// The unseen-idle split (SPEC.md, Status): `Idle` with
    /// `unseen == Some(true)` gets its own class and text, every
    /// other combination of status and `unseen` leaves the ordinary idle (or
    /// live) badge untouched — including `Running`/`Waiting`, which the
    /// module doc above states ignore the flag entirely, pinned here rather
    /// than left to be inferred from the match arms.
    #[test]
    fn idle_alone_reads_the_unseen_flag() {
        assert_eq!(
            status_badge(&SessionStatus::Idle, None, Some(true)),
            badge("idle unseen", "idle — new output", false),
            "unseen idle output gets its own class and its own hidden word"
        );
        assert_eq!(
            status_badge(&SessionStatus::Idle, None, Some(false)),
            badge("idle", "idle", false),
            "explicitly seen is the ordinary idle badge"
        );
        assert_eq!(
            status_badge(&SessionStatus::Idle, None, None),
            badge("idle", "idle", false),
            "an old helm that never answers the question draws the SAME ordinary idle \
             badge every other idle row gets — there is no separate legacy colour"
        );
        for unseen in [None, Some(true), Some(false)] {
            assert_eq!(
                status_badge(&SessionStatus::Running, None, unseen),
                badge("running", "running", false),
                "a pulsing dot already says \"look here\"; running ignores unseen \
                 entirely: {unseen:?}"
            );
            assert_eq!(
                status_badge(&SessionStatus::Waiting, None, unseen),
                badge("waiting", "waiting", false),
                "waiting is already an attention colour and ignores unseen too: {unseen:?}"
            );
        }
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
        assert_eq!(status_badge(&SessionStatus::Unknown, None, None), None);
        assert_eq!(
            status_badge(&SessionStatus::Unknown, Some("stopped by user"), None),
            None,
            "an annotation must not conjure a badge for a status that has none"
        );
        assert_eq!(
            status_badge(&SessionStatus::Unknown, None, Some(true)),
            None,
            "an unseen flag must not conjure a badge for a status that has none either"
        );
    }

    /// SPEC.md: "'stopped' is not a distinct status" — a user-stopped
    /// session is an EXITED session carrying a qualifier, so the badge
    /// must still SAY exited and add the supervisor's own wording after
    /// it, with the exit code still visible when there is one — and
    /// leading, not trailing, so it survives the badge's 32-character cap
    /// (`app.css`'s `.status-badge`) regardless of how long the annotation
    /// runs (the regression this pins: "exited — stopped by user (code
    /// 0)" put the code last, where a longer annotation would ellipsize it
    /// away). Rendering the annotation alone (an earlier shape of this)
    /// reads as a fourth status word and drops the one fact the badge
    /// exists to state. The `exited` CSS class is asserted alongside the
    /// text for the same reason: a stopped session must still LOOK like an
    /// ended one. The annotation is also the concrete reason ended badges
    /// kept a VISIBLE word while live ones gave theirs up — "stopped by
    /// user" is a fact no dot can carry.
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
                Some("stopped by user"),
                None
            ),
            badge("exited", "exited — stopped by user", true)
        );
        assert_eq!(
            status_badge(
                &SessionStatus::Exited { exit_code: Some(0) },
                Some("stopped by user"),
                None
            ),
            badge("exited", "exited (code 0) — stopped by user", true),
            "the code leads so it survives the badge's character cap ahead of a long annotation"
        );
        assert_eq!(
            status_badge(&SessionStatus::Running, Some("stopped by user"), None),
            badge("running", "running", false),
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

    /// [`replace_consequence`]'s own version of the no-guessing pin above:
    /// a live status must claim the kill, `Unknown` may only admit
    /// uncertainty, and neither may borrow the other's certainty.
    #[test]
    fn replace_consequence_wording_differs_between_live_and_unknown() {
        for live in [
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Idle,
        ] {
            let wording = replace_consequence(&live);
            assert!(
                wording.contains("still running") && wording.contains("kills the agent"),
                "every live status costs the same kill, so every one must say so: {live:?} -> \
                 {wording}"
            );
        }
        let unknown = replace_consequence(&SessionStatus::Unknown);
        assert!(
            unknown.contains("may still be running") && unknown.contains("will be killed"),
            "an unresolved status may not round up to a live status's certainty: {unknown}"
        );
    }

    /// Neither `Exited` nor `Interrupted` may claim a kill replace cannot
    /// perform — the same rule [`confirm_consequence`]'s own
    /// `interrupted_consequence_promises_no_kill` pins for delete, applied
    /// to replace's wording instead. `Interrupted` additionally names the
    /// reboot that is why there is nothing left to kill; `Exited` does not,
    /// since an ordinary exit needs no such explanation.
    #[test]
    fn replace_consequence_never_promises_a_kill_for_an_already_ended_session() {
        let exited = replace_consequence(&SessionStatus::Exited { exit_code: Some(0) });
        assert!(
            !exited.contains("kills the agent") && !exited.contains("will be killed"),
            "an exited agent leaves nothing for replace to kill: {exited}"
        );
        let interrupted = replace_consequence(&SessionStatus::Interrupted);
        assert!(
            !interrupted.contains("kills the agent") && !interrupted.contains("will be killed"),
            "a host reboot already ended the agent; replace cannot kill it again: {interrupted}"
        );
        assert!(
            interrupted.contains("reboot"),
            "interrupted's wording must say WHY there is nothing to kill: {interrupted}"
        );
    }

    /// [`confirm_consequence`]'s own `error_consequence_promises_no_kill_and_names_no_reboot`,
    /// mirrored for replace: an agent whose exec never succeeded leaves
    /// nothing for replace to kill either, and the wording must not borrow
    /// `Interrupted`'s reboot framing for an unrelated failure.
    #[test]
    fn replace_consequence_error_promises_no_kill_and_names_no_reboot() {
        let wording = replace_consequence(&SessionStatus::Error {
            detail: "exec_failed argv0=/nope errno=2".to_string(),
        });
        assert!(
            !wording.contains("kills the agent") && !wording.contains("will be killed"),
            "an agent that never started leaves nothing for replace to kill: {wording}"
        );
        assert!(
            !wording.contains("reboot"),
            "an exec failure is not a reboot; the wording must not borrow that framing: {wording}"
        );
    }

    /// Every status EXCEPT `Error` must warn that the conversation is
    /// discarded — the plan requirement no earlier test actually pins: the
    /// wording tests above check kill certainty, reboot framing, and the
    /// fresh-session suffix, but none of them would fail if an
    /// implementation quietly dropped the irreversible conversation-loss
    /// warning from every applicable arm. `Error` is the one deliberate
    /// exception: no agent conversation ever started, so there is nothing
    /// for that arm to say was discarded.
    #[test]
    fn every_status_but_error_warns_the_conversation_is_discarded() {
        let discards = [
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Idle,
            SessionStatus::Unknown,
            SessionStatus::Exited { exit_code: Some(0) },
            SessionStatus::Interrupted,
        ];
        for status in discards {
            let wording = replace_consequence(&status);
            assert!(
                wording.contains("discard"),
                "{status:?}'s wording must warn that the conversation is discarded: {wording}"
            );
        }
        let error_wording = replace_consequence(&SessionStatus::Error {
            detail: "exec_failed argv0=/nope errno=2".to_string(),
        });
        assert!(
            !error_wording.contains("discard"),
            "an agent that never started never held a conversation to discard: {error_wording}"
        );
    }

    /// The one property every arm must share, spelled out as its own test
    /// rather than folded into the others above: whatever a status costs,
    /// [`replace_consequence`] must always say a fresh session with the
    /// same settings takes the old one's place — the sentence that is the
    /// entire reason this prompt exists separately from
    /// [`confirm_consequence`]'s. A regression that dropped it from even
    /// one arm would leave that status's prompt reading exactly like
    /// delete's, with nothing telling the user a replacement is coming.
    #[test]
    fn every_replace_consequence_arm_promises_a_fresh_replacement() {
        let statuses = [
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Idle,
            SessionStatus::Unknown,
            SessionStatus::Exited { exit_code: Some(0) },
            SessionStatus::Interrupted,
            SessionStatus::Error {
                detail: "exec_failed argv0=/nope errno=2".to_string(),
            },
        ];
        for status in statuses {
            let wording = replace_consequence(&status);
            assert!(
                wording.contains("fresh session") && wording.contains("takes its place"),
                "{status:?}'s wording must promise a replacement, not just a consequence: \
                 {wording}"
            );
        }
    }
}
