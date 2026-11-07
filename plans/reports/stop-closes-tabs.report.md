## What this was about

Stop used to leave ordinary sessions' terminal tabs and their work running, while a containerized Stop ended them. The
maintainer chose consistent behavior and a confirmation when tabs are shown.

Stop now ends the agent and closes its terminal tabs, including their processes, while keeping the session and agent
terminal viewable. The row, the buttons above the terminal, and bulk Stop ask about visible tabs unless session
confirmations are skipped. Restart and an agent exiting on its own still leave tabs running. Tabs opened after Stop
close with the next Stop.

## Things you should know

A tab whose host cannot be read stays open and is named in a notice; healthy siblings still close. The GUI shows that
notice, and agent-requested Stop reports partial cleanup through its existing error response. Its approval card now
explicitly authorizes closing tabs.

Consent protects both resources independently: a screen showing no tabs cannot authorize a new tab's closure, and a
screen showing an ended agent cannot authorize stopping an agent restarted elsewhere. This also applies when
confirmations are skipped and separately to every bulk target. A refusal stops nothing and refreshes the display without
opening a question automatically.

Remote supervisors need updating because the protocol version increases from 68 to 69. An older helm otherwise could
close tabs without asking. An old browser's bodyless or empty-object Stop refuses open tabs; reloading obtains the new
confirmation flow. Stop adds no database migration.

The row above the terminal gained a Stop button. With a desktop tunnel pill present, persistent action labels and
spacing are slightly tighter to preserve the supported 650-pixel pane width; icons, labels, tab readability and both
port numbers remain.

The documentation changes cover Stop/restart/resume, working in a session, the session list, containerized sessions,
agent wrappers, host management and uninstall. No preview server was started for this unattended incidental
documentation work.

## Open questions and possible follow-ups

No required work remains. Process cleanup retains the existing limits for detached processes on hosts without a usable
systemd user manager.

## PRs

