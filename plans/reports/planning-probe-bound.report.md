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

### Landing

Landed as #2415, with the same content the report describes, rebased onto this round's main (see the end of these
notes). It was the tenth of sixteen plans in this landing round, after the nine listed in the earlier plans' notes, most
recently tab-attach-no-wait.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the nine plans earlier in this round, and documentation and
plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. The one
change near it is helm-lock-outlives-work, which keeps the helm's state-folder lock until the helm has finished shutting
down. This plan only caps an existing wait for a host check's diagnostic output at two seconds and then cancels it; it
takes no lock, so it cannot hold up a shutdown beyond that and does not interact with the lock change. A rule added to
SPEC_impl.md during this round prefers cleaning up on quit over sweeping leftovers at the next start; this plan leaves
nothing behind to clean up or sweep. The only conflicts were in the review feedback queue's index, where each plan
removes its own entries; all removals are kept.

#### Fixes made during the landing

None.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

The report was written on a main from before round 107's session-identity change, so its statement that no rebase was
needed and its focused checks no longer describe what landed; the combined runs above supersede them. Nothing else in
the executor's report became untrue at landing.
