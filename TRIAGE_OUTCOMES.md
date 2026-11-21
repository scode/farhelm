# Triage outcomes

## repository-search-reports-empty-before-reply.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. Claim: a partial repository query says "no repository matches" before the host has answered. Current main
  confirms it. `crates/farhelm-ui/src/list/create_form.rs:4129-4144` debounces 150 ms and then awaits
  `fetch_github_repositories`. While that is pending, `repository_result` is `None` and the suggestions are empty.
  `create_form.rs:4254-4259` builds the field's message by calling
  `empty_search_message(&format!("gh:{repository_text}"), false, false, false)`.
  `crates/farhelm-ui/src/launch_composer.rs:948` returns "no repository matches; enter owner/repo" unconditionally for a
  nonempty `gh:` value, and `create_form.rs:7001` shows that message whenever the list is empty. The `gh:` search scope
  (`create_form.rs:4266-4276`) also gets no repository-loading flag. The helper's own contract
  (`launch_composer.rs:917-922`, "Loading flags mean a read is still pending, not that a completed read failed") already
  handles pending reads this way for models ("loading models…") and templates ("loading templates…"). Repository
  discovery is the one pending source with no flag. SPEC.md:3228-3233 accepts that a repository search can be slow, but
  it does not accept a negative answer shown before the search finishes.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: give `empty_search_message` a repository-loading input. Make its `SearchScope::Github` branch,
  for a nonempty value, return a loading message (wording parallel to "loading models…") while discovery for that entry
  point's current query is pending, which covers both the debounce and the wait for the host. Keep "no repository
  matches; enter owner/repo" for a completed reply for that query with no rows. Pass the flag from both callers in
  `create_form.rs`: the dedicated repository field, and the `gh:` search scope's `search_message`. In each, compute it
  as: a live repository authority whose query is this entry point's query, with no accepted result yet. Errors keep
  their existing note. Extend `empty_search_messages_cover_labels_and_loading` in `launch_composer.rs` with the
  repository-loading case, and with a case showing that the negative message still appears when not loading. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over it, block and bring it back. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/launcher-repository-search-loading.md`.

## sticky-parent-guard-contradicts-own-docs.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. Claim: the recorder's and deflake's parent-directory checks accept any sticky parent, which contradicts
  their docs' promise to refuse a `/tmp/farhelm-tmp` that another account created first. Current main confirms it.
  `scripts/record-test-run.py:304` returns `st_uid in (euid, 0) or S_ISVTX`. Its docstring (296-300) says that in a
  sticky directory "only an entry's owner may rename or remove it", and names exactly the case where "another account
  could create `/tmp/farhelm-tmp` first". `deflake/bin/deflake:110` has the same check, and its docstring (96-102)
  promises refusal of "a `farhelm-tmp` it owns". `docs/test-run-evidence.md:120-121` says "otherwise another account
  that created the fixed-name directory first could swap or read the evidence." On Linux, the owner of a sticky
  directory can rename or unlink any entry in it. A sticky directory owned by this user or by root already passes the
  uid test, so the sticky clause only ever admits a sticky directory owned by another account, which is the case the
  docs promise to refuse. `scripts/test-record-test-run.py:1768-1793` asserts that acceptance (a stranger-owned 1777
  parent is safe) and repeats the wrong rationale in its docstring. SPEC_impl.md:5295-5297 ("Development tooling assumes
  a single-user machine… need not defend against one") keeps this out of `highest`. It does not make the docs' claim
  true.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: in `safe_parent` (`scripts/record-test-run.py`) and `prepare_state_root` (`deflake/bin/deflake`),
  accept the parent only when this user or root owns it, dropping the sticky alternative; root-owned `/tmp` still
  passes. Fix both docstrings' sticky rationale. In `docs/test-run-evidence.md:120-121`, replace "or be sticky like
  `/tmp`" with the owner-or-root rule. Change `test_roots_owned_by_another_user_are_refused` in
  `scripts/test-record-test-run.py` to assert that a stranger-owned sticky parent is refused while a root-owned parent
  passes, and correct its docstring. Add the matching refusal case to `deflake/test-deflake.py` if it covers
  `prepare_state_root`. (If the maintainer prefers a docs-only correction that cites the SPEC_impl single-user rule, the
  same files are touched and the test docstring is still corrected.) Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove
  the feedback file and its index entry.
- Execution: planned in `plans/queue/sticky-tmp-parent-refusal.md`.

## folder-refusal-classifier-matches-path-text.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. `FolderProblem::recognize` (crates/farhelm-ui/src/list/create_form.rs:1354-1366) checks the three supervisor
  phrases in a fixed order (Missing first) with `contains` over the whole refusal. The supervisor appends the literal
  cwd after its phrase (crates/farhelm-supervisor/src/service/core.rs:3551, 3556, 3574). A not-a-directory refusal for a
  path whose text contains "working directory does not exist: " is therefore classified as Missing, and the folder field
  says "This folder does not exist on <host>" for an existing file. The type's own docs say the marking "only points at
  the field the refusal is about", which means the classification must follow the supervisor's actual phrase, not text
  inside the path. The marking authorizes nothing, so the impact is an advisory annotation only.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: `recognize` picks the phrase that occurs earliest in the message (the supervisor's own sentence
  comes before the path it appends), not the first phrase in list order. Extend
  `folder_refusals_are_recognized_by_the_supervisor_words` with a not-a-directory refusal whose path contains the "does
  not exist: " phrase, and an unusable refusal whose path contains another phrase, each asserting its true problem. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over it, block and bring it back. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/launcher-folder-refusal-order.md`.