- [#3132](https://github.com/scode/farhelm/pull/3132/changes): Stop behavior, consent, notices, UI, tests,
  specifications and changelog; draft head `3f4172a13666`.
- [#3133](https://github.com/scode/farhelm/pull/3133/changes): CLI help, agent instructions, seven docs pages and the
  addressed TODO removal; draft head `18f698d46b24`, based on #3132.

## Checks run, reused and skipped

The stack was carefully rebased onto main `384ad994`, including the newly landed credential settings work. The full
upstream diff was read. Token injection changes agent and tab starts but never tab closure; the session confirmation
preference keeps its existing owner in the expanded Settings tree. Connection resources and credential schema changes do
not alter Stop authority. Both PR interdiffs were empty after rebase. Integrated checks target those shared launch,
Settings and compilation paths; unchanged Stop coverage is reused per command below.

Current integrated source is clean head `18f698d46b24`, recorder fingerprint
`e25a73f4b331c08b3316bc392c0c96cc53216c2a88cc53f5c64eeaa078cc55d3`.

- Integrated runtime: the focused `cargo nextest run -p farhelm -p farhelm-helm --lib --bin farhelm --test e2e`
  selection passed all ten behavioral cases in `b814c010-b24b-4670-9c41-8b6a9fba9656`: managed-token replacement and
  fallback across agent/tab starts and Restart; host-tab closure and descendants; unreadable-tab partial cleanup;
  Restart preserving tabs; both Stop consent guards; HTTP forwarding and response shape; and the real Podman tab case.
  The complete report also counts successful fixture setup separately. Output is complete, untruncated and has no
  runtime SKIPPED marker. Required pinned tmux and nextest ran with four slots and no retries. Native
  `cargo check -p farhelm-ui --features desktop` passed in `a0c5b071-6752-421d-b6af-d943c11ecb58`, covering the expanded
  Settings tree with Stop components.
- Integrated web build: `dx build --package farhelm-ui --platform web --release`, Dioxus 0.7.10, passed in
  `33ec4203-fdbc-4eee-b5b5-f2d20c427dd5`. This supplies the matching browser bundle.
- Integrated docs: `bun run build` passed, including all internal links, in `8ac815a8-0c42-40de-9d88-6598f6548a7e`. It
  covers the combined Stop and credential documentation.
- Browser integration: `npx playwright test` selecting Settings focus, header confirmation, row/header skipped
  confirmations with partial notices, per-target bulk consent and the native narrow header passed all 12 cases in
  `a1680181-af39-4fb3-a0a5-1fdd09e71b37`, six each on Chromium and WebKit. Complete validated reports and untruncated
  output show no failures, skips, interruptions, unstarted cases or global errors; all three stacks cleaned up. Matching
  CLI and web builds came from the integrated source.
- Reused Rust selection: `cargo nextest run` over the changed proto, supervisor, helm, UI and CLI targets and focused
  terminal-tabs, host-tabs, marker-model, Restart and container cases selected 97 behavioral tests in
  `caadbd16-5ebe-4654-be06-79d2f7665855`: 96 passed, one failed because its nominal live-agent fixture defaulted to
  Exited. That premise was corrected explicitly; the exact consent-model test passed in
  `a15d4112-1bcc-4624-8b20-47046230a28a`. The aggregate remains failed. These passes cover the unchanged Stop
  implementation; new upstream launch intersections receive the integrated run above. Linux, pinned nextest 0.9.148 and
  tmux 3.7c, four slots, zero retries; real Podman coverage ran, with no runtime SKIPPED marker in complete output.
- Reused desktop execution: four focused UI tests passed in `b154b933-ce94-4ec3-a4ae-3168859e37b6`, including mounted
  Stop/Delete/Replace callbacks retaining their rendered confirmation token. The new Settings panel does not change that
  token contract. This is Linux native-feature coverage, not a macOS GUI claim.
- Reused browser coverage: final `31383bc4-062e-4d74-8ee0-0f02f505deaf` passed all 48 selected cases, 24 each on
  Chromium and WebKit: all 23 new Stop cases and the 650-pixel native-tunnel header per engine. Earlier
  `e9b37c5b-7065-474b-a27b-b897f80bde9b` passed 101 of 104 cases; its unchanged Delete, bulk and ordinary-header cases
  remain applicable. `e15a6483-f6f6-4b38-83fb-0aaaac0f84bd` passed 56 of 58; its unchanged existing terminal
  Stop/error/in-flight cases remain applicable. These used the pre-credentials product build and cannot establish the
  integrated Settings result. Both-engine runs used three browser workers, zero retries and three owned stacks; all
  selected cases executed, with no skips or global errors.
- Reused lint: targeted `cargo clippy --all-targets` over proto/supervisor/helm/UI/CLI with the desktop UI feature
  passed with `-D warnings` in `1ee9fe0c-07bb-4e3e-8ce1-b225fb39f574`; shipped
  `cargo clippy -p farhelm --bins -- -D warnings` passed in `f734d510-552f-43b6-b9e1-ff6be2676c2f`. The rebase leaves
  the reviewed Stop patch unchanged; integrated native compilation separately covers the new component graph. The
  matching pre-rebase ordinary CLI build passed in `e8dcf561-2125-4f1e-9507-fad453b2177b`.
- Source checks: final integrated sleep checker inspected 420 delays with zero missing rationales;
  `cargo fmt --all -- --check` and `python3 releasing/check-changelog.py format` passed. Changed Markdown formatting and
  diff whitespace checks passed before the empty-interdiff rebase.

Retained development failures remain failures. `bbae6f00-c449-4b50-b8eb-1947f08cb70a` had a stale renamed test variable
and was interrupted after its compile error; `60e9369f-c64a-431f-88a4-80010f5a2d71` found two UI compile errors,
corrected before successful execution. `983339f8-7d50-4191-acde-8158b363ca64` invoked a different tool named dx and
failed before compilation; subsequent builds used the verified Dioxus executable.

The three browser failures in `e9b37c5b` were toolbar overflow on both engines and a fixture reading the merged tab
listing before its exact tab appeared. CSS was tightened only for the tunnel header, and the fixture now waits for the
returned tab ID. Narrow Chromium retry `95554da2-8fcd-47ef-acac-617cca302abe` passed both affected cases. In `e15a6483`,
one WebKit setup stopped the launch shim before the fake agent was ready; preparation now witnesses that agent's actual
output. Exact retry `e45c8859-ea75-43cf-aaa0-65b1465cc629` passed. The other WebKit case exceeded its unchanged
60-second deadline during cleanup after its header assertions passed; its cause remains unproven. Exact retry
`fc09971f-030d-4942-a24e-eea229529957` passed, followed by the 48-case both-engine pass. All four dark/light header
screenshots were inspected for fit and readability. No timeout or behavioral assertion was weakened. All failures remain
private; no latent-flake claim was made.

Reused precommit sources are identified by recorder fingerprints, rather than HEAD alone: `e85c6eb4b70c87e4` for the
97-case Rust run; `f3c33921ee23934d` for the corrected consent test, desktop tests, first browser run and ordinary CLI
build; `87e719572775aca4` for the second browser run and all-target lint; `d870f927978c380a` for shipped-bin lint; and
`f08de9a76f89ce6a` for the final 48-case browser run. These describe the then-uncommitted sources retained by the
recorder. Later changes were the documented CSS and local browser-readiness corrections before the empty-interdiff
rebase; the earlier passes are reused only where those changes leave the tested path intact.

The full workspace, full browser suite, installer/provisioning, release pipeline and live installation checks were
skipped: targeted lifecycle, consent, UI and launch evidence covers this change, and those other operations did not
change. No CI run was dispatched; CI is on demand and has no browser job coverage. No new instruction file, heading
rename or moved path requires the agent-instructions checker. A final fetch advanced main to `e348af78` only through
another plan's report and queue state; the entire diff was read and adds no product interaction requiring another rebase
or runtime check.

The portable summary from `scripts/summarize-test-runs.py --scan-output` covers the 20 explicitly selected retained
command records cited here: 14 successful child commands, five failed child commands and one signaled child command.
Discovery completed with no invalid inputs. Two failed compile attempts have no complete runner report; no test count is
inferred from them. Generic single-engine retries supply command outcomes but no validated per-engine counts. This is a
selected evidence inventory, not a total of distinct tests or every superseded build in the session.

## Review gate

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed each PR separately. The behavior review found
two consent gaps: a tabs-only answer could stop an agent restarted elsewhere, and an old browser's omitted body could
gain tab-closing authority. Both were corrected with independent agent/tab guards and conservative HTTP omission. A
same-model scope review confirmed this was the smallest design satisfying the existing displayed-consent contract.

The documentation review found one grammar defect, which was corrected. Both final review gates passed with no
unresolved findings, including bounded rechecks of later compile, layout and fixture corrections. A fresh same-model
scope check also confirmed that the readiness corrections use existing helpers and preserve the deliberate stale-display
races. Each commit/PR message passed its required cold read, and a separate documentation pass covered touched files and
test contracts.
