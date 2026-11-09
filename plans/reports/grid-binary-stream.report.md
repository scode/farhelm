# Compact terminal transfers and scrollback

## Since the last review

The maintainer accepted essentially unchanged typical memory after the earlier block. No PR was added or dropped. All
three drafts were rebased onto main at 1cf6e79a; their product changes are identical to the earlier reviewed stack. The
last PR, #3255, now removes the addressed TODO and records the accepted exception in its comparison and benchmark guide.
The original measurements, failed attempts and correctness requirements are preserved.

## What this was about

Opening or reconnecting to a new terminal used JSON for its screen and scrollback, and the page kept every styled
scrollback run decoded. A full history with many different colors could consume hundreds of MiB. The stack replaces the
screen/history wire format and the retained history together. Rows are decoded when drawing, copying or looking up words
and links; they are still fully validated before entering history. Hover and delayed link answers keep a stable line
identity across fresh reads. Existing drawing, input and history limits stay in place.

The opt-in benchmark was committed first. Its six-case baseline ran before any encoding change. The final comparison
alternated main and candidate three times each, with a fresh browser for each workload: 36 completed observations at
12,000 history lines, 200 columns and 60 screen rows. The committed comparison retains every raw sample, source/input
hashes and measurement limits.

## Things you should know

Typical Chromium memory did not clearly improve: settled resident-growth medians were 30,617,600 and 30,457,856 bytes,
only 0.52% apart, with overlapping ranges. The original requirement for clearly lower memory in all six cases was not
met. On 2026-11-09 the maintainer explicitly accepted typical memory as essentially unchanged: the goal is cutting the
extremes. The measured indexed-color and true-color reductions, smaller transfers and faster attach times satisfy that
decision. No further typical-case storage experiment was added. The addressed TODO entry is removed under that decision.

These are Linux x86-64 measurements with Chromium 156.0.8078.4, WebKit 27.2, Playwright 1.64.0 and Node 26.13.0. They
are not Mac desktop measurements. Resident growth follows a garbage-collection request and fixed settling observation;
it includes native allocations and allocator retention. Peak is the content process's lifetime highwater at history
drain minus its pre-attach resident reading. No sample retained its pre-attach highwater as its maximum. The shared
machine was loaded; no build or other browser test from this run overlapped the comparison.

Memory below is MiB; attach bytes are exact payload bytes. Each row is the median of three independent observations.

### Main

| Engine   | Output        | Resident growth | Peak above baseline | Attach bytes | Attach ms |
| -------- | ------------- | --------------: | ------------------: | -----------: | --------: |
| Chromium | Typical       |           29.20 |               45.77 |      3374337 |     388.4 |
| Chromium | Indexed color |          185.29 |              464.82 |     53416455 |    7629.3 |
| Chromium | True color    |          313.78 |              612.01 |     66544524 |   13475.8 |
| WebKit   | Typical       |           63.15 |               66.71 |      3374337 |     521.0 |
| WebKit   | Indexed color |          727.05 |              726.73 |     53416455 |    4506.0 |
| WebKit   | True color    |          896.21 |              953.18 |     66544524 |   10469.0 |

### Candidate

| Engine   | Output        | Resident growth | Peak above baseline | Attach bytes | Attach ms |
| -------- | ------------- | --------------: | ------------------: | -----------: | --------: |
| Chromium | Typical       |           29.05 |               40.08 |      2836127 |     257.1 |
| Chromium | Indexed color |          101.71 |              191.42 |     31459345 |    1270.7 |
| Chromium | True color    |          100.33 |              195.80 |     31459345 |    1248.6 |
| WebKit   | Typical       |           59.68 |               62.21 |      2836127 |     363.0 |
| WebKit   | Indexed color |          265.86 |              348.27 |     31459345 |    2637.0 |
| WebKit   | True color    |          296.90 |              341.43 |     31459345 |    3294.0 |

All six attach-size and attach-time medians improve. The first storage candidate still allocated temporary strings
during validation and missed memory/time targets. Profiling identified that cost; validating printable ASCII directly
from bytes removed it while preserving the full UTF-8 fallback. A bounded attempt to remove permanent byte-view wrappers
from stored lines also failed to establish a typical Chromium memory improvement. Workloads and accounting were kept
unchanged; the exception is an explicit decision, not a benchmark adjustment.

