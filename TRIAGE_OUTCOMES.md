# Triage outcomes

## tunnel-reuses-custom-helm-credential-origin.md

- Outcome: `discard` (revised 2026-10-26 from `fix code`; see Decision).
- Assessment: partly confirmed by code reading on main 2977e6c9 (not run). A desktop tunnel binds `127.0.0.1:0` and
  rejects only the remote port number, then opens `http://127.0.0.1:<local port>` in the system browser. A standalone
  helm signed into from that browser keeps its device secret in that origin's localStorage, with no expiry or port
  binding server-side, and stopping the helm revokes nothing. So when a tunnel gets a port a standalone helm once served
  on, the remote page can read the secret. The default 7433 is outside the ephemeral range; ports from `--port 0` or a
  custom port inside that range are affected. Narrower than the file: only a standalone helm accepts the secret, none
  can run on the desktop app's state directory while the tunnel exists, and later use needs local reach, so the strong
  route is the open tab driving a standalone helm later restarted on that port. SPEC.md: "A remote host must not gain
  unauthorized execution or access to secrets on the helm's machine or another host through Farhelm." The 2026-10-22
  `tunnel-can-take-standalone-helm-port.md` outcome fixed only the remote-number collision.
- Decision: the user first picked "Tunnels avoid ports a helm has served on" (2026-10-26 highest-bucket triage page,
  Q4). Superseded the same day while planning it: once planning review showed the fix needs a new durable record of
  served ports (a database table with no downgrade path, or a separate file) and still misses helms run with another
  state directory, the user answered on the planning page: "what, no, this is too much complexity. drop the whole thing,
  discard".
- Completion criteria: remove the feedback file and its index entry; no code or spec change. (The superseded criteria
  were: the standalone helm durably records each port it serves on, and tunnel port allocation skips recorded ports.)
- Execution: `pending`.

## timing-history-can-retain-failed-recorder-success.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. In `scripts/record-test-run.py` around lines 1882-1903, the comment says the durable timing record "is
  written only from a structurally complete report, after the outcome above is final". Yet `test_run_timings.publish`
  runs before the following `manifest.write()`, and a failure of that write flips the outcome to `recorder-error`/125.
  The permanent timing file (`scripts/test_run_timings.py` `publish`, which copies `outcome` and `recorder.exit_code`)
  then still says the run succeeded with exit 0.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: publish the timing record only after the post-collection manifest write's result has settled the
  outcome and exit code. A later manifest write (for example the trace-collection write) records `runner.timing_record`,
  and a timing-write failure still never changes the run's result. Add a recorder test for a Playwright run with a
  complete report whose first post-collection manifest write fails. It asserts that the recorder returns 125 and that
  the timing record, if written, carries outcome `recorder-error` and exit code 125. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over it, block and bring
  it back. Remove the feedback file and its index entry.
- Execution: `pending`.

## removed-deflake-items-leave-exclusions.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. Commit 2897c914 (#2487) dismissed the TODO entries "Shallow pause resets under load" and "Reconnect recovery
  misses its window under load" ("no longer wanted on the board"). On current main, `deflake/known-flakes.txt` still has
  `nextest farhelm::e2e terminal_backpressure::shallow_pause_resumes_without_reset_or_replay # shallow-pause-resets-under-load`
  and
  `playwright terminal-reconnect.spec.ts view-changes-do-not-postpone-a-recovery # reconnect-recovery-misses-its-window-under-load`.
  TODO.md has no matching entries. `deflake/SPEC.md` (~lines 32-37) says: "when the TODO.md entry is removed (because
  the flake was fixed or dismissed) the `known-flakes.txt` line is removed in the same PR. A stale line there is a test
  that silently never runs again". Root AGENTS.md, "TODO.md and FLAKES.md", says the same, as does the header of
  known-flakes.txt.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: remove those two lines from `deflake/known-flakes.txt`. No TODO.md change, and FLAKES.md entries
  stay as they are (append-only). There is no runtime test to add; check that the deflake tool still parses the file
  (its own self-test or check, if one exists) and run `python3 scripts/check-agents-md.py` only if instruction files are
  touched. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment); over it, block and bring it back. Remove the feedback file and its index entry.
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

## host-remove-doc-overstates-second-answer-guard.md

- Outcome: `fix spec`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. SPEC_impl.md:831-836 says of the four stay-up prompts (host Remove among them): "All four refuse a second
  answer while their request runs". For host Remove that is false. `answer_remove`
  (crates/farhelm-ui/src/hosts.rs:4542-4553) checks only `is_current`, and its own docs (hosts.rs:4537-4541) say a
  "remove" and a "remove, don't ask again" in one burst both pass: "the request's claim refuses the second request, but
  the second answer still stores the skip-confirmation preference ... accepted by SPEC.md ("Confirmed 2026-10-10")". The
  preference is written at hosts.rs:1759. SPEC.md:3494-3497 accepts actions delivered inside a sub-second window as
  delivered.
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: Reword SPEC_impl.md's "All four refuse a second answer ..." so it is true for host Remove: a
  second removal request is refused by the run's claim, but a second "don't ask again" answer in the same burst still
  stores the skip-confirmation preference (accepted per SPEC.md's Confirmed 2026-10-10 sub-second rule). Keep the other
  three prompts' guarantee and the Replace sentence as they are. No code or test change. Run
  `python3 scripts/check-agents-md.py` only if headings move. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment); over it, block and bring it back. Remove the
  feedback file and its index entry.
- Execution: `pending`.

## ordinary-setup-discards-deferred-agent-add.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. While the ordinary Add host dialog is open, a focused agent HostAdd request is moved into
  `agent_add.opening` (crates/farhelm-ui/src/approvals.rs:213-218) but not presented, because
  `!adding() && !add_submission_pending()` gates it (hosts.rs:2266). When the user confirms the ordinary setup,
  `on_add_submit` unconditionally runs `agent_add.opening.set(None)` (hosts.rs:2095), even when `from_agent_request` is
  false. That drops the parked agent opening, and its card must then be reopened by hand. SPEC_impl.md:5958-5962:
  "`HostModal` admission waits for existing dialogs and samples that predicate again before opening ... Several focused
  arrivals are presented in order after the current form closes". SPEC.md:2832: "An existing draft or another modal is
  kept; automatic admission rechecks focus when that modal leaves."
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: `on_add_submit` clears `agent_add.opening` only when the submission came from the agent dialog
  (`from_agent_request`). An ordinary submission leaves a parked agent opening in place, so it presents through
  `HostModal` once `adding` and `add_submission_pending` are both false. Add a browser test (approval-layout or the
  agent host-add spec): open ordinary Add, deliver a focused HostAdd request, confirm the ordinary setup, and assert
  that the agent's Add dialog then appears automatically for that request. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment); over it, block and bring it
  back. Remove the feedback file and its index entry.
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

## invalid-folder-style-overrides-focus-ring.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. `crates/farhelm-ui/assets/app.css:6835-6838` styles
  `.launch-composer-folder-picker input[aria-invalid="true"]` with
  `outline: 1px solid var(--danger); outline-offset: -1px`. That selector is more specific than the global
  `:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }` at app.css:2044-2047, so a focused invalid
  folder field loses the shared keyboard ring. The field is in the launcher's modal dialog
  (`crates/farhelm-ui/src/list/create_form.rs:5953-5957` sets `role: "dialog"`, `aria_modal: "true"`; :7049 and :7068
  set the picker class and `aria_invalid`). SPEC_impl.md:854-855 says: "Modal dialog containers have no focus ring;
  controls and the nonmodal notification panel retain their keyboard rings." The app.css:2030 comment also promises "One
  accent ring for every keyboard-reachable control".
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: a focused (`:focus-visible`) invalid folder field shows the shared 2px accent ring with its 1px
  offset. The danger outline applies only while the field is not focus-visible (for example by scoping the rule with
  `:not(:focus-visible)`), and the problem text below the field still marks it invalid. Add a browser test, alongside
  the existing CSS/focus specs such as `e2e/tests/modal-focus.spec.ts`, that produces a folder refusal, focuses the
  folder field with the keyboard, and checks its computed outline is the accent ring, and that the danger outline
  returns after blur. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over it, block and bring it back. Remove the feedback file and its index entry.
- Execution: `pending`.

