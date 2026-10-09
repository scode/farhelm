### What this was about

Every fresh GitHub checkout downloaded the repository again. The maintainer chose an always-on cache in each host's
Farhelm state directory, eviction after 30 days without use at supervisor startup, and a visible failure rather than
fallback when the cache cannot be used.

Fresh checkouts now update that host's cached branches and tags, then make an independent clone with its ordinary GitHub
origin. Cache initialization, fetch and clone run in the session terminal using the host's credentials. Concurrent
checkouts of the same repository take turns on the cache. The specs, checkout guide and Start a session website page
describe this behavior; the covered TODO entry is removed.

### Things you should know

The cache costs roughly one extra copy of each repository on each host. Existing checkouts keep their own objects and
remain usable after a cache is deleted or after they are archived. Uninstall retains caches with the rest of Farhelm's
data. Unused caches disappear only when the supervisor starts; a cache being used by a surviving launch is skipped.
Small permanent lock files remain after eviction so waiting launches and new arrivals keep using the same lock.

A private repository without a credential helper can ask twice, once for the cache fetch and once for the clone. A cache
error stops preparation and names its path. You can remove a cache directory by hand; leave its sibling lock file in
place, and avoid removal while a checkout is being prepared. There is no cache setting, repair behavior, fallback or
disk-size cap.

Launch instructions written before this change can still be read. An unfinished launch without a cache path refuses its
clone stage with an explanation; a checkout whose preparation already completed restarts normally without repeating Git
or its configured setup command.

The code and user docs are one PR. This keeps the new disk usage and authentication caveats with the behavior they
explain. This is a performance change with a Changed changelog fragment. Validation proves cache reuse and independent
clones; no network latency benchmark was taken, so no measured speedup is claimed.

### Open questions and possible follow-ups

Browser acceptance is incomplete. The checkout spec had 11 passes, six 60-second timeouts and three fixture-setup
failures. The scenario that keeps a shared checkout until its last stopped session is deleted also timed out on rebuilt,
clean main, so that failure predates the cache change. Its cause remains unknown. Increasing the sandbox from four to
eight CPUs did not resolve it. The other five timeouts were not separately compared with main, so they remain validation
limitations rather than proven pre-existing failures.

A separate test cleanup defect is established: after a timeout, session cleanup can throw before the Git fixture
restores its URL mappings. The next engine can then resolve a repeated repository name to the preceding fixture, causing
the three setup failures before product assertions. FLAKES.md and an open Deflake entry preserve the proven existing
timeout and this cleanup issue. No product or browser-fixture fix is included. A follow-up should first diagnose the
slow terminal typing and ensure fixture restoration runs even when session cleanup fails.

No requirement was dropped or expanded. The cache's explicit Git commands, ref filtering, independent clones, failure
behavior, old-launch compatibility, serialization and eviction have focused passing Rust evidence; browser failures
limit confidence in the remaining UI-driven lifecycle scenarios.

### The PRs

