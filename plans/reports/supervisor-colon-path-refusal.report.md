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

### Landing

Landed as #2361, unchanged from what the report describes. It was the seventh of eight plans in this landing round.

#### What landed since the stack was based, and whether it interacted

Since the PR's base, main gained link-hover-covers, workspace-trust-destination, session-header-and-row-fixes,
queued-launch-snapshot, payload-lock-reproduction, triage-discards-oct18 and plan bookkeeping. None of them changes how
the supervisor starts, the folder it puts on a session's command-search path, or the desktop app's startup of its
managed supervisor.

The closest work in the round was launch-enters-working-dir, which was blocked at landing and has not merged. It edits
the same two supervisor files as this plan, in different code: this plan checks the program's folder at startup, while
that plan makes sessions and terminal tabs enter their own folder. A fresh-context reviewer and I read the two together
and found no conflict between them. That plan would start terminal tabs through the Farhelm program itself, passing its
path as a plain argument rather than through the command-search path, so this plan's check is neither needed for it nor
affected by it. Its blocked question is about its own tests and error wording, not about this plan.

The only conflicts were in the review feedback queue's index, where every plan in the round removes its own entries; all
removals are kept. The landed code is identical to what the landing's checks below tested.

#### Fixes made during the landing

None.

#### Checks

Run now, on one tree holding this plan together with the round's other landing plans (launch-enters-working-dir removed
after it failed its checks):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop` (the desktop app's
  startup caller), the isolated test-sleep check, `dprint check` on every changed file, and
  `python3 releasing/check-changelog.py format`: all passed.
- Through the test recorder with pinned nextest and tmux, four slots and no retries, the whole protocol, helm,
  supervisor and UI libraries, the `farhelm` binary's tests, the agent command-line tests and the end-to-end tests for
  launches, tabs, checkouts, agent relay and session lifecycle: 3489 passed, run `cdabc40d-ecaa-418e-938f-01f2cbe4da64`.
  The run's output has no runtime `SKIPPED` marker. The UI's desktop-only Rust tests: 706 passed, run
  `4926be34-2adf-40e5-a588-663695e00161`. The executor had run only this plan's own tests.
- Skipped: browser tests for this plan (it changes no web UI), and the native Mac app's startup alert, as in the report.

The report's statements that main had not moved since the PR's base and that the PR was still an open draft were true
when it was delivered; main then moved as described above, and the PR has now merged. Nothing else in the report became
untrue at landing.
