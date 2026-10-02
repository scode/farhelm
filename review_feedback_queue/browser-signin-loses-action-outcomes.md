# Browser sign-in recovery silently loses outcomes of actions already in progress

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

A browser token prompt can silently lose the results of actions already running, including Delete, even though the
server continues the work.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F7 / COR-BROWSER-OUTCOME`, reviewer `correctness_data_flow`, pass 2. Confidence: **definite**. Review disposition:
**would surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-ui/src/lib.rs:1270`. Recorded from the completed review without
rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: The
completed desktop reauthentication ledger entry addresses a different render branch. `session-view-leaks-page-lock.md`
concerns a stranded lock, and `uploads-aborted-silently-on-remount.md` concerns terminal upload remounts; neither covers
pending browser mutation outcomes.

When a browser request receives the helm's authentication-required response, the UI replaces its authenticated component
tree with the token prompt. The removed components own the tasks waiting for session actions. A Delete accepted
immediately before token rotation can still be running when a later fleet read receives 401; that read opens the token
prompt and removes the task waiting for Delete. The server continues deleting, but the sign-in page says nothing about
the outstanding action or its uncertain outcome.

This also applies to other pending mutations and is independent of the desktop authentication gate. Preserve pending
action ownership and results above the browser's sign-in conditional, or carry explicit unknown-outcome notices through
the token prompt and subsequent sign-in. Validate with an accepted mutation held pending while a separate read receives
authentication-required. Recovering the triggering 401 alone does not satisfy SPEC.md's requirement that every started
action report a result or an unknown outcome.
