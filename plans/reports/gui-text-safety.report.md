### What this was about

A hidden end-of-paste marker could end a terminal paste early, turning the remaining clipboard text into typed input. In
a shell, that could execute commands. Hidden text could also disguise the model or approval wording in a launch-template
summary, or make two installation identities look alike when adopting a host. Single-line template editors also hid or
lost control and invisible formatting characters in launch and resume commands. The maintainer chose to strip embedded
paste terminators, show hidden host text visibly, and refuse unsafe template commands at the shared save boundary.

All seven outcomes are implemented together. Text pastes and drops now pass through the same sanitizer before the
terminal applies its configured paste handling. Template summaries in Templates and the quick switcher escape hidden
model characters. The adopt-host prompt shows identity spaces as `<U+0020>`. Saving either command refuses controls and
invisible formatting with an error naming the affected field, including writes from an agent.

### Things you should know

Pastes strip every ESC character as well as complete end-of-paste markers. Removing only complete markers could join
leftover fragments into another marker; the plan expressly allowed this broader removal. Other text, including ordinary
Unicode and line breaks, remains intact. File and image precedence and the terminal's own paste-mode handling stay in
the existing path.

Previously stored templates still launch. Saving any edit to one refuses until unsafe launch and resume commands are
repaired. The refusal applies to the shared save boundary rather than separately to each client.

The seven feedback entries were removed and their execution records point to the one change and draft PR. No outcome
tripped its complexity gate. The confirmed specification sentences were already on main and required no further
amendment.

### Open questions and possible follow-ups

No maintainer decision is outstanding. Browser coverage verifies the web UI on Chromium and WebKit; it does not exercise
a native desktop window or an operating-system clipboard.

### PRs

