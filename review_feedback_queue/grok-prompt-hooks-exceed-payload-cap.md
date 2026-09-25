# Grok prompt hooks may exceed the hook payload cap

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With Farhelm's Grok hooks installed, a very large prompt or very long answer might surface a hook error in Grok, and
inside Farhelm the Resume offer can take an extra turn to appear.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F21 / COR-GROK-PAYLOAD-SIZE`, tagged **possible**. Anchors and title: `crates/farhelm/src/hook.rs:299`,
`crates/farhelm/src/hook.rs:377` — The hook's payload-size assumption may not hold for Grok's prompt-path callbacks

The hook reads its JSON payload from stdin under two assumptions. Without a session credential (the agent was run
outside Farhelm with the callback still configured) it exits without reading stdin at all, justified by the comment at
L299 that a `SessionStart` payload is "far smaller than a pipe buffer, so declining to drain it cannot block the agent".
With a credential it reads at most 64 KiB + 1 bytes (L377) and drops anything larger as `oversized`, closing stdin on
exit.

The same entry point also serves Grok's `UserPromptSubmit` and `Stop` callbacks, which are installed globally, so they
fire outside Farhelm too. Per third-party descriptions, those payloads carry the user's prompt and the model's last
answer (`lastAssistantMessage`), which can exceed 64 KiB. For such a payload, Grok's write to the hook's stdin could
block until the hook exits and then fail with a broken pipe, which Grok might surface as a hook error. The module
promises the hook never shows up as a failure in the agent's UI; that promise now rests on a `SessionStart`-only size
assumption applied to events that carry user and model text. Inside Farhelm, an oversized event is also simply dropped,
so the resume offer may appear one turn later.

Suggested change: always drain stdin to EOF within the time budget, on the no-credential path too, discarding bytes past
the cap. At minimum correct the L299 comment.

Restater note: Grok's exact payload fields and how it reacts to a hook closing stdin early are unverified; the code
paths are as described.
