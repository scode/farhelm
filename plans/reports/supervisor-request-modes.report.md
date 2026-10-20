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