## host-edit-doc-overstates-preapproval-refusal.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. The docstring of `edit_host_for_agent` (`crates/farhelm-helm/src/agent_requests.rs:3034-3036`) says the
  alias is canonicalized before the card "so ... an alias that route would refuse is refused without a card". Before the
  card, the code calls only `crate::store::validate_alias` (:3060), and that function checks only trimming, control
  characters and length (`store.rs:1586-1600`). A collision with another host's display name is refused only after the
  Allow, when the write goes through `write_alias_locked` (`hosts.rs:1073-1078` notes that write's "refusals of its own
  (another host's name)"). SPEC.md:2857-2858 accepts this: "On Allow, an edit writes through the same routes as the host
  settings dialog ... with their refusals."
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: narrow the docstring so that only an alias failing the shape rules (control characters, length;
  whitespace-only means none) is refused without a card. It must also say that a collision with another host's name is
  checked by the alias write after the Allow and is refused there with nothing changed. No code change. A test is
  optional; if added, it shows that a colliding alias gets a card and is refused after Allow with nothing written. Size
  gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over it, block and bring it back. Remove the feedback file and its index entry.
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

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
- Execution: `pending`.

## removal-destination-collapses-distinguishing-spaces.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Destinations that differ only by repeated spaces render identically, because the body paragraphs and inline peer spans
  use normal white-space. SPEC.md:1531 says "A shortened heading retains the full destination in the body". However, no
  valid live SSH destination with repeated spaces was shown, and the copied-command extension depends on unverified
  clipboard and harm premises.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1).
- Completion criteria: add a filter to `review_feedback_queue/FILTER.md`: a finding whose harmful route is not
  established in shipped code (a missing premise, unverified platform or vendor behavior, an input sequence no shipped
  control produces, or speculation about unimplemented work) is not recorded; whoever establishes the route records it
  then. No code change. Remove the feedback file and its index entry.
- Execution: `pending`.

## instruction-file-case-alias-evades-check.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Confirmed that `scripts/check-agents-md.py:204` compares basenames case-sensitively against
  `{"CLAUDE.md", "CLAUDE.local.md"}`, so `claude.local.md` passes the check. The harm assumes Claude Code finds a
  differently capitalized file on a case-insensitive filesystem. The finding marks that vendor lookup as unverified.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## menu-close-dismisses-new-notifications.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: A
  closing session menu or notification list may hand focus back to its old toggle after the new notification panel took
  focus. The new list would then close and mark its entries read. The finding rates this "likely" and says the event
  ordering and the premature marking are unverified.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## probe-timeout-discards-parsed-remote-path.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: On the
  two-second cleanup deadline, `provisioning/backend.rs` replaces the drained diagnostics, including an already parsed
  resolved-path record, with the default. The trigger is a helper that holds stderr open, and no real configuration was
  shown to do that. The fix needs parsed metadata kept independently of EOF.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## launcher-source-supersession-route-unresolved.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Conditional lead. The reviewer says no shipped input sequence through the active modal reaches the parent's
  source-supersession handlers, and that server-side YOLO and replacement admission refuse the suspected harmful
  dispatch. The file calls itself AMBIGUOUS and names missing source premises.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## restart-selector-retains-source-conversation.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Reseeding a mounted Restart-with form keeps the conversation selector the user entered. The reviewer refutes the
  ordinary route (the header is modal-inert, and Cancel, success and New all unmount the form) and names two missing
  premises: a queued-event route that switches sources without unmounting, and a harmful restart that follows.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trash-alias-refresh-skips-movement-pause.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: A
  host-alias refresh can change the trash dialog's layout without changing its settle fingerprint (trash.rs read
  signature/fingerprint). The reviewer cannot establish any layout in which another enabled Delete lands under the old
  pointer position, so the harm is conditional.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## uninstall-final-scan-concurrent-mount-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: A mount
  created after uninstall's last containment scan could be traversed by the removal. The reviewer stops incomplete on
  whether any supported topology-change sequence exists. No spec text accepts or rejects this.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## uninstall-inspection-newline-identity-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Current
  main still has `resolved=$(readlink -f -- "$1" 2>/dev/null | tr -d '\n')` at
  crates/farhelm-helm/src/provisioning/backend.rs:2553, which strips newlines before the identity comparison. SPEC.md
  excludes deliberately made newline-target symlinks. The reviewer names no supported incidental source of
  newline-bearing ancestors and no layout that defeats every guard.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## older-direct-bun-omp-reporter-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Older
  direct-Bun OMP launches keep a permissive reporter corridor. No shipped nested interactive OMP that receives the
  reporter and the required ancestry has been established. OMP is not a first-class harness, and the older-release
  upgrade filter partly covers this.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## bun-option-value-retains-launcher-exemption.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  classifier at crates/farhelm-supervisor/src/agent_kind/omp.rs:70-79 can read `x` in `[bun, --cwd, x, entry]` as Bun's
  `x` subcommand. The reviewer could not verify how Bun actually parses this, or that a nested reporter route exists.
  The intended Bun argument grammar needs vendor evidence before the correct behavior can be stated.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## renamed-grok-reporter-ancestry-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: When
  the whole Grok binary is renamed, a nested child can pass the name check as the sole recognized emitter. Nothing shows
  that such a renamed Grok runs and issues the needed callbacks. SPEC_impl.md chooses name-based exceptions on purpose,
  and whether that choice covers this shape is ambiguous against SPEC.md.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## repeated-ssh-markers-collapse-incidental-config.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  scrubber in scripts/test-provision-centos.sh deletes everything between two identical begin markers and one end
  marker. No writer that produces that malformed layout around unrelated stanzas has been shown. Ownership of the
  ambiguous span under SPEC_impl.md's naming-convention rule is unsettled.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## narrow-outage-card-left-edge-unreachable.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  claim is that below 320 CSS px, the outage overlay's left offset (app.css near the `.session-unavailable-card` rules)
  may push text past the reachable left edge. The finding's own confidence line says the premise is unestablished: no
  rendering was done, and shrink-to-fit width and recovery geometry are unverified.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## native-app-binding-may-relaunch-crashed-candidate.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  claim is that a path-based getApp in the native provider might relaunch a candidate that crashed after the PID check
  (guest_ui.py:114-161). The finding states that the provider's launch-on-missing behavior is an "UNVERIFIED SOURCE
  PREMISE".
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## retained-refusal-can-clear-restart-delete-protection.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  retained-refusal settlement's UPDATE checks outcome but not generation, so in theory it could replace a terminal-less
  Launching row with Error and weaken Delete protection. The finding says "STOPPED INCOMPLETE": no supported producer of
  the prerequisite RetainedCreateRefusal during a concurrent Restart was established.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## claude-plan-abort-removes-established-report-pin.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Failed
  Claude Restart clears the report pin, and a delayed child report could replace the Resume target. The finding itself
  says no ordinary supported producer of the needed nested child report exists, and that spec coverage (SPEC.md
  reset/abort paragraphs) is ambiguous.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## omitted-ipv6-local-routing-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Teredo
  and IPv4-translated IPv6 literals are not classified as local, so a public-link grant applies to them. The finding
  states that no supported route mapping those literals to a local resource is established.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-fallback-lock-parent-replacement-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  held directory and the fallback lock are opened through different handles, so replacing the parent directory could
  separate them. The finding states that no supported replacement topology or harm is established.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## uninstall-refusal-missing-identity-wording-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  refusal wording could call an installation with no identity "different". The finding states that no shipped sequence
  reaches the missing-identity comparison.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-stage-real-folder-substitution-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Staging
  cleanup does not recheck the identity of a replacement real directory. The finding states that no ordinary producer of
  such a replacement is established.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-legacy-inode-approval-alias-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Legacy-app approval compares only the inode number, so a same-inode alias could inherit approval. The finding states
  that no supported Mac history reaching a harmful replacement is established.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-mounted-descendant-admission-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Recursive cleanup has no mount boundary. The finding states that no supported topology lets a mounted descendant
  survive the earlier moves.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-compatibility-folder-replacement-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Cleanup
  renames the current compatibility-lock folder by name before a non-recursive remove. The recursive-deletion part is
  refuted by the source, and no foreign replacement producer is established.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-terminal-backup-crash-persistence-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Power
  loss might keep the published Terminal link but lose the earlier backup rename in the same directory. The finding
  states that the needed filesystem/journal behavior is unestablished.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-moved-app-cleanup-durability-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Interrupted cleanup can flush the app at the usual location rather than the one Finder moved. The finding states that
  the harmful second-crash history is unestablished.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## automatic-reopen-can-launch-older-replacement-app.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  claim is that an older bundle substituted at the app's path during an automatic reopen could launch, because the guard
  depends on the launched app honoring `FARHELM_DESKTOP_REOPENED`, which older releases do not check. The reviewer did
  not establish Launch Services dispatch, the timing, or any consequence.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## uninstall-skipped-stage-resurrection-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  ordinary route is already repaired: readable stages are consumed under the installation lock before uninstall removes
  the app (`installation/install.rs` cleanup, `crates/farhelm/src/uninstall.rs`). What remains is the skip branch for
  unclassifiable or unreadable stage entries, with no supported history in which such a stage survives a successful
  uninstall and is later restored.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## claude-unread-parent-report-displacement-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: An
  unmarked, hooked nested Claude could overwrite the parent's unread `latest.json` report before being refused.
  SPEC_impl.md:3148-3152 accepts "a nested runtime that carries no such marker (a second Grok ...) can still take that
  slot ... and is then refused, which is an accepted gap". SPEC.md:2213-2214, however, calls "a new conversation Farhelm
  never learns of, leaving it undiscoverable long term" a defect, and no ordinary producer of a hooked, unmarked nested
  Claude was established.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## claude-oversized-stop-payload-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `crates/farhelm/src/hook.rs` rejects hook input above 65,536 bytes before parsing. Whether real Claude `Stop` payloads
  reach that size, and whether losing one loses tracking that SessionStart did not already record, is unestablished.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## claude-dialog-text-mimics-input-box.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  agent-written question text containing a `─` row followed by a `❯` row makes `claude_input_box_rule`
  (`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:536`) read a waiting dialog as idle. The code is
  unchanged. The finding calls itself dormant while agent prompts are off, says the premise that multi-line question
  text renders at column 0 is unconfirmed, and leaves both the possible fold into TODO.md and the "ordinary use"
  question to triage.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## codex-output-mimics-queued-question.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  `codex_queued_question` (`screen_reader.rs:706-717`) matches transcript rows, so agent output shaped like the
  queued-question hint reads as waiting even while the agent works. The code is unchanged. The finding notes that
  Codex's `• ` reply prefix may defeat the match, so the trigger is unverified, and leaves "clear, definite gap" to
  triage.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## hidden-trash-overlap-migration-result-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: This
  concerns a hypothetical helper in the unimplemented plan `plans/queue/hidden-trash-folder.md`, which is still queued
  on main. Current `checkout_trash.rs` keeps the record when lookup fails. No implemented or prescribed behavior shows
  the loss.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## tunnel-consent-conflicts-with-restart-dialog.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  choosing Tunnel and open on an approval card while Restart with is open can leave the two modals making each other
  inert. The mechanism is source-only, and confidence is "likely". The finding asks for native verification of the modal
  handoff and of Escape recovery before a fix is chosen, and notes that a general handoff would be too broad.
- Decision: the user picked "Discard all, and add a review filter" (2026-10-26 ten-question triage page, Q1); the filter
  is carried by `removal-destination-collapses-distinguishing-spaces.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

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

## older-handoff-reports-success-for-directory-program.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `install.sh` hands releases below `HANDOFF_FLOOR` to that release's own published installer (SPEC_impl.md:4612-4622).
  Per the finding, that installer (v0.27.0) `mv -f`s into a destination that is a directory and still prints success.
  The published bytes cannot change, so any fix belongs in the modern script's handoff (a post-check, a refusal, or
  something else). No spec text says which.
- Decision: the user picked "Accepted, discard all" (2026-10-26 ten-question triage page, Q2), adding "and SPEC. We do
  not concern our selves with thold generation installers prior to 0.31.1 unless it is a highly catastastrophic problem
  that isn't in some super rare edge case." The spec sentence is carried by
  `older-development-archive-moves-replacement-folder.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## older-installer-prunes-live-custom-state-version.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Installing v0.27.0 twice (a downgrade) can prune a newer version that a supervisor running with another state
  directory still launches from. SPEC_impl.md:4542-4548 keeps downgrades to older targets part of the contract, and
  :4658-4668 describes the modern installer's live-version protection, which the old installer lacks. The old
  installer's pruning cannot be changed. Whether the handoff should refuse, pre-check, or accept this is a product
  decision.
- Decision: the user picked "Accepted, discard all" (2026-10-26 ten-question triage page, Q2), adding "and SPEC. We do
  not concern our selves with thold generation installers prior to 0.31.1 unless it is a highly catastastrophic problem
  that isn't in some super rare edge case." The spec sentence is carried by
  `older-development-archive-moves-replacement-folder.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## automatic-legacy-installer-overwrites-later-terminal-choice.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: An app
  kept open across a Terminal downgrade may auto-update with an older installer that ignores `FARHELM_INSTALL_EXPECT`
  (SPEC_impl.md:4820-4821: "Installers from before this variable ignore it"). That installer could then overwrite a
  later Terminal install. The finding says no affected prerelease is shown to have been published and that the
  deployment history is conditional.
- Decision: the user picked "Accepted, discard all" (2026-10-26 ten-question triage page, Q2), adding "and SPEC. We do
  not concern our selves with thold generation installers prior to 0.31.1 unless it is a highly catastastrophic problem
  that isn't in some super rare edge case." The spec sentence is carried by
  `older-development-archive-moves-replacement-folder.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## older-handoff-keeps-damaged-version-program.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  v0.27.0 installer reuses any existing regular `farhelm` in a version folder without comparing it to the verified
  download, so a damaged program survives a handoff reinstall that reports ready. The modern installer repairs this case
  (SPEC_impl.md:4640-4648), but historical installer bytes cannot change, so the fix would have to be a new handoff-side
  check.
- Decision: the user picked "Accepted, discard all" (2026-10-26 ten-question triage page, Q2), adding "and SPEC. We do
  not concern our selves with thold generation installers prior to 0.31.1 unless it is a highly catastastrophic problem
  that isn't in some super rare edge case." The spec sentence is carried by
  `older-development-archive-moves-replacement-folder.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## older-handoff-power-loss-durability-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  historical installer fixture (`scripts/fixtures/install-v0.27.0.sh`) uses `cp`, `chmod` and `mv` without explicit
  flushes. Whether supported macOS utilities and APFS can leave an unusable installation after power loss is unverified.
- Decision: the user picked "Accepted, discard all" (2026-10-26 ten-question triage page, Q2), adding "and SPEC. We do
  not concern our selves with thold generation installers prior to 0.31.1 unless it is a highly catastastrophic problem
  that isn't in some super rare edge case." The spec sentence is carried by
  `older-development-archive-moves-replacement-folder.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-rerun-advice-omits-stale-lock-prerequisite.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  website/src/content/docs/docs/get-started/install.md:26-27 promises that rerunning the installer finishes a cut-short
  install. docs/install_uninstall.md:85-92 notes that a leftover lock folder from an older installer must be removed by
  hand first. The current installer uses a kernel lock that an interruption never leaves behind. The second part of the
  finding (a failure to mark the new folder leaving an unmarked one behind) rests on an unverified Mac topology.
