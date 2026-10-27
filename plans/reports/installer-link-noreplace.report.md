## What this was about

During Mac installation, another program could put a file at the `farhelm` Terminal-command location after the installer
checked it. The final link placement could overwrite that file. Migration from the older layout could also delete a
replacement at either old command's name after checking the previous contents.

The decision was to close those races with a small change, and document the weaker protection when installing an older
release through its original published installer. Both outcomes are implemented in separate draft PRs.

## Things you should know

The installer creates the Terminal link directly at its public name, which refuses an occupied name. On a late arrival
it leaves the app installed and the new file untouched, names the path and asks for another installer run. It also
reports any foreign file already moved aside earlier in that run. The app's updater handles this like its existing
directory refusal; its behavior is unchanged.

Both recorded old-layout command copies move to visible replacement names before their contents are checked. Only
matching recorded contents are deleted; a replacement is kept and reported. The implementation stays in one placement
function plus a test hook: 59 source lines added and 38 removed, including documentation and indentation changes, a net
increase of 21. It adds no retry loop, volume fallback or identity registry.

The specification now states that selecting an older release runs its own published installer with the protections it
had then. The current boundary is `0.31.1-dev.0`: a release older than that, including stable `0.31.0` and earlier, is
handed to its original installer. Those installers can still delete an unrelated app put at Farhelm's usual app location
while they build. This accepted limitation has no code change. The unused `.farhelm-link.<pid>` reservation remains in
the specification, as the plan permits.

## Open questions and possible follow-ups

None required by this plan. No live installation, service or running session was changed.

## The PRs

