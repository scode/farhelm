# Restricted create holds the lifecycle claim across the reply send

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

An agent that spawns a child session and then stops reading can freeze its own parent session: the operator cannot stop,
delete, or archive the parent until the child's connection drains.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite (correctness-state-lifecycle
p1). Coordinator confirmed the guard lifetime against the reply path.

The restricted-create arm claims the parent session's lifecycle lock (3093) to pin the session across the credential
re-check and the create. That guard lives in the arm frame across the whole `handle_create_session` call (3163) —
including the reply, which the helper sends internally by awaiting the bounded writer queue (677-688). Stop (924),
delete (1264), and archive (1356) all wait on that claim without a timeout. The full-authority create path never takes
the claim, so only the spawned-from-a-session shape wedges this way — the same no-lock-across-`send_reply` contract
violation as `agentrequest-refusals-hold-fence-across-reply.md`, on the lifecycle lock instead of the fence.

Suggested fix: narrow the claim to the check-and-create section — restructure the helper to return its reply and let the
caller send it after the guard drops, or otherwise move the reply out of the claim's scope.
