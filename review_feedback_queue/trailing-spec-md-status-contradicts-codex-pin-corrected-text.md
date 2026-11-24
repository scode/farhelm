# SPEC.md Status notification contract contradicts updated CODEX_PIN_CORRECTED_TEXT

Reviewed commit: 3f4f312202398d9eb42fe37c9db0f58ff80f6647

## TLDR

The Status section of SPEC.md still states the pre-positional wording for Codex pin-correction notifications (a nested
process of that agent, "would have resumed", and "the agent process that started it"), contradicting the updated Codex
notification text shipped in #4728.

## Details

**other.** Source inspection. Main reference `SPEC.md:3220`.

Commit 3f4f312202398d9eb42fe37c9db0f58ff80f6647 updated CODEX_PIN_CORRECTED_TEXT in
crates/farhelm-supervisor/src/service/notifications.rs:211-213 to: "Farhelm had been following a Codex conversation
reported by another process in this session. A Restart before now could have resumed that conversation. Farhelm now
follows the conversation reported by one of that process's ancestors." However, SPEC.md:3220-3223 (Status ->
session-tracking notifications) was not updated and still specifies: "a Claude, Codex, Muse or Antigravity launch whose
first conversation report came from a nested process of that agent, corrected when a recorded ancestor reports. That
correction says which conversation a Restart before now would have resumed and that Farhelm now follows the conversation
of the agent process that started it, without claiming no further ancestor can correct it." Under positional attribution
for Codex, a shell or other non-Codex process can report first (SPEC.md:4096), a child's fileless clear may not have
been resumable (making "could have resumed" more accurate than "would have resumed"), and CODEX_PIN_CORRECTED_TEXT
intentionally says "one of that process's ancestors" rather than "the agent process that started it". SPEC.md:4098
(touched in 3f4f3122) also still reads "explaining what a Restart before correction would have resumed".
CodexPinCorrected emission mechanics (crates/farhelm-supervisor/src/service/core/vendor/codex.rs:265-287,
store.rs:5837-5843, notifications.rs:143) are sound and match SPEC_impl.md. How to verify: compare SPEC.md:3220-3223 and
SPEC.md:4098 with crates/farhelm-supervisor/src/service/notifications.rs:211-213 at
1b3fe49af86979aebc2719965682a17f8c6366ac. Fix: align SPEC.md:3220-3223 (and SPEC.md:4098 / SPEC_impl.md:5833-5837) with
positional pin-correction wording across all pinned agents.

Reviewer metadata (g57_positional_hooks_codex/f3): suggested bucket: other (specification wording mismatch with shipped
notification text; no security, data-loss, or user-facing defect in the notification itself); confidence: confirmed;
possible cover: Overlaps with g56/f1 on SPEC.md:3220-3223 pin-correction notification wording across pinned agents..

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g57_positional_hooks_codex/f3`.
