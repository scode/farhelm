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
