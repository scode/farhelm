# Grok double-null aliased fields are refused

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If Grok sends a null transcript path at session start, Farhelm never records the Grok conversation and that session
cannot be resumed.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F19 / COR-GROK-DOUBLE-NULL`, tagged **possible**. Anchors and title: `crates/farhelm/src/hook.rs:519`,
`crates/farhelm/src/hook.rs:595` — A Grok callback sending both spellings of an optional field as null is refused
outright

Grok's hook payloads carry some fields twice, in camelCase and snake_case (`sessionId`/`session_id`,
`transcriptPath`/`transcript_path`). The `aliased` helper in `parse_grok_payload` (hook.rs:519) handles this. When only
one spelling is present, a JSON `null` counts as absent. When both are present, both must be strings with the same
value, or the whole payload is rejected. So `{"transcriptPath": null, "transcript_path": null}` fails with
`aliased-field-not-a-string` (the call at L595). The transcript path is optional at SessionStart, but the report is
all-or-nothing, so this one unknown optional value also discards the required session id and ordering timestamp, and
Farhelm never learns which Grok conversation the session is running.

The inconsistency is that `null` means "absent" or "invalid" depending on whether Grok happened to repeat the key.
Suggested change: treat a `null` in either spelling as absent before comparing, while still refusing a null next to a
string, non-string values, and two disagreeing strings. (The function's docstring says the refusal of half-malformed
duplicates is deliberate, mainly for the ordering timestamp; the suggested change keeps that.)

Restater note: whether Grok ever actually sends null values here is unconfirmed; the finding is conditional on that.
