### What this was about

Templates saved under older rules can contain command line breaks or session-name characters that today's save rules
refuse. Editing such a command in a single-line field could silently strip a line break and save a different command.
Deleting one offered Undo even though restoring it would fail the save check.

The maintainer authorized fixes only if they added trivial complexity. Both fit that gate using existing checks.

### Things you should know

An affected launch or resume command is now read-only, with advice to delete and recreate the template to change it.
Saving the unchanged invalid template still refuses. Before deleting a stored template that fails current save rules,
the button says "delete permanently" and explains that Undo is unavailable. Successful deletion offers no Undo and
clears any previous Undo notice. Ordinary valid-template deletion and Undo keep their existing behavior.

The deletion policy uses the saved template, even when an unsaved draft repairs its invalid value. No new API, persisted
state, restore path, migration or editing mode was added. Both specifications now state the Undo exception. Each outcome
has its own changelog fragment, ledger execution record and feedback removal.

### Open questions and possible follow-ups

None required. This does not establish a general policy for future template-format changes; the maintainer limited the
complexity decision to templates considered older at this triage.

### PRs

- [#2051](https://github.com/scode/farhelm/pull/2051/changes): guard edits to older launch and resume commands.
- [#2052](https://github.com/scode/farhelm/pull/2052/changes): warn before permanently deleting a template current save
  rules cannot restore.

### Checks run, reused and skipped

Run on the combined stack after a careful rebase: CLI and Dioxus web builds,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, changed-file `dprint check`, changelog format
lint and the isolated test-delay checker (299 delays, zero missing reasons). The first CLI build caught two incorrect
validator argument types; both were fixed and the corrected build passed.

Recorder run `56f556b9-2066-4ca8-bc0d-b8636f1db68a` passed all 12 selected browser cases in 40 seconds on Chromium and
WebKit, with one worker, zero retries and pinned tmux. It covers both command guards, permanent deletion for old
session-name/launch-command/resume-command values, and ordinary template create/edit/rename/Delete/Undo. Owned stack
cleanup completed. No runtime test failed.

Earlier first-fix evidence: recorder run `1e2401f2-b827-4d98-9c01-6673f14d9b4d`, six passes across both engines. The new
combined run covers those same cases again. The tests provide old-rules rows through the existing list API interception
because today's save API refuses them; they establish editor and deletion behavior, not a real persisted legacy database
fixture.

Incoming changes covered terminal file opening, desktop-only dependencies, unrelated specification policies and queue
bookkeeping. A later rebase incorporated conversation-tracking specification changes and neighboring feedback removals;
the tested component and browser tests stayed byte-identical, so runtime results were reused. No template validator or
editor contract changed. Feedback-index conflicts preserved both sides' removals. Broader Rust runtime and browser
suites were skipped: the changed behavior is confined to this dialog, with the new cases and ordinary Delete/Undo
neighbor covering the concrete integration risk. No new pure helper needs unit tests.

### Review gate

Each PR received the required fresh-context gpt-6.1-sol high source review through codex exec, with the full plan,
outcomes and test-authoring contract. The first had no findings. The second found the two validator argument-type errors
also caught by the compiler; both were corrected exactly as recommended, with no additional findings. Both reviews
confirmed the trivial-complexity gate. Commit and PR wording passed separate sol medium cold reads. Documentation passes
covered every touched file. The report also receives its required native cold read.

Implementation stayed in the main session; only the prescribed reviews and cold reads were delegated. Private review
accounting record `e81ebbb6-031d-4d50-93f6-ff1f857d5edb` retains source-review decisions and the external reviewers'
reported token counters. Some accounting events were recorded afterwards. The requested review models are known, but the
external results do not independently identify the model served; token usage for native cold readers and the main
session is unavailable. These accounting gaps do not change the validation results above. Both PRs remain drafts; the
executor has marked neither ready nor merged either.