- Decision: the user picked "Accepted, discard all" (2026-10-26 ten-question triage page, Q2), adding "and SPEC. We do
  not concern our selves with thold generation installers prior to 0.31.1 unless it is a highly catastastrophic problem
  that isn't in some super rare edge case." The spec sentence is carried by
  `older-development-archive-moves-replacement-folder.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## legacy-update-expectation-identity-collision-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Two
  two-field legacy records with the same Installed version produce identical expectations
  (`installation/mod.rs:709-746`: id None, generation 0). No updater or replacement history in which that collision
  causes harm was shown.
- Decision: the user picked "Accepted, discard all" (2026-10-26 ten-question triage page, Q2), adding "and SPEC. We do
  not concern our selves with thold generation installers prior to 0.31.1 unless it is a highly catastastrophic problem
  that isn't in some super rare edge case." The spec sentence is carried by
  `older-development-archive-moves-replacement-folder.md`.
- Completion criteria: Remove the feedback file and its index entry.
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

## delayed-refusal-reopens-cancelled-replace-question.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: A
  delayed refusal of a sidebar Replace request can reopen a Replace question after the user cancelled the header copy.
  The proposed fix fences stale refusals using cancellation freshness. The trigger is a late GUI–helm reply, and
  confidence is "likely", never run.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## host-dialog-reservation-does-not-exclude-settings.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Settings can open while a reserved host question is still mounting, and the two dialogs can then make each other
  inert, with Escape still recovering. Whether this happens depends on how the competing input is ordered. The width of
  that window is unestablished, and the subsecond filter might apply.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## queued-agent-dialog-inherits-elapsed-arm-timer.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  claim is that the next queued agent Add-host dialog, promoted from `add_openings` (approvals.rs:213-218), inherits an
  `armed` state whose 700 ms pause already elapsed, because the arm timer restarts only when the listed ids or the
  selection change (approvals.rs:226-236). The finding calls the dialog-mount interpretation ambiguous and found no
  harmful input sequence.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## quick-launch-mark-inverts-live-owner.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  bolt's onclick at crates/farhelm-ui/src/list/templates.rs:2181 works out the direction from live `preferences`
  (`!lit`), not from the rendered `row.anchor.marked` that drives aria_pressed and the tooltip. A click delivered after
  a fresh read changes the owner but before the redraw can therefore apply the opposite action. SPEC.md:3494-3496 ("an
  action a user takes inside a sub-second window before the screen updates ... may be carried out as delivered") may or
  may not cover this inversion. The one-line fix is obvious, but whether this is a bug depends on reading that spec
  sentence.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## quick-switcher-pending-read-keeps-old-shortcut-labels.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: While
  the switcher's opening preference read is still pending, it can show cached shortcut labels. SPEC_impl.md:1166-1169
  and TRIAGE_OUTCOMES.md accept stale labels after failed reads and later moves, and the finding calls coverage of the
  pending interval "ambiguous". The display corrects itself once the read completes.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## shortcut-batch-revalidates-with-new-host-snapshot.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  shortcut replay (create_form.rs:4503-4553) captures `shortcut_hosts`/`shortcut_catalog` before awaiting
  `shortcut_context_snapshot()`, validates the batch against those after the await, then applies each template through
  `accept_search`, which validates against its own render's hosts. A host refresh during the await could let an earlier
  template apply before a later one refuses. The mechanism is plausible, and SPEC.md requires a refused sequence to
  change nothing.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## startup-selection-read-outlives-intervening-visits.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  if startup's pre-pick preference read stays delayed while the user makes visits and then deletes a session, that
  read's old snapshot can reopen an older session. The trigger is a GUI-helm response arriving late, after later
  selection lifecycles. The proposed fix rejects late results and re-reads.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## new-opening-folder-inherits-fallback-host.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: A host
  refresh before New mounts can pair a remote session's folder with the fallback local host. Final Launch still shows
  and checks its target, and the finding calls SPEC/FILTER coverage ambiguous.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## browse-result-survives-automatic-host-fallback.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: On
  desktop, a browse result from the old host can be accepted as an explicit folder during the render-to-effect window
  after an automatic local fallback. The trigger is an event inside a Dioxus edit-flush interval. Launch still shows and
  checks its target.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## selection-saves-omit-intermediate-visits.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  latest-wins scalar preference queue coalesces intermediate selections, so the helm's history never sees them.
  SPEC_impl.md:218-244 keeps latest-wins scalar preferences, and the finding itself says SPEC.md's
  most-recent-distinct-visits promise is not reconciled with that and should be preserved for triage. It offers two
  fixes: carry visit events through the writer, or change the contract.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## history-reply-lacks-installation-provenance.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  a launch-history reply from an installation B, delivered late after the host row was retargeted from A to B and back
  to A, passes the client-made target-tag check and becomes A's frozen folder default. The route depends on an in-flight
  GUI–helm history GET whose reply arrives late after the GUI's state has moved on. The remedy the reviewer names binds
  the reply to the queried installation, which needs a new identity field in the history response.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## uncertain-launch-marker-overwrites-older-attempt.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  original duplicate-launch claim is withdrawn. What remains is an overwrite of a single marker by a later uncertain
  launch, which depends on a lost or unknown create reply and on an unestablished dispatch/admission ordering.
  SPEC.md:910-913 describes the guard but neither accepts nor clearly rejects this variant.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## rename-lost-reply-restores-old-shortcut-indication.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  rename repoint commits but its reply is lost, the GUI rolls back to the old mark, and the next shortcut resolves the
  new one. The trigger is a lost GUI–helm reply.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## template-cached-collision-blocks-freed-name.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Confirmed on current main. The Templates dialog's save handler (`crates/farhelm-ui/src/list/templates.rs:1810-1816`)
  returns on the cached `name_refusal` (:1560-1572), which uses `save_name_refusal` (:887-904), before it does the fresh
  `fetch_templates` read (:1828-1829). So a name freed by a CLI or agent delete keeps being refused until the list
  reloads. That is plainly unintended, but the fix can take several shapes: refresh the resource on a cached collision,
  defer collisions to the fresh read, or both. The cached check also gives instant feedback, which a fix may want to
  keep.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## quick-launch-clear-erases-new-agent-mark.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Clearing the mark (templates.rs:1637-1655) reads preferences and then PUTs `QuickLaunchTemplate(None)` with no
  expected owner. An agent or command-line mark written between the GET and the PUT is therefore erased. SPEC.md "One
  GUI at a time" says "A GUI write must not undo a change an agent or the command line made while that view was open",
  and the 2026-10-21 exception names only templates and number-key bindings. On the text this is a spec violation.
  Closing the GET-to-PUT window, though, needs a compare-and-set precondition on the preference write path through the
  API and the helm store, which is a new concurrency mechanism. The same window also exists for marking.
- Decision: the user picked "Accepted: discard all, and state the principle" (2026-10-26 ten-question triage page, Q3);
  the spec sentence is carried by `add-host-refusal-bypasses-dialog-admission.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## catalog-cleanup-can-start-untracked-test-session.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Test
  cleanup in `template-list-shortcut.spec.ts`, `launcher-enter.spec.ts`, `terminal-create-idempotency.spec.ts` and
  `destination-authority.spec.ts` can stop watching before a pending GUI create (key mint, then POST) finishes, leaving
  an untracked fake session. The finding rates the adverse scheduling as unmeasured.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## history-fixture-reset-races-selection-save.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `e2e/tests/sidebar.spec.ts` resets preferences through the API while an automatic selection save from the GUI may
  still be in flight. The finding rates the adverse ordering as unobserved.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## prompt-prefill-test-opens-before-source-selection.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  browser tests pin the source selection through preferences and then open New without waiting for the startup
  preference GET to apply that selection (`e2e/tests/templates.spec.ts:1067-1080` on main still calls `pinAutoSelect`
  and then `openNew`). The trigger is a test fixture racing a GUI-helm response.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## held-preference-reply-blocks-fixture-restoration.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  in `e2e/tests/popover-template-movement.spec.ts`, cleanup waits on a gate that is released only after a held
  preference reply is successfully fulfilled. If fulfillment fails, preference restoration and template deletion never
  run. The trigger is a failed held GUI-helm reply in a fixture.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## post-clone-test-save-overtakes-restoration.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: In the
  post-clone settings e2e test, the page's Save PUT can still be in flight when the `finally` block restores state
  through the request fixture, and that late PUT overwrites the restoration. This is a test fixture racing a GUI save.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## failed-test-response-does-not-fence-settings-write.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Browser-test cleanup in `e2e/tests/settings.spec.ts` (and the claude-trust and templates specs) can restore old
  settings before an earlier GUI save, whose browser response failed or was never awaited, commits on the server. The
  trigger is a GUI–helm request failing or its reply being lost, with a test fixture racing a GUI save.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## closed-model-enter-test-can-misattribute-restart.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claims
  the restart-with e2e test can false-pass if a closed-field Enter wrongly started a restart, because the early request
  could land after the immediate count check and satisfy the later primary-press assertions. Production behavior is
  correct; the gap only shows under a hypothetical mutation and an adverse browser schedule.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
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
- Execution: `pending`.

## replacement-test-supervisor-spawns-before-pid-publication.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  in `e2e/tests/terminal-multihost.spec.ts`, the replacement supervisor is spawned detached before its PID is written
  for stack cleanup. A worker killed between the two steps leaks that supervisor. The suggested remedies are an owner
  tether or a parent-owned launch-and-registration step.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## owner-death-child-precedes-pid-publication.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: In
  crates/farhelm/tests/spare_builder.rs the test arms `Indirect` only after `await_unrelated_child` returns (lines
  622-623 and 692-693). If the owner dies between spawning its `sleep 300` and publishing that child's PID, the sleep is
  left outside cleanup. The ledger entry the reviewer cites chose the narrower order "wait for PID, arm, then wait for
  readiness", and the reviewer calls full coverage by it ambiguous. The parent cannot own a PID it has not yet been
  told, so closing the gap would need a different mechanism, such as a process group, a subreaper or a parent-death
  signal, and no single obvious shape exists.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## checkout-test-skips-new-spare-setting-restoration.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  in `e2e/tests/github-checkouts.spec.ts`, the sequential callers' `finally` blocks can throw before `fixture.close()`,
  which then never restores the newly disabled prepared-checkout setting. The finding records that TRIAGE_OUTCOMES.md
  `shortcut-tests-skip-owned-cleanup.md` deliberately limited the same class of fix to named blocks, and asks for that
  coverage question to be resolved first.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## plan-pruning-can-interrupt-shared-cargo-build.md

- Outcome: `other`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  the close-time pruning in `plans/.agents/close-reset.md:65-100` deletes this checkout's `target/` artifacts without
  taking Cargo's lock. `.agents/machine.md:24-25` allows a `CARGO_TARGET_DIR` shared across workspaces, so another
  workspace's build could lose an artifact mid-build. The concurrent schedule was not reproduced, and the finding leaves
  open whether the rare-glitch filter applies.
- Decision: the user picked "Discard: fix flakes when observed" (2026-10-26 ten-question triage page, Q4), adding
  "unless the fix is obvious and trivial in which case it's worth fixing with complexity gate"
