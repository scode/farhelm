### What this was about

Farhelm reads the permission footer that Claude, Grok, Muse and Cursor draw under their prompt to tell whether a session
runs in YOLO mode, the mode that bypasses approval prompts. The opt-in drift tests, which drive the real installed
agents to catch vendor changes, checked that footer only for YOLO launches. They checked it only through the session
list's YOLO flag, which says "not YOLO" both when Farhelm recognized a normal-mode footer and when it could not read the
footer at all. A vendor rewording its normal-mode footer would leave Farhelm unable to read it, and no test would
notice. Your TODO entry asked the drift tests to check both YOLO and Guarded launches for Claude, Grok and Muse, and
YOLO (`--force`) launches for Cursor, and to catch vendor wording drift. The plan made that precise: a recognized
non-YOLO footer must be told apart from an unreadable one. You decided to extend the existing `permission_modes`
category rather than add one. You also asked for as much shared code as possible, and, if a part turned out
disproportionately complicated, for it to be left out and recorded as a follow-up.

What was built, in one PR:

- **Guarded launches.** The `permission_modes` drift category now also starts Claude, Grok and Muse with their Guarded
  flags, the ones the launcher uses. Their live footer must read as a recognized non-YOLO footer. A footer Farhelm
  cannot read now fails the check instead of passing as "not YOLO".
- **YOLO launches.** These must show a footer Farhelm reads as YOLO, and the session list must then report YOLO, as
  before.
- **One list for the tests.** The supervisor now states, next to its footer reader, which agents it reads a footer for
  and whether it recognizes a non-YOLO footer. The tests of the captured footer screens and the drift category both
  follow that statement, which replaces the two separate lists they kept. It is still written by hand beside the reader
  and must be kept in step with it. The desktop app's session row keeps its own separate copy for its YOLO shield, which
  this change did not touch.
- **Docs.** The drift tests' documentation says this, and the TODO.md entry is removed.

Nothing changes for anyone running Farhelm: the change is test-only and adds no release note.

### Things you should know

- **The new check already caught Claude drift.** Claude Code 2.1.332, started Guarded (`--permission-mode auto`), now
  draws its footer as "⏵⏵ auto mode on (shift+tab to cycle)". Farhelm's reader still expects the older "⏸ auto mode on",
  so it reads the footer as unreadable, and the live Claude case fails on its Guarded launch (retained run a5f380f2).
  That is the drift this check exists to catch. Until the reader learns the new spelling, the `permission_modes`
  category will keep failing for Claude, by design. Nothing visible changes for users today: Farhelm acts only on a YOLO
  reading, and Claude's YOLO footer is still read correctly. Updating the reader needs a newly captured footer screen
  and a change to the supervisor's reader, both outside this test-only plan, so it was not done here.
- **Cursor's footer check has not run live.** The live Cursor case (Cursor 2026.11.22) stopped before any footer check,
  on both its YOLO and Custom launches. Cursor showed its "Workspace Trust Required" dialog, which the drift tests
  recognize but do not answer. Its trust choice is highlighted with a ▶ marker, which the shared trust-answering code
  does not recognize as a selection, and the dialog also accepts an `[a]` key, which the code does not press (retained
  run f738a598). This is not new: the footer screens captured on 2026-10-15 already show that dialog, and this change
  touched neither the trust answering nor Cursor's launch. Until the trust answering handles Cursor, Cursor's YOLO
  footer is checked only by code and unit tests, not against the live agent.
- **Grok and Muse passed live** on both kinds of launch, including the new Guarded ones: Grok 1.0.92 and Muse Code
  1.4.25.
- **Agents were not upgraded.** The live run used `--no-upgrade`, as the plan required, so it tested the versions
  already installed on this machine.

### Open questions and possible follow-ups

- Teach the supervisor's Claude footer reader the "⏵⏵ auto mode on" spelling, with a newly captured footer screen, so
  the Claude case passes again. Say if you want this as a Plan follow-ups entry.
- Let the drift tests answer Cursor's trust dialog, either by recognizing its ▶ selection marker or by pressing its
  `[a]` key, so every drift category that launches Cursor gets past it, the footer check included. Say if you want this
  as a Plan follow-ups entry.

