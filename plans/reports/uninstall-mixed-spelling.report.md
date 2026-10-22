### What this was about

Uninstall from the hosts panel recently learned to accept a host whose home folder is reachable at two paths, such as
through a bind mount (the uninstall-aliased-home plan, [#2551](https://github.com/scode/farhelm/pull/2551/changes),
which landed but has not shipped in a release yet). To do that, the inspection uninstall runs on the host before
removing anything rewrites each path it looks at from the second home path to the login home's own path, keeping a
rewrite only when the shell confirms both name the same file. A trailing review found that because this is checked one
path at a time, one contrived layout gets past the rule that uninstall never removes the host's Farhelm data: the bind
mount leaves out a mount nested inside the home, the data folder lives under that nested mount, and the `~/.local/lib`
folder that holds Farhelm's install folder is a symlink into the bind. The install folder's path is rewritten to the
login home, the data folder's is not, the two no longer look nested, and uninstall would delete the data along with the
install folder.

SPEC.md already says uninstall "still refuses layouts it cannot tell apart safely", so triage recorded this as a code
fix without asking you, under the rule for findings the spec already settles, with a size gate of about 100 lines and
"over it, block and bring it back". On 2026-10-21, before triage moved it to that already-settled group, you had put
this item with the ones to "fix what is super cheap, discard the rest"; triage told you it was recorded as a fix and
that you can revise it.

### Things you should know

- The inspection now also reports every path exactly as the host resolved it, before any rewrite, and refuses with its
  usual "data directory ... is inside ..., which uninstall would remove" message when the data folder is inside the
  install folder in either form (rewritten compared with rewritten, original compared with original). The checks that
  the running Farhelm program is inside the install folder and that the service file is the one uninstall removes are
  unchanged: they test for a match, so a pair left in different forms can only make them refuse.
- No layout uninstall correctly accepted before is refused now. The original form is exactly what uninstall compared
  before the alias support existed, so the new refusal only covers layouts that were refused then too. The review gate
  agent checked this and agreed.
- A data folder that does not exist yet also stays in its original form (the shell has nothing to compare), so if one
  would sit inside the install folder, uninstall refuses that too. That case was refused before the alias support as
  well. The regression test uses it, because the real nested-mount layout needs mount privileges the test fixture does
  not have.
- The size gate was exceeded on a strict count, and I did not block. The final diff is 126 inserted and 34 deleted lines
  of code, tests and implementation notes, 160 changed lines against about 100. The fix itself is about 25 lines, which
  I read as "super cheap" in your sense; the rest is the regression test the triage required, an updated test of the
  inspection's output, and comments and spec text, some of which the review gate asked for. I delivered rather than
  blocking because the gap exists only on main, between #2551 and this PR: no release has it, but a release cut from
  main while this sat blocked would ship it, and landing the fix means it never ships.
- The changelog: the alias support has not shipped, so instead of a new entry this PR adds a context paragraph to the
  alias-support fragment, saying the gap existed only between the two PRs and needs no entry of its own.

### Open questions and possible follow-ups

- If you hold the size gate strictly, this landed against the agreed "block and bring it back". A follow-up can revert
  it, and the finding goes back to you. Trimming cannot reach about 100 changed lines while keeping the regression test
  the triage required: dropping the implementation-notes sentence and most new comments saves about 25 lines.
- The repository's check for unexplained delays in tests could not finish here (see Checks). Re-running
  `scripts/check-test-sleeps.py` on a quieter machine would close that gap; the diff adds no delay calls.

### The PRs

- [#2574](https://github.com/scode/farhelm/pull/2574/changes): uninstall refuses when the data folder is inside the
  install folder in either form; regression test; implementation-notes and changelog-fragment updates; closes the review
  finding.

### Checks

- Run: recorded nextest runs of the helm's uninstall unit tests
  (`cargo nextest run -p farhelm-helm --lib -E 'test(uninstall)'`, 30 tests). Run `e67fa02c` failed one test of the
  inspection's output whose hand-built output still had the old field count, which this change had to update; run
  `8672f4d3` passed all 30 after that. Run `22096300` ran the new test with the new check switched off, and it failed
  because uninstall went on to plan the removal, showing the test catches the bug. `cargo fmt --check` and
  `cargo clippy -p farhelm-helm --all-targets -D warnings` clean; `dprint check` on the changed Markdown,
  `releasing/check-changelog.py format` and `scripts/check-agents-md.py` clean.
- Reused: run `8672f4d3` for the final commit; the only Rust changes after it are comments.
- Incomplete: `scripts/check-test-sleeps.py` could not finish (its own 60-second budget ran out twice while the
  machine's load average was about 36).
- Skipped: the macOS installed-uninstall acceptance script was not run, because this machine is Linux. No browser,
  desktop or wider Rust runs, since the change is confined to how the helm decides what uninstall may remove, and the
  selected tests cover it.

### Review gate

One fresh Opus 5.5 review agent at high effort. It found no correctness problem; it checked the inspection's output, how
the helm reads it, every place the new original-path value is filled in, and the test against the repository's
test-authoring checklist, and it found the size defensible. Its six findings: two stale code comments; the placement and
wording of the implementation-notes sentence; a wrong description of which folder is the symlink in the commit message
and changelog paragraph; a one-line miscount of the deleted lines in the triage record's size; ragged comment wraps; and
a nit about the order in which the test's fake host builds its path answers. All but the last were fixed; that one was
left as is to keep the diff small.

### Landing

Landed as #2574, with the same content the report describes. It was the fourth of five plans in this landing round,
after triage-specs-oct21, cheap-ui-hardening-oct21 and start-remedy-quoting; refused-retry-rollback-fence followed it.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained popover-anchor-close and documentation commits, neither touching host uninstall. A
fresh-context reviewer and I read this plan against uninstall-aliased-home, the change it follows up, and the round's
other plans:

- It stays consistent with uninstall-aliased-home's alias handling: the checks that compare paths for a match are
  unchanged, and the new refusal compares the original spellings the way uninstall did before the alias support.
- It also closes the mirror case uninstall-aliased-home's landing notes recorded as a follow-up: with a bind mount of
  the home inside Farhelm's install folder and the data folder reached through it, the data folder's original path is
  still inside the install folder's original path, so uninstall now refuses. Neither report says so, and nothing else
  tracks that case, so it needs no further follow-up.
- The other plans in the round do not touch host uninstall.

#### Fixes made during the landing

None to the plan's code. The review-queue index conflicted with entries main had changed; the landing rebuilt it.

#### Checks

Together the five plans change about 650 lines of code, tests and specification text (more with review-queue
bookkeeping) across the helm's connection errors and uninstall, the supervisor's create path and store, and the UI,
including how every menu remembers its focused item and the terminal's file-link script; that reach is why this round
ran the whole Rust workspace and the full browser suite. All of the following ran on the five plans stacked together on
this round's main, including the landing's conflict resolutions. After the run, the landing moved changelog text between
fragment files in cheap-ui-hardening-oct21's last PR (see its notes); that changed only changelog files:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on the changed Markdown files, `python3 scripts/check-agents-md.py` (one warning,
  about a citation main already had), the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh` (which also builds the desktop app): all passed.
  `python3 releasing/check-changelog.py format` passed again after every changelog change.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  slots, no retries, run `461bfc4c-651d-4a70-8498-7a9b5ae524cf`: 4291 passed, none failed; 40 tests marked as ignored
  did not run.
- `cargo nextest run -p farhelm-ui --features desktop`, run `4073dcb5-a14f-42e5-bddd-8b007925da68`: 900 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `4086a6d6-96a5-4691-b05e-e50169546906`: 329 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `b5e88d8c-8a25-4e9b-be8f-ada4f9d48bc3`: 2475 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), 1 failed: on WebKit, "don't ask
  again on a refused YOLO replace marks the host, then replaces with the override", the open flake FLAKES.md recorded on
  2026-10-20, which the deflake sweep already excludes. It exercises a host's YOLO question during Replace, which none
  of the five plans touches; refused-retry-rollback-fence changes only the supervisor's handling of a stale create
  retry.
- An earlier start of these checks was stopped at its build step and discarded, because the landing changed the stack
  while it ran; nothing from it is counted.
- Reused: none. Skipped: the website build (no docs page changed), the doctests (no documentation example changed), the
  desktop smoke test (the desktop app crate is unchanged, and the menu marker fix is covered by the desktop-feature unit
  tests above, which mount real menus), the native Mac app (not runnable here) and the installer (unchanged).

The report's checks were the helm's uninstall tests; its follow-up that the test-delay check could not finish there is
closed by the round's run above, where that check passed. Nothing in the executor's report became untrue at landing.
