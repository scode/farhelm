### What this was about

A template saved from a cloned remote session can contain a model or folder with invisible or direction-changing
characters. The template editor showed the stored text directly, so the visible spelling could conceal what a later
launch would use. The maintainer chose the editor's existing read-only approach for commands, rather than a new editing
mode.

Affected model and destination fields now show visible character escapes and stay read-only. A hint explains how to
remove and add the field again to replace its value. Saving another field keeps the original text byte for byte. Model
suggestions show escaped labels and tooltips while selecting the original model id. The same escaping now applies to the
existing read-only launch and resume commands.

### Things you should know

Ordinary field values remain editable. No stored template is migrated or sanitized, and this introduces no new save
refusal. Older commands rejected by the existing save rules still cannot be saved unchanged; they now display their
hidden characters visibly. A model suggestion can explicitly replace a read-only model value.

The PR includes a fixed changelog fragment and closes the recorded finding. It remains a draft for the landing monitor;
no release or website deployment is involved.

### Open questions and possible follow-ups

None. The existing controls supplied the required replacement path, and execution required no scope departure.

### The PRs

- [#2087](https://github.com/scode/farhelm/pull/2087/changes): reveal hidden characters in template fields.

### Checks run, reused and skipped

- Ran `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `python3 releasing/check-changelog.py format`, and dprint on the triage ledger and feedback index; all passed. The
  isolated-interpreter `python -B scripts/check-test-sleeps.py` inspected 301 delays with zero missing reasons.
- Ran `cargo nextest run -p farhelm-ui --lib` through the pinned recorder: all 499 tests passed, none skipped; run
  `3bd3684c-f614-48fa-bb0c-34f9f3a867b1`, four slots and zero retries. The new test covers invisible, direction-changing
  and line-break characters, ordinary Unicode, absent fields and explicit resets.
- Built the application and web bundle, then ran five selected template browser cases in both engines through the
  recorder: ten passed, none skipped or unstarted; run `21259428-6f3c-4c32-befd-160e20926bbb`, one worker and zero
  retries. These cover escaped display, read-only typing, raw preservation after another field's save, removal and
  recreation, raw suggestion selection, legacy command refusal, and ordinary destination and model controls. The owned
  test stack shut down normally. The first web-build attempt selected an unrelated `dx` from PATH; the corrected Dioxus
  0.7.10 build passed.
- Reused the successful checks and source review from change `nuvsruqyxzyk` at `08d1b54e` through final head `48869290`.
  The later edits only add the PR link and rebase. The complete incoming differences were read: specification limits for
  deliberate local setup preserve the remote-host boundary, trash confirmation copy is separate from templates, and the
  remaining filter, triage, queue and report changes affect no executable input this fix relies on. Both rebases kept
  the patch unchanged. Final ledger/index formatting passed.
- Skipped broader workspace, browser, desktop, installer and website checks: the affected shared editor is covered by
  its UI tests and both browser engines, and this changes no desktop integration, installation behavior, API or schema.

### Review gate's outcome

A fresh gpt-6.1-sol reviewer at high effort, through the plan's required external harness, found no issues after
reviewing the full diff, goal, triage decision, surrounding editor/save code and specification, and the full test
authoring checklist. A separate native gpt-6.1-sol reader at medium effort understood the commit and PR wording and
found no unsupported claim or convention violation. A native reader checked this report for standalone clarity and
public-repo hygiene. No unresolved source findings remain.

Private orchestration evidence retains the external review's reported usage. Native reader and orchestrator counters
were unavailable; no cost or model-attributed token total is inferred from them.

### Landing

Landed as #2087, the third of the round's four plans, tested stacked with the other three on top of main (see
claude-trust-decimals for the shared runs).

#### Review

An independent fresh-context review found nothing blocking. The change replaces the earlier command-only read-only check
(#2051) with one shared check covering launch and resume commands, model and destination, with one hint and the same
"remove this field with ✕ and add it again" advice. The inputs show escaped text read-only and every handler respects
that, so the escaped spelling never reaches the draft; an unchanged template saves its raw values as before. Model
suggestions are escaped but still set the raw id. The browser test confirms the stored bytes are unchanged after saving
another field.

#### Fix made while landing

The changelog fragment said values were preserved "when another field is saved", which was already true before, and did
not say which fields or how to change one. It now names the template editor, the model, folder or repository and the
launch and resume commands, shows what the codes look like, says why it matters, and says how to replace such a field.

#### Left as is

- The hint mentions only removing and re-adding the field; clicking a model suggestion also replaces a read-only model.
- Possible follow-up, outside this outcome: a session name copied from a cloned remote session into "save as template"
  is shown raw in the editor.

#### Checks

See claude-trust-decimals: the UI crate (651 passed) and the templates, quick-launch and tooltip-coverage specs on both
engines (128 passed).