### The PRs

- #5169 `test: check Guarded permission footers in the drift tests`

### Checks

- The supervisor's footer and screen-reader tests and the drift category's unit tests, through the test recorder
  (`cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/agent_kind::status_line_fixtures::/) | test(/agent_kind::screen_reader::/) | test(/agent_kind::screen_fixtures::/) | test(/^real_agent_capture::drift::/)'`):
  passed before review (run 8ef20e8a) and after review (run 95b8f516). The final edits, a timeout message that also says
  whether a YOLO reading was ever seen, and comment fixes, were checked with the footer fixture and drift unit tests
  alone: 47 passed (run 8c81dd68).
- New tests seen failing as intended: with the Guarded launch dropped and Muse wrongly listed as having no non-YOLO
  reader, both the captured-footer test and the drift category's new unit test failed (run 263ed0a5).
- The live drift run
  (`python3 -B scripts/run-harness-drift.py --no-upgrade --category permission_modes --harness claude --harness grok --harness muse --harness cursor`),
  before review:
  - Grok PASSED (run 865ebdf1) and Muse PASSED (run 1bbe3fe6).
  - Claude FAILED on its Guarded launch, on the drift described above (run a5f380f2).
  - Cursor FAILED at its trust dialog, as described above (run f738a598).

  Review's changes made the YOLO wait accept a YOLO footer seen once; the Guarded check is unchanged. The live run was
  not repeated, since its verdicts do not depend on that change.
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, the test-sleep check
  (`python -B scripts/check-test-sleeps.py`: 312 delays, none lacking a rationale), and `dprint check` on the docs page
  and TODO.md: clean.

Skipped or limited:

- `cargo clippy -p farhelm --tests` reports one error, an unused function in the helm's notification code, which this
  work does not touch. It reports nothing in this change's files.
- The drift runner's own tests: the runner script did not change.
- Every other test: only test code and a test-only hook in the supervisor changed.

### Review gate

A fresh-context Opus 5.5 agent at high effort reviewed the PR, then verified the fixes; the gate passed. It found no
bug: no check can pass on stale evidence or hang, and the new test-only hook in the supervisor reads the screen exactly
as the supervisor's periodic screen check does. Its findings were applied, except two it marked optional, which were
declined: failing a check as soon as the footer shows the opposite mode (rather than at the 15-second timeout), and
merging the footer reader with the statement of which agents it reads, so the two cannot disagree. The applied ones:

- the YOLO wait accepts a YOLO footer seen once, so a footer briefly covered by a dialog does not fail a correct launch;
- the tests' questions about whether an agent has a footer reader are written so that a new kind of reader must be
  placed deliberately, as the code rules ask;
- the captured-footer test requires a YOLO screen for every agent with a reader, and no longer keeps a hand-written
  list;
- assertions that could never fail were removed, and the docs wording corrected.