## old-process-regression-misses-cli-premise.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. `test_old_running_process_cannot_pass_candidate_relaunch`
  (releasing/mac-vm-test/test_guest_operations.py:195-204) has the docstring "Installed bytes alone do not prove that
  Restart actually returned the candidate process", so it means to isolate the running-version predicate. Its fixture
  omits `cli`, and `managed_result` (guest_tester.py:291-294) raises the same "managed running-version premise was not
  established" error when the CLI stdout is missing. The test therefore stays green even if the
  `version not in versions['running_versions']` clause were removed. The sibling test at :220-229 shows the intended
  isolation style, with the other fields correct and only one wrong.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: Add `'cli': {'stdout': 'farhelm 0.30.0\n'}` to the fixture's versions observation, so the old
  running version (`['0.29.0']`) is the only premise that fails. The test still expects the running-version error, and
  removing the running-versions clause from `managed_result` would now make it fail. Run the mac-vm-test Python unit
  tests through the repository's recorded test-run path. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/brick-test-guard-oracles.md`.

## installer-prerequisite-test-conflates-two-guards.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. `test_old_installer_waits_for_both_fixture_and_verified_inputs`
  (releasing/mac-vm-test/test_guest_operations.py:145-151) has the docstring "A clean Mac alone cannot justify
  overwriting an old or dirty Linux fixture later in setup". It dispatches `install_previous` with neither prerequisite
  completed, so it passes whether the guard at guest_operations.py:265-266
  (`not {'fixture_clean', 'verify_previous'} <= self.completed`) requires one prerequisite or both. The test's name and
  docstring promise that each one is independently required, which the fixture cannot show.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: The test (or two subtests) dispatches `install_previous` once with only `fixture_clean` in
  `operations.completed` and once with only `verify_previous`. Each must raise the "clean and verified" error, with the
  runner set to fail if it is invoked. Keep the existing none-completed case or replace it. Run the mac-vm-test Python
  unit tests through the repository's recorded test-run path. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the
  feedback file and its index entry.
- Execution: planned in `plans/queue/brick-test-guard-oracles.md`.

