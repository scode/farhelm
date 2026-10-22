### What this was about

Five findings from the 2026-10-21 triage of the highest-priority review feedback, all settled without code changes.

Two of them were about prepared checkouts (the "keep a checkout ready for faster launches" setting). Farhelm keeps those
in a reserved hidden folder, `.farhelm-spares`, inside the checkout root. A code review found two ways Farhelm can
delete a folder there that someone else replaced, losing whatever was in it. First, a launch that takes a prepared
checkout renames whatever sits in that checkout's place to a name Farhelm treats as its own launch leftovers, without
checking that it is the folder Farhelm built, and a later supervisor startup deletes those leftovers. Second, the
cleanup after building a prepared checkout deletes its build folder even after noticing that the folder was replaced.
The specification promised that Farhelm never deletes a folder replaced behind its back, including inside this hidden
folder. You decided this was "just fix spec, accept it", with one principle covering both.

The third was about the error a session shows when its agent never starts, which can quote the last few lines of the
terminal. The specification promised to leave that quote out whenever Farhelm could not be sure the text came from the
failed launch. A review found, from tmux's source, one case where it can still quote a few lines of the agent's previous
run: the session's agent is restarted in the same terminal, and that previous run still had a full-screen program
showing when the terminal was narrowed. You chose to accept it in the spec. Triage had judged it a misleading diagnostic
rather than a privacy problem: the quote stays in that session's own error, and agents reading the session's status from
other hosts see only the word "error", so the old text reaches no one who could not already read that terminal.

The last two were discards. You chose to fix only what is super cheap with no real downside, and neither was:

- Repository labels Farhelm shows for sessions, including on the summary card, can look identical when a repository's
  metadata is malformed. Launching rejects such names, and every session action is tied to the session rather than to
  its label, so no harmful choice was shown; changing how every repository label is drawn would have a visible cost.
- When two attempts to finish an interrupted move of a checkout into Farhelm's trash overlap and pick the same place for
  it, the checkout could stay on disk but drop out of the trash Farhelm shows; no file contents are lost. That needs a
  filesystem that reports "destination exists" before "source missing". Linux checks the other way round, macOS's source
  did not settle it either way, and no supported filesystem with that order was found.

### Things you should know

- SPEC.md now says that inside `.farhelm-spares`, anything Farhelm did not put there, including a folder removed and
  recreated, is out of band, and Farhelm may treat it as its own and delete it along with its contents. The protection
  against deleting a replaced folder still applies everywhere else (managed checkouts, checkout roots, the trash), and
  the removal still never follows symlinks or crosses mounts. The new wording sits in the dedicated-ownership paragraph
  under Supported user environments, with a pointer from the prepared-checkout paragraph under Managed checkouts. The
  implementation notes now say which steps rely on it.
- SPEC.md's failed-launch excerpt rule now says the quote can, in rare cases, include text from the previous run in the
  same terminal, with the narrowed-while-full-screen restart as the example, accepted 2026-10-21. The implementation
  notes explain the tmux behavior and say it was read from tmux 3.7c's source and not reproduced. The reviewer of #2565
  re-read that source and agreed with the mechanism; nobody ran it.
- Nothing changed in code, so nothing Farhelm does changed; these PRs make the specification describe what it already
  does.

### Open questions and possible follow-ups

- A few code comments still describe the old promises and now disagree with the specification. Two comments in the
  prepared-checkout cleanup code say a prepared checkout is "never adopted", and two comments in the terminal code
  describe the failed-launch quote as containing only the failed launch's own output. I left them alone because this
  plan ruled out any code change. I recommend correcting them in a small comment-only follow-up, as both reviewers
  suggested, so a future reader of that code is not misled into treating the old promises as still required.

### The PRs

- [#2563](https://github.com/scode/farhelm/pull/2563/changes): specification accepts that Farhelm may delete replaced
  folders inside `.farhelm-spares`; closes both prepared-checkout findings.
- [#2565](https://github.com/scode/farhelm/pull/2565/changes): specification accepts that a failed launch's error
  excerpt can quote the previous run; closes that finding.
- [#2566](https://github.com/scode/farhelm/pull/2566/changes): closes the two discarded findings, with no spec or code
  change.

### Checks

- Run: `dprint check` on every changed file, and `python3 scripts/check-agents-md.py` after each PR's queue removals;
  both clean.
- Reused: none.
- Skipped: every runtime test (Rust, browser, JavaScript, installer). All three PRs change only specification text and
  review-queue bookkeeping, which the repository's validation rules exclude from runtime tests.

### Review gate

Each PR had one fresh Opus 5.5 review at high effort. None found a correctness problem; each checked the new
specification text against the code, and the reviewer of #2565 also checked it against tmux's source. The review of
#2563 caught a wording mismatch with SPEC.md's own terms and a sentence that credited the deletion to the launch rather
than the later startup cleanup; both were fixed. The review of #2565 made the tmux mechanism and the SPEC.md example
more precise (it takes a narrowing, not any resize). Both suggested correcting the stale code comments above; I did not,
for the reason given there. The review of #2566 found nothing to change.

### Landing

Landed as #2563, #2565 and #2566, merged bottom-up, with the same content the report describes. It was the first of five
plans in this landing round, which stacked all five and checked them together before any merged: this one,
cheap-ui-hardening-oct21, start-remedy-quoting, uninstall-mixed-spelling and refused-retry-rollback-fence.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained popover-anchor-close (menus closing when their rows move) and documentation commits.
None of them touches the prepared-checkout or failed-launch-excerpt rules this plan's specification text describes. A
fresh-context reviewer and I read the new text against the code and against the other plans' specification changes in
this round: it matches what the code does (a launch that takes a prepared checkout does not check who created its
folder, as the new text allows), and no other plan in the round edits those paragraphs.

#### Fixes made during the landing

None to the plan's changes. The review-queue index conflicted with entries main had changed; the landing rebuilt it from
main's index minus the entries whose files the round's plans delete.

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

The plan changed only documentation and its report ran only documentation checks; the round's checks above cover it
together with the other plans. Nothing in the executor's report became untrue at landing.
