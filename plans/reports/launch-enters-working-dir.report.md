# Launches enter the session folder

## Since the last delivery

PR #2363 is updated in place after the maintainer answered the landing block. The seven supervisor tests now keep their
intentional agent-launch failures while starting terminal tabs through the real helper. Directory-entry failures name
the folder; a missing agent program reports the program failure. Mac terminal tabs use the supervisor's own versioned
program for their helper.

## What this was about

When a session folder existed but could not be entered, tmux could silently start an agent or terminal shell in the home
folder. Farhelm still showed the selected folder, so unattended work could run somewhere the user had not chosen. The
maintainer required a launch to enter its session folder or fail clearly, and accepted ordinary symlink resolution.

Agent launches now enter that folder after checkout preparation and login-shell startup, and pass matching `PWD` to the
agent. Terminal tabs enter before starting their shell. The existing error paths report entry failure without starting
the requested program elsewhere.

## Things you should know

- An agent launch overrides a `cd` in shell startup files, as requested. A terminal tab retains ordinary shell behavior:
  its startup files may change directory after the shell starts in the session folder.
- Launch specifications written by older supervisors lack the optional directory field and keep their inherited
  directory behavior. New Create, retry, Restart and Replace attempts use the shared builder that fills it in. No
  supervisor protocol or database migration changes.
- The failure seen during landing was reproduced in one exact test, then all seven affected tests passed after repair.
  The earlier failed run remains failed evidence; the fresh passes are separate observations.
- Whether Mac privacy protection for Documents or Desktop routinely triggers the original problem remains unverified.
  This execution ran on Linux; no live installation was changed.

## Open questions and possible follow-ups

None required for this plan. Mac privacy behavior can be checked separately if it becomes a reported trigger.

## The PRs

