# Rotation recovery: the unanswered-reads hunt

Historical record written 2026-09-16. This is not a diagnosis — the stall's location was still unestablished when
writing stopped — it is the evidence trail and the fix fork as they stood, so the decision does not need another hunt
to revisit. Like every lore entry it is frozen: later work may supersede it, but this file stays as written.

The test is `rotation logs out an open client and drops its feed and terminal sockets`, in
`e2e/tests/auth.spec.ts`. Two shapes were seen: Chromium observed an aborted recovery detail read in a broad run, and a
missing sidebar row after a successful detail read in an exact run.

## How it started

The 2026-09-10 FLAKES.md entry ("authentication recovery assertions vary between runs") records the original sightings.
Broad run `7653ea10-0f7d-411d-88cc-872ac7d1906b` failed on the aborted detail read; exact candidate run
`498f139d-3aa6-41a3-8f0f-00b10a1a4a62` failed the sidebar-row assertion after a successful read; a prior composer build
and exact candidate `62dbfa15-2692-4217-91af-79c69ec4215f` passed. That shard also produced a cascade — the shared
credential file was updated only after the recovery assertions, so later cases ran on stale credentials — and the shard
was canceled as invalid evidence.

The cascade got contained first. The 2026-09-12 follow-up entry ("rotation recovery still unreproduced; cascade
contained") moved the suite credential refresh to right after the exchange, so a post-exchange failure no longer leaves
later tests unauthenticated. Neither shape reproduced locally in that round: batch `f45de613` passed the exact test
fifteen times on both engines, and batch `55d47716` passed the full auth spec five times on both (a sixth attempt died
on setup when another session's stack took the shared port mid-batch — environmental, not a test outcome). A code read
found the recovery path sound on its face — selection survives the token gate, mount reads fire unconditionally, the
default view's reads are not authoritative for absence, dropped futures cannot reopen the prompt — so no mechanism was
claimed. The TODO entry narrowed to the recovery provenance itself.

## Provenance, then receipts

Provenance came on 2026-09-12: two new reproductions of the missing-sidebar-row shape via recorded hunts (batch
`d1859bb8-dc57-4c74-bc41-1c9ca25036cd`). The row is missing because the recovery batch's `/api/sessions?sort=activity`
and `/api/hosts` fetches were never sent — trace network snapshots showed `send: -1` from creation until teardown,
twice in a row (the feed-handshake re-read batch too), while the same frame's profiles/detail/preferences reads were
served in tens of milliseconds throughout, and the reconnected feed and terminal sockets' upgrades queued 4.3s and 45s.
The helm and supervisor were silent through the whole window, and every request the helm received was answered.

That round also surfaced the budget trap that makes this shape fatal rather than slow: the UI's request timeout is 60s,
and only a failed read hands the surface to the retry ladder, so any hung read outlives the test's own 60s budget. The
60s value was never tuned against this failure mode — the funnel docs in `crates/farhelm-ui/src/api.rs` call it
"deliberately generous rather than tuned", shared with host mutations that do real work on another machine.

The next rung was renderer-level receipts in the UI's fetch wrapper — dispatch and completion per request,
console-carried — to catch a never-dispatched fetch in the act. Those landed in #666.

## The instrumented hunt

A 20-repetition Chromium hunt on 2026-09-16 (run `58d176c8-40d0-457a-8047-f08c0e5f1035`, 19 passed) caught the
missing-row shape in repeat 6 with full pairing: receipts #16 and #20 (`GET /api/sessions?sort=activity`) dispatched
from Rust and never completed, while same-batch detail, hosts, and profiles reads completed in tens of milliseconds.
The repeat-6 trace is preserved unpacked under `analyst-supplement/` in the run record — Playwright wiped the
checkout's `e2e/test-results` on a later run, so the run record is the only copy.

Here is where the wording matters, because an earlier draft of this history overstated it. Both uncompleted reads show
no recorded response or timing in the trace network — a missing answer, not a proven never-sent fetch. The `-1` timings
cannot distinguish a request the browser shelved from one the helm never answered, so the stall's location is
unestablished. Receipt #17 (`GET /api/hosts`) also lacks a completion, but its network entry shows the 401 arriving in
the same millisecond the logout unmounted the tree, so a dropped task — not a second stall — is the favored reading
there. The unanswered #20 holds the sessions surface past the test's 60s budget while its own 60s request timeout would
fire about 3s too late by the clock, so the retry ladder starves exactly as predicted. Two feed upgrades in the same
window likewise show no recorded response; the funnel receipts do not cover sockets, so feed-side dispatch remains
unobserved. Same-batch, same-millisecond discrimination by URL is observed twice but unexplained, and no cache headers
differ between the endpoints.

## The fork

Two options, and the first needs maintainer judgment:

- Split idempotent reads to a shorter timeout, so an unanswered read fails into the retry ladder inside the budget. This
  changes real-user behavior on slow networks, which is why it is the maintainer's call and not a test fix.
- Instrument the transport to locate the stall before changing any timeout. More evidence, no behavior change — but the
  Playwright trace provably does not carry what is needed, so this means new instrumentation with an unknown payoff.

Do not weaken the recovery assertions meanwhile. Whatever the stall turns out to be, the test is asserting the right
thing.
