## What this was about

Codex can ask whether to trust a folder with “Trust and continue” and “Quit”. Farhelm recognized only the version whose
second answer returns to Agent Command Center, so the reported Quit question did not trigger the reminder that Settings
can answer trust questions for managed checkouts.

The recognizer now accepts both choice/footer pairs. Quit requires its `esc quit` footer and Back requires `esc back`;
the heading, question, first answer, open menu and final footer still have to be present. The existing Back regression
remains, and the Quit regression covers either selected answer, a wrapped question, subsequent transcript output,
mismatched pairs and rejection by other readers.

## Things you should know

The maintainer authorized a real capture attempt and a derived fixture fallback. Installed Codex 0.164.0, started as
`codex -s read-only -a on-request` in a fresh empty scratch directory, settled directly at its ordinary composer. No
trust answer or model prompt was submitted. This does not establish why the question was absent or which launch
conditions produce Quit. The committed `waiting-derived-trust-quit` screen is explicitly derived from the existing
captured 0.159.0 trust screen, changing only the second answer and footer, with its title preserved. It is not a new
0.164.0 capture.

A separate static audit checked when the reminder appears. The session must belong to a managed checkout, show a
recognized trust question, and have its agent terminal selected rather than a shell tab. Farhelm must have successfully
read that agent's trust setting as off; a failed read hides the reminder. Don't show again hides that agent's reminders
across clients sharing the helm, independently of other agent types. × hides only this session's current question in the
current client, including after navigating away and back; it resets when the question leaves the screen. Command
sessions remain excluded, including commands declared as running Codex. The implementation and existing workspace-trust
browser assertions match these conditions (`workspace_trust.rs` and `session_view.rs`). No UI bug was found, so no
separate UI PR was needed. These conditions are unchanged.

## Open questions and possible follow-ups

A later real capture that reaches Quit could replace the derived evidence. No additional folder-dialog shapes are
recognized.

Two additional browser scenarios would be optional coverage. First, a command session can tell Farhelm which agent it
runs; the current exclusion test makes the session use Codex's status interpretation but does not explicitly declare
Codex on the command itself. Reading the code confirms that both kinds of command are excluded. Second, the existing ×
test covers refresh and navigation, but does not make a Settings read fail after ×. Reading the code confirms that a
failed read cannot undo that local dismissal or make the same question's reminder reappear: the dismissal survives
changes of session view and resets only after an accepted session update observes the question ending. These are
coverage limits, not observed product failures; neither is required to finish this recognizer fix.

## The PRs

[PR #2718](https://github.com/scode/farhelm/pull/2718/changes) — recognize the Quit trust dialog, with derived evidence,
regressions and implementation notes.

## Checks run, reused and skipped

`python3 scripts/record-test-run.py --runner nextest --kind development --selection 'Codex trust reader, all screen fixtures and trust sample lifecycle' --concurrency '4 nextest slots; retries 0' --tmux required -- cargo nextest run -p farhelm-supervisor --lib -E 'test(agent_kind::screen_reader::) | test(agent_kind::screen_fixtures::) | test(trust_evidence_reaches_replies_and_resets_with_the_run)'`
passed: 21 selected tests, zero failures/errors/flaky outcomes, 1,292 selected-out tests. Recorder run
`64140ea7-3316-4bf6-b47d-fbf87f47448a` has a complete report, complete untruncated output and clean cleanup; the
sample-lifecycle test actually ran and no runtime `SKIPPED` marker appeared. The run used pinned nextest 0.9.148 and
repository-built pinned tmux. Source identity is base `c3d0013d` plus the working diff, complete fingerprint
`25a70bff96e769f52c40b271eb6de8bf1d0102abcf215a23898e9b148e0982e8`.

Non-runtime checks on the source diff: `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` passed,
covering the changed integration and its tests. `cargo fmt --all -- --check` and `dprint check SPEC_impl.md TODO.md`
passed for Rust and Markdown formatting. The isolated `python -B scripts/check-test-sleeps.py` passed with 323 delays
inspected and zero missing rationales. `python3 releasing/check-changelog.py format` passed. `git diff --check` passed
with only the derived terminal grid excluded: the unrestricted invocation flags its intentional blank screen rows at
EOF, inherited from the captured fixture. Fixture tests check its loading, state and public-data hygiene separately.

All checks above were run during this execution and reused after rebasing onto `a48bf50d`: the intervening changes only
claimed other plans, revised unrelated TODO entries and added the hidden-trash plan. The recognizer, tests, fixtures and
specification patch stayed byte-for-byte identical; only TODO blob hashes changed. No runtime rerun was warranted.
Browser execution is skipped because the UI is unchanged and the plan explicitly calls for static verification of those
gates. Full Rust, desktop, installer and release suites are skipped because the change is confined to one pure screen
recognizer, its fixtures and documentation; the focused screen and sample-lifecycle selection covers the affected
behavior. No live installation was used.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no findings. It reviewed correctness against
the full acceptance criteria, the paired/current-footer design, idiomatic Rust, derived-fixture provenance and changed
tests using the full test-authoring checklist. No findings were rejected, and no scope departure required reassessment.
A separate cold reader understood the commit/PR wording and found no contradicted claims or convention violations.