- Completion criteria: at execution, fix the test only if the fix is obvious and trivial (one clear shape, about 30
  lines or less, no new test mechanism); otherwise treat this as a discard and say so in the execution record. Either
  way, remove the feedback file and its index entry.
- Execution: `pending`.

## guarded-cancellation-test-lacks-effect-witness.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  the edited-launcher cancellation regression in `e2e/tests/github-checkout-composer.spec.ts` can reach its manual
  launch before the browser has consumed a deliberately held, obsolete preview reply. That makes it a missing test
  witness, not a demonstrated false pass.
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## launcher-subscription-test-masks-missing-observers.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  `final_owner_search_keeps_conditional_agent_subscriptions` (now
  `crates/farhelm-ui/src/list/create_form/draft.rs:4319`) replays Harness and Model in one effect. Its harness-change
  control also changes the model, permissions and wrapper, so losing one subscription could go undetected. This is a
  static oracle concern, with no mutation run to show a false pass.
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## held-font-test-lacks-mount-attempt-witness.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  claim is that the held-font browser test can pass against a broken mount gate, because nothing it waits on shows that
  a terminal mount was attempted inside its four-second no-mount window. Current main's
  e2e/tests/terminal-font.spec.ts:217-260 still has that shape. Its comment says "the terminal appearing once bold is
  released below is what proves a mount was waiting on them", which does not hold if the first mount attempt comes after
  the release. terminal.js exposes no mount-attempt witness today (only `__farhelmTermReady`/`__farhelmTerm` at
  terminal.js:5502-5504).
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## conformance-test-never-admits-management-candidates.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  generic responsiveness conformance test saturates management permits, so its candidates are refused as busy before
  reaching the waits it means to exercise. Fixing it needs a new admitted observation that keeps independent wait gates
  and proves each candidate reached its wait.
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trash-failed-host-refresh-test-lacks-application-witness.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `e2e/tests/trash-dialog.spec.ts` asserts that the failed host's retained entries and warning remain after both listing
  requests arrive, but it does not wait until the failed host's reply has been applied, so a regression applied later
  could go unnoticed.
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-interruption-test-rejects-no-swap-fallback.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `a_same_version_replacement_never_leaves_the_name_empty`
  (crates/farhelm-supervisor/src/installation/install/tests.rs:~723-730) asserts that `Step::VersionNameEmpty` is never
  hit. Its failure message names the assumption outright: "(this test's volume can swap)". On a volume without
  RENAME_EXCHANGE, install.rs:~687-695 takes the documented fallback and emits that step. The test deliberately tests
  the swap path. Whether to inject the capability or accept the fallback trace is a design choice, and no failure has
  been observed.
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## installer-leftover-test-rejects-fallback-lock-file.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  claim is that `no_leftovers_beside_the_app_but`
  (`crates/farhelm-supervisor/src/installation/install/tests.rs:126-133`) allows only the app plus one named entry, so
  it would reject `.farhelm-install.lock` (`FALLBACK_LOCK_FILE`, `installation/mod.rs:71,199-229`), which the lock code
  creates and keeps only when the folder itself cannot be flock'ed. The suite already assumes a lockable temp folder:
  `mod.rs:864` asserts that the fallback file is absent ("a lockable folder needs no lock file"). No supported test
  volume was shown to take the branch, and no failure was reproduced.
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## window-check-export-budget-omits-standalone-captures.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  claim is that the window-drag check's ten standalone captures can be dropped by the 16 MiB result-evidence budget
  (guest_tester.py around 467-480), while the gate counts occurrences only in raw events. BRICK-TEST.md:320-321 says the
  window-drag check "retains its ten separate image occurrences", and RECIPE.md:180, 306 says the same. The finding
  notes an interpretation caveat (raw-event retention may satisfy this) and has not measured real capture sizes.
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## window-check-inherits-preflight-image-history.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `releasing/mac-vm-test/guest_operations.py:380` still adds frames to a `window_frames` history shared by the whole
  pass, and per the finding, provenance validation checks every accumulated image against only the current attempt's
  events. The consequence is limited to release-test availability. The finding notes that the trace and provenance scope
  "must agree on which attempt owns each image".
- Decision: the user picked "Discard: not worth a redesign" (2026-10-26 ten-question triage page, Q5).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## landing-leaves-baseline-flake-unrecorded.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Several
  landing reports observed recurring failures on unchanged main and deferred the FLAKES.md, Deflake TODO and
  known-flakes writes. One part is now recorded: the held-save test `a_hung_save_releases_waiters_at_their_deadline` has
  a TODO Deflake entry and a known-flakes line on main. The other tests (initial-prompt prefill, templates busy root,
  remove-and-re-add-host, accepted ADD, sidebar Replace) are still unrecorded. Recording them requires checking private
  run evidence and judging whether each counts as an established latent flake.
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake."
- Completion criteria: SPEC.md states that plans, plan reports, landing and delivery records, validation accounting and
  review or approval bookkeeping are operational artifacts, not product code, and are outside code review; reviewers
  ignore them. `review_feedback_queue/FILTER.md` gains a matching filter so such findings are excluded at intake if they
  still arrive. No code change. Remove the feedback file and its index entry.
- Execution: `pending`.

## uninstall-completion-bypasses-size-decision.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  the uninstall-mixed-spelling report admitted 160 counted lines against a gate of about 100 that required
  block-and-return, yet was marked complete for unattended landing. The report has since been approved (49e2634f,
  "chore(plans): approve uninstall-mixed-spelling", 2026-10-23). The other incidents the finding lists turn on the
  undefined tolerance of "about 100" and on authorization that is unrecorded but may exist.
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake." The spec sentence and filter are carried by
  `landing-leaves-baseline-flake-unrecorded.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## steel-split-restart-availability-wording-conflict.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `plans/queue/steel-session-chrome.md:120-122` says "Greyed-out conditions and their explanations apply to both parts".
  The same plan at `:167-172` limits greying to the matching part and menu item, and SPEC.md:1263-1268 lets Restart with
  work independently. The plan is not executed yet, and no GUI regression exists.
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake." The spec sentence and filter are carried by
  `landing-leaves-baseline-flake-unrecorded.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## delivery-reports-reuse-cases-from-failed-commands.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Delivered plan reports (`plans/reports/trash-delete-blocking.report.md`, `emoji-streaming-corruption.report.md`,
  `steel-foundation.report.md`) credit passing cases from commands that failed overall, which conflicts with the
  per-command reuse rule in `.agents/validation.md`. The failures were disclosed, and no product defect is alleged.
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake." The spec sentence and filter are carried by
  `landing-leaves-baseline-flake-unrecorded.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## steel-visual-delivery-omits-required-states.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: The
  steel-foundation report lists states that were reviewed only from code, while its plan required captures of every
  changed state unless an omission was authorized. Whether a maintainer waiver exists is unverified, and no rendered
  defect is established.
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake." The spec sentence and filter are carried by
  `landing-leaves-baseline-flake-unrecorded.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## stale-report-approval-retires-unseen-followup.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `scripts/plans-queue.py:946-956` still accepts an `approve` by slug alone and has no report digest, so an approval
  pasted from an old review page could retire a follow-up report that landed later. This is maintainer planning tooling,
  and the finding notes the gap predates the commit it reviewed.
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake." The spec sentence and filter are carried by
  `landing-leaves-baseline-flake-unrecorded.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## restart-template-plan-overstates-refusal.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  plans/queue/restart-with-template-refusal.md:22-24 still says "today no saved template can be applied in Restart with
  at all", which is wrong for kindless legacy templates (SPEC.md keeps them). The plan has landed (plans/REPORTS.md:14,
  #2711) and is awaiting maintainer approval. plans/AGENTS.md, "Editing rules", says planning PRs may revise only a
  pending or blocked plan, "never ... a landed plan's file", and "Code PRs never touch `plans/`".
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake." The spec sentence and filter are carried by
  `landing-leaves-baseline-flake-unrecorded.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## schema-disposition-removes-excluded-variant-tracking.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Asks
  whether an authorized queue removal in TRIAGE_OUTCOMES.md also discarded an excluded equal-number schema variant. The
  finding says the disposition is "AMBIGUOUS" and no incompatible migration pair or corruption route was established.
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake." The spec sentence and filter are carried by
  `landing-leaves-baseline-flake-unrecorded.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## retention-plan-omits-live-retry-downgrade-limit.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  `plans/queue/create-reservation-retention.md` disclosed only the deleted-session downgrade caveat, not that older
  builds refuse recovery of a still-live converted checkout request. That plan has since landed and was approved and
  removed (c16bc247, "chore(plans): approve create-reservation-retention"), so the plan-text part is moot. The residual
  is whether the release disclosure should name the live-request limitation. That depends on the compatibility scope,
  which SPEC.md:3493-3497 leaves to a per-feature decision.
- Decision: the user answered (2026-10-26 ten-question triage page, Q6): "SPEC - these things are not part of review
  flow and should be ignored by reviewers. they are operational artifacts, not product code. also FILTER.md to exclude
  if they still show up during intake." The spec sentence and filter are carried by
  `landing-leaves-baseline-flake-unrecorded.md`.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## legacy-page-server-rebinding-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `scripts/serve-page.ts` still has no Host/Origin validation (no host/origin checks in the file on current main). The
  finding says browser DNS-rebinding admission and any confidential served page are unestablished and the instructed
  workflows that used it were retired.
- Decision: the user picked "Discard: tooling trusts its inputs and its one user" (2026-10-26 ten-question triage page,
  Q7).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## release-heading-can-close-review-page-script.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `releasing/notes-review-page.py:307` puts the `##` heading token into an inline `<script>` through `json.dumps`, which
  does not escape `</script>`. The generator is legacy (its docstring says stable releases no longer use it), its input
  is maintainer-curated text in the documented `## vX.Y.Z` form, and its docstring says it does not check format.
- Decision: the user picked "Discard: tooling trusts its inputs and its one user" (2026-10-26 ten-question triage page,
  Q7).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## launcher-submit-callback-retains-render-snapshots.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  `crates/farhelm-ui/src/list/create_form.rs:4833` still builds `submit_composer` with `Callback::new` in the component
  body, and Dioxus keeps those allocations until the scope drops. The same pattern appears at other call sites
  (`create_form.rs:4745,4757`, `view.rs:343`, `app_bar.rs:612`), and the finding says older callbacks share it.
  Practical growth is unmeasured.
- Decision: the user picked "Discard unless measured" (2026-10-26 ten-question triage page, Q8).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## cancelled-upload-commit-retains-reply-entry.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  cancelling an upload during its commit leaves the commit's pending-reply entry in place until the connection ends,
  because the supervisor may omit the reply when its queue is full. Current main confirms the mechanism.
  `UploadGuard::drop` (`crates/farhelm-helm/src/client.rs:4157-4180`) removes only `admission_req_id`. The
  `CommitUpload` request registered at `client.rs:4064-4069` is never retired. The supervisor documents that it drops
  the reply (`crates/farhelm-supervisor/src/service/uploads.rs:300-306, 336-341`), assuming connection teardown will end
  the wait. TRIAGE_OUTCOMES.md:12867-12875 discarded a close neighbor (a dropped final upload reply) as "a rare trigger,
  and nothing is lost".
- Decision: the user picked "Discard unless measured" (2026-10-26 ten-question triage page, Q8).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## closed-popovers-repeat-prefix-scans.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claim:
  ordinary list refreshes do prefix scans and sorting for each row even when no popover is open. Practical latency is
  unmeasured, and SPEC.md:1512-1520 permits whole-list processing at the supported size. The finding itself says a
  change needs measurement first.
- Decision: the user picked "Discard unless measured" (2026-10-26 ten-question triage page, Q8).
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

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

