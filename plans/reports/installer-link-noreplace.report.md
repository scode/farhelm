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