- [#1720: speed up repeated GitHub checkouts](https://github.com/scode/farhelm/pull/1720/changes), including
  specifications, user docs and the required deflake record. Draft at `7f87edad3426c4fffc560d2a1f6350bc6e176a1f`; not
  marked ready or merged by the executor.

### Checks run, reused and skipped

Checks run:

- `cargo fmt --all -- --check`, both `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo build` passed. These cover formatting, test-enabled code
  and the shipped binary configuration.
- The isolated `python -B scripts/check-test-sleeps.py` passed: 269 delays, zero missing reasons.
  `python3 releasing/check-changelog.py format` and `dprint check` on the four changed Markdown files passed. Dprint
  does not select the MDX page; it was inspected manually and validated by the website build.
- The website's `bun install --frozen-lockfile && bun run build` passed; the final explanatory edit also passed a fresh
  build, including its internal link.
- Recorded nextest run `83308afc-693d-4d38-980b-bbeb583b981a` passed
  `missing_cache_field_refuses_clone_but_ready_restart_still_works`. Run `96f80f2b-8e5c-47da-b4cf-264694ff51e0` passed
  `concurrent_checkouts_serialize_cache_fetch_and_copy` after its review correction.
- Recorded `cargo nextest run -p farhelm --test e2e -E 'test(github_checkouts::)'`, run
  `d75e1174-1d05-44dd-a0d9-f85de0a41c2d`, passed both product scenarios: a real fresh checkout completes clone, setup
  command and agent launch; a lost success response replays after settings change and helm restart. The report's third
  pass is fixture setup, not another product scenario.
- The web UI release build (`dx build --package farhelm-ui --platform web --release`) passed. Recorded checkout-spec run
  `687d4902-b93f-4b51-8aac-c992e53883c9` ran 20 scenarios on clean `1aba59e8`, both engines, one worker and zero
  retries: 11 passed (seven Chromium, four WebKit), six timed out and three failed fixture premises. No scenario was
  skipped. The sandbox had four CPUs and 8 GiB. The failures and cleanup cascade are described above; this is not a
  passing browser verdict.

The exact Chromium shared-checkout scenario timed out again on clean `1aba59e8` in
`ef15cc41-c2b8-4631-8e67-6369ab1a0538` at four CPUs and `22f6e517-77a1-40b4-ab51-c6410d6cb0e5` at eight CPUs, then on
rebuilt, clean main `7b2c390b` in `0217e0f1-a5fc-4400-ac7c-99c94c008d62` at eight CPUs. Each used a fresh stack, one
worker, zero retries and the existing 60-second test limit. An earlier exact selector selected no tests
(`bccdcac6-baaa-469c-aef0-2bbebb0fa7b5`), so it provides no product coverage. All records and traces are retained
privately. The main control establishes only that this particular timeout predates the cache.

Checks reused:

- Publication commit `7f87edad` is based on `89963a5f`. The complete intervening main diff since tested `7b2c390b`
  contains two unrelated TODO entries and plan/report bookkeeping, with no executable changes. The rebase retained both
  upstream TODO additions. All production code, tests, specifications, user docs and the fragment are byte-identical to
  reviewed/tested `1aba59e8`; later edits only record the browser failure in FLAKES.md, TODO.md and the deflake
  exclusion list. The passing lint, build, website and runtime evidence above remains applicable; no new runtime battery
  was needed. After these metadata edits, `dprint check TODO.md FLAKES.md` passed.

- Run `26cd1ff8-bdb2-488a-afc2-af6608f2f8fd` selected `launch::tests::` and `repo_cache::tests::` on the working draft
  based on `edbb379a`: 56 passed and one newly authored compatibility test failed because it reused a stale test-harness
  error report. Its failed record is retained. Reuse 55 unchanged passing scenarios; replace the compatibility and
  concurrency evidence with the two focused passes above. Together these cover all 57 selected launch/cache scenarios.
  Production behavior stayed unchanged, and the rebase to `7b2c390b` brought unrelated screenshot tools/docs and plan
  approvals. The final commit's later changes concern those two fixtures, formatting and explanatory prose, so the
  unchanged cases retain their coverage.
- Setup refusal `beaeebb8-9606-4286-b3ef-210637535485` (exit 125) is retained separately: container Git ownership
  prevented source/substrate discovery before tests started. Container configuration was corrected. It is not a runtime
  failure or pass.

Browser preparation records are retained separately: `58e9f3a8-7be7-464c-8fb0-69656b52dbd5` and
`d82ee5aa-b632-47cb-83bd-47cb948b126d` failed because the controlled browser runner could not find the installed
executables in its default cache; `a3db2361-7aaa-4934-9d3e-1e3e7233fe60` then failed because the sandbox lacked browser
libraries and the mounted WebKit binary targeted a different Ubuntu version. Each failed all 20 launches before product
assertions. Corrected the private sandbox's default cache, installed its native browser build and dependencies, and
verified both engines launched and loaded a page in recorded readiness run `d1b961bf-515e-4711-a504-ec74717d1ad7`. This
readiness check is substrate evidence, not checkout coverage.

Runtime Rust checks used pinned nextest 0.9.143 and tmux 3.7c, four global slots and zero retries. Required substrate
identity was verified, and retained output contains no runtime `SKIPPED` message.

Checks skipped:

No full Rust or browser battery, desktop runtime, installer or release gates were run: the changes concern
fresh-checkout preparation and host-local cache lifetime, covered by focused unit and Rust checkout scenarios, with
partial browser checkout coverage and its failures disclosed above. Further runtime runs were skipped because the exact
main control established an existing timeout, while the changed cache behavior already has focused passing evidence.
Repeating the spec would not resolve the unknown browser cause. The website change is explanatory prose and does not
change controls or screenshots. No live installation was modified.

### Review gate outcome

Fresh-context gpt-6.1-sol at high effort reviewed correctness, design, idiomatic code, acceptance criteria and the full
test-authoring checklist. It found one test-fixture issue: releasing a parked Git command could wait forever if that
command died after announcing readiness. The release now checks liveness and uses a nonblocking FIFO open with
diagnostics; the same reviewer confirmed the finding is resolved and found no product-code issue. Runtime checks are the
executor's evidence, not the reviewer's claim.

The required cold reader also checked the commit and PR wording. Implementation and validation stayed local under
no-workhorse mode; the only delegated work was the required resume checks, code review and wording/report cold reads.
Galaxy-brain session evidence is retained under session UUID eec309e5-9251-4738-a762-93388ad02c77; native usage
accounting was unavailable.

### Landing

Landed on 2026-10-09 (UTC) as #1720 (fresh GitHub checkouts clone through a per-host repository cache), one squash
commit on main.

#### What else was on main, and what lands with it

Between the commit the change was built on and the landing, main gained the save-as-template change to the launcher
(#1717), documentation and TODO entries, and the planning queue's bookkeeping. Two other plans land right after this
one, in the same round: row-marks-desktop-utf8 (the desktop app's text encoding) and conversation-notice-hook-restart
(the missing-conversation warning, including a supervisor database step to version 29). The rebase met one conflict:
FLAKES.md, where #1717 and this change each appended an entry; both were kept, #1717's first.

A separate reviewer that had not worked on any of the three plans read them against each other and against main before
anything merged. This change and conversation-notice-hook-restart both edit the supervisor's session code, in different
places that merge cleanly and do not interact: the cache sweep at supervisor start and the cache path in a checkout's
preparation here, the launch's hook record there. This change adds no database step: the cache path lives in each
launch's own preparation record, and an old record started under the new build fails with a named error, as the report
says. Uninstall already keeps everything under Farhelm's state directory, the cache included.

#### The report's incomplete browser evidence

The report says its GitHub-checkout browser spec had six timeouts and three fixture failures, and that only one timeout
was shown to fail on main too. The reviewer looked for a way the cache could cause them and found none: all cache work
finishes, and its lock is released, before the agent starts; Git's background maintenance is forced to run in the
foreground, so nothing outlives that stage; the slow typing in the traces happens after the clones finished; and each
test uses its own repository name, so checkouts within a test never wait on each other's cache lock. The landing did not
re-run that spec on this busy machine, so the five uncompared timeouts remain unexplained, as the report says.

The reviewer also noted three small things the landing did not change: evicting a large unused cache at supervisor start
delays the supervisor's readiness (at most once every 30 days per cache); a second checkout of the same repository
waiting for the cache lock shows an empty terminal while the first sits at a credential prompt; and an Enter typed into
a credential prompt during preparation starts the missing-conversation warning's clock, which this change makes more
likely for private repositories without a credential helper, since they are now prompted twice.

#### Checks

- Run now, on all three plans stacked together in landing order: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `dprint check`, `python -B scripts/check-test-sleeps.py` and
  `python3 releasing/check-changelog.py format`, all clean; the supervisor's and the protocol crate's unit tests in full
  through the test-run recorder (run `0bc2d861`, 1176 passed, 2 failed); the desktop UI's unit tests (run `b9f0a31b`,
  545 passed).
- The two failures predate the three plans. One is the restart test FLAKES.md already records
  (`unconfirmed_restart_is_refused_only_while_the_agent_is_working`); the other,
  `more_than_one_hundred_distinct_matches_are_truncated`, ran out its five-second scan budget while the machine's load
  average was around 20 to 23 on 18 cores. Five repetitions of both on the stack failed eight times; five on main
  without any of the three plans failed four times, with the same failure shapes. The landing did not log the discovery
  test as a flake on this evidence.
- Reused: the report's other checks.

Nothing in the report above was made untrue by the landing.
