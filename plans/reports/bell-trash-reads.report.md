# Read Trash when cleanup changes

## What this was about

Every session status update made each open window re-read every connected host's Trash. Those unnecessary reads used the
same limited browsing capacity as the launcher's folder picker. The maintainer chose to follow each host's cleanup
changes instead.

## Things you should know

The helm now reports a per-host Trash change counter through its existing host listing. Accepted cleanup or Retry and
changed cleanup notices advance it. Session status updates and fallback polling leave idle Trash readers alone.
Reconnects, opening Trash, archived session deletions and active-removal polling keep their existing reads. The bell's
notice refresh still follows the fleet feed.

As agreed, a change made through a different helm is noticed only on reconnect, app load or opening Trash. One helm per
host is the supported model. No supervisor protocol, notice resolution rule, endpoint or push channel changed. The
implementation is 290 added-plus-deleted lines excluding the TODO removal and changelog fragment, within the size gate.

## Open questions and possible follow-ups

None identified within the agreed scope. Browser tests model another window's cleanup through changed host observations;
they do not open a second helm. Separate Rust tests verify the actual cleanup and notice handlers publish those changes.

## The PRs

[3315: avoid Trash reads on session status updates](https://github.com/scode/farhelm/pull/3315/changes), draft,
`plan/bell-trash-reads/01-epoch-trigger`, head `2c69afbeeba2f53b2fa7f76efcd67a576bb16a2d`.

## Checks run, reused and skipped

- Rust: `cargo nextest run -p farhelm-helm -p farhelm-ui --lib` with the notices, Trash policy and host wire-contract
  filter passed 25 tests. Run `3f9da63c-8195-4a98-9a8c-d3c0e495d26a`, nextest 0.9.148, four slots, zero retries, pinned
  tmux 3.7c. The 2,605 unexecuted cases comprise 2,603 expression-filter exclusions and two ignored-test exclusions. No
  executed case returned a runtime `SKIPPED`. Source was `5bd6d3ad` plus this change; report, output and cleanup were
  complete, with no retained-output loss or forced cleanup.
- Browser: `npx playwright test 'rail-notices.spec.ts|trash-dialog.spec.ts'` passed all 46 cases, 23 on Chromium and 23
  on WebKit, with zero retries or skips. Run `4fa6a3e3-0781-438d-992d-98c5ef71f54d`; three owned stacks used terminal
  hosts and finished cleanup. Coverage includes zero Trash requests after a session status change, host isolation,
  coalescing, notices, Retry and the existing Trash dialog behavior. Source was `182a133f` plus this change, before a
  test-comment wrap that changes no executable behavior. Reports and retained output were complete, without loss or
  forced cleanup.
- `cargo clippy -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`, CLI and web builds,
  `cargo fmt --all -- --check`, targeted `dprint check`, `python3 releasing/check-changelog.py format` and the
  test-delay checker passed. The latter inspected 429 delays with no missing rationale. The close-pruner's classifier
  also passed its one test, run `a4701d48-b4fc-4db2-a8d9-bdba76ca7d14`; this is helper validation, not product coverage.
- The first Rust attempt, `7524f5af-d142-4281-9183-4f1d91f45575`, timed out during compilation before any tests ran. A
  queued attempt, `d83de020-607b-4a6d-bb0b-8fc97b092d0b`, was interrupted before compilation to avoid the same limit.
  Both are retained and excluded from passing evidence. The successful retry used a longer build allowance. Initial
  compile errors from fixture fields were corrected before the successful checks.
- Reused the completed checks after careful rebases onto `636f5105`. The intervening changes add an unrelated TODO
  entry, adjust Tensorlake SSH identity options and host-progress hover behavior, and record plan landings. Their full
  diffs were inspected: no Trash reader, notice handler, cleanup dispatch or tested request-count mechanism changed. The
  only conflict combined adjacent TODO removals; both plans' completed removals were kept. Formatting passed again.
- Skipped the rest of the workspace and browser suites because the targeted checks cover the changed behavior and the
  retained cleanup paths. No cloud calls, live-install actions or hosted CI were needed or performed.

## Review gate's outcome

The single required fresh-context gpt-6.1-sol reviewer at high effort found no issues, applying the full test-authoring
charter. A separate documentation pass covered every touched file. The commit-message cold read passed after its first
reader identified an ambiguous title; the rewritten title and body conveyed the intended behavior and limitation.

### Landing

#### What landed with it

Nothing else landed in this round. While it was being checked, main gained only documentation and triage changes about
keyboard focus, none of which touches Trash, the notification bell or the host listing.

#### Fixes made while landing

A second, independent review of the diff found one gap, fixed before merging. Deleting a session can move its managed
checkout into Trash. Before this plan, every open window re-read Trash on the status update that followed, so its count
caught up. With this plan, another window caught up only when it saw the session disappear from its sidebar. A window
with a search filter, or with more sessions than the sidebar lists, cannot tell a session disappeared, so its Trash
count stayed stale until a reload, a reconnect or opening Trash. The helm now treats a session Delete or Replace that
archived a checkout as a Trash change on that host, like an accepted cleanup, so every window reads that host's Trash
again. The Delete test now checks this, and SPEC_impl.md says so. This corrects the report's statement that archiving
deletions keep their existing reads: that read covered only the window that made the deletion.

The same review noted two smaller things, left as they are:

- When you retry a stuck Trash cleanup and the host refuses it, nothing marks that host's Trash as changed. Normally the
  window that sent the Retry reads Trash again and updates the bell notice with the new failure, which then reaches the
  other windows. If that window closes first, the others keep showing the earlier failure's reason and time until a
  reload, a reconnect or opening Trash. The bell notice stays unresolved and does not alert again. A possible follow-up,
  though a small one.
- The helm never forgets the Trash change counter of a host that was removed. Each counter is a single number and host
  ids are not reused, so this is harmless.

#### Checks

Test runs went through the recorder with the pinned tmux first on `PATH`, and none of the runs printed a runtime
`SKIPPED` line. Browser runs were on Tensorlake sandboxes at the maintainer's request, split across several sandboxes;
the sandboxes run Node 24.16 against 24.20 locally, a difference no result pointed to.

Before the landing fix, on the plan's own tree:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`,
  `cargo clippy -p farhelm-ui -p farhelm-helm --all-targets -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop` passed, and `node --test` in `crates/farhelm-ui/js-tests` passed 563
  tests.
- `cargo nextest run -p farhelm-helm -p farhelm-ui --lib`: 2,628 passed (run `d790923c-5ad3-4400-a75c-94240665016c`).
- `npx playwright test --project=<engine>-<spec> --repeat-each=2 --retries=0` for every spec that reads Trash
  (rail-notices, trash-dialog, bulk-session-actions, github-checkouts, modal-layout, popover-header-movement,
  popover-width-movement, sidebar-resize and Chromium's interface-fonts): all passed except one WebKit run of
  bulk-session-actions' `bulk Stop reports an engaged row without skipping its eligible sibling` (run
  `c3da2ac1-a512-468f-91de-effdf3cc561b`). That is the open bulk Stop failure TODO.md already records from the previous
  landing, unrelated to Trash. Run ids: `9403db91-f90e-4942-b338-de68e79e1658`, `2c08bddd-d3ca-4903-8701-1c632e969907`,
  `c3bc4a79-8872-4b1d-bf73-3750cee94b33`, `c3da2ac1-a512-468f-91de-effdf3cc561b`,
  `69a8a114-766c-4496-9cac-fc047bb48471`, `71fc760d-a9b7-4144-9452-3a9b5868292e`, `b0ae732c-789b-448a-a74e-e7611185f338`
  and `df773595-5451-4808-8389-841415ad77c6`.

After the landing fix, on the tree that merged (main gained only documentation since):

- `cargo fmt --all -- --check`, `dprint check` and the same clippy passed.
- `cargo nextest run -p farhelm-helm --lib`: 1,676 passed (run `0e6870d5-94fa-4fa4-9844-0df58b633bc1`).
- The same browser specs once per engine, interface-fonts on both: 216 tests, all passed (runs
  `57f072fb-a1c5-45cc-bb1f-451cc94726f8`, `cda0b49f-327a-4d4d-a1dd-ffa6e0c0ca86`,
  `c8d51897-3032-4ad7-b420-42155b4162a8`, `3d78a326-7ddd-4b97-926f-1507f0eae622`,
  `b8551210-a5dc-4d6e-a3d5-7f0ca85e1f7b`, `6875493e-4f22-4da2-bdad-d726c5333aa2`,
  `35163a82-cfa1-4ec8-92a5-990ab1f2fa10`, `3c8bb778-6c89-4d2f-9245-5424f2899c37` and
  `c2b4c4f5-ed78-4985-a0a3-f11346b6b578`).
- terminal-link-clicks, which runs only against old (tmux) terminals, on both engines
  (`FARHELM_E2E_SUITE_TERMINALS=tmux`): 36 passed (run `ae319b7e-6921-4942-89b3-0d41009e341f`).

Reused: nothing from the executor. Skipped: the rest of the browser suite and the workspace's other Rust tests, because
the change reaches only the helm's host listing and Trash handlers and the UI's Trash readers; hosted CI (manual-only,
not dispatched).
