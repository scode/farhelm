# Test run evidence

`scripts/record-test-run.py` runs one command and leaves a private, bounded record of what ran, where it ran, which
source tree it saw, and how the process ended. It is meant to provide input for later run tracking and flake hunting. It
is not a test scheduler or retry loop, and a label never makes a run reproducible. Explicit nextest and Playwright modes
also retain runner policy and reported test counts.

NOTE: The command argv and combined stdout/stderr are retained exactly. Do not put credentials in argv. Output is
private local evidence, not publication-ready material; review it before copying any part of a run directory elsewhere.

## Finite Rust repetition

For an operator-selected nextest scope, `scripts/hunt-rust-tests.py` runs a finite number of recorder-backed attempts
and keeps each attempt's evidence under its own private child root. The default is a side-effect-free JSON plan; add
`--execute` to run it. The command requires a package, target, workspace, or filter scope and accepts the same nextest
selection allowlist as the recorder:

```console
python3 scripts/hunt-rust-tests.py --repeat 20 --timeout 60 \
  -- cargo nextest run -p farhelm-supervisor --lib -E 'test(shutdown_acks)'
```

`--repeat` accepts 1 through 1000 and `--timeout` is a finite positive number. The plan reports the maximum sum of child
command time (`repeat * timeout`), four shared nextest slots, zero retries, required tmux validation, and the fact that
metadata and cleanup time is outside that sum. The hunt does not build or install tmux; a missing or mismatched pinned
substrate is an explicit recorder refusal. Ordinary failed attempts continue so mixed outcomes remain visible, while a
later pass cannot make the batch pass. Cancellation stops scheduling and returns `128 + signal`; incomplete recorder or
evidence state returns 125. The batch index is bounded and contains summaries only; manifests and reports remain the
source of truth for command, environment, and exact test counts.

Only nextest's ordinary test-failure exit permits another attempt. Runner signals, build/setup failures, incomplete
cleanup, missing stream EOF, and truncated retained output stop the batch. A complete report can still supply counts for
an interrupted runner, but it does not prove that cleanup finished. Incomplete or contradictory reports supply no counts
in the index; their raw evidence remains available for diagnosis.

The command prints the private `index.json` path on stderr when execution starts. The index is written before the first
attempt and after each state transition. Its states are `running`, `completed`, `failed`, `incomplete`, and `cancelled`;
`running` after the process disappears means the batch may have been killed before it could publish a terminal state.
SIGKILL and host loss can leave an in-progress index and do not authorize an automatic resume or retry. The index is a
bounded summary; use each retained recorder manifest and JUnit report for the exact command status and test counts.

## Planning hunts for changed inputs

From the checkout, compare a named base with the current tracked working tree and untracked non-ignored files:

```sh
python3 scripts/plan-test-hunts.py --base main --repeat 20 --timeout 60
```

This reads bounded Git metadata and Cargo manifests and prints JSON; it never runs the suggested commands. Each
suggestion has a working directory, exact argv and copyable shell command, concurrency, widening reasons and a maximum
sum of child-command time. The displayed commands include `--execute`: run one only after reviewing its selection and
cost and preparing the required tools on a sandbox. Prerequisite builds, metadata and cleanup add wall-clock time.

Rust integration-test changes select the complete test target. Shared fixture changes therefore cannot accidentally
select only a filename-derived test name; refine the suggested command with a verified nextest filter when a narrower
reproduction is appropriate. Other Rust changes include reverse package dependencies. Browser spec changes select their
files under both engines; shared browser inputs, deleted specs and application changes widen to the browser suite.
Package-wide suggestions subsume narrower targets for the same package. Known cross-package fixture imports are mapped
explicitly; unresolved helper ownership also appears in the manual-review list. Large browser selections are partitioned
to respect the runner's argument-count and byte limits, and each partition contributes to the displayed total cost.

`manual_review_paths` contains inputs the planner does not map. An empty command list with manual-review paths is not
evidence that no validation is needed. Prose paths are listed separately for applicable document checks.
Feature-specific tests, doctests and other non-hunt checks still require author judgment: this is not a complete build
dependency analyzer or a replacement for the finishing instructions. Regenerate after source edits; discovery is not an
atomic snapshot, and actual execution records the source again. Incomplete or oversized discovery is refused rather than
silently planning a subset.

## Running a command

