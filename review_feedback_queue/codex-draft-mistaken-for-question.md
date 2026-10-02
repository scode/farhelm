# An unsent Codex draft can be mistaken for an agent question

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Pasting question-shaped diagnostics into an unsent Codex draft can make the sidebar say the agent needs an answer when
it is idle.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F3 / COR-CODEX-DRAFT`, reviewer `correctness_general`, pass 1. Confidence: **definite**. Review disposition: **would
fix**. Queue priority at recording: **other**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:454`. Recorded from the
completed review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

The supervisor can classify an idle Codex session as waiting for an answer based on text the user has not submitted. Its
normal dialog checks exclude text inside the prompt editor, but a separate queued-question check scans the last eight
nonblank screen lines without that exclusion. It accepts adjacent lines where the first starts with `?` and contains
`question`, and the next contains `to answer`, ignoring indentation. An unsent draft beginning
`› Please explain this UI`, followed by `? 1 question` and `shift+← to answer`, therefore matches before the reader
reaches its idle check.

Pasting terminal or UI diagnostics into a draft can make the sidebar report an agent question when the agent has
received nothing. Restrict queued-question recognition to the screen area above the live prompt editor, excluding all
draft continuation rows. Add paired cases containing the same marker text above the editor and inside an unsent
multiline draft; the existing draft tests exercise the separate dialog-footer checks.