## removal-copy-hidden-unicode-warning-unresolved.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer: Claims
  a retained uninstall notice can copy a state-directory path containing hidden (non-control) Unicode without a warning.
  The file itself refutes the different-field bypass and says the warning requirement and any harmful downstream paste
  are "unestablished"/"AMBIGUOUS". No SPEC text found requiring such a warning.
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

## browser-suite-ignores-explicit-port.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main e9ea6d08 (code reading, not run), which left it for the maintainer:
  Confirmed. `e2e/playwright.config.ts:24` uses `harnessStackSlots()`, which reads only `FARHELM_E2E_STACK_PORTS`
  (`e2e/stack-port.ts:73-85`), while `e2e/start-stack.sh:7-10` still says the config "takes an explicit
  FARHELM_E2E_PORT". The singular helper is still correctly used by the docs-shots, readme-hero and readme-video
  configs. With `STACK_COUNT = 3` one port cannot serve the suite. The finding itself offers "preserve compatibility
  where meaningful or reject the old input clearly and document migration".
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

## session-actions-use-unseen-liveness.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main 7a7899dff (code reading, not run): confirmed. The desktop app builds a
  lifecycle action's precondition from the session as the store holds it at the click, not as the clicked frame drew it
  (session_list/menu.rs documents "The row as the store holds it now: what any question is drawn from and any
  precondition is taken from"; the skipped Delete sends at once; the launcher reads its warning state at the press).
  GPUI dispatches a click to the previous frame's handlers, so a push landing in that frame can make a nothing-alive
  Delete carry an agent-alive precondition. SPEC.md "Lifecycle operations" (confirmed 2026-10-01): "When the session
  setting skips a prompt, the click authorizes only the state the screen showed, under the same guards. The request
  carries the matching precondition"; the launcher "carries the precondition matching what the launcher showed at the
  click". Bucket stays highest: the route kills a restarted agent without consent.
- Decision: recorded as `fix code` under the clearly-settled rule without asking (2026-11-18 highest-bucket triage).
- Completion criteria: Delete, Stop, Restart, Replace and the Restart with / Replace with launcher send the precondition
  matching the state the clicked frame drew, so the supervisor refuses when more is alive than was shown. Uses the
  shared standard and mechanism the user required on 2026-11-18 (Q3; see header-actions-follow-unseen-session.md), not
  an ad hoc per-site fix; the mechanism itself counts against that item, not this gate. Size gate: about 100 lines of
  diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: planned in `plans/queue/drawn-frame-actions.md`

## trailing-sandbox-stop-rendered-destructive-guards-refresh-before-dispatch.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main 7a7899dff (code reading, not run): partly correct. The route it established
  used the removed webview app's display delay, but the GPUI port has the same design: "Stop and suspend" on a sandbox
  or container session builds its two guards (agent running again, tabs opened) from the session in the live store at
  the click (session_list/environment.rs, menu.rs, stop.rs), and GPUI dispatches a click to the previous frame's
  handlers (established for session-actions-use-unseen-liveness.md). SPEC.md "Lifecycle operations": "a Stop whose click
  showed an ended agent is refused if the agent is alive again or cannot be confirmed ended, before stopping anything".
  The same bug was fixed for the old app in #3337. Bucket: high until the GPUI route is shown end to end, highest once
  it is.
- Decision: recorded as `fix code` under the clearly-settled rule without asking (2026-11-18 highest-bucket triage).
- Completion criteria: The environment Stop and suspend in the row menu and the header carries guards and verb from the
  state the clicked frame drew, with a test. Uses the shared standard and mechanism the user required on 2026-11-18 (Q3;
  see header-actions-follow-unseen-session.md), not an ad hoc per-site fix; the mechanism itself counts against that
  item, not this gate. Size gate: about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the
  queue removal or the changelog fragment); over that, execution blocks and brings the item back to the user. Remove the
  feedback file and its index entry in the same change.
- Execution: planned in `plans/queue/drawn-frame-actions.md`

## header-actions-follow-unseen-session.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main 7a7899dff (code reading, not run): confirmed. The open session's header
  (Stop, Restart, Replace, Delete, environment Suspend/Resume) and the interrupted-session card's Replace look up the
  open session and its state when the click is handled (session_pane/header.rs open_key and menu::actionable;
  session_pane/card.rs), while GPUI dispatches the click to the previous frame's handlers. So when the open session
  changes from A to B (the command line deletes A and the window selects B), a click on A's header in that frame acts on
  B, immediately with Skip session confirmations on. The sub-second race filter does not apply (it excludes acting on
  the wrong session). SPEC.md "Lifecycle operations" says the click authorizes only the state the screen showed;
  applying that to the targeted session is a reading, not its words. Bucket: highest.
- Decision: the user picked "Yes for the target and the drop point; spec note for paste order" (2026-11-18
  highest-bucket triage page, Q3), in their words: "But this must not be done in a random ad hoc way for the two places.
  We need to establish a clear pattern that all components can use that have this problem. I don't want this to turn
  into a whack-a-mole. We need to set the standard, spec it, and have an implementation that implements the key
  mechanisms that get reused whenever this problem arises." Principle: a user action in the desktop app acts only on
  what the frame it landed in showed (which session, its state, where a drop landed), and this is one specified standard
  with one reusable mechanism, not per-site fixes.
- Completion criteria: SPEC.md (and SPEC_impl.md for the mechanism) state the standard that an action acts on what the
  clicked frame showed: its target and the state its consent or precondition is taken from. The desktop app gets one
  reusable mechanism for capturing what a control drew and acting on that, and the header controls and the card's
  Replace use it. The mechanism is what `session-actions-use-unseen-liveness.md` and
  `trailing-sandbox-stop-rendered-destructive-guards-refresh-before-dispatch.md` (both already `fix code`) are fixed
  with too, so they are planned together rather than as separate ad hoc fixes. Tests show a stale-frame click acting on
  the drawn session or being refused. Expect this to exceed a single small PR; plan it as such. Remove the feedback file
  and its index entry in the same change.
- Execution: planned in `plans/queue/drawn-frame-actions.md`

## wayland-file-drop-uses-entry-position.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main 7a7899dff (code reading, not run): confirmed in GPUI's Wayland backend (the
  vendored `gpui-pre-linux` crate, vendor/gpui-pre-linux/src/linux/wayland/client.rs): while the drag's file list is
  being read, pointer motion is ignored, and when the read finishes the drag position is set to the entry point; a drop
  with no motion since is delivered there, and the terminal accepts a drop anywhere on its pane. So a file dragged in
  over the terminal and released over the sidebar can be uploaded to the session's host. Releasing before the list
  arrives is discarded safely. The sub-second filter does not apply (a local file crosses to a host). Bucket: highest
  (route established in code; narrow trigger).
- Decision: settled by the same pick as `header-actions-follow-unseen-session.md` (2026-11-18 highest-bucket triage
  page, Q3), with the user's instruction that this follow one specified standard and reusable mechanism, not an ad hoc
  fix; see that entry.
- Completion criteria: a drop is delivered where the pointer was when it was released (the latest position recorded
  while the file list was being read), as a new patch layer on the vendored GPUI Linux crate with its patch file
  (vendor/README.md), plus a unit test. Where the standard from `header-actions-follow-unseen-session.md` applies to
  drop delivery, follow it rather than inventing a separate rule. Remove the feedback file and its index entry in the
  same change.
- Execution: planned in `plans/queue/drawn-frame-actions.md`

## stalled-tab-request-resets-healthy-helm-channel.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: confirmed. The
  helm's `open_tab` and `close_tab` routes (crates/farhelm-helm/src/sessions.rs) wait on the supervisor with no bound,
  and the supervisor request documents "There is no timeout" (crates/farhelm-helm/src/client.rs). The desktop app's
  channel watchdog fires after 30 s with a command in flight (crates/farhelm-helm-client/src/channel.rs), and the reset
  closes every layer, a half-typed launcher draft included (crates/gui/farhelm-gui/src/session.rs). SPEC_impl.md says
  "every host-waiting route the GUI sends is bounded" and "N is set above the helm's own bound on host waits". The
  hung-systemd filter does not apply: the whole window and every host are affected. Unverified: which real host
  conditions stall these requests that long. Bucket: high (one slow host resets the whole window; a lost draft can be
  retyped).
- Decision: recorded as `fix code` under the clearly-settled rule without asking (2026-11-18 GUI UX triage).
- Completion criteria: the tab open and close routes answer within the helm's host-reply bound through the existing
  `within_host_reply_bound_saying` helper, with a timeout reply saying the operation carries on, and a paused-clock test
  that a stalled supervisor answer no longer trips the desktop channel's watchdog. Size gate: about 100 lines of diff
  (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## old-suspension-stamp-labels-later-external-pause.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: confirmed. Only
  Farhelm's own Resume clears the stored suspension time (crates/farhelm-helm/src/tensorlake/lifecycle.rs), a later
  suspension reuses it (crates/farhelm-helm/src/manager.rs), and the app labels it "Suspended from Farhelm" with the old
  time (crates/gui/farhelm-gui/src/environment_ui.rs). So after a Farhelm Suspend, a resume outside Farhelm and a later
  Tensorlake idle suspension, details wrongly say Farhelm suspended it, at the old time. SPEC.md "Topology" (Tensorlake
  hosts): "Its details say since when, and whether Tensorlake suspended it on its own." Bucket: other (diagnostic
  wording).
- Decision: recorded as `fix code` under the clearly-settled rule without asking (2026-11-18 GUI UX triage).
- Completion criteria: the stored Farhelm suspension time is cleared once the host is seen running again, without racing
  a Farhelm Suspend (which holds the connection), so a later suspension is attributed to Tensorlake with its own time;
  with a test. Size gate: about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue
  removal or the changelog fragment); over that, execution blocks and brings the item back to the user. Remove the
  feedback file and its index entry in the same change.
- Execution: `pending`.

## approval-answer-acknowledged-after-expiry.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: confirmed. The
  approval waiter can choose Expired while its answer channel is still alive, and the answer handler's ordinary path
  ignores whether the answer was used and replies 204 (`let _ = entry.answer.send(verdict)` in
  crates/farhelm-helm/src/approvals.rs); only Host Add waits for a receipt. The app shows its late-answer notice only on
  410, so the card disappears silently while the agent got "expired", and an Always allow setting may still have been
  saved. SPEC_impl.md: the answer route "returns 410 once the request no longer waits", and "the answer gets 410 and the
  setting stays on, and the GUI's notice says the answer did not take effect". The rare-glitch filter does not apply
  (success reported for an operation that did not take effect). Bucket: other.
- Decision: recorded as `fix code` under the clearly-settled rule without asking (2026-11-18 GUI UX triage).
- Completion criteria: ordinary approval answers use the receipt Host Add already uses, so an answer that lost the race
  with expiry gets 410 and the GUI shows its existing notice; with a paused-clock test. Size gate: about 100 lines of
  diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## stale-reply-test-lacks-processing-witness.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Late-reply test
  can't tell matched reply from newest reply. Terminal-host Info: the test sends both replies back to back, so a broken
  "newest reply wins" client also passes. Confirmed; production is correct. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: send reply 1, assert the second call still waits, then reply 2 (~10-15 lines). At execution,
  fix the test only if the fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism);
  otherwise treat this as a discard and say so in the execution record. Either way, remove the feedback file and its
  index entry.
- Execution: `pending`.