The wire change requires matching helm/supervisor protocol versions. The draft uses protocol 71. The separate transfer
chunk-size plan also uses 71 in its unlanded draft; whichever lands second must take the next free number, as both goal
files allow. Terminal-host persistence is unchanged. The existing transfer targets and pause/resume thresholds stay.

## Open questions and possible follow-ups

No unresolved decision remains for this plan. Compression of older history remains outside its scope; these results do
not establish whether it would be worthwhile. Typical-case storage experiments were explicitly stopped by the decision.
The separate page-kept back-buffer TODO remains separate and unchanged.

## The PRs

- [#3245: opt-in benchmark](https://github.com/scode/farhelm/pull/3245/changes).
- [#3252: binary screen and history transfers](https://github.com/scode/farhelm/pull/3252/changes).
- [#3255: compact retained scrollback and measured comparison](https://github.com/scode/farhelm/pull/3255/changes).

## Checks run, reused and skipped

This resumed round integrated main's scoped-cleanup and per-host checkout-folder changes. The incoming audit covered all
52 paths and 148 hunks. The rebase needed no conflict resolution; each PR's before/after patch is unchanged before the
three-file completion amendment. Exact source comparisons and SHA-256 readings confirm that the measured assets,
benchmark driver, workloads, codec, producer and their dependencies are unchanged. The earlier observations below are
reused for those contracts, not described as new executions of the integrated revision.

All runtime attempts used the test recorder. The substrate was Linux x86-64, Rust 1.99.1, pinned nextest 0.9.148 and
tmux 3.7c where required, with zero retries. Completed nextest selections used four global slots; browser selections
used both engines and three workers. Counts exclude selected-out or ignored tests and do not treat runtime skips as
substrate evidence.

- Integration refresh:
  `cargo nextest run -p farhelm --test e2e -E 'test(a_scope_launched_stop_kills_through_the_cgroup_without_the_marker_sweep) | test(a_scoped_stop_reaps_jobs_in_a_session_started_stock_tmux_server)'`,
  recorder 27b56787-13b9-479f-8499-5ac0b9b80b5a at integrated revision f17edaa4, two tests passed plus the fixture-build
  setup case (three JUnit passes); 545 cases were selected out. Complete source/report/output and cleanup, no runtime
  skip, systemd scope premises exercised, stock tmux 3.4 observed. The first case attaches through the binary grid and
  distinguishes scope-only Stop from a marker sweep. The second exercises the stock-tmux fallback with real host
  sessions; it does not itself attach a grid. This selection rebuilds the integrated Rust dependencies.
- Final browser comparison: six serial recorder runs, each completing all six engine/workload cases. Main/candidate
  pairs: 0f188160-9c78-46f1-b27d-e3183fb1b81a / 12484986-7323-4788-b626-4afea16cb8fe;
  67b10264-1870-41e4-920c-a4a81936b3d2 / f019e04e-84f4-4385-ac55-5c0ecb6ea3aa; 5d8390e6-b0d3-45cf-bd17-36da5b84e947 /
  db337122-6fae-4f25-8dd1-b7805656c5c2. The driver SHA-256 is
  a3a19ca1bb57dc3599ab4f5dd7762a3b01fb718a000128536fdd1d0b6af3d8c1. Main assets came from b80071ca; candidate product
  assets from 51f49e0f. The final source retains those exact measured assets. These static-page runs establish no live
  helm or SSH coverage.
- Initial before-change baseline: ebe5dbfc-0025-497f-a2ea-d405cd8b3ecd at benchmark revision 9e0dc93c, six cases. Actual
  Rust workload generation: 78ec16eb-e8be-45bc-af3b-101b6153e428 for protocol 70 and
  2fb6405b-c46b-4dd4-83ca-c726eba240d7 for protocol 71. Reused inputs are byte-identified; intervening main changes
  leave their generator, workloads and producer dependencies unchanged.
- `node --test` in the UI asset unit harness: 04a0ebf6-0fa3-4399-8294-9ed20592eeed, 547 passed after the final storage
  correction at 51f49e0f. Covers binary validation, independent Rust fixtures, exact retained ownership, cap/reset
  release, fresh row reads and stable identity. No runtime skip.
- `npx playwright test --config grid-client/playwright.config.ts`: 4bd1cdbb-cceb-40ee-bcdc-20dc958a3a8b, 204 passed on
  both engines after the final product correction. Includes selection/copy, links, deferred history validation,
  hover/reset identity and the incoming fitting/handoff behavior. No runtime skip.
- `npx playwright test '(grid-(terminal|flood)|terminal(-links|-files|-switch-geometry)?)\.spec\.ts'`:
  09a8d7fb-c80a-4e27-9f6b-5f82858ba80e, 178 passed (89 per engine), three owned local/localhost-SSH stacks creating
  actual terminal-host sessions. Complete report, no skips/global errors and complete cleanup. Matching pinned web and
  CLI builds passed before execution. This is not real cloud/provisioning coverage.
- `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-helm --lib -E 'package(farhelm-proto) | test(/^grid_output::/) | test(/^service::grid_stream::/) | test(/^terminal::tests::/) | test(/^client::tests::/)'`:
  cf39117c-dc44-40d9-9ca0-b59778021dfa, 395 passed on the binary-encoding candidate later published as b88cd936. No
  runtime skip; report and cleanup complete. Storage-only JavaScript changes leave these code paths unchanged.
- Six real grid attachment/tab/restart cases selected from `cargo nextest run -p farhelm --test e2e`:
  f0508a9f-3512-4630-8f17-9ae92f1ea712, six passed plus the build-fixtures setup case. Covers host attach, helm/browser
  grid transport, host tabs across supervisor restart and agent restart preserving history. Pinned tmux and actual
  runtime premises ran; no runtime skip. These remain useful attach/history evidence; they do not prove the newly scoped
  cleanup fast path, which is why the focused integration refresh was selected.
- Rust fixture generation: 37270cdf-0008-487d-8aa0-5f07eba0b52f, one explicit ignored writer generated 30 valid and 47
  malformed binary pairs through the production Rust encoder, with mutation premises checked. The final Node harness
  checks their expected descriptions and the test-only writer's byte equality.

Earlier development failures remain retained: two benchmark process-attribution attempts failed before measurement
(49f3beb0-0452-4e7a-974e-0c0fcb6ddc0c and 2a0d8076-0422-45e0-9b00-8aaa499c6a42); their confirmed collapsed-process-title
cause was corrected, with the focused run cbd8e469-04f2-42de-a089-aedc9bd07dea passing. The first binary Node run
cdec4143-1bb1-4a43-a43a-0dc073ba0077 passed 536 of 537 and caught two omitted fixture-account names. The corrected
fixture selection e4848980-03cc-446b-9df4-a10e02ffa386 passed all nine; the later full 547-case run passed. Later passes
do not erase those observations. These were same-session harness/accounting corrections, not established latent flakes.

The final bookkeeping amendment changes ordinary Markdown and TODO only. `dprint check` on those three files,
`python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format` passed; the instruction checker
reported its existing unrelated TRIAGE heading warning. No runtime test is warranted by that amendment. The earlier
sleep checker inspected all 430 test files with zero missing annotations; this round changes no tests, so that source
check is reused for this plan's unchanged tests. Rust formatting, proto/supervisor Clippy and matching application
builds passed in the earlier round. They are not claimed as a fresh build or full lint of incoming main; the focused
Rust refresh supplies the useful combined compilation evidence.

No full workspace/browser battery, CI dispatch, desktop installation, release or live-install action is part of this
validation: the unchanged client evidence and focused lifecycle refresh cover the identified interactions. The benchmark
establishes no native Mac result. No additional performance experiment was run after the maintainer's decision.

## Review gate's outcome

Each implementation PR passed its required single fresh-context gpt-6.1-sol/high adversarial gate. Storage corrections
and the final measurement/provenance report were reviewed by the same storage reviewer, with no outstanding findings.
The same reviewer also checked the resumed three-file completion amendment against the accepted Decision and the clean
rebase: no actionable findings. Commit/PR wording passed cold readers. The three drafts were pushed and independently
verified open, draft and correctly based at heads 18ab54fd (#3245), 1690bc88 (#3252) and 2dd41cf7 (#3255). The report's
required cold read checked decision context and public-repository hygiene before delivery. No PR was marked ready or
merged by the executor.
