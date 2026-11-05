## What this was about

Five review findings concerned what Farhelm can forget after an OS crash, power loss or failed storage write. Revoked
permissions could return after their revocation was acknowledged. Separately, if the helm's host registry rolled back
and reused a host number, a newly registered host could complete the earlier host's retained, approved launch with the
same request and retry key.

You chose to protect trust while accepting loss of recent history. This plan strengthens permission revocations and
binds saved retry approvals to the requesting host entry. It also documents three existing limits you accepted, rather
than adding recovery machinery for them.

## Things you should know

- Changes to host command, YOLO, public-link and local-network-link permissions now use the existing stronger database
  durability barrier. Changes or clearing of tunnel consent use it too, so acknowledged revocations get the same
  protection as browser-token rotation. Unrelated preference writes keep their ordinary durability. This reuses the
  existing database helper; there is no schema change or separate store.
- Retry approvals now follow the host entry's random identity, so a number reused after registry rollback carries no
  earlier entry's approval. Requests already in flight across this upgrade ask for approval again. If approved, that
  retry creates a new session or checkout instead of returning the earlier one, as accepted in the plan.
- On macOS, even the stronger database writes are durable only up to the drive's cache; SQLite does not request
  `F_FULLFSYNC`. The specification now states this limit for both the supervisor and helm. You accepted it, judging the
  extra protection marginal and expecting most Macs running Farhelm to have batteries.
- The specification now explicitly accepts forgetting a conversation Farhelm learned in the last moments before an OS
  crash or power loss, like other recent state. Restart may then have no conversation to resume or may resume the
  previously recorded one. This clarifies existing behavior and leaves ordinary supervisor-outage recovery unchanged.
- A full disk now explicitly falls under the accepted unhealthy-filesystem outcomes. An existing Resume offer can be
  lost if Restart with fails and the write restoring that offer also fails. This is a specification clarification; it
  adds no code change or new recovery promise, and keeps the boundary protecting the helm and other hosts.

The two code fixes stayed within the agreed size gates: 100 and 96 changed code, test and specification lines,
respectively, excluding ledger updates, queue removals and changelog fragments. All five outcomes record their execution
and remove their matching feedback items.

## Open questions and possible follow-ups

None. No product decision remains unresolved; the three documented limits above are the accepted decisions.

## The PRs

