## What this was about

Four triage outcomes covered an incomplete template approval card, host-controlled text in debug logs, a terminal
recovery limit and an old-template finding. You chose to show the whole resulting template, escape all peer-supplied
text the helm logs or retains, name the accepted shell-paste consequence after a recovery gap, and discard the finding
about wrappers saved by older releases.

## Things you should know

Template approval cards now show the container image, proxy port and GitHub token-file path, or an explicit "none" when
the write removes the container setting. The card shows the path without reading the credential. This repairs the
template card only; the launch card is outside the agreed outcome. The shipped command-line flags do not send container
edits, so silently changing isolation through the original omission required a hand-built protocol request.

When terminal attachment debug tracing is enabled, a host-supplied tab name is bounded and escaped before either the
start or end event logs it. The ordinary agent-terminal field remains unchanged for the desktop smoke check. This trace
is off by default. The specification now states that the escaping rule covers all peer-supplied text the helm logs or
retains, including identifiers, rather than errors alone. No new validation framework was added.

The terminal recovery-gap caveat now explicitly says that a multi-line paste into a shell may run line by line rather
than arrive as one block when lost output leaves the terminal's input modes wrong. This names the consequence you
accepted; it adds no recovery behavior or mitigation.

The old-wrapper finding was removed under your decision to leave old templates unchanged. Wrappers saved by 0.30–0.32
can still make a newly supplied prompt be parsed as shell syntax. Current wrapper editing behavior is unchanged. This
outcome is a discard, not a repair.

The two code outcomes stayed within their size gates: 68 and 43 changed source, test and documentation lines, excluding
the ledger, feedback removal and changelog fragments. Each of the four outcomes records its change ID, bookmark and PR
URL, and removes its own feedback file and index entry. There is no schema or protocol change.

## Open questions and possible follow-ups

None. The remaining paste-after-gap and old-wrapper behavior above are the recorded decisions, not unfinished fixes.

## The PRs

- [#3035 — show container settings on template approvals](https://github.com/scode/farhelm/pull/3035/changes), draft.
- [#3036 — escape tab names in attachment logs](https://github.com/scode/farhelm/pull/3036/changes), draft.
- [#3037 — name the paste risk after a recovery gap](https://github.com/scode/farhelm/pull/3037/changes), draft.
- [#3039 — retire the old-wrapper prompt finding](https://github.com/scode/farhelm/pull/3039/changes), draft.

## Checks run, reused and skipped

The runtime commands use the test recorder with pinned nextest 0.9.148, four global slots, zero retries and tmux marked
unused. These units do not need a tmux server.

- `cargo nextest run -p farhelm-ui --lib -E 'test(approvals::tests::a_template_)'`, recorder
  `32812d33-4d3f-4510-8b7a-3869f1108eec`: five passed, including configured container values, no token file, removal and
  omission, plus existing template card cases. The 853 other tests were outside the selection. Recorder and child exited
  zero with complete output and JUnit; no runtime substrate skip occurred.
- `cargo nextest run -p farhelm-helm --lib -E 'test(/^terminal::tests::/) | test(/^client::tests::/)'`, recorder
  `0f5d61b2-9a40-437b-85b4-cd1cf8f5dda7`: all 103 passed, including exact escaped tab fields on both trace events and
  the existing attachment integration that checks the plain agent field. The 1,385 other tests were outside the
  selection. Recorder and child exited zero with complete output and JUnit; no runtime substrate skip occurred.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- `cargo fmt --all -- --check`, dprint on changed Markdown and `python3 releasing/check-changelog.py format` passed. The
  isolated pinned interpreter's `python -B scripts/check-test-sleeps.py` inspected 412 delays with no missing reasons.
  `python3 scripts/check-agents-md.py` reported zero errors and three existing unrelated reference warnings.
- Reused across ledger-URL amendments and the later prose-only outcomes: the card run's source on base `7b826e40`,
  fingerprint `262f1d3965b08e4fef5e57fc48cadecb35491d7cc33ac10c2d2f813e08f8954a`, and the reviewed UI lint result. The
  tested card code is unchanged. The helm run covers the PR2 working source on base `5bd1a1f1`, fingerprint
  `f89cbd591a772a09090830938687c5218c7e077dce7ec2cdf9ba5300a78155fb`; later changes only add ledger URLs and prose, so
  its executable coverage still applies. Upstream changes through `9949406d` contain only queue state, a different
  plan's answer and a report; they change no code, specification or dependency used here, so no rebase or repeated
  runtime check was warranted.
- Skipped: the full Rust workspace, browser suites, desktop runtime and desktop smoke. The executable changes are
  confined to card data rows using the existing renderer and the debug trace's tab-name field. Focused card tests and
  actual trace-event assertions cover those changes; the existing relay integration checks the unchanged plain agent
  field. No CSS, browser interaction or desktop lifecycle changed. The two prose/bookkeeping outcomes need no runtime
  tests. No hostile-host exploitation or real shell recovery experiment was performed.

## Review gate's outcome

Each PR passed its required single fresh-context gpt-6.1-sol review at high effort, with no findings. The changed code
and tests received a separate documentation pass. All commit and PR wording passed fresh cold reads; the discard message
was clarified to name the actual prompt-handling concern. No scope departure or unresolved review finding remains. The
four PRs are open drafts in the required linear order; nothing was marked ready or merged.

### Landing

Landed with no fixes at landing. Template approval cards now show a template's container settings, host-supplied tab
names are escaped in the attachment debug trace, the shell-paste risk after a recovery gap is named in the
specification, and the old-wrapper finding is discarded as decided.

- #3035: the template approval card shows the container image, proxy port and token-file path, or "none" when removed.
- #3036: tab names are bounded and escaped in the attachment debug trace.
- #3037, #3039: the paste-after-gap caveat in the specification, and the discarded old-wrapper finding.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the printed-link fix and its documentation (#3018, #3019, #3021) and the v0.35.0
release-notes illustrations (#3038); #3018 changes the terminal grid's link handling, not the recovery-gap caveat this
plan amends. In this round it landed last, after unlikely-races-nov4, durability-revocations and
delete-unreadable-terminals. durability-revocations changes the helm's agent requests and store, but not the shape of
what the approval card reads; the fresh-context review found no interaction with it or the others.

#### Things you should know

- The approval card does not show the container engine (Podman or Docker), while SPEC.md says the card shows the whole
  resulting template. Adding it is a small follow-up.
- The specification sentence #3036 added says all peer-supplied text the helm logs or retains is escaped. That is
  broader than what this plan checked, which is the attachment trace's tab name; whether every other log site meets it
  was not verified.

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
current main, with their content unchanged; only the surrounding lines of the review queue's index differ, because the
plans below it removed neighboring entries. Nothing else in it became untrue at landing.