It agreed that the Claude reader and Cursor's trust answering stay out of this PR, and confirmed Cursor's trust dialog
predates it.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the command line's parsing changes (cli-flag-parsing-audit, #5163
and #5165), which the drift tests do not meet: the only `farhelm` command their sessions run is the hook command the
agents call, whose free-text option still accepts a value that starts with a dash. In the same round, before this plan,
terminal-tab-shortcuts landed (#5164, #5166, desktop app only) and drift-base-isolation-and-turns (#5167, #5168), which
changes the drift tests' shared trust answering (below).

An independent reviewer and I read the change and found nothing to fix. The supervisor's new statement of which agents
have a footer reader is built only into tests, never into Farhelm itself, and answers each agent explicitly with no
catch-all. The reading the drift tests take from a live screen is the one the supervisor's own sampler takes. The
rewritten test of the captured footer screens still checks every screen, and fails when an agent with a reader has no
screens, lacks either kind it needs, or has screens but no reader.

#### Fixes made while landing

- **A TODO.md conflict.** The round's plans each removed their own TODO entries, and one added a follow-up entry, so
  stacking them conflicted there. I kept main's TODO.md with each plan's own TODO change applied; no code changed.

#### What the landing made untrue in the report

- **Cursor's trust dialog may no longer stop the Cursor case.** The report says the live Cursor case stopped at Cursor's
  "Workspace Trust Required" dialog, because the shared trust answering does not recognize its ▶ selection marker. The
  other drift plan landed in this round taught the shared trust answering exactly that: Cursor's boxed trust dialog with
  its ▶ marker, and the answered box left on screen. So the report's second follow-up may already be done. The two
  changes have not run live together, so Cursor's footer check is still unverified against the real agent. In the other
  plan's live runs Cursor got past the dialog and then refused the turn for lack of usage on the account.

#### Worth knowing

- **The Claude case will fail until the supervisor learns Claude's new footer.** As the report says, Claude Code 2.1.332
  started Guarded draws "⏵⏵ auto mode on", which Farhelm cannot read yet. There is no TODO entry for it; the report's
  first follow-up asks whether you want one.

#### Checks

Run once, on the round's three plans stacked on main as it stood when the landing started (terminal-tab-shortcuts, then
drift-base-isolation-and-turns, then drift-permission-footer), so each was tested with the others and with the earlier
changes above. The desktop plan got the desktop app's tests, screenshots and end-to-end smoke test. The drift plans'
real-agent cases are opt-in and spend vendor turns, so they were built and listed, not run, and the ordinary tests of
the drift code and of the supervisor's footer reading ran instead. The SSH test worker was unreachable, so the smoke ran
on a boat sandbox (an x86_64 Ubuntu 24.04 VM), and the rest ran locally:

- `cargo clippy --all-targets`, and `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent`
  (the build with the desktop app's test-automation hooks): each reported one warning and nothing else. That warning is
  the unused ntfy settings method in the helm, on main since #5120 and outside this round, which makes `-D warnings`
  fail, so these ran without it. `cargo clippy -p farhelm-gui --all-targets -- -D warnings` (the desktop app's GUI
  crate, which does not depend on the helm's code): clean.
- The GUI crate's whole default test suite (`cargo nextest run -p farhelm-gui`), recorded run
  790d4ebc-9b6a-46d4-b81b-12cdbc9a08f6: 3,603 passed, none failed, and it skipped its 4 opt-in tests, among them the
  screenshot test the next item runs.
- The full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run 9be720dc-7a79-40ae-b77a-eec843eafb24:
  passed.
- On a boat, the desktop app's end-to-end smoke test (`scripts/gui-e2e-smoke.py`), recorded run
  94e567ef-c5b4-4bbd-9a83-c0bac7dd24de: 48 of its 49 steps passed, and the paste step was skipped, as on every Linux
  run. Its steps include Delete from a session's menu; none presses Cmd+T or Cmd+W.
- The supervisor's per-agent tests, the footer reader and its captured screens among them
  (`cargo nextest run -p farhelm-supervisor --lib -E 'test(/agent_kind::/)'`), recorded run
  7c2a5d10-f11a-46a9-a5d5-7ff23f798176: 112 passed, none failed.
- The ordinary tests of the real-agent test code, without the opt-in cases
  (`cargo nextest run -p farhelm --test e2e -E 'test(/real_agent_capture/)'`), recorded run
  56b688c2-75da-476f-baad-df4c39ca17a7: 50 passed, none failed, and no test reported that it skipped for a missing
  system service. Listing the opt-in drift cases
  (`cargo nextest list -p farhelm --test e2e --run-ignored only -E 'test(/real_agent_capture::drift::/)'`) built them
  and found 71.
- The docs website build (`bun run build` in `website/`): built. `python3 scripts/check-agents-md.py` for the drift
  tests' docs page, which the instruction files point to: passed.
- `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown: passed.

Reused: nothing. Not rerun: the drift plans' live runs against the real agents, described in their reports; the landing
ran none, since they spend vendor turns, so the drift plans have not run live together. Skipped: the rest of the Rust
workspace's tests, since nothing else changed; the other opt-in tests; and any run on a Mac, since none was available to
the landing. No Mac ran Cmd+T or Cmd+W: the Mac's rule is covered only by the tests that pass the platform in.
