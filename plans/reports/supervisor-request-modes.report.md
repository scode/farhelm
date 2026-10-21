## What this was about

Starting a session or checking a fresh GitHub checkout could wait behind another session operation and stop typing in
every terminal using that host connection. The plan also addressed the recurring cause: each supervisor request chose
its execution and cancellation behavior separately, without a complete declaration or a test enforcing reader progress.

The five drafts first give connection channel IDs one owner, then declare every control-message variant's execution
contract, then test those contracts under contention. Separate fixes move creation and checkout reconciliation onto
supervisor-owned tasks. Later terminal frames keep moving while admitted work waits, and disconnecting loses the reply
without cancelling accepted creation, refusal or recovery. Both completed TODO entries are removed.

## Things you should know

A host at its management limit now refuses a well-shaped create or reconciliation promptly with its existing busy
message. Cheap malformed-request checks still win over that refusal. An admitted request still waits for intent and
directory ownership; directory contention alone is not a refusal. Keyed durability, replay, recovery approval, lock
order, protected size checks and final replies retain their existing contracts. Independent later requests can now
finish before creation or reconciliation, which is the intended consequence of freeing the reader.

The inventory covered all 66 control-message variants and the handshake, connection pre-dispatch path and both
authority-selected dispatchers. The historical second create path no longer exists: session-authenticated direct Create
already returns Unauthorized, while agent requests go through the helm. That refusal and the restricted relay's
credential validation remain unchanged. Tab attach already refuses a contested lifecycle claim without waiting.

The declaration has 33 inbound representatives and 33 explicit non-request variants. Its final contention test covers
all 31 requests promising reader progress, with no known-stall exceptions. Checkout preview remains deliberately inline
under the specification's slow-host allowance; the dedicated session-authenticated agent relay retains its intentional
serial round trip. Channel allocation stays inline so later input, resize, credit and abort frames see its route.

The channel table preserves ID history, collision precedence, upload tombstones and teardown order. Creation and
reconciliation preserve genuine panic payloads across their new task boundary with a bounded, capacity-one handoff to
the connection reader. Existing handlers' panic policy and permit lifetimes were not standardized or otherwise changed.

The entire intervening main diff was read before rebasing onto `5437cae1`. Launch-sequence and focus changes, their
tests/docs, release tooling, instruction checks and plan bookkeeping do not alter the supervisor paths or control
variants here. The archive-trust specification change documents existing archive cleanup, which this stack does not
change. All five rebased per-PR interdiffs were empty; upstream TODO removals are preserved.

## Open questions and possible follow-ups

No maintainer decision or new product follow-up remains for this plan. The evidence is focused Linux supervisor and wire
coverage; it is not a native macOS or full browser-suite verdict. No live installation was changed. The drafts remain
unmerged for the landing monitor.

## The PRs