[PR #1811](https://github.com/scode/farhelm/pull/1811/changes) — safe GUI pastes, visible template/identity text, and
refused unsafe template commands; draft, based on main.

### Checks run, reused and skipped

- Focused Rust modules:
  `cargo nextest run -p farhelm-proto -p farhelm-ui -p farhelm-helm --lib -E 'test(launcher::tests::) | test(peer::tests::) | test(list::templates::tests::) | test(templates::tests::)'`,
  through the recorder with four slots, zero retries and no tmux: 25/25 passed after the dependency upgrades, run
  `0ed32a72-dfb2-4204-ae8e-aa395f62dfd5`, on base `e5f61bff` plus the recorded implementation. The 1582 excluded tests
  are outside this selection. This evidence is reused after the rebase onto `499d6160`: the selected save/rendering
  boundaries and the reviewed implementation are unchanged; the newly landed code changes other consumers and
  independent supervisor/tooling paths. These cover both command fields, legacy template application, the real HTTP save
  refusal and unchanged catalog, model summaries, and identity rendering.
- UI JavaScript harness: `node --test` in the UI harness, through the recorder: 207/207 passed, run
  `d1d0ed93-44fb-4c88-846d-438c7b31eb30`. This includes complete paste markers, fragments, ordinary Unicode/newlines and
  the shipped sanitizer export. Reused from base `23771635` plus the implementation: the tested asset and harness inputs
  are unchanged.
- Current browser proof:
  `npx playwright test 'terminal-attachments\.spec\.ts' -g 'pasted and dropped text cannot close bracketed paste early|pasted text that looks like a path|file and text pastes are intercepted|a pasted image is uploaded|a completed upload keeps focus|clipboard facts survive|a file reference beside raw image data'`,
  through the recorder with Playwright 1.64, one worker, zero retries and required pinned tmux: 14/14 passed, seven each
  on Chromium and WebKit, run `3fdc264d-9199-41f6-924d-32d248e1b576`. All failure, skip, interruption and unstarted
  counts are zero. This ran on base `499d6160` plus the implementation. The earlier framing 2/2
  (`0eebf568-5f66-4ae3-8b1c-5fc298291d48`) and compatibility 12/12 (`0bef8631-0d3d-445c-81f0-c2c682bd37cb`) passes used
  Playwright 1.62 on base `9046a0db`; the current run supersedes them for final browser coverage.
- Scoped `cargo clippy -p farhelm-ui -p farhelm-proto -p farhelm-helm --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, changed-Markdown dprint, the isolated test-delay checker, and changelog-format
  validation passed. Scoped Clippy passed on base `499d6160` plus the implementation. The CLI and fixture build
  (`cargo build -p farhelm -p farhelm-fixtures`) and release web build
  (`dx build --package farhelm-ui --platform web --release`) passed on that same tree before the current browser run.
  Formatting and the isolated delay checker (282 recognized delays, zero missing rationales) passed on base `e5f61bff`
  plus the implementation; their changed-file coverage remains applicable. Changed Markdown and changelog format passed
  again after the final execution URLs were added (43 fragments).
- Careful rebases reviewed the intervening dependency/Playwright upgrades and the landed host-text, Git-isolation,
  OS-readback, maintainer-tooling and SSH-config fixes. The shared formatter used by this change is unchanged; no
  product-contract conflict was found. The feedback-index conflict was mechanical, keeping both sets of removals. The
  implementation remains identical to the source-reviewed patch. The final rebase onto `89edb1b0` added only
  documentation/capture changes, website CSS and queue bookkeeping. Those inputs do not affect the tested UI assets or
  template save boundaries, so the current checks are reused without another runtime run. The final pushed head is
  `159d7f54783eb125874cb2214c0745d9c562d01d`; its additional changes are the seven execution URLs.
- Full Rust and browser suites were skipped because the focused modules and paste scenarios cover the changed
  boundaries. Installer, provisioning, release and native desktop runtime checks were skipped because their behavior is
  unchanged. Hosted CI was not dispatched: it runs only on demand, and the targeted local evidence covers the identified
  risks.
- Browser-library preparation needed isolated dependency repairs. A later engine installer unexpectedly
  garbage-collected older versions from a shared cache. All eight removed directories were restored and checked for
  complete installations. This plan’s engines were copied into private container storage, and only its own default-cache
  symlink was changed; future installation no longer points at the shared cache. A host-PID resource guard also refused
  the web-build launch when called inside the container; the CLI build had passed, the host watchdog remained healthy,
  and the corrected host-side guard allowed the web build to start. These are preparation outcomes, not product-test
  results.
- Three earlier retained runtime records produced no product assertions: `1726694e-8049-4316-8408-431cf090612a` refused
  before spawning because Git rejected checkout ownership; `5a89edd6-0435-4a7c-b8c1-c131d20448c1` had fourteen
  engine-launch failures because the recorder removed a cache-location environment override;
  `7630eb36-a589-4090-b424-0805b743a571` selected no tests because its grep was overanchored. Exact-checkout trust,
  default cache discovery, and an unanchored unique selector corrected those setup issues; all records remain retained.

### Review gate outcome

The prescribed fresh source reviewer, requested as gpt-6.1-sol at high effort, found no actionable correctness, design
or idiomaticity findings. Its charter included all seven decisions and the full test-authoring checklist. The wording
cold reader, requested as gpt-6.1-sol at medium effort, understood the motivation and compatibility caveats; its
remaining installation-identity terminology question was accepted as established product vocabulary. A fresh inherited
native resume checker reconciled the saved implementation and remaining work. The fresh inherited native report cold
read passed after the opening was clarified to state the shell-command execution risk.

Implementation and investigation were done by the executor under the plan's no-workhorse rule. Only the prescribed
reviews were delegated. Native model/effort and usage attribution were not exposed by the harness; requested model names
are not measurement evidence.

### Landing

Landed on 2026-10-10 (UTC) as #1811, first of three plans landed together (then ui-interaction-fixes and
helm-cli-fixes). Since the plan was based, main gained only plan bookkeeping; the rebase was clean.

#### Review before merging

A separate reviewer read the change by reading the code only. The paste finding is about the bracketed-paste end marker
on the input side: a host could fill the clipboard (through the OSC 52 allowance) with text containing an early "end of
paste" marker followed by a command and a carriage return, so pasting it into a shell would run that command. Paste and
dropped text is now cleaned before xterm frames it, so the marker cannot survive; the output parser, OSC 8 links, OSC 52
and window titles are unchanged. Every template save path now runs the new command check, and the escaping changes
escape each value exactly once.

#### Decided at landing: older templates

The reviewer found that a template saved before this change can still hold a line break or invisible character in a
command; opening it in the one-line editor drops the character from view, and editing and saving then passes the new
check with the command silently changed. You chose to accept this: there are no users, so it affects no one. Nothing was
changed for it.

Smaller notes left as they are: one connected-host detail shows an installation id without making spaces visible, unlike
the other identity labels; paste cleaning depends on the attachments feature being installed, which today it always is;
the escaped spelling of an identity does not escape `<`, so a literal `<U+0020>` and a real space look alike, which
supervisor-minted ids never contain; and SPEC_impl.md does not record that pasted text loses every escape character.

#### Checks

- Run now, on the three stacked in landing order: `cargo fmt --all -- --check`, the changelog lint,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the supervisor, helm, UI
  and protocol unit tests in full through the recorder with pinned tmux 3.7c, four slots and no retries (2709 of 2709),
  the UI JavaScript tests (230 of 230), and on Chromium and WebKit with one worker and no retries the
  terminal-attachments, terminal-clipboard, templates, quick-switcher, approval-layout and sidebar specs (run
  `7d69e2db`, 355 passed, 9 skipped: WebKit cannot be granted clipboard permissions, and the real-Claude cases). The two
  failures were one sidebar test, "composer menu-closed Tab order follows the displayed launch groups", on both engines;
  it fails the same way on plain main (run `4bb205ad`), because the managed-checkout destination buttons added earlier
  today were never added to its expected Tab order. The browser-sidebar-test-oracles plan carries that fix.
- After the landing fixes: `dprint check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and the UI menu
  tests (17 of 17).
- These three landed on their own rather than with the six plans claimed later in the round; see the monitor's
  2026-10-10 change to the landing instructions.

Nothing in the report above was made untrue by the landing.
