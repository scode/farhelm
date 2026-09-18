# Archive drops its admission permit before the metadata rebuild and reply

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Under a burst of archive requests, the supervisor can pile expensive unadmitted work onto itself while its own
concurrency bound reports headroom — archive's costliest tail work escapes the limit.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: possible (correctness-state-lifecycle
p1). Only the rebuild-plus-reply tail escapes, not the teardown, so the consequence is bound dilution under archive
flood rather than a demonstrated overload.

Slow handlers hold one of 8 process-wide admission permits for the whole mutation including its reply. The archive arm
unpacks its outcome as `Ok((Ok(entry), _permit)) => entry` (1402): the named `_permit` binding drops when the match
evaluates, before `session_info_now` rebuilds the metadata (1429 — the full observed-and-recorded pass with tmux round
trips and store writes) and before the reply is queued. Delete, the sibling mutation, holds its permit through the reply
(`Ok((Ok(()), _permit)) => send_reply(...)`, 1305 — the binding lives to the end of the arm), and the rename docs state
the parity rule (whole mutation including the reply). Coordinator verified the drop timing against both siblings.

Suggested fix: carry the permit out of the unpacking step and drop it only after the reply is sent (mirroring delete),
or move the metadata rebuild inside the admitted section.