## shutdown-wait-test-does-not-witness-waiting.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. `the_stop_wait_is_bounded_and_covers_launches_still_starting`
  (crates/farhelm-supervisor/src/service/core.rs:~27112-27166) has a stated purpose in its inline comment: "the wait
  must not end while it is in flight, and must pick it up once it registers". It takes a `starting_launch()` guard,
  spawns a task that sleeps 200ms, writes the mark, calls `track_pending_hold`, then drops the guard. It then only
  asserts that `wait_for_pending_holds` returns true. A broken wait that ignored `starting_launches` would find nothing
  pending and return true at once, so the test would still pass. The function's own doc (core.rs:~7003) promises: "the
  wait does not end while any are in flight".
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: Make the test fail if the wait returns before the in-flight launch registers. For example, have
  the registration task set a shared flag (e.g. an `Arc<AtomicBool>`) after `track_pending_hold` and before
  `drop(starting)`, and assert the flag is set as soon as `wait_for_pending_holds` returns, before joining the task. Do
  not rely on `JoinHandle::is_finished`, which can race the task's final poll. Update the test's comment to say what
  witnesses the wait. Optional confirmation: make the wait ignore `starting_launches` and check that the test now fails.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over it, block and bring it back. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/stop-wait-starting-launch-witness.md`.

## installer-file-symlink-test-masks-nofollow-removal.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. `files_are_replaced_whole_and_links_are_never_followed`
  (crates/farhelm-supervisor/src/installation/dirs.rs:~303-331) checks that `open_file(dir, "link")` fails. But "link"
  points at the temp directory, and `open_file` (dirs.rs:~95-111) also refuses any non-regular file after opening ("is
  not a regular file"). So the assertion "a link is never read through" would still pass with `OFlags::NOFOLLOW`
  removed. The `open_file` doc promises "refusing a symbolic link", and the test's doc promises "a ... link is never
  followed when classifying or opening".
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: Add a symlink to a regular file (e.g. to the existing "target") and assert that `open_file`
  refuses it, so the test fails when `NOFOLLOW` is removed from `open_file`. Keep the directory-link assertions for
  `open_folder`/`entry_kind`. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment); over it, block and bring it back. Remove the feedback file and its index
  entry.
- Execution: planned in `plans/queue/installer-symlink-file-oracle.md`.

## legacy-expectation-is-parsed-as-cli-option.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. `Expectation::render` (`crates/farhelm-supervisor/src/installation/mod.rs:714-721`) writes an absent id as
  `-`, so a two-field record yields `-:0:<installed>`. `scripts/install.sh:1189-1190` passes it as a separate argument
  (`set -- "$@" --expect "$FARHELM_INSTALL_EXPECT"`), and `InstallArgs.expect` (`crates/farhelm/src/main.rs:1440-1443`)
  is a plain `#[arg(long)]` without `allow_hyphen_values`. I checked clap_builder 4.6.7 (the version in Cargo.lock):
  `parser.rs:213-225,893-898` parses a following `-…` token as a short flag unless the pending option allows hyphen
  values, so the CLI rejects the value before `Expectation::parse` runs. SPEC_impl.md:4816-4820 requires the form:
  "Otherwise the record's id (none for a two-field record), its generation and the Installed version become the
  expectation the installer is run with, as `FARHELM_INSTALL_EXPECT=<id or ->:<generation>:<installed or ->`. ... A
  two-field record matched by a "no id" expectation is given its id by that run". The only test
  (`scripts/test-install-sh.sh:2826-2831`) uses expectations that have ids.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: `farhelm internal install --expect -:0:<version>` is accepted and reaches `Expectation::parse`.
  Either set `allow_hyphen_values = true` on `InstallArgs.expect` (the repo's existing convention for hyphen-leading
  values) or have `install.sh` pass `--expect=<value>`. Add a test that pushes an ID-less expectation through the CLI
  boundary: a clap parse test of `-:0:1.2.3`, or a `scripts/test-install-sh.sh` case with a two-field record and a
  matching `FARHELM_INSTALL_EXPECT` that asserts the install succeeds and the record gains an id. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over it,
  block and bring it back. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/installer-expect-hyphen-value.md`.

## older-development-archive-moves-replacement-folder.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claims
  a pending checkout-archive journal written by pre-fix code can, on recovery, move a replacement folder that reuses the
  recorded device/inode when birth time is unavailable. The finding itself marks the development-build remainder
  AMBIGUOUS and says real occurrence is unverified. The released-writer subcase may fall under FILTER.md "Upgrades from
  older releases combined with another rare condition", but development builds are not "an older Farhelm release".
  Current recovery in `crates/farhelm-supervisor/src/working_copies.rs` still moves the archive back.
- Decision: the user picked "Accepted, discard all" (2026-10-26 ten-question triage page, Q2), adding "and SPEC. We do
  not concern our selves with thold generation installers prior to 0.31.1 unless it is a highly catastastrophic problem
  that isn't in some super rare edge case."
- Completion criteria: SPEC.md (installation) states the principle: defects in the installers of releases older than
  0.31.1 (the handoff floor), and in state they or development builds left behind, are not addressed unless the problem
  is highly catastrophic and not a very rare edge case. Coordinate with `installer-link-noreplace` (its spec PR adds the
  related sentence that an older release's installer keeps the protections it had); one paragraph may carry both. No
  code change. Remove the feedback file and its index entry.
- Execution: `pending`.

## add-host-refusal-bypasses-dialog-admission.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: If
  Settings opens while confirmed host setup waits, a later refusal reopens the Add-host form outside the admission
  wrapper, and the two dialogs make each other inert. The trigger is a GUI–helm setup request whose reply arrives late.
  The finding rates this "likely", says browser behavior is unverified, and proposes two shapes (defer the reopen, or
  hand off the active modal).
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3).
- Completion criteria: SPEC.md's GUI concurrency text gains the principle: convenience state in the GUI (labels, marks,
  remembered folders and selections, dialog order) may lag behind a delayed reply or another client's change; only what
  a destructive action or a launch is shown and fenced against has to be current. No code change. Remove the feedback
  file and its index entry.
- Execution: `pending`.

## spare-test-fifo-write-can-outlive-reader.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  in `crates/farhelm/tests/spare_builder.rs` (FIFO at 747-750, `tokio::fs::write` at 775), the test can block in a FIFO
  open if the guarded shell exits before the test releases it. nextest's outer timeout still bounds the run. The code is
  unchanged on current main.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: planned in `plans/queue/spare-builder-fifo-release.md`.

## ensure-host-warning-names-unavailable-setup-action.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  startup log warning at `crates/farhelm-helm/src/ensure.rs:198-206` still tells the operator to "repair the stored host
  with its set up action". Per the finding, an ordinary remote host with no failed setup run is not offered that action,
  and Update refuses it. The correct replacement advice is not obvious from the code or the spec, because no repair
  route for a stored non-absolute remote path is clearly offered.
- Decision: the user picked "Fix each with the smallest truthful correction" (2026-10-26 ten-question triage page, Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the feedback file
  and its index entry.
- Execution: `pending`.

## quick-launch-preparation-refusal-uses-plain-alert.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: A quick
  launch that cannot read recent setups (or the host list) sets `preparation_error` (create_form.rs:4619-4662), which
  renders as a plain `p.create-session-error` with role=alert (create_form.rs:6052-6053) rather than the bordered "Not
  launched." alert. SPEC.md:949-953 requires that alert for "every refused launch (New, Clone, Replace with, quick
  launch)" but says "Errors that are not a refused launch ... keep the plain line".
- Decision: the user picked "Fix each with the smallest truthful correction" (2026-10-26 ten-question triage page, Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the feedback file
  and its index entry.
- Execution: `pending`.

## session-view-fallback-omits-start-time.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  SPEC.md:1609 says "The summary may clip very long values or whole rows in a short window; the session view provides
  those details in full", and SPEC.md:1608 lists start time among those details. The session view shows only effective
  activity time. That wording came from the `fix spec` outcome at TRIAGE_OUTCOMES.md:16809-16823, which targeted the
  directory. The fix could narrow the sentence or add a start-time display.
- Decision: the user picked "Fix each with the smallest truthful correction" (2026-10-26 ten-question triage page, Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the feedback file
  and its index entry.
- Execution: `pending`.

## download-interruption-promise-omits-crash-exception.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  SPEC.md:2053-2054 promises "no partial file after failure or interruption" for desktop downloads. SPEC_impl.md:98-107
  ("Cleanup on quit") leaves crashes and force-quits uncovered by default. The finding itself says its key premise is
  unverified: that SPEC.md's "interruption" covers process death.
- Decision: the user picked "Fix each with the smallest truthful correction" (2026-10-26 ten-question triage page, Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the feedback file
  and its index entry.
- Execution: `pending`.

## implementation-version-doc-omits-schema-update-guard.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  SPEC_impl.md "Version and skew" (around 5310-5325) covers hello versions and the dev-helm ordering, but not the
  separate refusal of Update when a host reports a newer database schema (SPEC.md:119-125). The finding itself says the
  existing sentence ("never refuses Update as a downgrade") "is not literally refuted".
- Decision: the user picked "Fix each with the smallest truthful correction" (2026-10-26 ten-question triage page, Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the feedback file
  and its index entry.
- Execution: `pending`.

## launch-input-database-duplication-rationale-overstated.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Confirmed on current main. SPEC_impl.md:3433-3436 says a launch spec carries "nothing the session's own database row
  does not already hold", and `service/launch_artifacts.rs:597-600` repeats this. But `Supervisor::spawn_agent` appends
  the create-time initial prompt to the spec's argv (`service/core.rs:14625-14633`; test `:16459-16525`), while the
  database keeps only its SHA-256 in the request fingerprint (`core.rs:1705-1727`: "The message itself belongs only to
  the create, never the durable session launch"). The sentence is factually wrong. However, the maintainer's acceptance
  of keeping current-generation specs until Delete (TRIAGE_OUTCOMES.md:1641-1705,
  `superseded-launch-specs-never-removed.md` and its follow-ons) rested on "A leftover spec therefore exposes nothing
  new".
- Decision: the user picked "Fix each with the smallest truthful correction" (2026-10-26 ten-question triage page, Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the feedback file
  and its index entry.
- Execution: `pending`.

## settings-save-races-final-replacement.md

- Outcome: `fix spec`.
- Assessment: confirmed by reading the code on main 1fd93044 (not run). When a Tensorlake recipe carries credentials,
  Farhelm merges them into Claude Code's `~/.claude/settings.json` and Codex's `~/.codex/config.toml` on the sandbox at
  setup, on each connection whose recorded delivery is out of date, and when a named token is replaced (SPEC.md
  "Topology"), so some deliveries happen while agents run. Since #3173 the final install compares the destination with
  the bytes read, but the check and `chmod` + `mv -f` are separate steps in one remote command
  (`crates/farhelm-helm/src/provisioning/backend.rs`), so a save by the agent or the user that lands in that gap of a
  few milliseconds (estimated, not measured) is overwritten silently and the delivery recorded as done. Contradicts the
  archived principle "Farhelm's delivery into an agent settings file never undoes a change saved to that file after
  Farhelm read it" (`TRIAGE_OUTCOMES_ARCHIVE.md`, `agent-settings-save-overwrites-concurrent-editor.md`). Fully closing
  it needs a lock the agents also honor; none is known. No coverage.
- Decision: the maintainer asked why delivery edits these files after agents start, then whether credentials could move
  out of the agents' settings files; the page answered that Claude's API endpoint and Subscription and Codex's API
  endpoint could go into the launch environment (Codex's provider through `-c` overrides and an `env_key` token), Codex
  Subscription cannot (Codex owns `auth.json`), and that this is a redesign. They picked "Accept the gap in the spec,
  and add a TODO for launch-environment credentials" (2026-11-10 high-bucket triage page, Q4), and said in conversation:
  "the todo item for credentials hsould be in near term bucket".
- Completion criteria: SPEC_impl.md "Adding Tensorlake hosts" states that a save landing between the final comparison
  and the replacement can still be overwritten, because closing it needs a lock the agents would also honor; the
  archived principle is narrowed to match where SPEC_impl.md states it (it is not edited in the archive). TODO.md's
  `Near term` bucket gains an entry: deliver recipe credentials through the agent's launch environment instead of its
  settings files (Claude `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`; Codex provider via
  `-c` overrides with an `env_key` token; Codex Subscription stays in `auth.json`), noting the costs raised in triage
  (only Farhelm-started shells get the credentials, rotation reaches agents at next launch, a one-time cleanup of keys
  earlier releases wrote, spec rewrite) and that it would remove this race. No code change. Remove the feedback file and
  its index entry, and move this entry to `TRIAGE_OUTCOMES_ARCHIVE.md` in the same change.
- Execution: `pending`.
