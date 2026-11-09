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

### Landing

Landed alone in its round. Since its stack was based, main had gained the rail notification bell, smaller bulk-transfer
chunks (mux-chunk-size, which raised the protocol version to 71), a queue-picking rule for plan executors, and planning
bookkeeping. A fresh-context review read the plan against all of that and found nothing that must change before merging,
and two things worth fixing at landing (below). The steadier terminal sizing on session switch was already in the plan's
base. This plan's change to the grid component only touches how a link hover recognizes its lines, so the fix that keeps
text from shifting on a session switch is intact. Grid messages do not travel through the transfer path mux-chunk-size
shrank: they still go one message per frame, the supervisor packs rows into messages of about 32 KiB (a single huge row
can go alone, up to 1 MiB), and the helm passes them on unread. Browser input stays JSON within its 64 KiB bound.

You decided while this plan was blocked that typical output's memory staying essentially unchanged does not fail it,
since the goal is cutting the extremes. The report and the specs record that exception.

#### Fixes made at landing

- The protocol version is now 72. The rebase had dropped this plan's own bump to 71, because mux-chunk-size had already
  taken that number. But main had then been speaking 71 with the old JSON rows for several hours, and this plan makes 71
  mean binary rows. A development helm and supervisor from either side of this landing would have connected and then
  failed inside the terminal stream, with reconnect loops instead of a clear version refusal. No release carries 71 (the
  newest tags, the v0.38.0 release candidates, are at 70), so 72 costs nothing for users. The pinned version test,
  SPEC_impl.md and the commit message now say 72 (#3252). Where the executor's report says the change uses protocol 71,
  read 72.
- A by-hand Rust measurement of attach cost in the supervisor, ignored in normal runs and described in the check
  inventory (`.agents/validation.md`), looked for the screen frame by its JSON text. With this plan's binary frames it
  no longer reported when the screen arrives, and its optional dump wrote binary messages one per line, which cannot be
  split back. It now recognizes the binary frame record, and the dump is length-prefixed; the inventory says so (#3255).
  The report's attach-cost figures come from the browser benchmark this plan adds, not from this measurement, so they
  stand. Rerun after the fix, the measurement put the screen at 1.7 s for typical output and about 0.6 s for the two
  colored kinds over its simulated 10 Mbit/s link.
- Another TODO.md entry still referred to the encoding overhaul entry this plan completes and removes; it now names this
  plan (#3255).

#### Things you should know

- Remote hosts need updating after this release anyway (mux-chunk-size); with 72, a host or helm on a development build
  from the last few hours of main is refused with a version mismatch rather than half-working.
- From the review, small and left as they are: two helm comments still call grid messages "JSON"; the benchmark imports
  `playwright` without declaring it in `e2e/package.json` (it is installed as part of `@playwright/test`); the
  link-hover lookup in `grid-terminal.js` repeats a line mapping that also lives in `grid-buffer.js`, so a change to one
  must change both.

#### Checks

Run on the plan stacked on main before the landing fixes (top at `980ac423`), on this development machine and, at the
maintainer's request, on Tensorlake sandboxes:

- Here: `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, the test-sleep checker, `scripts/check-desktop-assets.sh` (no desktop asset was
  added or removed), `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop` passed.
- Here, run directly rather than through the test recorder, so without run ids (their output was read in full): the UI's
  JavaScript unit tests and the benchmark's own tests (`node --test`), all passing with none skipped; and the grid
  component's browser suite (`npx playwright test --config grid-client/playwright.config.ts`, Chromium and WebKit), 204
  passed.
- Tensorlake, browser, using the release web build: `grid-terminal`, `terminal`, `terminal-switch-geometry`,
  `terminal-reconnect`, `terminal-clipboard`, `terminal-links`, `terminal-unicode`, `grid-flood`, `terminal-flood` and
  `terminal-tabs` on Chromium and WebKit: 295 passed, none failed, skipped or retried. Runs `30ad9da2`, `12d95394`,
  `bb88f060`, `bdc6f228`, `27403bf2`, `9c4e4025`, `81da8c8a` and `90fdba94`, in the recorder's plain command mode (its
  Playwright mode cannot pick one engine per file) under the sandbox's slightly older Node 24. Every sandbox reported
  its per-user systemd manager and its SSH to itself working. I also asked for `terminal-scroll-freeze`, which belongs
  to the old-terminal set that only a separate run includes, so its two sandboxes found no such project and ran nothing
  (runs `9ead4f3c`, `7f5e4344`). That spec is about old terminals, which this plan does not change.
- Tensorlake, Rust: every Rust test in the workspace except the desktop app's (the sandboxes cannot build it; the
  desktop compile check above covers its code), from one archive over nine sandboxes. Runs `a501bfb4`, `b989bfca`,
  `1701c463`, `d230a97e`, `51c036ad`, `837e35b0`, `1ff1c1f2`, `67c4a065` and `ced86e3c`. Two tests failed, neither in
  code this plan touches:
  - `attribute_compile_contract_matrix` needs a nested `cargo` the sandboxes lack; its crate is untouched, and it passed
    here in an earlier round on a tree differing only outside it (run `05991d87`).
  - `service::core::tests::a_failed_create_replays_its_error_and_a_changed_override_conflicts` (run `d230a97e`,
    retained) timed out waiting 10 seconds for a create reply on a loaded sandbox, as it did in the previous round. It
    passed five times in a row here on the merged tree (run `9f342a7d`). It is recorded as a latent flake in a separate
    bookkeeping PR after this landing.

After the landing fixes (protocol 72, the measurement test, documentation), run here: formatting, `dprint check`,
`python3 scripts/check-agents-md.py`, `cargo clippy --all-targets -- -D warnings`, and recorded
`cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm --lib --test e2e` filtered to the
protocol version pins and version-skew tests, the grid encoding and the grid stream: 109 passed (run `b7d9f262`). The
version-skew tests are written against the constant, so they followed the bump. The ignored attach-cost measurement then
ran by hand for all three kinds of output (run `cb939566`) and again reports when the screen arrives. Nothing else was
rerun: the fixes changed one constant that only the pinned test names, one ignored test, and documentation. Main after
the merge differs from the tested tree only by planning bookkeeping that landed meanwhile.

Reused: the earlier-round pass of `attribute_compile_contract_matrix` named above, and the pre-fix runs carried to the
merged tree for the reason given (the fixes touched one constant, one ignored test and documentation). Nothing from the
executor's own runs is relied on.

Skipped: the rest of the browser suite (the specs above cover the terminal paths this plan changes), the old-terminal
browser set (old terminals are unchanged), the desktop smoke (no desktop-only code changed) and hosted CI (manual-only,
not dispatched).
