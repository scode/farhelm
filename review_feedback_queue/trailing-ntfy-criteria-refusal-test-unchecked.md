# custom_criteria_save_keep_reset_and_refuse says an over-cap save changes nothing, but never checks the stored settings after the refusal

Reviewed commit: 93294dee7dd395d28ec12f86294891c594015773

## TLDR

This is a test gap with no user-visible bug today. The test documents that over-long criteria are refused without
changing anything, then moves straight on to a reset save. A regression that wrote the settings before refusing, or that
cleared the stored criteria on refusal, would still pass.

## Details

At crates/farhelm-helm/src/ntfy_tests.rs:590-633, the docstring specifies that "text over the cap is refused with the
fixed message and changes nothing". After the refused save the test checks only the error message, then runs the
whitespace reset save, which would hide any change the refusal made. Today Config::saved
(crates/farhelm-helm/src/ntfy.rs:45-103) returns its error before the UPDATE in save_ntfy_config_noting_switch_on
(:653-680), so the behavior is correct, but nothing pins it. To fix, read `store.ntfy_config()` after the refusal and
assert that the criteria are still "the build is red" (and the switches unchanged) before the reset step.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other, because it is a test-fidelity gap only.
- Confidence: confirmed (from the test source).
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g22/f2 (finding), group g22 "Asking agents to notify the user when blocked: spec, protocol, helm", findings log
  line 2.
