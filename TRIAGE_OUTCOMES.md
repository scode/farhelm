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
