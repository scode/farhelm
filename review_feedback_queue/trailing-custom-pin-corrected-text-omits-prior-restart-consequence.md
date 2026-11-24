# CUSTOM_PIN_CORRECTED_TEXT omits what an earlier Restart would have resumed, diverging from SPEC.md:3222 and SPEC_impl.md:5836

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When a custom agent's parent process corrects a conversation first reported by a child process, the session notification
omits explaining what an earlier Restart would have resumed (unlike the other five pinned agents) and says "its parent"
even when a higher recorded ancestor makes the correction.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/notifications.rs:238`.

SPEC.md:3222-3223 states that a pin-correction notification "says which conversation a Restart before now would have
resumed and that Farhelm now follows the conversation of the agent process that started it, without claiming no further
ancestor can correct it", and SPEC_impl.md:5836 states "Each correction records the consequence for Restart before
adoption". All five built-in pin-correction strings in crates/farhelm-supervisor/src/service/notifications.rs:211-235
include that prior-Restart sentence (e.g. "A Restart before now would have resumed that conversation."). By contrast,
CUSTOM_PIN_CORRECTED_TEXT at crates/farhelm-supervisor/src/service/notifications.rs:237-239 reads: "Farhelm was
following a conversation reported by a child process in this session. Restart now uses the conversation reported by its
parent." It omits the prior-Restart consequence despite its own doc comment on line 237 ("Preserve the fact that an
earlier Restart would have used the child process's conversation.") and says "its parent" even though
vendor/positional.rs corrects from any recorded ancestor in ReportPin.ancestors.

How to verify: Compare crates/farhelm-supervisor/src/service/notifications.rs:211-239 against SPEC.md:3220-3223 and
SPEC_impl.md:5833-5836. (Note: CUSTOM_PIN_CORRECTED_TEXT was added in commit 7164b111 / group
g42b_custom_agents_sup_service and surfaced here during cross-spec verification against SPEC.md:3222-3223.)

Fix: Update CUSTOM_PIN_CORRECTED_TEXT in crates/farhelm-supervisor/src/service/notifications.rs:238-239 to include what
an earlier Restart would have resumed and avoid assuming the correcting ancestor is always the immediate parent
(matching CODEX_PIN_CORRECTED_TEXT / ANTIGRAVITY_PIN_CORRECTED_TEXT).

Reviewer metadata (g55_positional_hooks_shared/f2): suggested bucket: other (notification wording inconsistency against
SPEC.md; no security, data-loss, or material UX degradation).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g55_positional_hooks_shared/f2`.
