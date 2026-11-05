## What this was about

Five triaged terminal races could affect unrelated work or retained terminal history. The maintainer asked for very
straightforward fixes, with a specific accepted-case filter when a complete repair exceeded about 100 changed lines,
including tests and documentation. Process-number reuse during the internal verification-to-signal handoff was already
chosen for acceptance.

This stack fixes the registry test's signal target and records four specific accepted cases. Production terminal
behavior is unchanged. Every outcome has its own draft PR, ledger execution record and feedback-queue removal.

## Things you should know

- The registry test previously found a process by a socket's basename in any directory and could kill an unrelated
  terminal host. It now takes the peer of a connection to its own exact socket and requires that peer to answer before
  signalling it. Its remaining basename searches only make non-destructive assertions.
- Interrupted starts can still shut down the winning terminal when overlapping starts draw the same random socket name.
  A safe ownership check must avoid taking over the winner before inspecting it; the complete probe, fixtures and
  documentation need about 141 changed lines. The approved fallback records this precise collision and work-loss case.
- An old input task already inside a send can still deliver Enter after a replacement window starts typing, submitting
  an unfinished command. A 99-line join candidate missed other detach routes; covering four more reached 103 lines
  before the restart and tab-close ownership fences were addressed. The partial implementation was removed. The filter
  accepts only that in-progress-poll ordering gap, not newly received input after ownership was lost.
- Extreme applied resizes on a quiet terminal can still exhaust its recovery journal without making a save due. With the
  default cap, about 524,288 distinct applied resizes can consume it. A later crash or skipped update-time save can lose
  earlier terminal history. A complete resize-aware repair measured 207 lines, already 140 before the final
  documentation pass and formatting. The fallback corrects both specifications and accepts only this extreme recovery
  case; ordinary slow-viewer and flow-control guarantees remain.
- Forced termination can still signal an unrelated process if a terminal host exits and its process number is reused
  between worker-thread identity verification and the waiting task's signal. The filter records the maintainer's
  specific acceptance of that handoff, without changing the general rules for stored numbers or other asynchronous
  waits.

These are source-established conditional routes. No collision, harmful input, extreme resize loss or process-number
reuse was reproduced during execution. The filters carry the 2026-11-04 acceptance date.

## Open questions and possible follow-ups

No further decision is needed for this plan. The four accepted cases remain possible. Fixing any of them beyond the
agreed small-change limit would be separate work.

## The PRs

- [#3020: identify the registry test's own host](https://github.com/scode/farhelm/pull/3020/changes), draft.
- [#3022: accept interrupted terminal-start collisions](https://github.com/scode/farhelm/pull/3022/changes), draft.
- [#3026: accept the in-progress input handoff race](https://github.com/scode/farhelm/pull/3026/changes), draft.
- [#3030: accept extreme resize-only history loss and correct its specifications](https://github.com/scode/farhelm/pull/3030/changes),
  draft.
- [#3033: accept process-number reuse in the signal handoff](https://github.com/scode/farhelm/pull/3033/changes), draft.

## Checks run, reused and skipped

- Ran `cargo nextest run -p farhelm --test terminal_host_registry` through the recorder with pinned nextest, four global
  slots, zero retries and no tmux dependency: one test passed, none skipped, real hosts exercised. Run
  `cf2e5346-10c4-4dac-8003-548f04fa6511` covered the working test change on `42faf91b`, source fingerprint
  `d19a11e13ec5a34060a2a3b7a73788a530ac3d555f5eb0b51ff01c0d362479af`.
- Ran targeted `cargo clippy -p farhelm --test terminal_host_registry -- -D warnings`, Rust formatting for the changed
  test, dprint for changed Markdown, the isolated test-sleep checker and the instruction-reference checker: passed. The
  sleep check found 412 delays with no missing rationale; the instruction check had zero errors and three pre-existing
  citation warnings.
- The abandoned input-join candidate failed compilation because its test borrowed a non-Sync receiver across an await;
  its all-target lint failed for the same reason. No runtime cases executed. Evidence remains under recorder run
  `6ef13ace-0df8-497b-964b-796489fa6b40`; that candidate and its fragment were removed, so no pass or input-race fix is
  claimed. The private resize proposal had formatting and application checks only.
- Reused the registry run, targeted lint, source formatting and sleep-check evidence after a careful rebase onto
  `397c3bd0`: every PR's interdiff was empty. The full upstream audit found only unrelated fixtures, CSS, planning and
  bookkeeping changes, with no registry, client, host-spawn or test-support dependency change. The final combined scope
  adds no interaction beyond the individually reviewed test and documentation. Fresh dprint and instruction-reference
  checks passed on the rebased files; no new runtime run was warranted.
- Skipped broader runtime suites, browser and desktop checks, installer/provisioning checks and CI: the final executable
  change is confined to one test target, and the other four PRs change documentation and review bookkeeping only.

## Review gate's outcome

Each PR passed its single fresh-context gpt-6.1-sol review at high effort. The input candidate's missing detach paths
were accepted as findings and led to the authorized fallback. The resize review found a product-specification conflict;
the same narrow exception now appears in both specifications. No finding was rejected and no unresolved finding remains
in the final scopes. Commit messages and PR text passed their independent cold reads.
