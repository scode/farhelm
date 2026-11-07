# Brick recovery truncates checkpoint budget

Reviewed commit: d6d8da6e673228f2d430052beb424e75c15822c7

## TLDR

A recoverable provider refusal can cut release checks below their required time budget or wrongly prevent their retry.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `releasing/mac-vm-test/brick_test.py:855 at d6d8da6e673228f2d430052beb424e75c15822c7`.

Source assessment (read with the later corrections below) (review record g022/l1, event 11):

Reviewed commit: d6d8da6e673228f2d430052beb424e75c15822c7. TLDR: A recoverable provider rate limit can leave slow
agent-continuity checks with less than half their intended time, causing an avoidable failed release test; with a small
operator timeout it can wrongly deny recovery altogether. Core finding and exact contract citation preserved:
brick_test.py:850 creates the initial task using turn_timeout, but :855 starts checkpoint_deadline using raw timeout.
checkpoint_contract.py:38-45 raises the default 1200 seconds to 2400 for p0-updater-upgrade old-fixture and 1800 for
both old-session checkpoints. After an immediate pre-action 429 and 60-second backoff, brick_test.py:900-903 forwards
only about 1140 seconds to task_for; guest_workflow.py:655-667 forwards that value unchanged. BRICK-TEST.md:444-446
explicitly requires the longer turns because these checkpoints wait for genuine agent conversations and Restart.
Synthesis of adjacent admission: :880-882 also uses the same incorrect deadline before admitting a retry. For example,
--timeout 30 is accepted at :764-765 and the initial old-fixture task still gets 2400 seconds, but the admission check
always rejects its first 60-second backoff against a raw 30-second deadline. This is another consequence of the same l1
arithmetic, not a new finding. Initialize the shared deadline from the effective initial task timeout, then subtract
elapsed time/backoff both for admission and the replacement task. Named cross-group g024/c2 confirms the retry tests use
p0-fresh, mocked pause without elapsed-time advancement, and no retry-budget assertions; separate updater tests assert
initial floors without 429. Their passing assertions therefore do not refute l1. The corrected g024/c2 already records
this limitation, so no duplicate test finding is needed. A repair regression should refuse a floored checkpoint before
any action, advance an injected clock during backoff, and check both admission and the remaining envelope budget.
Suggested bucket: other (release tooling availability; no established end-user harm). Confidence: confirmed by pinned
source; no runtime tests. proposed_drop: false. possible_cover: unknown for queue coverage, which was not searched;
named g024/c2 is supporting coverage analysis of this same finding. Caveats: time truncation causes failure only if the
later attempt needs the lost budget; the small-timeout admission example fails deterministically after an otherwise
retryable refusal. No false product pass, security route, user-owned-work loss or catastrophic tooling consequence is
established. Initial hypothesis that retries reapply floors remains refuted: the actual defect is the shorter shared
deadline. No later changes to the five assigned files through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b,
checked during the initial pass.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g022/l1`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