- [#2739](https://github.com/scode/farhelm/pull/2739/changes) — preserve late Terminal-command arrivals and old-layout
  replacements; draft.
- [#2741](https://github.com/scode/farhelm/pull/2741/changes) — document older published installers' weaker app
  protection; draft, based on #2739.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(installation::install::tests::)'` through the recorder: 21
  passed, 1306 selected out, run `9fb6df49-0e8c-4649-b266-a0ee0c5092ac`. Pinned nextest 0.9.148, four slots, zero
  retries, no tmux needed. This covers late arrivals, replacements of either legacy copy, preservation notes on refusal
  and interruption/retry behavior.
- `cargo build -p farhelm`, then recorded `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm`: 603 checks,
  zero failures, run `9f0226e8-37ad-46c5-be50-6e42d12f538d`. Linux host with a Darwin/arm64 uname shim and private
  fixture homes. Covers the real shell-to-binary handoff, migration, ownership refusals and closing messages. No native
  Mac or Alpine/BusyBox run is claimed.
- Both runtime records have complete source capture and output, stream EOF, successful cleanup and no runtime skip
  markers. An earlier 20-test run passed before the preservation-note correction; it is superseded by the complete
  21-test command above.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, targeted
  `dprint check`, `git diff --check`, and `python3 releasing/check-changelog.py format` passed. The isolated
  `python -B scripts/check-test-sleeps.py` inspected 322 delays with no missing rationale.
  `python3 scripts/check-agents-md.py` passed with zero errors and two existing unrelated citation warnings.

The module tests and Clippy covered the final Rust patch on base `7497b4f9`; the shell run used the built PR1 code at
`71c29a4b`. Later changes were the documentation-only second outcome and execution-ledger PR links. Those successful
commands are reused individually. Fresh main `6c787d47` differs from the base only in queue state and another plan's
report, whose complete diff was inspected; neither affects this stack. No rebase or additional runtime run was needed.
No failed command is reused.

Browser, desktop and whole-workspace suites were skipped because no UI or updater behavior changed and the targeted
installer module and shell harness cover the affected operation. PR2 needed only Markdown checks. Native macOS-specific
filesystem behavior was not exercised locally; placement uses ordinary symlink creation rather than platform-specific
rename flags.

## Review gate's outcome

Each PR received its required single fresh-context gpt-6.1-sol review at high effort. PR1's reviewer found one missing
preservation note on a later migration refusal; it was fixed, tested and rechecked by the same reviewer with no
remaining findings. PR2 had no findings. The separate documentation pass and both commit/PR wording cold reads are
complete.

### Landing

Landed as built. During a Mac installation, a file another program put at the `farhelm` Terminal command's location
after the installer had checked it could be overwritten when the installer placed its link, and migrating from the older
layout could delete a replacement put at the name of either old command (`farhelm` or `farhelm-desktop`). The installer
now creates the link directly at its public name, which fails instead of overwriting, and moves the old commands aside
before checking them, so a late file is kept. When that happens the app is still installed, the Terminal command is not
created, and the installer names the path and asks for another run. The specification now also says that choosing a
release older than the current installer's handoff floor runs that release's own published installer, with the weaker
protections it had then.

- #2739: keep a file that arrives at the Terminal command's location during installation.
- #2741: say in the specification that older releases install with their own installer's protections.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained the Steel foundation (the interface restyle) and review-queue and triage records;
none of them touches installation. A fresh-context reviewer and I read all four plans of this round together and against
what main gained, and found nothing that interacts.

#### Fixes made at landing

None.

#### Things you should know

The triage decision asked for a rename that refuses to replace an existing file, with a fallback for disks that lack
that option. The plan instead creates the link at its public name directly, which refuses an existing entry on every
disk and needs no fallback. The effect is the one you asked for; only the means differ.

#### Checks

This round landed four plans together, in this order: no overwrites during the Mac installer's command link placement
(installer-link-noreplace), folder identity for Restart with's automatic trust (restart-with-trust-identity), a link
refusal kept below an open session confirmation (link-refusal-confirm-layer) and clearing a stale Resume target when a
Pi or OMP reporter cannot be prepared (named-restart-reporter-clear). All four were built before the Steel foundation
landed earlier the same day. This round ran the following on all four stacked together on main with the Steel
foundation. The review queue's index and the triage record conflicted mechanically between these plans and entries main
had added; the resolutions keep main's entries, apply each plan's own update to its triage entry, and remove from the
index exactly the five findings these plans address (two for the installer plan, one for each of the others).

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason, `dprint check`
  across the repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two
  warnings, both about citations main already had), the workspace debug build and the UI's release web build: all
  passed.
- The tests of the UI's own JavaScript and stylesheet (`node --test` in `crates/farhelm-ui/js-tests`), run
  `ff182543-0d7a-46fb-a5f8-3b3c6214ad44`: 358 passed.
- The terminal link-click browser spec, `npx playwright test tests/terminal-link-clicks.spec.ts` in `e2e/`, on Chromium
  and WebKit, three workers each with its own helm and supervisors, no retries, run
  `eff1253f-b467-4151-bf3b-9f5081e7c1b3`: all 34 passed, including the new tests that keep a Delete confirmation's
  warning above a link refusal in both opening orders, now with the Steel restyle's buttons and fonts.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, four tests at a time, no retries, run `8433a77d-dde8-4a20-95c7-b1bd78c1f791`: 4455 passed, 40 skipped by design
  (helper entries that only run as child processes of other tests, and tests that need a real signed-in Claude or
  Codex), and 1 failed: the terminal backpressure test "shallow pause resumes without reset or replay", a known flake
  with three entries in the flake log (the latest records it failing 2 of 3 times when run alone). Run alone on the same
  code three times, it passed each time (runs `a3b3ea0f-39c8-45ee-be09-e64b81ed92a3`,
  `76989247-159f-4240-9c5b-325259d1109c` and `b525f8e0-98e9-41b4-a217-3984ca31bf47`). No plan in this round touches the
  terminal's flow control. The deflake sweep skips this test on behalf of a TODO.md Deflake entry that does not exist;
  either add the entry or drop the skip.
- The Mac installer fixture, `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm` after
  `cargo build -p farhelm` (it runs the installer against fake releases on Linux, standing in for macOS), run
  `a4f7acf6-7f4f-490e-b15f-e76c5143a4cf`: all 603 checks passed.
- Reused: none. Skipped: the full browser suite (only one plan changes the UI, and its spec file ran on both engines),
  the desktop app checks (no desktop code changed), real Pi, OMP, Claude and Codex runs (the reports say what their
  tests stand in for), and a real Mac installation (not runnable here).

The report's checks were the installer's own tests; this round also ran the whole Rust workspace and the installer
fixture with the other three plans. Nothing in the executor's report became untrue at landing.