## process-presence-fixture-leaves-sleep-descendant.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Passing test leaves
  a sleep running up to 30 s. Kills only the stand-in shell; its sleep child lingers ≤30 s. Confirmed, harmless. Bucket:
  other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix if a group kill or childless stand-in fits (~5-10), else discard. At execution, fix the test
  only if the fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise
  treat this as a discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## checkpoint-rejection-test-asserts-before-processing.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Rejected-checkpoint
  check runs before the rejection is read. The "no checkpoint" assertion can run before the supervisor processes the
  rejection. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: move the assertion after the status frame (~3 lines). At execution, fix the test only if the
  fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## tab-reap-deadline-excludes-listing-request.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Polling helper's 15
  s limit doesn't cover a stuck check. A hung listing waits for nextest's 5 min kill instead of failing at 15 s.
  Confirmed, never observed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: wrap each check in the deadline (~5-8). At execution, fix the test only if the fix is
  obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## held-start-test-lacks-prelaunch-fencing-witness.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Restart test doesn't
  check the held start ends before the new launch. Would pass if Restart launched first. Confirmed; production orders it
  right. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard (needs a new fixture observation, ~10-20)).
- Execution: `pending`.

## keeper-can-win-one-shot-interrupted-start-gate.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Second session can
  take the gate meant for the first. A race can make two reclaim tests fail or time out (false failure, not false pass).
  Never observed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: copy the sibling test's held marker (~10-15). At execution, fix the test only if the fix is
  obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## rebuild-order-test-misses-persistence-boundary.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Rebuild test doesn't
  check the new snapshot is saved before the old is deleted. Compares API call order only. Confirmed; production order
  is as specified. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard (needs a new fake hook, ~20-40)).
- Execution: `pending`.

## stty-tool-format-mismatch.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Terminal-settings
  test compares two different stty programs. Fails on a Mac with GNU coreutils first on PATH. Confirmed from source.
  Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: give the reference command the host's PATH (~2 lines). At execution, fix the test only if
  the fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this
  as a discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## closed-stdin-epipe-witness.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: The fake-ssh test
  never proves the failed write happened. A test of a test fixture; a buffering refactor would leave it untested.
  Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard (needs a forwarder witness, ~15-25)).
- Execution: `pending`.

## notification-cleanup-row-premise.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Upgrade test checks
  notification cleanup without first showing it exists. An already-empty table would pass. Confirmed; broader coverage
  exists elsewhere. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: assert the count is 1 before the delete (~6). At execution, fix the test only if the fix is
  obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## successor-exit-processing-witness.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Exit-code test may
  read before the new supervisor caught up. A slow journal replay makes a correct restart fail the test. Never observed.
  Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: wait for the dead agent first (~3). At execution, fix the test only if the fix is obvious
  and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a discard and
  say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## hook-waiter-missing-invocation-binding.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A repeated hook
  callback can be credited with an earlier one's clean finish. Markers aren't tied to one call; no redraw happens in the
  fixture as written. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard (needs per-call nonces, ~30-60)).
- Execution: `pending`.

## tab-survival-missing-live-exchange.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Tab-survival test
  never asks the shell to do anything after restart. A shell that died in the handoff could still pass. Confirmed gap.
  Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: run a command in the shell after reattach (~8-10, existing helper). At execution, fix the
  test only if the fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism);
  otherwise treat this as a discard and say so in the execution record. Either way, remove the feedback file and its
  index entry.
- Execution: `pending`.

## claude-recreation-test-clears-old-stamp.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage:
  Recreated-Claude-login test clears the stamp it should check. Would pass without the incarnation comparison.
  Confirmed; production has it. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: add a sibling test that keeps the old stamp (~25). At execution, fix the test only if the
  fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## catalog-retry-test-does-not-witness-inventory.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Restart-with catalog
  Retry test doesn't prove the retried catalog was used. The desktop port inherited the browser test's gap: Retry clears
  the error on click and the sent body is the same either way. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: assert a catalog-only effect (~5-15). At execution, fix the test only if the fix is obvious
  and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a discard and
  say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## destructive-action-tests-omit-request-targets.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Destructive-action
  tests don't check which session/host the request targets. ~20 desktop tests would pass if Delete/Stop/Replace went to
  a different target. Confirmed at spot-checked sites. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard under the rule (80-150 lines via shared
  helpers); say so if you want it kept).
- Execution: `pending`.

## launcher-escalation-test-misses-banner-premise.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "Agent switch clears
  the banner" test never checks the banner appeared. Passes if the banner never showed. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: assert the banner in the fixture (~3). At execution, fix the test only if the fix is obvious
  and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a discard and
  say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## launcher-model-height-test-accepts-collapsed-list.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Model-list height
  test passes for a zero-height list. Checks only an upper bound. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: add a lower bound (~5-10). At execution, fix the test only if the fix is obvious and trivial
  (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a discard and say so in the
  execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## checkout-fixture-inherits-git-repository-selector.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Test Git fixtures
  can write into another repo under an inherited GIT_DIR. Two Python release-test fixtures don't scrub GIT_* for their
  git children (the Rust part is gone). Archive precedents #1798/#2158 fixed the same class. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: child-only env scrub in both fixtures (~15-25). At execution, fix the test only if the fix
  is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## signal-test-child-lacks-unwind-cleanup.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Test processes
  outlive a failed test because cleanup is armed late. Several fixtures start stand-ins before their kill-and-reap
  guard; two never end on their own. Only on already-failing runs. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard (all sites ~100-250); say so if you want
  the two never-ending stand-ins fixed (~20-40)).
- Execution: `pending`.

## inherited-lock-test-self-dup.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Version-lock test
  may never hand the lock over. If the lock file already has fd 8, dup2(8,8) leaves it close-on-exec and the test proves
  nothing. Conditional; sibling test handles it. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: copy the sibling's guard and assert the premise (~10-15). At execution, fix the test only if
  the fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this
  as a discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## failed-client-close-test-lacks-writer-premise.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "Endless sender"
  test can run before the sender starts. Would still pass if the final read lost its limit. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard (no clear witness shape)).
- Execution: `pending`.

## stderr-readiness-lacks-live-holder-premise.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "Don't wait for
  stderr" test never checks something holds stderr. The holder sleep is never shown alive. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: require the holder pid and assert it alive (~5-10). At execution, fix the test only if the
  fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## late-listener-blocks-start-test-cleanup.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A slow stand-in
  listener can hang a test until the runner kills it. accept() without a deadline; 300 s kill instead of prompt failure.
  Not observed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: accept with a deadline (~10-15). At execution, fix the test only if the fix is obvious and
  trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a discard and say so
  in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## staged-start-test-lacks-reader-drain-witness.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "Nothing reaches a
  terminal before its owner is recorded" test can miss early bytes. No witness that the reader drained. Confirmed;
  production correct. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard (needs a new fixture mechanism, ~30-60)).
- Execution: `pending`.

## readiness-test-does-not-establish-first-stage-exit.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "Start returns after
  its first stage exits" test can't tell. A live first stage also passes. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: also assert the parent isn't a live child (~5-10). At execution, fix the test only if the
  fix is obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## peak-memory-test-lacks-baseline-headroom.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Snapshot memory test
  could hide a spike under an earlier peak. The main case got a premise in #3209; the fixups case deliberately has none,
  and a false pass is unshown. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard the remainder (record #3209 as fixing the
  rest)).
- Execution: `pending`.

## malformed-snapshot-fixtures-mask-intended-validators.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Corrupt-snapshot
  fixtures are refused by a size check before the check they name. Five fixtures are truncated, so the named validators
  could regress unnoticed. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard under the rule (~30-50 lines); say so if
  you want it kept).
- Execution: `pending`.

## recording-loader-rejects-valid-json-field-spacing.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Private-recording
  loader rejects JSON with a space before the colon. Maintainer-only ignored test; legal JSON panics. Confirmed. Bucket:
  other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: skip whitespace before the colon (~5). At execution, fix the test only if the fix is obvious
  and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a discard and
  say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-removed-live-alternate-screen-reattach-oracle.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: No test checks
  reconnecting to a live full-screen program shows its screen. The tmux-era test was deleted without a terminal-host
  replacement. Confirmed coverage gap. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: Remove the feedback file and its index entry (discard (a new test is ~40-60); say so if you want
  it added).
- Execution: `pending`.

## trailing-converted-old-session-warning-retention-assertion-vacuous-all.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Old-terminal test
  passes even if the old warning vanished. `.all` on a possibly empty list. Confirmed. Bucket: other.
- Decision: the user picked "Apply the rule as listed" (2026-11-18 GUI UX triage page, QA), reapplying the 2026-10-26
  Q4/Q5 rule: fix a test gap only when one obvious shape of about 30 lines or less with no new test mechanism fixes it,
  otherwise discard.
- Completion criteria: fix: assert exactly one resolved warning (~3-5). At execution, fix the test only if the fix is
  obvious and trivial (one clear shape, about 30 lines or less, no new test mechanism); otherwise treat this as a
  discard and say so in the execution record. Either way, remove the feedback file and its index entry.
- Execution: `pending`.

