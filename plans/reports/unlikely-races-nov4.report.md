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

### Landing

Landed with no fixes at landing. The terminal-host registry test now signals only the host behind its own socket, and
four rare terminal races are recorded as accepted cases; no product behavior changed.

- #3020: the registry test identifies its host by its exact socket before signalling it.
- #3022, #3026, #3030, #3033: the accepted cases (an interrupted start colliding on a socket name, an old input task
  delivering Enter after a takeover, extreme resize-only history loss, process-number reuse in the signal handoff).

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the printed-link fix and its two documentation PRs (#3018, #3019, #3021) and the
v0.35.0 release-notes illustrations (#3038); none touches terminal hosts or the specification sections this plan edits.
In this round it landed first, below durability-revocations, delete-unreadable-terminals and triage-misc-nov4.
delete-unreadable-terminals also changes terminal-host recovery code, and triage-misc-nov4 amends the same part of the
specification (its paste-after-gap caveat sits near this plan's resize exception); a fresh-context review read the four
plans against each other and found no contradiction: this plan's accepted cases describe starts, input handoff, resizes
and signal handoff, none of which the other plans change.

#### Checks

This round landed four plans, stacked in queue order on main `ab98921b`: unlikely-races-nov4, durability-revocations,
delete-unreadable-terminals and triage-misc-nov4. The checks ran once, on the whole stack. While they ran, two
browser-test fixes landed on main: #3042 (the browser suite's setup now waits for each stack's startup session) and
#3043 (a quick-launch test drains host replies before cleanup), each with its flake bookkeeping. No plan in the round
touches a browser test, TODO.md, FLAKES.md or the flake exclusion list, so the runs below still describe what landed.

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 412 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the workspace debug build and the UI's release web build: all passed.
- The whole Rust workspace with the pinned tmux, `cargo nextest run --workspace --exclude farhelm-desktop`, run
  `f07cd604-0561-4944-82ec-d22ceee500d1`: 5604 passed, 70 skipped (ignored by design), 1 failed. The failure is
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, a recorded flake that sweeps already exclude.
  The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `ffb56e46-5d09-4c86-957e-82c35d390db5`: all 858 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `f02122ed-7f9b-4c1d-bbe2-b54636673478`: all 1047 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack), runs `f684cd58-3a29-47db-83c0-9ed452c461c1`, `dffb760d-dbf5-475f-aead-c23f786551bd`,
  `57cccb94-ac0c-47c4-b617-ab5f3bd8a485`, `d80fd8f4-1646-4bae-8eb5-ad7c1b832f03`,
  `b618b816-61d8-4e2f-9c6c-5e17f5e6320e`, `c2b42d58-9c6c-4a82-92c2-b275559a0cfe`,
  `4b38a57a-3ad1-4220-916f-cd8c59370431`, `89515c90-b088-427f-8460-c2ce26e92697`,
  `3ac6a8a3-606c-4d49-9a47-ed284c005719`, `1422993b-e223-4dd2-855f-68ebbb953be9`: 2996 passed, 18 skipped by the specs'
  own design, 6 failed. Then the same suite with every stack's terminals in terminal hosts, since the supervisor's
  terminal listing and recovery changed (`FARHELM_E2E_SUITE_TERMINALS=host`; this run covers only the specs listed for
  host terminals in `e2e/host-terminal-specs.ts`, so it holds fewer tests), runs `dd43611c-53e6-440e-9b4d-b067b5f8ecaf`,
  `c9f23f58-6955-4127-be9d-23b7d6247d3e`, `c4d61d6b-3b1a-46bf-9705-dbac5d651a28`,
  `ed839b79-675c-4b1d-8c41-e857f88ddc96`, `2e48a313-928b-45e7-9cb5-6cf8df812cb0`,
  `dd213786-0132-4825-b5f4-a3f3923c6659`, `182db21b-bab2-49f2-9b28-7896032965b4`,
  `1cc9cda2-2faa-4a2e-b5a4-a9a458636715`, `7d969e49-d11a-47bb-ab2d-03c4413f218e`,
  `5ef8c251-2019-4f99-8173-78b65f89eb86`: 2081 passed, 75 skipped by the specs' own design, 4 failed. Of the ten
  failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `f684cd58`,
    `d80fd8f4`, `ed839b79`, `1cc9cda2`), because the Add host form's "SSH" and "Tensorlake sandbox" choices have no
    hover text. Those choices came with the Tensorlake sandbox hosts (#2973), on main before every plan's base, and the
    previous round saw the same four failures; no plan in this round touches the form. It is a break on main and a
    follow-up there.
  - Recorded flakes, each once: the feedback dialog's disabled Send color on WebKit, in "the dialog refuses blank and
    over-long messages without sending" (run `c2b42d58`); "trash header and global confirmation remain reachable at
    width 360" (Chromium, run `ed839b79`); and the launcher's "Runs in YOLO mode" step on WebKit, in
    "create-intent-key-rebinds-on-retarget: a moved host is a new intent" (run `ed839b79`).
  - Three single failures in the ordinary suite, each of which then passed three re-runs on the same build, also on
    sandboxes (the run ids below are the re-runs). "Agent managed checkout approval and keyed recovery" (WebKit; the
    test timed out at 60 seconds, then its cleanup failed because the page had closed; failed in `1422993b`) exercises
    the keyed retry of an agent's checkout request, which durability-revocations changed to key by the requesting host
    entry, so it was re-run on both engines: WebKit `0b58da4b-d9fe-4e02-9f25-38056211276a` and Chromium
    `e0863d93-7ea6-4601-a7f4-75fa98203b5c`, all six passed. "Settings keeps the pending Claude hint choice clickable
    through a held read" (WebKit; a 60-second timeout waiting for an intercepted save to finish, like the recorded
    pending-save flakes; failed in `c2b42d58`, re-run `b0de5934-d629-4547-886b-76e85605d844`). "Expanded setup failure
    closes the menu below it" (Chromium; the host actions panel did not finish measuring within 5 seconds; failed in
    `4b38a57a`, re-run `3a9117e4-f2ad-4000-95ed-18f8aa21a056`). Each is a single failure, not recorded as a flake.
- Not run: the doctests (no doc examples changed), the UI's JavaScript tests (no JavaScript changed), the grid
  component's browser suite (no grid code changed), the desktop smoke tests, the installer tests, the development
  screenshot review (no shot changed) and the native Mac app (not runnable on Linux), so the macOS-specific paths
  (delete-unreadable-terminals' cleanup on hosts without systemd containment) ran only through their Linux tests.

The executor's report calls the PRs drafts and names the main it was built on; what landed is its commits rebased onto
current main, with their content unchanged. Nothing else in it became untrue at landing.