- [#3023 — protect permission revocations from power loss](https://github.com/scode/farhelm/pull/3023/changes), draft.
- [#3025 — keep retry approvals with the requesting host](https://github.com/scode/farhelm/pull/3025/changes), draft.
- [#3028 — qualify macOS database durability](https://github.com/scode/farhelm/pull/3028/changes), draft.
- [#3031 — document recent conversation loss after power failure](https://github.com/scode/farhelm/pull/3031/changes),
  draft.
- [#3034 — document full-disk loss of a Resume offer](https://github.com/scode/farhelm/pull/3034/changes), draft.

## Checks run, reused and skipped

Both runtime commands ran through `scripts/record-test-run.py` in development nextest mode, with pinned nextest 0.9.148,
four global slots and zero retries. Together they passed 126 tests. Their retained source records, JUnit reports and
output are complete, with output EOF and no omitted output, failures or runtime `SKIPPED` notices.

- `cargo nextest run -p farhelm-supervisor -p farhelm-helm --lib -E 'test(db::tests::) | test(store::tests::trust_revocations_) | test(store::tests::browser_permissions_) | test(store::tests::hosts_start_asking_) | test(store::tests::rotation_) | test(store::tests::preferences_)'`,
  recorder `7ef912e4-d021-46a6-b061-36cb3073dce3`: 13 passed. These cover the existing database durability helpers,
  host-permission behavior, token rotation and the new regression checking all five revocations, consent clearing and
  ordinary preference durability. No separate `preferences_`-named test matched; broader preference-unit coverage is not
  claimed. Tmux was marked unused because these selected database units do not need it. The tests observe SQLite's sync
  setting inside the real writes and its restoration afterward; they do not simulate physical power loss.
- `cargo nextest run -p farhelm-helm --lib -E 'test(agent_requests::tests::)'`, recorder
  `1fad4b6e-8cc4-4e08-9b03-fc803211ba20`: all 113 agent-request tests passed, including equal-number/distinct-entry
  retry separation and both creating request paths. Required pinned tmux 3.7c was present and matched the recorded
  substrate. The regression models reused numbers with real stores; it does not reproduce a physical power cut.
- `cargo clippy -p farhelm-supervisor -p farhelm-helm --all-targets -- -D warnings` passed for the first fix;
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed for the retry-key fix.
  `cargo fmt --all -- --check`, dprint on changed Markdown and `python3 releasing/check-changelog.py format` passed. The
  isolated pinned interpreter's `python -B scripts/check-test-sleeps.py` check inspected 412 delays, with zero missing
  reasons. `python3 scripts/check-agents-md.py` reported zero errors and three pre-existing reference warnings.
- Reused after later specification and ledger-URL amendments: the 13-case run's reviewed code on base `f67868fb` (source
  fingerprint `d5d0e55c4e2a015eb3fe40efc7d55dca2ddb84738423b1ef14ce5825cf0adb39`) and the 113-case run's reviewed code
  on base `87987f01` (source fingerprint `52da4b9a56e8247bec557e8715649d7233f4789e14c5104894cd02423a5753a5`). Those
  amendments changed no tested code. The stack was carefully rebased onto `397c3bd0`. The complete upstream diff through
  `7b826e40` contained unrelated test fixtures, CSS, planning and bookkeeping changes; none changes the storage or
  request-key dependencies examined here. The only conflicts were adjacent independent removals from the feedback index.
  Each resolved PR removes only its own entry, retaining the already-landed removals. The three changed source files are
  byte-identical across the rebase, and per-PR comparisons found no other patch changes. Fresh dprint checks passed on
  the rebased Markdown; the existing runtime, lint, Rust formatting and test-delay results still cover the final
  executable changes, so no additional runtime run was warranted.
- Skipped: the full Rust workspace, browser suites and desktop runtime suites. The executable changes are confined to
  database write durability and retry-approval ownership, which the focused runs cover; no UI behavior, layout or
  desktop lifecycle changed. The three specification-only units received dprint checks without additional runtime runs.
  No physical power-loss or Mac drive-cache experiment was performed.

## Review gate's outcome

Each of the five PRs passed its required single fresh-context gpt-6.1-sol review at high effort, with no findings. The
changed code, tests and specification text received separate documentation passes. All commit and PR wording passed
fresh cold reads. The small adapter around the existing durability helper also passed a separate scope check; it
introduces no new persistence mechanism.

### Landing

Landed with no fixes at landing. Revoking a host's command, YOLO or link permissions, or its tunnel consent, now
survives power loss (on macOS only as far as the drive's cache, as stated below); saved retry approvals stay with the
host entry that asked; and the specification states three accepted limits (macOS drive caches, a conversation learned
just before a crash, a full disk).

- #3023: permission and tunnel-consent revocations use the database's stronger durability barrier.
- #3025: retry approvals are keyed by the requesting host entry's identity, not its number.
- #3028, #3031, #3034: the macOS flush, resume-after-power-loss and full-disk statements in the specification.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the printed-link fix and its documentation (#3018, #3019, #3021) and the v0.35.0
release-notes illustrations (#3038); none touches the helm's store, agent requests or the durability sections. In this
round it landed second, after unlikely-races-nov4 and before delete-unreadable-terminals and triage-misc-nov4. The
fresh-context review checked it against them: triage-misc-nov4 changes the helm's attachment logging and the template
approval card, not the store or retry keys, and no plan in the round changes a database schema. The review also
confirmed that every place these permissions and tunnel consent are revoked takes the stronger barrier.

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