## transition-accessor-comment.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Store test comment
  names a method that no longer exists. Transition::pane is now Transition::address. Confirmed. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix comment (1 line)). Size gate: about 100 lines of diff (code, tests and
  docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and
  brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## settings-close-tooltip-promises-immediate-discard.md

- Outcome: `fix spec+code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Settings' Close
  tooltip says edits are discarded, but recipe edits ask first. Both the tooltip and SPEC.md "closing discards unsaved
  fields" omit the recipe leave question. Confirmed. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix spec+code: add the exception to both (~4)). Size gate: about 100 lines
  of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over
  that, execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## trailing-spec-automatic-tab-reap-omits-failed-close-retry.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Spec sentence on
  automatic tab cleanup forgets the failed-close retry. SPEC_impl contradicts itself; the code follows the more specific
  rule. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix spec (~2-4)). Size gate: about 100 lines of diff (code, tests and
  docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and
  brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## trailing-agent-file-size-only-read-consistency-docstring.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Comment on sandbox
  settings reads overstates what it detects. Only size is compared; the later write's digest guard covers the rest.
  Confirmed. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix comment (~3)). Size gate: about 100 lines of diff (code, tests and
  docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and
  brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## trailing-late-activity-test-doc-retains-removed-screen-read-await.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Test explanation
  names a removed screen-read wait. The read is now synchronous; the test is still valid. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix comment (~3)). Size gate: about 100 lines of diff (code, tests and
  docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and
  brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## trailing-list-reply-doc-prescribes-nonexistent-sentinel-read-failure.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Listing comments say
  an unreadable launch file fails the list. Contradicts SPEC_impl ("does not fail lists") and the code. Confirmed.
  Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix comments (~4)). Size gate: about 100 lines of diff (code, tests and
  docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and
  brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## trailing-status-documentation-probe-versus-processed-exit-evidence.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Status comments say
  a terminal can look dead before its exit is processed. The code reads evidence that stays live until the exit record
  is processed. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix comments (~6)). Size gate: about 100 lines of diff (code, tests and
  docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and
  brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## bash-source-admission-retains-literal-splitter.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Some valid Bash
  commands are refused despite SPEC's "reaches Bash unchanged". The shell-word check and exact placeholder slots are
  deliberate in SPEC_impl; SPEC.md promises more. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix spec: name the limits in SPEC.md (~3-5); widening the code is
  ~80-150). Size gate: about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal
  or the changelog fragment); over that, execution blocks and brings the item back to the user. Remove the feedback file
  and its index entry in the same change.
- Execution: `pending`.

## peer-text-normalization-contract-overstates-retained-scope.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "All host text is
  normalized the same way" is too broad. Same section keeps raw bytes for diagnostics and identities. Spec text only.
  Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix spec (~2)). Size gate: about 100 lines of diff (code, tests and docs,
  not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and brings
  the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## keyboard-overview-omits-in-sequence-number-action.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Shortcuts overview
  doesn't say what a number chord does inside a sequence. Inside a sequence it queues the unfinished word first; no row
  says so. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix: add an in-sequence row (~5)). Size gate: about 100 lines of diff
  (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## digit-leading-template-limit-overstated.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Spec and docs say
  digit-leading template names can't be typed. On Shift-digit layouts (French) they can. Confirmed in code. Bucket:
  other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix spec + docs page: qualify to a bare digit (~4)). Size gate: about 100
  lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment);
  over that, execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the
  same change.
- Execution: `pending`.

## checkout-refusal-conflicts-with-failed-save-rule.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: General "failed save
  is discarded" rule ignores the checkout-folder exception. Code keeps the refused draft as SPEC.md:1700 says; the
  general rule doesn't name its exceptions. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix spec: general rule names its exceptions (~2-5, shared with the next
  item)). Size gate: about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal
  or the changelog fragment); over that, execution blocks and brings the item back to the user. Remove the feedback file
  and its index entry in the same change.
- Execution: `pending`.

## failed-template-save-retains-attempted-draft.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Template editor
  keeps a failed draft, against the general rule. SPEC.md now says the editor keeps it (#3597); same unacknowledged
  conflict as above. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix spec, together with the item above). Size gate: about 100 lines of
  diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## omp-permission-docs-retain-yolo-default.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: SPEC_impl still says
  OMP defaults to YOLO. SPEC.md and code say Guarded is the default. Confirmed. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix spec (~2)). Size gate: about 100 lines of diff (code, tests and docs,
  not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and brings
  the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## omp-readiness-comment-excludes-early-save.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Comment says OMP
  saves only after the first reply. SPEC says OMP saves a new conversation eagerly. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix comment (~2)). Size gate: about 100 lines of diff (code, tests and
  docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and
  brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## gui-migration-comments-need-current-contracts.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Desktop code
  comments still describe the migration as unfinished. Several contradict shipped behavior; 358 lines mention
  coordinator/orchestrator overall. Bucket: other.
- Decision: the user picked "Smallest truthful correction for each" (2026-11-18 GUI UX triage page, QB), as for wording
  on 2026-10-26 (Q10).
- Completion criteria: change the wording so it is true, without adding behavior: where the code and the text disagree,
  the text is corrected to describe the code (fix only the comments that are now false (~15 sites, ~60 lines), not a
  wholesale sweep). Size gate: about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue
  removal or the changelog fragment); over that, execution blocks and brings the item back to the user. Remove the
  feedback file and its index entry in the same change.
- Execution: `pending`.

## desktop-state-rewrite-blocks-startup.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A bad desktop
  settings file stops the app at a startup alert. Only an outside edit or corruption produces a bad file; read/write
  failures fall under SPEC.md "Healthy local filesystems". The refusal is deliberate. Bucket: re-bucketed from high to
  other (no established route to a high consequence).
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## attention-list-refresh-failure-scope-unresolved.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A host whose session
  list stops refreshing gets no attention row. SPEC's attention list is an enumerated list; Hosts details does show the
  failure. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## failed-setup-refresh-overwrites-held-lifecycle-status.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: After a failed
  registry sync, a refresh shows Connecting over a running Suspend. Needs the helm's own database read to fail; the
  display corrects at completion. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## notification-read-failure-leaves-stale-cell.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A failed
  notification reload leaves the bell wrong until the next event. Needs a local SQLite read failure; next lifecycle
  event or restart fixes it. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## attention-save-error-can-exhaust-list-height.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A save error plus a
  wrapped header leaves no room for attention rows. Unreachable at the app's 720×560 minimum window. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## short-progress-card-can-hide-overflow-notice.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A very short
  progress card clips "Full details in Hosts". Card limit is ≥550 px at the minimum window; effectively unreachable.
  Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## open-source-notices-window-failure-is-silent.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Open-Source Notices
  does nothing if its window can't open. No realistic macOS trigger for a failed window creation. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## desktop-startup-reopen-can-wait-on-reused-pid.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Reopen helper waits
  forever if the old process number is reused. Needs macOS to reuse a pid within ~0.1 s. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## open-types-acknowledgement-only-in-dialog.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "The helm stores any
  file-type list without the acknowledgement". Incorrect: "acknowledged save" means a save the helm stores first, not a
  user acknowledgement step. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-supplied-timing-comment-defect-is-source-confirmed-and-already-owned-by-g020-l7.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Second copy of the
  relay-ordering comment finding. Duplicate of the queued
  trailing-environment-wait-comment-promises-a-relay-ordering-the-budgets-do-not-guarantee.md. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-shared-rollback-allowance-does-not-remove-the-refusal-tests-observable-boundaries.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Stand-in cleanup
  does not hide destructive actions. Refuted by its own reviewer; the fixture counts destroys. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-deleting-extends-the-known-incomplete-backend-phase-test-matrix.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Host-phase test also
  misses Deleting. Duplicate of the queued trailing-host-phase-vocabulary-test-omits-stopped.md. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-the-nothing-decided-terminology-refutation-does-not-cover-the-stronger-no-change-comment.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "Nothing was
  decided" wording does not promise nothing changed. Refuted; the stronger comment was in deleted code and is tracked
  separately. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-rest-admission-owns-registration-before-the-launch-task-is-spawned.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Dropped client can't
  strand a half-registered environment launch. Refuted by its own reviewer; the work is helm-owned. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-provider-preparation-callbacks-await-insertion-before-reporting-phases.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Preparation progress
  steps are not misattributed. Refuted by its own reviewer. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-atomic-install-failed-cleanup-clears-safe-session-limit-refusal.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Failed-cleanup path
  loses the "SSH session limit" explanation. A less specific warning when two failures coincide; the failure is still
  reported. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-deletion-ownership-refutation-remains-separate-from-warning-repair.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Recipe deletion
  keeps its hold when the client disconnects. Refuted; the pruning it set aside is fixed (#4085). Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-launch-excerpt-full-row-allocation-before-500-byte-trim.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: 500-byte
  launch-error excerpt briefly holds more text. A small fraction of an allocation the same path already makes; no
  latency shown. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-launch-excerpt-whole-row-intermediate-allocation.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Launch-error excerpt
  memory + earlier-run quoting. Quoting part is covered by plan triage-misc-nov18; memory part duplicates the item
  above. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## gone-retry-forgets-pinned-foreground-child.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A reconnect that
  finds the host gone forgets the foreground program. The remaining route is inside SPEC's accepted limit (unscoped
  host, a program that hides its environment). Bucket: re-bucketed from high to other (no established route to a high
  consequence).
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## normal-quit-misses-empty-download-directory.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Quit can leave an
  empty hidden download staging folder. Microsecond race; no file lost. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## numeric-version-guard-hides-artifact-filenames.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Filenames starting
  with a version number get no file actions. 1.2rc1-notes.txt is treated as a version; ./ works. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## tab-helper-can-fail-after-open-succeeds.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A slow tab can
  report opened, then vanish without the folder error. The plan unusable-folder-refusals removes the main case; the
  remainder needs the folder to change mid-start. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## spare-ancestry-fallback-skips-destination-recheck.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Refused prepared
  checkout clones into a swapped-in folder. Needs the user to swap the allocated folder during the fetch; nothing
  existing is lost. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## network-state-wal-failure-topology-unresolved.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Database journal
  mode on a network-mounted state folder. No supported layout producing the failure was found. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## muse-unmarked-same-process-child-attribution.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A Muse sub-agent
  report might be taken as the session's conversation. No Muse version is known to send such a report. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## refused-add-rewrites-new-destination-owner.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Refused host setup
  can rewrite another busy host's install paths. Needs two retargets inside a millisecond window with no remote work.
  Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## desktop-owner-record-lacks-boot-domain.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Linux desktop
  supervisor record can match a different process after reboot. No realistic match shown; the stop is an orderly
  SIGTERM. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## changed-project-key-cleanup-absence-unresolved.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: After switching to a
  key for another Tensorlake project, cleanup may forget resources. Depends on unverified provider 404-vs-403 behavior.
  Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## first-state-directory-ancestor-durability-unresolved.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: First-run state
  folder's parents aren't explicitly flushed. No supported filesystem shown to lose it. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## ambiguous-codex-seed-renewal.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Re-sending a saved
  Codex login after an uncertain upload can overwrite a renewed one. The replay is accepted in SPEC_impl; the
  broken-login consequence is unverified. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## launcher-live-draft-submission-source-gap.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Launcher reads
  current choices at submit (unresolved). All cited code is the deleted Dioxus launcher; no GPUI sequence found. The
  drawn-frame plan sets the rule. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## header-copy-warning-survives-different-copy.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Copy warning
  describes the earlier field for up to 1.5 s. Clipboard bytes are correct; self-corrects. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## secondary-button-erases-intercepted-primary-ownership.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A second mouse
  button during a link click stops the link from opening. Chorded gesture; a second click recovers; nothing leaks to the
  program. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## truncated-empty-filter-result-omits-clear-actions.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Empty result from a
  capped listing has no Show all links. Deliberate in code; only above the per-host cap; the rail still works. Bucket:
  other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## transfer-result-height-can-hide-destination.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A long download
  result line can be cut off. Capped at three lines; full text in the accessible name. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## backup-warning-can-imply-inconsistent-live-copy.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Backup sentence in
  v0.33.0 release notes reads like a recipe. Published notes are frozen; the sentence isn't false. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## long-launch-status-crops-painted-edge.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Launch progress
  pill's edge runs off screen. Old CSS cause is gone; desktop layout likely fine (unmeasured). Cosmetic. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## subline-progress-preview-hides-diagnosis.md

- Outcome: `discard`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Host diagnostic
  preview can round to zero lines. Ported rounding; a real window producing it is unverified. Bucket: other.
- Decision: the user picked "Discard all" (2026-11-18 GUI UX triage page, QC): unestablished route, bounded internal
  race, below the minimum window, unmeasured perf, refuted by its own reviewer, or a duplicate of a queued item.
- Completion criteria: Remove the feedback file and its index entry.
- Execution: `pending`.

## layout-preference-save-failures-are-silent.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Sidebar width, text
  size and rail expansion revert silently after a failed save. SPEC.md's best-effort list doesn't name these three (made
  helm preferences 2026-11-13); trigger is a helm database write failure. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix spec: add the three to the best-effort list (~3). Size gate: about 100 lines of diff (code,
  tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution
  blocks and brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## closing-settings-can-hide-ntfy-save-refusal.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Closing Settings
  before an ntfy save is refused loses the refusal. SPEC already drops errors on close for some switches, not for ntfy.
  Confirmed. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix spec: accept dropping the refusal on close, as for the switches (~2). Size gate: about 100
  lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment);
  over that, execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the
  same change.
- Execution: `pending`.

## trailing-retired-session-tcp-relay-cache-only-admission.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A port tunnel can
  open for a retired container session if its cached record is unreadable. The supervisor's tunnel start doesn't check
  retirement (uploads do); the helm comment claiming it does is false. No shipped trigger. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: supervisor refuses tunnels for retired sessions + fix the comment (~10-15 + test). Size
  gate: about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the
  changelog fragment); over that, execution blocks and brings the item back to the user. Remove the feedback file and
  its index entry in the same change.
- Execution: `pending`.

## recovered-agent-retains-stale-launch-hold-handle.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: After a terminal
  reconnect, Mac supervisor replacement waits 3 s and warns wrongly. The shutdown check reads the ended terminal's
  handle. Confirmed. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code (~15-30). Size gate: about 100 lines of diff (code, tests and docs, not counting
  TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and brings the item back
  to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## agent-endpoint-refuses-valid-at-sign-path.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Endpoint URL with @
  in its path is refused as credentials. https://api.example.com/@team/v1 can't be saved. Confirmed. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: refuse @ only in the host part (~20 with tests). Size gate: about 100 lines of diff
  (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## approval-scrollports-lack-keyboard-entry.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Keyboard users can't
  scroll clipped approval facts. SPEC.md: every scrollable region gets a keyboard tab stop. Confirmed in desktop code.
  Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: tab stop on overflowing one-line facts (~40-80). Size gate: about 100 lines of diff
  (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## header-questions-follow-anchor-across-sessions.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A header question
  can show under the next session's button. Only route found: the open session is filtered out and deleted elsewhere;
  answering is then refused by the helm. Bucket: re-bucketed from high to other (no established route to a high
  consequence).
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: session key in the anchors, as the carets do (~45 with test); bucket other. Size gate:
  about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog
  fragment); over that, execution blocks and brings the item back to the user. Remove the feedback file and its index
  entry in the same change.
- Execution: `pending`.

## session-navigation-interrupts-close-all.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Switching sessions
  during tunnel Close all leaves the rest open. SPEC: Close all attempts every listener shown. Confirmed in desktop
  code. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: keep the run alive until it ends (~10-20 + test). Size gate: about 100 lines of diff
  (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## bulk-completion-focuses-sort.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A finished bulk
  action sends focus to the sort title, not the terminal. SPEC says focus goes to the terminal when a control goes away;
  code chose consistency with Clear/Escape. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: send focus home on completion, and Clear/Escape alike (~3-10 + test). Size gate: about
  100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog
  fragment); over that, execution blocks and brings the item back to the user. Remove the feedback file and its index
  entry in the same change.
- Execution: `pending`.

## selected-template-state-contrast.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Selected template
  barely stands out in Dark. 2.76:1 against the dialog, no other marker. Confirmed. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: palette tweak or selected marker (~2-30). Size gate: about 100 lines of diff (code,
  tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution
  blocks and brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## selected-template-shortcut-contrast.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Shortcut hint is
  unreadable on the selected template. Muted ink on accent fill, 1.14:1 in Light. Confirmed. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: use the inverse ink like the summary line (~3-10). Size gate: about 100 lines of diff
  (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## sentence-case-sweep-leaves-authored-lowercase-text.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Launcher held-field
  refusal starts lowercase. "agent type cannot change here: ..." in Restart/Replace with. Confirmed. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: capitalize (~10 incl. tests). Size gate: about 100 lines of diff (code, tests and docs,
  not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and brings
  the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## rail-tooltip-merges-direction-boundaries.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Host tooltips join
  host text with app words as plain text. SPEC_impl "Host-supplied text": host text is never concatenated with app words
  outside a host sentence. Confirmed in three call sites. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: build them as host sentences (~30-50). Size gate: about 100 lines of diff (code, tests
  and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks
  and brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## brace-literal-command-display-is-bash-inexact.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Displayed command
  shows x{a,b} unquoted. Bash would expand it; what runs is one literal argument. Display only. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: quote words with { } ! ^ (~5-15). Size gate: about 100 lines of diff (code, tests and
  docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution blocks and
  brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## template-collision-refusal-mixes-unisolated-name.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: "Name already taken"
  embeds the name as plain debug-quoted text. Template names are shown as host text elsewhere. Confirmed. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: build the refusal as a host sentence (~5-15). Size gate: about 100 lines of diff (code,
  tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that, execution
  blocks and brings the item back to the user. Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## gui-labels-and-refusals-name-different-actions.md

- Outcome: `fix code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Dialogs and messages
  name the same action differently, a few wrongly. E.g. "Seen:" and "Sandbox:" prefixes, the model Retry tooltip, the
  tab-limit placeholder. Confirmed at spot-checked sites. Bucket: other.
- Decision: the user picked "Accept each row's recommendation" (2026-11-18 GUI UX triage page, QD).
- Completion criteria: fix code: only the wrong-action/wrong-cause messages (~30-60). Size gate: about 100 lines of diff
  (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog fragment); over that,
  execution blocks and brings the item back to the user. Remove the feedback file and its index entry in the same
  change.
- Execution: `pending`.

## linux-sound-resolves-player-on-path.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Linux notification
  sounds run the first pw-play on PATH. No route lets host content influence it; the user's PATH is theirs. Bucket:
  other.
- Decision: the user picked "Yes, one SPEC paragraph" (2026-11-18 GUI UX triage page, QF). Principle: the user's own
  local configuration is theirs to keep sane; Farhelm does not defend against it (helper programs found on the user's
  PATH, a custom command's own agent flags, unusually large agent settings files).
- Completion criteria: one SPEC.md paragraph (about 5 lines, in the trust section) states that the user's own local
  configuration is trusted and theirs to keep sane: helper programs the desktop app runs are found on the user's PATH,
  flags a custom command passes to its agent are the user's (Farhelm's added hook flags may clash with them), and an
  agent settings file the user grew near Farhelm's read limit is theirs to manage. No code change. Remove this feedback
  file and the other two QF items (muse-current-hook-override-semantics-unresolved.md,
  agent-settings-write-exceeds-next-read-limit.md), with their index entries, in the same change.
- Execution: `pending`.

## muse-current-hook-override-semantics-unresolved.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: A Muse custom
  command that sets its own hooks may clash with Farhelm's. Farhelm adds its hook flags without reading the rest; the
  docs already say such flags are yours to avoid. Bucket: other.
- Decision: the user picked "Yes, one SPEC paragraph" (2026-11-18 GUI UX triage page, QF). Principle: the user's own
  local configuration is theirs to keep sane; Farhelm does not defend against it (helper programs found on the user's
  PATH, a custom command's own agent flags, unusually large agent settings files).
- Completion criteria: carried by `linux-sound-resolves-player-on-path.md`.
- Execution: `pending`.

## agent-settings-write-exceeds-next-read-limit.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Saving agent
  settings can push a near-1 MiB file over the next read's limit. Only for agent settings files near 1 MiB; realistic
  files are far smaller. Bucket: other.
- Decision: the user picked "Yes, one SPEC paragraph" (2026-11-18 GUI UX triage page, QF). Principle: the user's own
  local configuration is theirs to keep sane; Farhelm does not defend against it (helper programs found on the user's
  PATH, a custom command's own agent flags, unusually large agent settings files).
- Completion criteria: carried by `linux-sound-resolves-player-on-path.md`.
- Execution: `pending`.

## irreversible-confirmation-focus-contract.md

- Outcome: `fix spec`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: Two focus rules
  compete for permanent trash deletion. SPEC.md says trash deletion starts on its confirming answer, then that risky
  irreversible actions start on Cancel. The app follows each rule in different places. Bucket: other.
- Decision: the user picked "Confirm-first stays for the named list" (2026-11-18 GUI UX triage page, QG), in their
  words: "the trash cdeletion case is when the user is already in a flow where the intent is completely obvious and
  clear so a confirm makes sense. you're not being prompted for a "whatch out, this might be an unexpected risk" (like
  you are when you try to launch yolo on a box w/o yolo allowed for example)." Principle: Confirm-first applies where
  the user is already in a flow whose intent is plain; Cancel-first is for prompts warning of an unexpected risk.
- Completion criteria: SPEC.md's focus rules say which wins: confirmations named in the Confirm-first list (permanent
  trash deletion among them) start on their confirming answer because the user is already in a flow whose intent is
  plain, and the Cancel-first rule is for prompts that warn of an unexpected risk (YOLO on a host that does not allow
  it, for example). No code change (about 2-3 lines). Remove the feedback file and its index entry in the same change.
- Execution: `pending`.

## sequence-following-enter-does-not-count-as-moving-on.md

- Outcome: `fix spec+code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: confirmed in source.
  After a Cmd+X quick-launch sequence is launched with Enter, the terminal keeps focus while the launch is pending. The
  moved-on check (crates/gui/farhelm-gui/src/focus.rs `note_press`) treats a press as a repeat whenever it is the same
  keystroke on the same focused control as the previous press, with no time limit, so if the first key the user presses
  afterwards is Enter in that terminal (reaching the shell or agent), it does not count as moving on and the finished
  launch still switches to its session and opens the naming dialog. SPEC.md "Opening and closing" exempts only "an
  accidental double-click or repeated Enter on the launcher's own submit"; the code applies it to any control. Bucket:
  high (ordinary but narrow trigger: Enter must be the first key; the app takes the keyboard to a dialog; nothing is
  lost).
- Decision: the user picked "Drop the exemption everywhere" (2026-11-18 GUI UX triage page, QE-2, round 5), after asking
  in their words: "except for esc spamming I don't agree about swalling when the user is the one initiating the irst
  press to make the dialog/whatever go away. if we apply that consistently, then this seems to also go away - for both
  the launcher window AND the sequence case". Principle: when the user's own press dismisses, answers or submits
  something, their next presses are ordinary input; they reach whatever has focus and count as moving on. Only
  Escape-spamming out of menus and dialogs gets the half-second swallow. Auto-repeat of a held key and a lone modifier
  are not new presses and still do not count as moving on.
- Completion criteria: SPEC.md "Opening and closing" drops the exemption for "an accidental double-click or repeated
  Enter on the launcher's own submit" (keeping auto-repeat and a lone modifier), and states the principle above with
  `enter-falls-through-after-dismiss-or-approval.md`'s fence change (one paragraph may carry both). The moved-on count
  in the desktop app counts every fresh press and click after the launch's answer, from the launcher window and from a
  quick-launch sequence; the time-unbounded same-key check goes. Tests: a delayed launch where a second Enter on Launch,
  and an Enter in the terminal after a sequence, each leave the user where they are with the "Started" pill. Size gate:
  about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal or the changelog
  fragment); over that, execution blocks and brings the item back to the user. Remove the feedback file and its index
  entry in the same change.
- Execution: `pending`.

## enter-falls-through-after-dismiss-or-approval.md

- Outcome: `fix spec+code`.
- Assessment: by a delegated checker on main bc98e4ec3 (code reading, not run), reviewed by triage: confirmed in source.
  After Enter dismisses an empty quick-launch sequence or answers an approval card, focus is (or returns to) the
  terminal, and the user's own held Enter or a second tap reaches the shell or agent. The pill and cards are outside the
  after-close fence, which SPEC.md's focus rules give to dialogs and menus: "After an Enter or Escape closes the last
  open dialog or menu, the same key held or pressed again within half a second goes nowhere". In both routes the surface
  goes away because the user answered it. Bucket: high as filed.
- Decision: the user said the case itself is fine ("(1) is fine", rounds 2-3: the user actually answered, nothing else
  removed the surface) and picked "Keep this case as is, and narrow the fence to Escape" (2026-11-18 GUI UX triage page,
  QE-1, round 5), applying the principle recorded with `sequence-following-enter-does-not-count-as-moving-on.md`:
  outside Escape-spamming, presses after the user's own answer are ordinary input.
- Completion criteria: no change for the sequence pill or approval cards. SPEC.md's after-close fence keeps the
  half-second swallow for Escape only: after an Escape closes the last open dialog or menu, Escape held or pressed again
  within half a second goes nowhere; an Enter after an Enter closed a dialog or menu reaches what has focus like any
  press. The desktop app's focus code (crates/gui/farhelm-gui/src/focus.rs closing-press repeat handling) and its tests
  follow. Size gate: about 100 lines of diff (code, tests and docs, not counting TRIAGE_OUTCOMES.md, the queue removal
  or the changelog fragment); over that, execution blocks and brings the item back to the user. Remove the feedback file
  and its index entry in the same change.
- Execution: `pending`.
