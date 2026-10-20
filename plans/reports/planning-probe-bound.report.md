## What this was about

A local SSH helper could keep a host check waiting for diagnostic output after its probe process exited. The helm allows
four host checks at once, so four such waits could prevent further host setup, updates and removals until the helm
restarted. No real-world SSH configuration triggering this has been demonstrated. The maintainer approved a fix only if
it stayed within about 15 non-test lines; otherwise the finding was to be discarded.

Probe cleanup now waits at most two seconds for that diagnostic reader, matching the existing bound on another probe
exit path, then cancels an unfinished reader. The non-test source change is 13 added/deleted lines including comments,
within the gate.

## Things you should know

Ordinary completed diagnostics are retained. If the reader reaches the bound, its unfinished diagnostic result is
discarded. The direct probe process and its process group are still terminated and reaped first. This limits a wait for
output; it does not promise to terminate a helper that escaped that process group.

The tests use an intentionally open diagnostic stream and a real, already-reaped isolated child. They verify the bound,
reader cancellation and ordinary diagnostic retention without claiming a real SSH-helper reproduction. During actual
installation or update, a helper retaining output can still leave that host busy until a helm restart, as BUGS.md
describes. This change covers the preliminary host checks. The feedback was removed and its execution recorded. The
working log follows the `farhelm-plan-<slug>-log.md` rule beside the checkouts.

## Open questions and possible follow-ups

None required. The separate installation/update issue retains its existing documented scope and rationale.

## The PRs

- [#2415 — bound host planning probe cleanup](https://github.com/scode/farhelm/pull/2415/changes), draft.

## Checks run, reused and skipped

Runtime commands ran through `python3 scripts/record-test-run.py` in nextest development mode with pinned nextest
0.9.148, the repository's pinned tmux, four global slots, zero retries and a 600-second command limit:

- `cargo nextest run -p farhelm-helm --lib -E 'test(provisioning) & test(probe)'`, run
  `a4f12a84-c9e9-4eec-9e40-5189221ca538`: 16 passed, one failed, 1,194 selected out. The new test demanded an exact
  two-second virtual duration and observed 2.001 seconds. The failed run is retained. Only the assertion precision and
  its explanation changed; production code did not change.
- `cargo nextest run -p farhelm-helm --lib -E 'test(=provisioning::backend::tests::stop_probe_bounds_and_cancels_an_open_stderr_reader)'`,
  run `d866dad2-a6f1-477f-8d8f-cb4b7bf3c8ed`: the corrected test passed, 1,210 selected out. It permits timer rounding
  within the existing three-second failure guard, still requires the two-second wait and independently observes EOF
  proving reader cancellation.

Both runs have complete source, report, output and cleanup evidence, with no runtime substrate skip reported. They cover
base `5bade3e6` plus the recorded working diffs. The 16 unchanged passing cases from the first run remain valid for the
final product, including completed diagnostics and existing probe classification/process cleanup. Across the two runs
there were 18 executions: 17 passed and one failed, covering 17 distinct tests. The later pass does not replace the
failed observation.

`cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed before the test-precision correction. That result is
reused: product code, fixture and test structure are unchanged; the assertion/comment correction was compiled and
independently reviewed. `cargo fmt --all -- --check`, changed Markdown dprint checks,
`python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the isolated
`python -B scripts/check-test-sleeps.py` passed. The delay checker inspected 304 delays with none missing a reason.

New main changes through `6fe5a692` contain only plan claims and delivery reports. They do not interact with the probe,
so no rebase or runtime rerun was needed. Final commit and PR-link bookkeeping preserve the tested code.

Skipped: broad workspace Rust/Clippy, browser and desktop runtime, and full installation/provisioning suites. The change
is confined to a diagnostic cleanup wait; the selected probe tests cover its concrete risks. UI and installation
execution behavior did not change. CI was not dispatched.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design, idiom,
test-authoring or size-gate issues. The same reviewer checked the test-precision correction and found no additional
issues. Commit wording passed its cold-read loop after clarifying the four-check limit and the planning scope. A
separate report cold read passed, including the treatment of the failed attempt and public-repository hygiene.