Use a Unix Python interpreter with `os.waitid` and `WNOWAIT`. On macOS this requires Python 3.13 or newer; older
interpreters are refused before any child starts. The unreaped child is what keeps group cleanup tied to the original
process identity. Release setup selects a compatible interpreter explicitly.

The recorder requires a run kind, selection label, concurrency label, and an explicit `--` before the one command argv:

```console
python3 scripts/record-test-run.py \
  --kind development \
  --selection 'asset JavaScript unit harness' \
  --concurrency 'node default discovery' \
  --timeout 60 \
  --tmux none \
  -- bash -c 'cd crates/farhelm-ui/js-tests && node --test'
```

The labels are recorded verbatim; coverage is never inferred from them. Generic mode invokes the argv directly with the
caller's current directory as the child's current directory. There is no shell joining, retry, or scheduling layer.

`--kind` accepts `development`, `repetition`, or `release`. `--timeout` is an optional finite positive number of seconds
and starts after the command is spawned; metadata probe time is separate. `--tmux` accepts `warn` (the default),
`required`, or `none`. `--output-root` overrides evidence storage. `--keep-farhelm-env NAME` may be repeated, but each
name must begin with `FARHELM_` and must not contain a value. `--termination-grace` sets the finite positive cleanup
allowance after interruption or timeout, in seconds (default 2, maximum 60). A runner that forwards signals to its own
test process groups needs an allowance longer than its internal termination grace.

`--help` and `-h` work without the required labels or a command. Help after the `--` boundary belongs to the child argv.

The default root is `$XDG_STATE_HOME/farhelm-test-runs` when `XDG_STATE_HOME` is set, otherwise
`~/.local/state/farhelm-test-runs`. This is deliberately separate from live product state. The recorder rejects roots
equal to or below the tested Git checkout, the conventional `~/.local/state/farhelm` tree, or `$XDG_STATE_HOME/farhelm`
when XDG state is configured. A newly created root and every run directory use mode `0700`; private files use mode
`0600` for recorder-created files. Runner-created files may use their own file modes inside the private run directory.
An existing root must have no group or other permission bits, and the recorder does not change its owner permissions.

Every invocation gets a full UUID directory. A retry always creates another directory, so a later passing run cannot
overwrite the failure that prompted it. The path is written to stderr as `test-run evidence: PATH` once the directory
exists, including when required tmux validation refuses the command.

## Recorded nextest runs

