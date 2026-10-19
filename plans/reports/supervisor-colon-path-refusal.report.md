# Refuse program folders containing colons

## What this was about

Farhelm puts its program folder first on a session's command-search path. A colon in that folder splits it into
unintended search entries, possibly including one relative to the session's checkout. On a Mac, a slash entered in
Finder can produce a colon in the underlying path. Command hijacking was not demonstrated; triage authorized a cheap
startup refusal, with the desktop app explaining the failure clearly.

The supervisor now refuses before creating its state, naming the folder and the PATH separator problem. The desktop app
applies the same check before starting a managed supervisor, so its existing startup alert carries that reason. The
specification records the refusal, and the feedback item is removed with its execution recorded in the ledger.

## Things you should know

The small-change gate held: the supervisor adds fourteen production lines, including its call to the shared check, plus
a regression. The separately authorized desktop addition is five production lines. The Mac versioned layout checks the
forwarder's folder, which sessions actually receive. A colon confined to the executable filename remains allowed. An
already answering supervisor continues to be reused.

Review caught a relative desktop executable override missing a colon in its working directory. The desktop now resolves
that path before checking it. No new launch protocol or stderr capture was introduced. The desktop startup caller
compiled on Linux. The existing native Mac alert was checked by reading its error path; it was not exercised in a
running Mac app.

Final fetch left main unchanged at `4a460e59f9ad1160073b4ce57fe583106c64096d`, so no intervening main changes needed
reconciliation. The PR is open and draft for the landing monitor.

## Open questions and possible follow-ups

None required by this plan. This implements the agreed refusal; it does not establish an exploit or change how sessions
choose command-search directories.

## PRs

- [#2361: refuse program folders containing colons](https://github.com/scode/farhelm/pull/2361/changes) — bookmark
  `plan/supervisor-colon-path-refusal/01-colon-path-refusal`, change `rquzzryupktvqtnykrumlqmtzmstpyov`, head
  `08a2e0e455143b22afad304843cb258429b0f012`.

## Checks run, reused and skipped

- Ran through `scripts/record-test-run.py --runner nextest --kind development --tmux none`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(a_colon_in_the_session_program_folder_is_refused_at_startup) | test(a_non_utf8_farhelm_path_or_state_dir_is_refused_at_startup)'`.
  Run `2e7e809b-0cd2-4576-9915-6f0fee94fd97`: two passed, 1,159 selected out, complete evidence, no runtime `SKIPPED`
  marker. Pinned nextest 0.9.148; these refusals return before tmux setup. The new regression covers ordinary and
  versioned app folders, the named effective folder, absent state after refusal and a filename-only colon.
- That runtime run covered working tree `e262d9d658bede711fbb618c52e0cc05320d9c53` (recorder Git HEAD
  `4a460e59f9ad1160073b4ce57fe583106c64096d` plus its tracked diff). Reused for the final head: the subsequent desktop
  path correction and ledger URL do not change the tested supervisor or regression.
- Ran `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop`: all passed at working tree
  `a5ce9fc8d31af0b5185d0507ab20c5ab57be9bbb`, including the desktop correction. Reused after commit wording and the
  ledger-only URL amend, which changed no compiled source.
- Ran `cargo fmt --all -- --check` at that revised tree and
  `dprint check SPEC.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` again after the URL amend: passed.
- Ran `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and the isolated
  interpreter's `python -B scripts/check-test-sleeps.py`: passed at `e262d9d658bede711fbb618c52e0cc05320d9c53`; the
  sleep checker inspected 304 delays with none unannotated. Reused because later edits added no fragment, path, heading
  or test delay.
- Ran `diff -u` between sorted queue index targets and feedback filenames, excluding infrastructure: no differences.
- Skipped broader Rust, browser, JavaScript, installer, provisioning and desktop runtime suites: the risk is a startup
  refusal covered by the constructor regression, plus a small desktop caller compiled in both targets. No GUI control,
  session launch execution, installer or provisioning behavior changed. No failed runtime attempts occurred.

## Review gate

The single required fresh-context gpt-6.1-sol reviewer at high effort reviewed the complete diff against the goal,
decisions, full ledger entry, complexity gate and verbatim test-authoring checklist. Its one P2 finding about a relative
desktop override was fixed and rechecked by that reviewer, with no remaining findings or acceptance gap. It agreed that
a test merely repeating the standard library's path resolution would add no useful evidence. The commit/PR message cold
reader found no unclear or contradicted claims or convention violations. The final amend only adds the PR URL.