- [#2363](https://github.com/scode/farhelm/pull/2363/changes), `fix: enforce the session folder at launch`, updates the
  existing draft with the original fix and the answered landing corrections.

## Checks run, reused and skipped

Fresh recorded runtime checks use pinned tmux 3.7c and nextest 0.9.148 with four runner slots and no retries.

- Reproduction: `cargo nextest run -p farhelm-supervisor --lib -E '<exact opened-tab scope test>'`, run
  `a0df684c-97b3-4d32-9134-b5bb821fa9d1`, failed because its configured placeholder could not execute the tab helper.
  That was before this round's repair.
- `cargo nextest run -p farhelm-supervisor --lib -E '<seven repaired tab fixtures and launch-helper contracts>'`, run
  `fb8ba128-147f-45fd-a73f-d8e40253e10e`: 17 passed, including every formerly failing test. Complete JUnit and retained
  output; no runtime substrate skip. A subsequent fixture-comment correction changes no tested behavior.
- `cargo nextest run -p farhelm --test e2e -E '<Create, startup-order, error and tab-containment cases>'`, run
  `6fa74a7a-cefe-4dd1-96ca-089d43dd5202`: nine product tests passed, plus the separate fixture-build setup. The
  permission-denial and systemd containment cases executed; no runtime skip, output truncation or incomplete report.
- `cargo nextest run -p farhelm-supervisor --lib -E '<shared builder, scope-prefix and versioned-program contracts>'`,
  run `4fc9a952-1661-44ad-a1f4-d0da5147966f`: four passed, complete report and output, no runtime skip.
- Both `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings` passed.
- Rust format, dprint for the changed Markdown, changelog format, agent-instruction consistency, and the test-sleep
  checker passed. The delay checker inspected 304 calls with no missing rationale.

The successful runtime commands cover 30 product tests. Final PR head `5244adda` preserves their executable behavior;
subsequent edits only clarified comments and specification wording, so no further runtime repetition was needed.

No previous runtime pass is needed as evidence for this delivery. The earlier failed landing run
`acae51f5-18dc-4dc0-af71-f9bda72e0a19` and the pre-repair reproduction remain recorded in the working log. The whole
supervisor or workspace battery was not repeated: the exact affected fixtures and launch boundaries provide the relevant
coverage. Browser, desktop, installer and provisioning suites were skipped because this change adds no behavior at those
boundaries. No manual Mac privacy test was run.

## Review gate

A fresh-context gpt-6.1-sol reviewer at high effort read the whole updated PR, the latest decision and the full test
authoring checklist. It reported no findings. The independent resume check also confirmed the existing PR and prior
work, and its complete upstream-diff audit found no additional functional conflict. No substantial departure from the
plan or new protocol was needed.

### Landing

Landed as #2363 in its own landing round, after the maintainer answered the block from the previous landing (fix the
seven tests and the error wording; the Mac detail at the executor's discretion). The landed change is what the report
describes, plus one documentation fix made at landing (below).

#### What landed since the stack was based, and whether it interacted

The redelivery was based on a main that already held everything from the previous landing round, including
supervisor-colon-path-refusal (the supervisor refuses to start when its program folder contains a colon) and
workspace-trust-destination (trust follows the destination folder's managed-checkout identity). Since then main gained
only plan bookkeeping. A fresh-context reviewer read the redelivery against those two and against the block's three
findings:

- The seven broken supervisor tests now give terminal tabs a small stand-in program that runs the real tab helper, while
  agent launches in those tests still fail on purpose as before. The stand-in sits in a test folder without a colon, so
  the colon refusal does not affect it.
- A launch error now names the session folder only when entering the folder failed; a missing agent program is reported
  as a program failure. Two new tests check both directions.
- Mac terminal tabs now start through the supervisor's own versioned program rather than the app's forwarder.
- The trust change only decides whether a folder counts as a managed checkout, and the folder trust settles on is the
  folder the launch then enters, so the two do not conflict.

The reviewer also checked that no other supervisor, end-to-end or browser test starts tabs with a placeholder program;
they all use the real built program.

#### Fixes made during the landing

- SPEC_impl.md's section on side-by-side versions in the Mac app, and the matching comment in the supervisor, still said
  that only the agent launch helper uses the supervisor's own versioned program. Both now name the terminal-tab helper
  too, and why: it only enters the tab's folder and then becomes the shell, so it never outlives the supervisor that
  started it. Documentation only; folded into #2363's commit.

#### Possible follow-ups

- One existing test that checks Restart keeps an open tab running (it checks that the tab's process survives) is not one
  of the seven and was not edited, but its stand-in program just sleeps, and tabs now start through that program, so it
  no longer exercises the real tab helper and shell. It still checks what it says; having its stand-in run the tab
  helper would restore the original coverage.
- A tab that cannot enter its folder is reported only if the tab dies within the existing 250 ms check when the tab
  opens. The helper is now the full Farhelm program, so on a heavily loaded host it may take longer than that to fail;
  the tab would then open and be removed without the error being shown. Nothing ever runs in the wrong folder. The same
  window already existed for a shell that dies at start. Found by reading the code.

#### Checks

Run now, on the redelivered PR rebased onto main (the documentation fix came after these runs and changes no code):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed file, and `python3 releasing/check-changelog.py format`: all passed;
  `dprint check SPEC_impl.md` and `cargo fmt --all -- --check` passed again after the documentation fix.
- Through the test recorder with pinned nextest and tmux, four slots and no retries: the whole protocol, helm,
  supervisor and UI libraries (including the seven tests that failed at the previous landing), the `farhelm` binary's
  tests, the agent command-line tests and the end-to-end tests for launches, terminal tabs, tab lifecycle edges, restart
  with resume, supervisor stop, managed checkouts, structured launches, session lifecycle, agent relay, agent listing
  and wrapper launches: 3528 passed, run `34aee641-ab76-4d76-9854-1da67a296fe2`, with no runtime `SKIPPED` marker. The
  UI's desktop-only Rust tests: 706 passed, run `e724c44c-b194-48d9-9f00-f14eefa38040`. `node --test` in the UI's
  JavaScript test directory: 311 passed, run `dcdeb76c-0181-4ae4-9834-c386fa782f88`.
- After `cargo build` and the release web build,
  `npx playwright test terminal-tabs.spec.ts terminal-restart.spec.ts spawn.spec.ts replace.spec.ts restart-with.spec.ts terminal.spec.ts`
  through the recorder on Chromium and WebKit, one worker, no retries: 330 passed and 2 skipped, run
  `c47b092e-b4bc-48f9-9cd0-f7203bac5b39`. The two skipped are the real-agent spawn test on each engine, which needs
  vendor credentials and is skipped by design.
- Skipped: the native Mac app (where the tab helper change applies) and physical permission failures on macOS, as in the
  report.

Nothing in the executor's report became untrue at landing.