- [#2446](https://github.com/scode/farhelm/pull/2446/changes): connection channel ownership; head
  `a26c4fc29be706876b8355ac39f16f25219aa773`, based on main.
- [#2449](https://github.com/scode/farhelm/pull/2449/changes): exhaustive execution contracts; head
  `56302f92f4ea179daba1f01b156af07ea45fcbb5`, based on #2446.
- [#2451](https://github.com/scode/farhelm/pull/2451/changes): real-route contention enforcement; head
  `8231e7cf9f9fd82a90d6e6f52520d5bf5c5d2ce9`, based on #2449.
- [#2456](https://github.com/scode/farhelm/pull/2456/changes): creation ownership and reader progress; head
  `8501adc3d7640378a61ab1441436833caa356ba9`, based on #2451.
- [#2457](https://github.com/scode/farhelm/pull/2457/changes): checkout reconciliation ownership and reader progress;
  head `624087bf3d74daa2160c82e0ce3c9a67dd427c18`, based on #2456.

## Checks run, reused and skipped

All runtime commands used `python3 scripts/record-test-run.py`, nextest 0.9.148, four slots, zero retries and verified
tmux 3.7c. The commands below use `cargo nextest run -p farhelm-supervisor --lib -E '<focused selection>'`; the
selection is described for each run. Executed output, source identity, reports and cleanup were inspected. Successful
runs have complete reports and stream EOF, no runtime `SKIPPED` marker, no output truncation and no forced-cleanup
limitation. Nextest's unselected cases are not claimed as execution coverage.

- Channel table: six policy cases passed in `dfeae86a-a157-49fa-85d0-0e31e40371f4`; six wire/transfer ordering cases
  passed in `0fc0a27a-dc20-4cbc-90a4-5e69b87c602f`. The policy cases preceded a field-order-only correction; their
  policy bodies are unchanged, and the wire run used the corrected order. These cover the first draft's code, originally
  submitted as `d5487eb0`.
- Declaration construction and exhaustive representatives: one case passed in `50f92054-cf29-4d26-ac28-d5ed37c8693c`,
  covering the second draft's code, originally `f9a707a6`.
- Contention fixtures and matrix: seven cases passed in `a4438c50-575b-42e4-8ad1-ed17000966e4`, covering the third
  draft's code, originally `2216fb39`. The matrix then passed three finite repetitions overlapping an owned four-worker
  CPU load: `e1285b8d-ffa8-445d-b98b-e311e1c0e565`, `1cc3b413-da73-4cbc-97cf-95d1c0051b6b` and
  `b970d2a5-c58f-40c1-b8ea-9575d17d125a`. The repetition command was
  `python3 scripts/hunt-rust-tests.py --repeat 3 --timeout 60 --execute -- cargo nextest run -p farhelm-supervisor --lib -E '<matrix>'`.
- Creation: five new ownership, panic, validation-order and matrix cases passed in
  `d901c070-434a-471b-a5de-0a1102a58ad6`; fourteen existing cap, refusal, durability, replay and authority cases passed
  in `d3268356-3cb3-41e7-8997-1b0284244d71`. Both captured the fourth draft's frozen source, submitted as `4638ae2d`.
  The passing panic regression deliberately emits a panic sentinel; it is not a failed case.
- Reconciliation: four new progress, cancellation and refusal-precedence cases plus the final complete matrix passed in
  `f76e5f1b-2dc5-4f1e-864d-7bc7c42e593d`. Ten existing identity, durable refusal, recovery approval and ambiguous
  launch-evidence cases passed in `d06e3799-8dda-4ff6-8be7-544ad843e372`. Both captured the fifth draft's frozen source,
  submitted as `624ab6a4`; the final matrix passed with no exceptions.

Two development attempts failed compilation before any test ran: `685059c6-e9dc-4649-a0c0-8bc75b7f0940` used a private
intent-lock field, and `8162828e-6123-408a-a1b8-6e1f8a2020fb` returned the lock without the Arc its claim method needs.
The test-only accessor corrected both. Their evidence is retained; neither is a test pass or an established flake.

Each review unit passed supervisor Clippy (`cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`), Rust
formatting and the isolated `python -B scripts/check-test-sleeps.py` check. Markdown formatting and
`python3 releasing/check-changelog.py format` passed for the bookkeeping and fix fragments. Final rebased formatting,
fragment and delay checks passed again; 305 delays had reasons. No instruction file, executable example or Markdown
heading was changed by this stack.

The per-command runtime and Clippy evidence is reused after the careful rebase: all five per-PR interdiffs are empty,
and no intervening change alters the tested supervisor paths or their protocol contracts. No whole-workspace, desktop,
installer, provisioning, JavaScript or browser battery was added: the real connection tests, lock-wait tests and durable
state regressions cover the identified risks without changing those other subsystems. CI is manual-only and was not
dispatched.

## The review gate's outcome

Every PR received exactly two independent fresh-context gpt-6-astra/high reviews: general adversarial correctness,
design and idiomatic code, and a separate review limited to observable behavior. All gates are clear. The latter
confirmed behavior preservation for the three structural/test drafts and the precise targeted-fix exception for the two
fix drafts, including bounded busy refusal and accepted work surviving disconnect.

The conformance reviewer found that the initial channel-control cases exercised unknown routes. That finding was
accepted: the final cases establish real attached terminals, pending uploads, credit-blocked downloads and live relays
before sending their controls. The same reviewers accepted the correction. No review finding was rejected or left open.
Separate scope assessments kept the declaration descriptive and the new panic handoff bounded. Documentation passes
covered every touched file, and all commit/PR wording passed fresh cold reads.

### Landing

Landed as #2446, #2449, #2451, #2456 and #2457, merged bottom-up, with the same content the report describes, rebased
onto this round's main. It was the third of four plans in this landing round, after structured-settings-dialog and
launcher-enter-feedback.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 109's six plans, the browser suite's move to three stacks run in parallel, a
TLS certificate fix, documentation and plan bookkeeping. A fresh-context reviewer and I read them and the other plans of
this round against this plan. Round 109 changed no supervisor or protocol code, so it added no request kinds, and the
plan's table of every control message still covers them all. Round 108's tab-attach-no-wait was already in this plan's
base, and the declared contract for tab attachment matches it: a contested tab is refused without waiting.
helm-owned-by-default, later in this round, keeps Stop and tab operations cancellable on the helm side while this plan
has the supervisor own an accepted Stop; the two agree, since a helm caller that disconnects only loses the reply. Once
the conflicts below it in the stack were resolved, this plan's own commits applied cleanly; its removal of its two
TODO.md entries is kept.

#### Fixes made during the landing

None.

#### Things to know

- As the report says, when every one of a host's management slots is taken, a new create or checkout reconciliation is
  now refused at once with the existing "try again" message instead of waiting. (The review named concurrent stops or
  deletes as one way that happens.)

#### Possible follow-ups

- The terminal backpressure test with an open deflake entry now fails nearly every time on main on this machine (nine
  runs in ten alone; see Checks), against one in five during round 109. That is too often to tell whether this plan
  changes it, and the deflake entry or FLAKES.md should be updated with the new rate.

#### Checks

Every plan in this round was executed before round 109's six plans landed, so none had been tested against them, and
while this round ran, main also moved the browser suite onto three stacks run in parallel (each test worker runs its own
helm and supervisors). The plans were stacked again onto that main before the checks that count, so the browser evidence
below comes from the harness they land on. Together the four change how the supervisor runs every request, which helm
actions finish after a client disconnects, the Settings dialog and the launcher's Enter key; that reach is why this
round ran the whole Rust workspace and the full browser suite. All of the following ran on all four plans stacked
together on that main, including the fixes made while landing them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `scripts/check-desktop-assets.sh`, and the website build (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `fa6052db-ebdf-4204-8d20-25b8780367ec` (kept): 3977 passed, 1 failed:
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, which has an open TODO deflake entry ("Shallow
  pause resets under load"). Because supervisor-request-modes changes how the supervisor reads messages from a host
  connection, I ran that test alone ten times on main without this round and ten times on this round's stack, minutes
  apart on the same machine: 9 and 10 failures. Both sides fail almost every time, so that comparison cannot show
  whether the plan changes the test; it does show the test is now close to always failing on main on this machine, where
  round 109 saw it fail one time in five. See the possible follow-up in supervisor-request-modes' notes.
- `cargo nextest run -p farhelm-ui --features desktop`, run `d8ff6545-9b27-45f8-aa33-7dfac8f07108` (kept): 724 passed, 1
  failed: a timing assertion in the tests of how the UI orders preference saves, code no plan here touches. Ten
  repetitions alone all passed, so I judge it machine load during the full run; on one observation it is not recorded as
  a flake. `node --test` in `crates/farhelm-ui/js-tests`, run `b88528dc-6d84-4d63-996d-56eb15c69510`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers, no retries, run
  `ac3bef42-b713-4f70-8e6e-0fd7b86e0a16` (kept): 2227 passed, 14 skipped by design (WebKit clipboard cases and tests
  that need a real signed-in agent or run on one engine only), 3 failed, all on WebKit:
  - a terminal test and a two-host test each saw a session marked as having new output where they expected a plain
    status. Neither test file is changed by this round, and three repeats of each on both engines then passed (run
    `8158eb60-714f-4b18-bf7f-51c8654840b8`), so I judge both timing under load with three stacks running; on one
    observation neither is recorded as a flake;
  - "the sidebar app bar explains the helm and window builds", which fails inside the browser tests' own way of faking
    the helm's build stamp: a faked reply arrives after its request was already settled. It fails on main without this
    round too, two runs in five (run `db0c82af-6a1e-4359-8724-7fa17c65dbba`), and first showed up on round 109's landed
    code, so it is not from this round; see the possible follow-up in structured-settings-dialog's notes.
- Before the parallel suite landed, the plans also ran on the earlier main. A narrow run there (run
  `87ee3a27-84d7-4a4e-ab9f-8d478f5f4b35`) found the Settings test failure fixed in structured-settings-dialog, and a
  full browser pass, stopped part-way when the parallel change landed (run `b8b3e14f-b202-4015-b7d9-1ef15efb820d`),
  showed the app-bar test above and a launcher search test fixed in launcher-enter-feedback.
- Skipped: the workspace and desktop doctests (no plan adds or changes a documentation example),
  `cargo check -p farhelm-desktop` and the desktop smoke test (the desktop app's own crate is unchanged), the native Mac
  app (not runnable here) and the installer (unchanged).

The report's checks ran on a main from before round 109's landings and were focused on the supervisor; its note that the
evidence is not a full browser-suite verdict is superseded, since the landing ran the full browser suite above. Nothing
else in the executor's report became untrue at landing.
