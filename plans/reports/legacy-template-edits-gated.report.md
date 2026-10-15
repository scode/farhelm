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

### Landing

Landed as #2051 and #2052, bottom-up, the round's only plan. While it landed, main gained only plan bookkeeping,
including the delivered reports of two other plans; nothing touched the template editor.

#### Review

An independent fresh-context review found nothing blocking and confirmed the change stays within the maintainer's
trivial-complexity gate. The read-only guard uses the same check as the save refusal, so it applies exactly when saving
would be refused and never to a valid command; read-only also blocks typing, paste and drop. Whether Undo is offered is
decided from the saved template, which is what Undo would restore, so an unsaved draft that repairs the value cannot
make it restorable; a permanent delete clears any earlier Undo notice, and valid deletions keep Undo.

#### Fixes made while landing

- The read-only hint told the user to delete and recreate the template to change such a command. Deleting it also
  removes the template's quick-launch mark and shortcuts, while removing the field with ✕ and adding it again already
  replaces the command without either loss. The hint and the changelog fragment now say that, and the browser test
  checks the new hint.
- The permanent-delete tooltip now says what clicking does ("delete this template now; it fails current save rules, so
  undo is not available").

#### Left as is

- Follow-up, outside this item's scope: an older template's session name with a line break has the same problem. Editing
  the name silently drops the line break and the result passes the save check. The triage item covered only launch and
  resume commands; the fix would be one more case in the same read-only check, with a hint that does not say "command".
- The read-only check also triggers for a tab or zero-width character, which the field could keep; it matches the save
  refusal, so the boundary is right.
- Nits in the new test: the check that no restoration was written can never fail because nothing clicks Undo (the real
  check, that no Undo is offered, is there too), and the two fragments are formatted differently.

#### Checks

- Run now, on the rebased stack with the fixes in:
  - `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker;
  - `cargo clippy --all-targets -- -D warnings` and the UI crate's clippy with the desktop feature;
  - the UI crate in full with the desktop feature through the recorder, run `839165f8-541a-47b3-9147-a07b99bdfd24`: 634
    passed;
  - the JS harness: 248 passed;
  - the templates, quick-launch and tooltip-coverage specs on Chromium and WebKit, run
    `99c57b83-e088-47a1-b8a9-5b191892bed0`: 120 passed, 6 failed. The six were the permanent-delete cases, whose
    assertion still expected the tooltip wording from before this landing's own change. With the assertion updated, the
    templates spec on both engines, run `5fe1cf63-f127-436d-8ac2-4de373d8c5d2`: 68 passed. The quick-launch and
    tooltip-coverage results from the first run still apply; only that assertion changed since.
- Reused: the executor's template browser runs, superseded by the above.
- Not run: the Rust workspace beyond the UI crate and other browser specs; only the template editor changed.
