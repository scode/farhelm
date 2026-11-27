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