Use `--runner nextest` from the checkout root with Python 3.11 or newer and exactly cargo-nextest 0.9.143 on `PATH`
(macOS still needs Python 3.13 for the recorder's wait-ownership contract):

```console
nextest_dir=$(python3 scripts/install-pinned-nextest.py) && \
tmux_dir=$(scripts/build-pinned-tmux-ci.sh) && \
PATH="$nextest_dir:$tmux_dir:$PATH" python3 scripts/record-test-run.py \
  --runner nextest --kind development --tmux required \
  --selection 'one session roundtrip' --concurrency '4 nextest slots; one selected test' \
  -- cargo nextest run -p farhelm --test e2e -E 'test(=session_lifecycle::create_attach_and_roundtrip_input)'
```

The installer uses `.github/nextest-pins.json` to verify both the public release archive and its executable. It supports
glibc Linux on x86_64/ARM64 and macOS on Intel/Apple Silicon, requires curl 8.4.0 or newer for downloads, and caches
only under `.ci-nextest/` by default. Cached executables are rehashed before reuse. `--output-root PATH` selects a
different tool directory; release jobs should use their temporary directory instead of a shared mutable cache. A failed
download or checksum leaves any existing executable untouched and returns failure, so keep the guarded assignments
above. The installer's stdout is only the resulting directory; it does not replace the user's Cargo tools. Pin upgrades
must update both archive and executable hashes from the verified upstream artifacts, together with the recorder's
required version and runner config.

This mode accepts package, target, feature and filter selection; it refuses options that replace the runner policy. It
invokes the resolved cargo-nextest binary directly with four global slots, zero retries, failure on a flaky outcome, and
failure on an empty selection. `.config/nextest.toml` adds the e2e group's four-slot cap, the supervisor tmux modules'
two-slot cap, per-test timeout and five-second signal grace. The recorder allows ten seconds for runner cleanup by
default and refuses a shorter allowance. Ambient `NEXTEST_*` settings are removed from the child environment, and
nextest's user config is disabled. The names removed are retained, never their values. Generic recording remains
available for explicit experiments with other policies, but such a command does not acquire these controlled-run claims.

Each run retains `nextest.toml`, `nextest-store.toml`, the actual executable path/hash/version probe, the requested
argv, and the actual argv. The tool config directs output to that run's `nextest/default/junit.xml`; it cannot overwrite
an earlier run. The [JUnit report](https://nexte.st/docs/machine-readable/junit/) includes selected-out and ignored
tests, so skipped cases stay separate from passes. Standard output remains in the recorder's bounded log and per-test
traces, without duplicating it into XML. XML inspection has a 16 MiB cap; missing, partial, oversized or contradictory
reports are recorded as incomplete. The raw runner-written report is retained in place; the inspection cap is not a
quota on nextest's writes. Review that file before exporting it, like other raw run evidence. Runtime skips that print
`SKIPPED` and return success remain passed cases in JUnit; their retained console output is required to interpret which
substrates were actually exercised.

Release failure collection uses `python3 scripts/test_run_nextest.py RUN_DIR EXPORT_DIR` to copy only the two runner
configs and `nextest/default/junit.xml` (exported as `nextest-junit.xml`). Configs are capped at 64 KiB each and the
report at 16 MiB; symlink components, nonregular files and existing output files are refused. Its JSON stdout names
copied files, hashes and omissions. Missing or malformed report content remains evidence, never a zero-test pass:
malformed bytes may be exported, while absent or oversized files make collection incomplete. This command does not walk
arbitrary test state.

`runner.report.complete` concerns report structure and totals, not source identity, test coverage or an assertion that
the process exited successfully. The command status remains separate. A zero command exit with incomplete evidence, no
executed cases, failures or flaky outcomes becomes recorder error 125, with the actual child status still retained. An
existing nonzero command status is preserved. A recorder killed before collection leaves the report's completeness
unproven. Nextest does not run doctests; maintained suite commands must retain a separate doctest invocation.

When changing this integration, run the small Python policy/recorder checks and, on a Linux sandbox with the pinned
runner, `python3 scripts/test-nextest-cleanup.py --output-root /tmp/nextest-cleanup-evidence`. Repeat with
`--trigger test` to exercise nextest's own test deadline instead of the recorder's execution deadline. The fixture
shortens only its copy of the per-test execution period for that case, retaining the real five-second signal grace. Both
commands compile a dependency-free fixture and verify that nextest kills a SIGTERM-resistant test and its child in the
runner-owned test group. These are explicit lifecycle checks, not a per-edit stress requirement or an added CI job.

## Recorded browser runs

Use `--runner playwright` from the checkout's `e2e` directory after explicitly preparing the application builds,
installed npm dependencies, browser binaries and pinned tmux described in AGENTS.md. Browser builds and runtime checks
belong on a sandbox when they are expensive. The recorder neither installs prerequisites nor starts a build for you.

```sh
# From the checkout root, after preparing the required builds and pinned tmux:
tmux_dir=$(scripts/build-pinned-tmux-ci.sh) && \
cd e2e && \
PATH="$tmux_dir:$PATH" python3 ../scripts/record-test-run.py \
  --runner playwright --kind development --tmux required --timeout 300 \
  --selection 'feed spec, both engines' --concurrency 'one browser worker; retries 0' \
  -- npx playwright test 'feed\.spec\.ts'
```

The requested `npx playwright test` prefix is a selection syntax; execution uses resolved Node and the locally installed
Playwright CLI directly. Both Playwright packages and their lock entries must match 1.62.0. The maintained config runs
with one worker, zero retries, one execution per selected case, no focused-only tests and no snapshot updates. Only
file-pattern and grep selectors are accepted; project, reporter, config and execution-policy overrides are refused. Both
configured engines must execute cases before a zero child exit can support recorder success. A single-engine debugging
command can still use generic recording, which does not claim validated browser counts.

Each run owns `playwright.json`, `playwright-policy.json` and `playwright-artifacts/`. The supplementary reporter
records resolved engine and project policy before execution and atomically replaces its own initial record at
completion. Collection reconciles the reports, bounds reads and traversal, and keeps actual
pass/failure/skip/interruption/unstarted counts separate from expected outcomes. An expected failure is an executed
assertion, not a passing result. Missing or malformed evidence has no inferred zero denominator. The JSON read limit is
16 MiB and policy read limit is 64 KiB; these are collection limits, not disk quotas on runner output. Raw reports and
attachments remain private and may contain sensitive application data; no arbitrary attachment path is followed or
exported.

Timeout or operator interruption delivers SIGINT to Playwright so its teardown and reporters can run. The recorder
retains the original operator signal or timeout as its own exit status and allows 60 seconds for owned-group cleanup.
Even an already exited leader can keep the group visible during this interval; that wait preserves group ownership until
the final signal. A forced kill can leave terminal reports incomplete. Report validation never overwrites an earlier
nonzero child, timeout or interruption result.

### Explicit browser repetitions

From `e2e/`, plan a finite reproduction with a file pattern or grep selection:

```sh
python3 ../scripts/hunt-browser-tests.py --repeat 20 --timeout 60 \
  -- npx playwright test tests/terminal-keys.spec.ts -g 'plain Enter sends bare CR'
```

Add `--execute` only when the displayed selection and cost are intended. Planning does not probe tools, create evidence,
install dependencies or build the application. Execution needs the browser recorder's prerequisites above, including the
pinned tmux already on PATH. Each attempt uses both Chromium and WebKit, one worker, no retries and no internal
repeat-each loop. Use generic recording for an intentionally single-engine debugging run.

The private batch index retains each attempt and its validated actual and expected outcome counts per engine. A later
pass does not erase an earlier failure. Ordinary failed assertions and individual case timeouts may proceed to the next
requested attempt; runner-wide timeouts, interrupted or unstarted cases, global errors, missing reports and incomplete
output or cleanup stop the batch. Expected failures count as executed assertions; intentional skips do not. Before
scheduling again, the wrapper revalidates the retained reports and checks that they match the recorder manifest.

The batch returns 0 only when every requested attempt passed, 1 when all attempts completed but at least one failed, 125
for incomplete evidence, or `128 + signal` for operator cancellation. The plan's `repeat * timeout` is the maximum sum
of child command time, not a wall-clock deadline: metadata and the browser recorder's 60-second cleanup grace are
additional. This is an operator-invoked investigation tool, not a required per-edit test or proof of flake absence.

## Exit status and lifecycle

The manifest's `outcome` is one of:

- `running`: the initial durable record. Seeing this after the recorder process is gone means it could not finalize, as
  with SIGKILL or host loss.
- `completed`: the child exited normally or was signaled without the recorder itself receiving an interrupt.
- `refused`: a pre-spawn policy check, currently required tmux validation, rejected the run.
- `timed_out`: the command exceeded `--timeout`.
- `interrupted`: the recorder received SIGINT or SIGTERM and initiated command-group shutdown using the runner's signal
  policy.
- `recorder-error`: spawning, evidence IO, or lifecycle management failed, so an ordinary command result would not be
  trustworthy.

Normal child exit statuses pass through unchanged. A child killed by signal `N` returns `128 + N`. Refusal and recorder
error return 125; timeout returns 124; recorder interruption returns `128 +` the received signal.

The command starts in a new POSIX process group. On recorder interruption or timeout, the recorder forwards the signal,
waits the declared termination grace, then sends SIGKILL to its owned group. Pipe EOF never substitutes for child exit.
The grace and command timeout are recorded separately; termination grace is additional time after the execution
deadline. If the group leader exits while descendants retain the output pipe, draining ends after two seconds and owned
remnants are killed. A final half-second drain follows cleanup. The `recorder.forced_cleanup` and
`recorder.cleanup_limit` fields say what happened. A descendant that creates another session has escaped the owned
group; the recorder bounds its pipe drain and discloses that such a process may remain, but cannot claim to clean it up.

The post-kill deadline also bounds waiting for a leader that remains alive: a runnable recorder closes its pipes,
attempts a bounded final wait, and records incomplete cleanup instead of polling forever. It cannot force a process out
of an uninterruptible kernel wait. Losing child wait ownership records a recorder error; unavailable child exit status
stays null rather than being inferred as success.

SIGKILL cannot be handled. If it kills the recorder, `manifest.json` remains at its last complete atomic update (usually
`running`), and output already written to chunk files remains available. This does not promise cleanup of the child
group, crash-safe filesystem durability, or cleanup of descendants that escaped the group.

## Manifest schema 1

Git top-level discovery runs first because the recorder needs it to reject evidence storage inside the tested checkout.
Once that storage-safety preflight succeeds or records that Git is unavailable, `manifest.json` is written before the
full source capture, tmux probes, or child spawn and replaced atomically as evidence changes. Consumers must check
`schema_version` before interpreting fields and must check component `complete` flags before using fingerprints.

The top-level fields are:

- `schema_version`: integer `1`.
- `run_id`: the full UUID used as the directory name.
- `outcome`: one of the lifecycle states above.
- `started_at` and `finished_at`: UTC RFC 3339 timestamps. `finished_at` is null while running.
- `duration_seconds`: total monotonic duration, or null while running.
- `command`: exact string `argv`, actual caller/child `cwd`, requested `timeout_seconds` and
  `termination_grace_seconds`, `graceful_stop_signal_override`, `require_complete_console`, and terminal
  `duration_seconds`. Runner modes also retain `requested_argv` before resolving executables and controlled options.
- `runner`, when a runner mode is selected: preparation status, resolved tool evidence, removed environment names,
  configuration hashes, execution policy, and report completeness/counts. Nextest also retains its controlled configs.
- `labels`: caller-supplied `kind`, `selection`, and `concurrency`, plus a reminder that they are descriptive.
- `environment`: locale identity and FARHELM variable-name handling.
- `platform`: OS release, machine architecture, logical CPU count, and Python identity. Processor probing is omitted
  because it can launch an auxiliary subprocess outside the bounded probe lifecycle.
- `source`: Git checkout evidence and its limits.
- `tmux`: expected and actual substrate evidence, or an explicit `none` record.
- `output`: retention policy, byte counters, and ordered files. It is null before command output storage starts.
- `console`: observed, forwarded, rejected, and pending/dropped byte counters for best-effort console streaming, plus
  `worker_finished`. In-process repetition requires the forwarding worker to have stopped before another run starts.
- `test_traces`: private trace-root identity and fixed-layout collection evidence, or null before command setup.
- `child_status`: Python's raw return code plus normalized `exit_code` or `signal`.
- `recorder`: recorder exit code, forced-cleanup flag, cleanup limitation, and error text.

Manifest updates use a mode-`0600` temporary file, a complete write, `fsync`, and atomic replacement. This prevents
readers from seeing half-written JSON. It does not promise that the newest directory entry survives power loss.

Here are selected fields from a small successful record. Digests and detailed probe objects are shortened, and the
environment, platform, console, and repetitive probe fields are omitted from this example:

```json
{
  "schema_version": 1,
  "run_id": "00e0f0db-23d9-4937-a54e-d848f84dc62f",
  "outcome": "completed",
  "started_at": "2026-09-06T06:00:00.000Z",
  "finished_at": "2026-09-06T06:00:01.250Z",
  "duration_seconds": 1.25,
  "command": {
    "argv": ["python3", "-m", "unittest"],
    "cwd": "/path/to/checkout",
    "timeout_seconds": 120.0,
    "duration_seconds": 0.75
  },
  "labels": {
    "kind": "development",
    "selection": "Python recorder fixtures",
    "concurrency": "one process",
    "interpretation": "descriptive labels supplied by the caller; no test counts inferred"
  },
  "source": {
    "complete": true,
    "head": "0123456789abcdef0123456789abcdef01234567",
    "fingerprint_sha256": "...",
    "tracked_diff": { "complete": true, "bytes": 0, "sha256": "...", "probe": {} },
    "porcelain": { "complete": true, "bytes": 0, "sha256": "...", "probe": {} },
    "untracked_tree": { "complete": true, "entry_count": 0, "sha256": "...", "probe": {} }
  },
  "tmux": { "mode": "none", "checked": false, "uses_tmux": false },
  "output": {
    "limit_bytes": 8388608,
    "observed_bytes": 3,
    "retained_bytes": 3,
    "omitted_bytes": 0,
    "truncated": false,
    "files_in_read_order": [{ "name": "output-head.log", "bytes": 3, "role": "head" }]
  },
  "child_status": { "raw_returncode": 0, "exit_code": 0, "signal": null },
  "recorder": { "exit_code": 0, "forced_cleanup": false, "cleanup_limit": null, "error": null }
}
```

## Environment evidence

The child environment is copied from the recorder, then every `FARHELM_*` variable is removed except names supplied with
`--keep-farhelm-env`. An absent requested name remains absent. The same controlled environment is used to resolve and
run tmux. Before the command starts, the recorder sets `FARHELM_TEST_TRACE_DIR` to its new private `test-traces`
directory. This name is reserved: even an explicit `--keep-farhelm-env FARHELM_TEST_TRACE_DIR` cannot redirect capture
to an ambient location. Metadata probes run before this override.

The manifest records only FARHELM variable names: `ambient_names`, `requested_names`, `retained_names`, `removed_names`,
`requested_but_absent_names`, `overridden_names`, `recorder_owned_names`, and the resulting `child_names`. It never
records ambient FARHELM values or hashes of them. The recorder-owned trace location is recorded relative to the run.
Locale evidence records `LANG`, `LC_ALL`, `LC_CTYPE`, Python's preferred encoding, and filesystem encoding.

## Persistent test traces

Tests using `farhelm_testtrace::test` write bounded incremental evidence under the private trace root. Complete
successful captures clean up their own slots after runtime teardown. Failures, incomplete captures and abnormal exits
retain raw files. This covers participating tests; an empty root says nothing about whether an unwrapped test emitted
events.

After the command's process cleanup, the recorder exports only `slot-000` through `slot-127`, and only `metadata.json`,
`head.jsonl`, and `tail-0.jsonl` through `tail-2.jsonl` in each slot. Metadata is capped at 4 KiB and each event file at
256 KiB. The collector opens private effective-user-owned slot directories through a held root descriptor and accepts
only regular files without following final symlinks. Unknown names are never inspected or uploaded. Partial JSON is
retained as bytes rather than discarded as a parse error.

`traces.tar` is an uncompressed, mode 0600 USTAR archive created only when at least one file is retained. The fixed
layout bounds payloads to 128.5 MiB plus tar headers/padding; each member's byte count, SHA256 and truncation flag is
recorded. Collection checks a 10-second deadline between filesystem operations and keeps at most 32 error details.
Kernel-blocked filesystem calls remain outside that deadline guarantee. A read fault retains the prefix already
observed; an archive write fault stops export and marks the partial archive incomplete. A prefix is archived only while
output remains possible within the shared deadline; expiry can leave bytes available only in the raw files. Raw files
remain available locally. The recorder publishes the command result before exporting traces. If collection or
publication of its summary fails, the earlier command result and exit status stand; the durable manifest may still
describe traces as uncollected.

`test_traces.collection.collection_complete` describes only collection of those fixed names. Consult each capture's own
metadata for event loss and completion; neither collection success nor syntactically valid JSON proves that a test
finished. Collection observes files over time, not an atomic filesystem snapshot. Use it after the test processes are
quiescent; a descendant that escaped process cleanup may still be changing its files.

The recorder writes the root's relative path and held device/inode before spawning the command. If the recorder itself
dies, the manifest can remain `running`, or retain a completed command result with an `uncollected` trace component.
Recover those raw files without rerunning:

```sh
python3 scripts/test_run_traces.py /path/to/run/test-traces /path/to/recovered-traces.tar > /path/to/trace-collection.json
```

The destination must be new; an earlier archive is never overwritten. Review the JSON collection status even when the
export command exits successfully. The standalone summary identifies the device/inode it opened, so it can be compared
with the recorder's original root identity. It describes a new observation of raw files and does not export an earlier
recorder archive. Existing release failure collection uses `recovered-traces.tar` with that independent summary beside
the original run manifest and output. The manifest's original `traces.tar` is not included in that hosted export; its
hashes and completeness must never be applied to the recovery archive. Preserve the original archive locally when
needed. No additional tests are scheduled by collection.

## Source identity

Git top-level discovery begins at the caller's current directory. Once found, every remaining source probe runs at that
top level, so invocation from a nested directory still describes the entire checkout.

`source` contains these components:

- `head` and `head_probe`: the exact 40-digit commit identity and bounded probe evidence.
- `tracked_diff`: SHA256 and byte count of the raw `git diff --no-ext-diff --no-textconv --binary HEAD --` stdout. Raw
  diff bytes never enter metadata.
- `porcelain`: SHA256 and byte count of the complete NUL-delimited porcelain stream plus a bounded base64 prefix. Base64
  preserves invalid-byte paths without decoding them.
- `untracked_tree`: a deterministic aggregate over byte path, filesystem type, size, and the content digest or
  symlink-target digest for every nonignored untracked entry. Symlinks are not followed.
- `fingerprint_sha256`: a digest binding HEAD and the three component digests. It is present only when every component
  is complete.

The untracked aggregate processes Git's byte-sorted path stream in order. For each entry it hashes an unsigned
eight-byte big-endian path length, the raw path, a one-byte type-name length, the ASCII type name (`file` or `symlink`),
an unsigned two-byte big-endian permission mode (`stat.S_IMODE`), an unsigned eight-byte big-endian content length, and
the 32 raw bytes of the SHA256 content or link-target digest. The overall fingerprint hashes four ASCII records in this
order: `head`, `tracked_diff`, `porcelain`, and `untracked_tree`. Each record is `label`, NUL, lowercase hexadecimal
digest, NUL. These encodings are part of schema 1.

This identifies the working files a normal build reads, within Git's tracked/nonignored selection. It does not hash
ignored build artifacts or staged blob contents separately: two different index contents can share a fingerprint when
the working files and porcelain status are identical. A command that consumes the index or ignored files needs separate
evidence for those inputs. Capture is an observation of a mutable tree, not a filesystem snapshot; avoid source edits
while collecting a run whose input identity matters.

Probe stdout and stderr are streamed, hashed, and represented by bounded samples. Each probe has a five-second deadline
and a bounded post-leader pipe drain. Untracked enumeration retains no unlimited per-file list: it records counts and at
most 32 problem samples. A 100,000-entry cap and a 1 MiB unterminated-path buffer cap protect memory. Unreadable files,
unsupported types, invalid HEAD output, a failed or timed-out probe, an entry cap, or truncated inherited probe pipes
makes source capture incomplete and suppresses the overall fingerprint. Git failure does not by itself prevent the
requested local command. A probe whose child wait ownership is lost or whose leader cannot be reaped after cleanup sets
`lifecycle_complete` to false. Its bounded output and any previously observed status remain in the manifest, along with
the other source components; the recorder then exits 125 before starting the requested command. This error takes
precedence over simultaneous interruption, which remains recorded on the affected probe. Pending interruption prevents
new probe children from starting.

The file-content and executable hashing routines check their deadline between reads. Standard-library file IO cannot
interrupt a filesystem syscall that blocks inside the kernel, so this is not a hard wall-clock bound for a broken
filesystem.

## Tmux evidence

`--tmux required` checks both identities before child spawn:

1. Resolve `tmux` through the controlled child `PATH`, run that exact executable with `-V`, and compare its output with
   the `TMUX_VERSION` value parsed as plain text from `.github/release/source-pins.env`.
2. SHA256 the resolved executable and compare it with the repository-built `.ci-tmux/tmux` executable.

The source-pin file is never executed. Its source archive checksum is not treated as a binary hash. Missing files,
failed probes, unreadable executables, invalid pins, version drift, and binary drift all refuse required mode before the
child starts.

`--tmux warn` records the same evidence and prints every mismatch, then runs the command. `--tmux none` records
`checked: false` and `uses_tmux: false`; use it only when the recorded check does not use tmux. The recorder never
builds or replaces tmux.

## Output records

Stdout and stderr are combined in arrival order, streamed to stdout on a best-effort basis, and retained with an 8 MiB
payload cap. `output-head.log` holds the first 1 MiB. Up to seven `output-tail-NNNNNN.log` files hold the newest seven 1
MiB chunks; older tail chunks are removed as new chunks roll in. Writes go directly to the OS as data arrives, so useful
early and late evidence can survive recorder SIGKILL.

Read files in `output.files_in_read_order`. The manifest records `observed_bytes`, `retained_bytes`, `omitted_bytes`,
and `truncated`; chunk file names and sizes make every retained range and omission visible. Manifest size is outside the
payload cap but its samples and problem lists are independently bounded.

Console output is not authoritative. A bounded queue and raw-descriptor daemon writer keep a slow or broken consumer
from disabling timeout and evidence retention. `console.dropped_or_pending_bytes` can therefore be nonzero even when all
bytes are present in the private output chunks. A caller that must parse the forwarded stream can request
`--require-complete-console`: an otherwise successful command becomes recorder error 125 if any observed bytes were
dropped or still pending when forwarding finished, or the combined child output never reached EOF. The manifest's
`output.eof_observed` distinguishes an actual stream end from forced closure at the drain deadline; forwarding every
observed byte cannot prove that no later output was lost. The actual child status is preserved, and an existing nonzero
result is never replaced by this policy. The CentOS provisioning gate uses this option because missing output could hide
its runtime `SKIPPED` witness. This flag does not make an output line proof of behavior; the caller must still validate
its test-specific witnesses.
