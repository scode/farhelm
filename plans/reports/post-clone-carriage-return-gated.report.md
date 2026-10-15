### What this was about

Settings edits the shell command Farhelm runs after cloning a managed checkout. Commands stored through the command line
or API can contain literal carriage returns. An unrelated edit in Settings could silently turn those characters into
newlines, changing what future checkouts execute. The maintainer authorized a fix only if it added trivial complexity.

### Things you should know

The fix meets that gate. When the stored command contains a carriage return, Settings refuses a nonblank save with an
inline explanation. It says to change the command through the command line or clear it by saving an empty field.
Clearing remains available; after it succeeds, ordinary editing and saving work in the same open Settings dialog.
Commands without carriage returns keep their existing behavior. Opening Settings alone still writes nothing.

The refusal also covers an unchanged nonblank Save while the stored command has a carriage return. This deliberately
avoids a separate editing mode or lossless representation. The API and command line still accept and preserve carriage
returns; there is no new endpoint, persisted state, escaping scheme or migration. The existing visible-refusal and
clear-command specification rules remain satisfied without a specification change.

The finding is removed from the review queue, its decision is retained in the triage ledger with its execution link, and
a fixed changelog fragment describes the limitation and alternatives.

### Open questions and possible follow-ups

None required. This does not add a way to edit carriage returns losslessly in Settings. The command-line route is the
supported alternative when keeping those characters matters.

### The PR

- [#2053](https://github.com/scode/farhelm/pull/2053/changes): refuse unsafe post-clone command saves while allowing
  clearing. It remains a draft; the executor has neither marked it ready nor merged it.

### Checks run, reused and skipped

CLI and explicit Dioxus web release builds passed, as did `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, ledger/index `dprint check`, diff whitespace validation, changelog format lint and the
isolated test-delay checker (299 delays, zero missing reasons). Two sandbox launch attempts failed before compilation
because tool paths and the toolchain mount were missing; corrected launches passed. A formatter invocation selecting
only files outside its configured plugins/exclusions reported no files; the applicable Markdown checks passed.

Recorder run `1459072b-c190-4aa5-8708-bc38e4aeaf22` passed all 28 selected cases on Chromium and WebKit, 14 per engine,
with one worker, zero retries and pinned tmux. The selection covered Settings and notification Settings; the new case
proved CR and CRLF normalization, refusal without a write or stored-value change, clearing and ordinary saving in the
same mounted field. Existing command load/save/dismissal cases passed too. No cases skipped or failed, retained reports
are complete and owned stack cleanup completed. This is the real local helm/supervisor with fixture agents, without live
remote-host or vendor-agent coverage.

The careful rebase incorporated conversation-tracking specification changes, its feedback closures and unrelated plan
reports. Those changes did not alter the command component, API or browser tests; reviewed and built code stayed
identical. A later main update only claimed another plan; it did not require another rebase or runtime run. The final
ledger amendment only records completion and the real PR URL. Broader Rust runtime, browser, desktop, installer and
release suites were skipped: the change adds a local save refusal, and the selected browser cases cover that refusal
plus the existing load/save/clear/dismissal behavior. No new pure helper or backend behavior needs a separate unit
suite.

### Review gate outcome

The required fresh-context gpt-6.1-sol high source review through codex exec reported no findings. It checked
correctness, design, Rust/TypeScript idioms, the explicit trivial-complexity gate and the full test-authoring contract.
The commit and PR wording passed a separate native sol medium cold read. A separate documentation pass covered every
touched file, including the normalization reason, clear/refetch boundary and test intent. The report receives its own
fresh native cold read before delivery.

Implementation and investigation stayed in the main session. Private galaxy-brain record
`ca9caa6f-9f42-45cb-8728-45ea4905b4cd` retains the source-review decision, launch, outcome and raw terminal counters;
some local/wording accounting events were not recorded at launch. Exact served-model attribution and native/main usage
are unavailable. These gaps do not establish additional review or runtime coverage.

### Landing

Landed as #2053, after confirmations-save-error (#2054) in the same round; the two change different Settings fields and
were tested together on top of main.

#### Review

An independent fresh-context review found nothing blocking and confirmed the change stays within the maintainer's
trivial-complexity gate. The refusal is judged from the saved command, not the draft, so the field's own newline
conversion cannot slip past it; clearing is always possible, and after a clear the field edits normally in the same open
dialog. No other surface edits this command: the launcher and template editor do not, per-host overrides are
command-line only, and the command line and API store the text unchanged.

#### Fix made while landing

The refusal said to change the command "through the command line" without saying how. It now names
`farhelm helm checkout-config set-post-clone` on the helm's machine, matches the field's other lowercase error, and says
plainly that the field would turn the carriage return into a newline; the browser test checks the new text. The
changelog fragment names the same command, and a loop variable in the test's teardown no longer shadows the `request`
fixture used right after it.

#### Left as is

- If the command line removes the carriage return while Settings is open, the field keeps refusing until Settings is
  reopened. This fails safe.
- SPEC.md does not mention the refusal; it is consistent with the existing rule that the field shows a refused value's
  reason.
- Suspected, not seen: after the clear, the test waits for the refetch at the network boundary rather than for the field
  to apply it, a narrow window under several browser round trips.

#### Checks

- Run now, together with #2054 on top of main, with all fixes in: the format, lint, changelog and sleep checks, both
  clippy runs, the UI crate in full (run `441aab8b-8554-4cf8-a41b-8c47b249ecb7`, 634 passed), the JS harness, and the
  settings, session-confirmations, yolo-guard, ntfy-settings, notifications and tooltip-coverage specs on Chromium and
  WebKit (run `9be5c0fa-2e23-4f0b-b256-5331d2ce290e`, 70 passed).
- Not run: anything wider; only the post-clone command field changed.
