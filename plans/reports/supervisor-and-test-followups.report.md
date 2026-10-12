## What this was about

An exited terminal tab could freeze status updates for every session on its host while stubborn background processes
went through cleanup. The maintainer approved moving that cleanup out of the status pass while retaining the existing
session exclusion. Two accepted test findings concerned a hook test falsely failing release builds and opt-in real
Claude tests launching the ordinary fake CLI. A fourth finding concerned a systemd startup probe disabling protection
against dollar-sign expansion after a transient error; its decision allowed only a few-line fix, otherwise discard.

Three fixes were implemented in one draft PR. The systemd item followed its authorized discard path. All four settled
Outcomes remain unchanged, their Execution fields record completion or the gate discard, and their feedback files and
index entries were removed.

## Things you should know

Automatic tab cleanup now starts owned work and lets status sampling continue. Each pass starts at most four closes. A
close retains the existing session lifecycle claim, which prevents overlapping closes and defers sibling tabs until a
later pass. A user's explicit Close still waits for its final result. Stopping the ticker prevents new automatic starts;
closes already started continue until completion or runtime shutdown, as existing explicit closes do after their caller
disconnects. Planned output cleanup shares the close's attachment lock and cleanup barriers and stops new sink
admission. This change does not promise that process teardown finishes before runtime exit.

The held-stdin hook test still runs in debug. Release discovery reports it ignored with a reason because its short
budget override exists only in debug; the production budget and test observer were not weakened.

With `FARHELM_REAL_AGENT=1`, only the fake Claude wrapper is omitted. Ordinary runs retain all wrappers. The maintainer
must make the real CLI usable in the stack's private login home, including login and configuration; the repository adds
no credential handling. Other structured specs selecting Claude also reach the real CLI in this mode, so opt-in runs
must select only the two real-agent tests with a title filter. Both tests retain their explicit opt-in and loud skips.

The tests are “drives a real `claude` session through the trust dialog to a detected reply” in `real-agent.spec.ts` and
“a real Claude creates a jj workspace and spawns into it without refreshing the observer” in `spawn.spec.ts`. After
preparing the real CLI login in the private test home, select exactly those cases from `e2e/` with
`FARHELM_REAL_AGENT=1 npx playwright test real-agent.spec.ts spawn.spec.ts -g 'drives a real|a real Claude creates'`.

The systemd probe issue remains unfixed. When a session launch path or argument contains a dollar-sign reference, a
transient startup-probe error followed by a successful unflagged retry can leave later launches failing or expanding
that reference instead of preserving the intended literal text, until the supervisor restarts. This trigger and a
wrong-target launch were not reproduced. The approved decision requires evidence from the actual unknown-option error,
but the existing probe discards that stderr. Reliable classification needs capturing that evidence or adding a bounded
probe, beyond the authorized few-line change. The planner's help-text proposal would also provide weaker evidence than
the ledger requires. The item was therefore discarded per the maintainer's decision, without a probe code change.

## Open questions and possible follow-ups

No maintainer decision is required for this delivery. Reopening the systemd issue would require authorizing a broader
probe change; it is not represented as fixed. The real-Claude path was checked by source reading, not by spending a
vendor turn. Its next manual run still depends on the maintainer's login setup.

## PRs

- [#1933 — keep status fresh during tab cleanup](https://github.com/scode/farhelm/pull/1933/changes), draft; one change
  `rymmyqmq`, bookmark `plan/supervisor-and-test-followups/01-followups`.

## Checks run, reused and skipped

- Run: eight selected supervisor tab-reap regressions passed with pinned nextest and tmux, four slots and zero retries,
  recorder `30b4afae-aee7-4597-b334-ae3788804b59`. Coverage includes fresh sampling during held teardown, duplicate
  exclusion, a waiting explicit close, four-start admission across five sessions, dead siblings, pane-death wakeup, busy
  claims, daemon cleanup, marker exclusion and dead-agent handling. No selected test reported a runtime substrate skip;
  unselected tests are not counted as passes.
- Run: the exact debug held-stdin hook test passed, recorder `bee3e257-fe66-4e2b-9f57-8ea7b6668bd0`, four nextest slots
  and zero retries on pinned tmux. Its required fixture build succeeded. Release behavior was checked from the matching
  `debug_assertions` conditions and preservation of test attributes, without a release build.
- Run: Rust formatting, changed Markdown formatting, shell syntax and ShellCheck, isolated source-only sleep checker
  (291 delays, zero missing rationales), changelog format lint and exact queue inventory parity; passed. The private
  bookkeeping helper passed formatting, type checks and its ledger-drift and wrapped-index tests.
- Run: `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings`; passed. These cover affected supervisor and CLI targets and the
  shipped CLI configuration without test seams.
- Reused after careful rebase from `66dc1a19` onto `82e95f34`: the two intervening commits only delivered a plan report
  and recorded its landing claim. Every diff was read; no code, specification or lifecycle contract changed. The source
  behavior covered by the recorded ticker and hook runs remains intact. Documentation refinements removed an unused
  import and kept a scope-marker rationale adjacent to its operation.
- Skipped: broad Rust suites and full-workspace Clippy beyond affected targets; focused runtime tests and
  affected-target lints cover the changed Rust behavior. Scope-probe tests were unnecessary because that code was not
  changed. No release build was warranted for one conditional ignore attribute. Browser runtime and opt-in vendor tests
  were not run: the wrapper condition is exact and preserves ordinary installation by reading, while opt-in needs
  maintainer credentials and spends a real turn. Desktop, installer, provisioning and website paths were unaffected.

## Review gate outcome

The required fresh gpt-6.1-sol high reviewer found no actionable source findings after reading the full diff, plan,
ledger and relevant specification and lifecycle contracts. The full test-authoring checklist was supplied verbatim. The
executor read and reconciled the complete artifact. A separate documentation pass inspected every touched file,
including changed tests and private helpers. Two fresh gpt-6.1-sol medium wording readers checked the commit and PR
prose; the revised message's paraphrase matched intent and its claims and conventions held. Implementation and
validation stayed with the executor; no decomposition work was delegated.

The required inherited-model report reader accepted the revised account as sufficient for a maintainer decision and
found no public-repository hygiene violation. The executor read its complete findings and addressed its requests for
exact manual test selection and product context for the remaining systemd issue.

Private galaxy-brain session evidence reuses `29405ad6-d9d8-4919-8d82-c1a7afa9b9f2`. Native tools do not expose
independently reported model attribution, effort or token usage. Those remain evidence gaps, including omitted dedicated
wording launch events recorded as a gap; no cost claim is made.
