# Report: keep the copyable supervisor-start command intact

## What this was about

When the helm connects to a remote host over SSH and finds Farhelm installed there but no supervisor running, the host's
connection error suggests a command to start one, built from the host entry's configured Farhelm program and state
folder. The helm quoted those paths correctly for the shell, but then escaped the whole error text a second time (that
escaping is what keeps a host from planting terminal control characters in the text the helm shows and logs). The second
escaping doubles every backslash. So a configured path with a backslash, or with an apostrophe (which shell quoting
writes as `'\''`), was shown as a different command. Pasting it could start Farhelm on a different folder, and a
trailing review traced that to possibly migrating an older, inactive installation's database there. The code's own test
used a state folder with an apostrophe and passed, because it checked the command before the second escaping rather than
the text the user actually sees.

During triage on 2026-10-21 you chose to fix what is super cheap with no real downside and discard the rest; this item
was estimated at 20 to 50 lines, with a size gate of about 100 lines (over it, discard).

The plan's fix, which is what landed: the helm shows the exact command only when the second escaping leaves it
unchanged. Otherwise it shows the descriptive instruction it already used for overly long paths: it shows only the bare
`farhelm supervisor run` and tells the user to add the host entry's configured Farhelm program and state directory
themselves. Ordinary paths, spaces included, still get the exact command. The escaping itself is untouched.

## Things you should know

- Any configured program or state folder containing an apostrophe, a backslash, a double quote or a control character
  now gets the descriptive instruction instead of a pasteable command. For an apostrophe that is unavoidable with this
  shape (shell quoting always introduces a backslash), and the triage entry accepted it.
- Size: about 107 changed lines of code, tests and docs (the code change itself is about 20; the rest is the regression
  test the completion criteria ask for, plus one adjusted sentence in SPEC_impl.md). That is at the edge of "about 100",
  and I counted it as within the gate.
- SPEC_impl.md's description of SSH exit status 75 (the status Farhelm on the host reports when it is installed but no
  supervisor is running) said the error "supplies the start command". It now says the helm supplies it, or describes it
  when the shown command would not be exact (too long, or altered by the escaping). SPEC.md needed no change.
- The alternative of exempting the command from the second escaping would keep the exact command for every path, but it
  would carve an exception into the protection against host-supplied text, so I did not take it.

## Open questions and possible follow-ups

- The review found a second start-command suggestion with the same double escaping: the message the helm adds when an
  SSH channel closes before the supervisor handshake completes. According to the reviewer, real SSH connections do not
  reach that message today, only scripted test connections. I did not verify that independently, and fixing it would
  have pushed this PR over its gate. My recommendation is a small follow-up of the same shape only if a check confirms
  real connections can reach it; otherwise it is not worth the change.
- A backtick in a configured path survives the escaping. The suggested command is shown between backticks, so a reader
  could take a backtick in the path as the end of the command. The reviewer found the app does not split the text on
  backticks, and a backtick is literal inside the shell quoting, so the pasted command is still the right one if copied
  whole. I left it alone.

## PRs

- [#2567](https://github.com/scode/farhelm/pull/2567/changes) `fix: never suggest an altered supervisor start command`:
  the fix, its regression test, the SPEC_impl.md sentence, the changelog fragment, and the triage bookkeeping (feedback
  file and index entry removed, Execution recorded).

## Checks

Run now:

- `cargo nextest run -p farhelm-helm --lib -E 'test(start_remedy_is_exact_after_the_final_escaping_or_descriptive) | test(ssh_natural_exit_categories_preserve_the_remote_evidence) | test(long_remote_paths_get_descriptive_instead_of_truncated_commands)'`
  (the tests of the suggested start command and of the SSH exit-status diagnostics), through the test recorder: run
  `fd8b58bc-3c06-4e6a-b539-92e99ccfbcd0` on the first version, and run `bda013b2-f27a-48ec-ad63-7cd7ddb81577` on the
  final version after the review fixes. Both 3 of 3 passed.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`: clean, on the first version and again on the final one.
- `cargo fmt --all`, `dprint check` on the changed files, and `python3 releasing/check-changelog.py format`: clean.

Reused: none.

Skipped:

- The workspace-wide Rust battery, browser tests and desktop checks: the change is confined to one helm function that
  composes error text, and the selected tests cover every path through it.
- `cargo clippy -p farhelm --bins`: the shipped binary's configuration differs only by a supervisor test feature that
  this change does not touch.

## Review gate

One fresh-context reviewer (Opus 5.5, high effort, reviewing correctness, design and idiom, with the repository's
test-authoring checklist) found nothing blocking. I made two of its low findings' fixes: the check now calls the same
escaping function the error uses instead of reproducing it, and a test docstring that no longer matched (it promised the
configured paths for exit 75) was reworded. Its third finding is the follow-up above. Its fourth agreed with leaving
backticks out. The reviewer confirmed the size is within the gate but close to it.

### Landing

Landed as #2567, with the content the report describes and one changelog change described below. It was the third of
five plans in this landing round, after triage-specs-oct21 and cheap-ui-hardening-oct21; uninstall-mixed-spelling and
refused-retry-rollback-fence followed it.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained popover-anchor-close and documentation commits. A fresh-context reviewer and I read
this plan against them and the round's other plans:

- popover-anchor-close's landing notes recorded that a host's changing connection-attempt line closes menus below it.
  The start command this plan changes is the same on every attempt, so it adds nothing to that, and no UI code reads the
  command.
- The other plans in the round do not touch the helm's connection errors.

#### Fixes made during the landing

- The plan's changelog fragment described a fix to the suggested start command that connection-stall-phase (#2505) added
  on 2026-10-21, after the last release (v0.30.0, 2026-10-18), so it has not shipped. Following the repository's rule
  for fixes to unreleased work, the landing deleted the new fragment and added its content as a context paragraph to
  connection-stall-phase's fragment, in this PR.
- The review-queue index conflicted with entries main had changed; the landing rebuilt it.

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

The report's checks were the helm's focused tests on its base; the round ran the whole Rust workspace and the browser
suite above. Nothing in the executor's report became untrue at landing, apart from the changelog fragment moved as
described above.
