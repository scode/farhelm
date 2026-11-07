## Since the last review

Both existing drafts remain in the stack. #3132 corrects the session-row rendering regression, restores the two
bottom-row menu tests' geometric premise, and removes the requested protocol-comment rationale. #3133 is unchanged apart
from its rebase. No new PR was added.

The unchanged rendering tests reproduced both failures before the correction and passed afterwards. The new Stop
confirmation callbacks had unstable identities: omitted no-op defaults were recreated on parent refresh, and the
production confirm/cancel closures lacked the stable handles used by neighboring actions. Omitted callbacks now remain
absent, and production callbacks retain their identity while receiving current state.

The menu legitimately grew with Stop's tab-closing description. At the old 500-pixel fixture height, the row-aimed
pointer was already inside the menu, so the fixture could no longer distinguish the clamp it checks. Only those two
fixture heights changed to 650 pixels. Their scrolling, pointer, containment, overflow and last-action assertions and
their timeouts are unchanged; both engines independently established the required geometry and passed.

The reported Stop process-cleanup failure did not recur in three fresh runs on both engines, or in the required broader
browser run. Its cause remains unconfirmed, and no speculative cleanup change was made. All 280 browser cases passed.

## What this was about

Stop used to leave ordinary sessions' terminal tabs and their work running, while a containerized Stop ended them. The
maintainer chose consistent behavior and a confirmation when tabs are shown.

Stop now ends the agent and closes its terminal tabs, including their processes, while keeping the session and agent
terminal viewable. The row, the buttons above the terminal, and bulk Stop ask about visible tabs unless session
confirmations are skipped. Restart and an agent exiting on its own still leave tabs running. Tabs opened after Stop
close with the next Stop.

## Things you should know

A tab whose host cannot be read stays open and is named in a notice; healthy siblings still close. The GUI shows that
notice, and agent-requested Stop reports partial cleanup through its existing error response. Its approval card
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
port numbers remain. The documentation covers Stop/restart/resume, working in a session, the session list, containerized
sessions, agent wrappers, host management and uninstall. No preview was started for this unattended documentation work.

## Open questions and possible follow-ups

No required work remains. The two frozen documentation-shot specs still describe the old header/Stop behavior, as the
latest Decision explicitly leaves them to the next screenshot refresh. Process cleanup retains the existing limits for
detached processes on hosts without a usable systemd user manager. The previously observed cleanup timeout remains an
unexplained observation; successful repetitions do not establish that it cannot recur.

## PRs

- [#3132](https://github.com/scode/farhelm/pull/3132/changes): Stop behavior, consent, notices, UI, tests,
  specifications and changelog; draft head `e788059ae517`.
- [#3133](https://github.com/scode/farhelm/pull/3133/changes): CLI help, agent instructions, seven docs pages and the
  addressed TODO removal; draft head `bdb93a6f56f3`, based on #3132.

## Checks run, reused and skipped

The stack was carefully rebased onto main `d0ba1d49939d`, including instant trash deletion, old-terminal warnings and
their resolution, notification docs links, and release announcement data. Every upstream diff was read. Both original PR
units and the correction had empty interdiffs after rebase. New warnings persist while old tabs run, which agrees with
Restart preserving tabs; Stop closes them. No lifecycle-lock, consent or protocol conflict was found.

The final integration tests captured head `9abbc7dac889` plus the correction, fingerprint
`bc746204357116bab6e4c5d531571be1cc5857697bdc2d543bf9624cebe976ed`. Folding the correction into #3132 changed history
only: the complete working-tree diff between that tested source and the published stack is empty.

- Post-rebase `cargo nextest run -p farhelm-ui` passed all 908 tests, with zero skips, in
  `ab78c17d-82dd-493f-a2c9-0b67f84d7554`. This includes the new bell/migration cases alongside the unchanged
  rendering-regression tests.
- The exact real-host warning tests passed in `17fc53c8-f6f5-4e68-89f5-2e94b618df1f`:
  `host_migrations::a_restart_with_an_old_tab_open_keeps_one_old_terminal_warning` and
  `host_processes::an_old_session_after_the_switch_to_hosts_is_reported_as_one_to_move`. They establish that Restart
  preserves an old tab and its one unresolved warning, and that closing the old tab then stopping its agent resolves
  that warning. The complete JUnit report counts two behavioral passes and successful fixture setup separately; 543
  selected-out tests are not passes. Pinned tmux 3.7c and nextest 0.9.148 ran with four slots and zero retries; complete
  output has no runtime SKIPPED marker.

The current correction was tested above rebased head `611b6e192e57`. The final correction's recorder fingerprint is
`bc35bf03fa0c9a2be65e54e84adf6cae581f0869edfd74a4d47ec6a6824421e6`. Earlier products and UI execution used
`90f7837a09181ffb92e7b0e3a72bbbdc269094912413ce6286d5ccf67f0aa4b0`; only the equivalent Rust tuple alias and the two
browser fixture heights/comments changed afterwards. Compilation/lint checked the final alias. Browser tests used the
matching CLI/fixtures build `83690e33-fea7-4606-9c6674eeaeac` and Dioxus 0.7.10 web build
`95bf539e-6541-4b59-a739-62fca7ba6382`, both successful and identified by that earlier fingerprint.

- `cargo nextest run -p farhelm-ui` passed all 905 tests in `4432bff8-a5f3-44b3-b1b9-0cf4461e0c48`, with no skipped
  tests. Before the correction, the exact two rendering tests failed in `81f4c414-b9d8-4900-9e61-2bb1a6f41574` (65
  versus one unchanged-row renders, and six versus five total selection renders). Their unchanged assertions passed in
  the exact retry `3b21aa21-7ef4-4f75-b9fa-8417d6155a0d`.
- `npx playwright test` selecting the complete sidebar, session-confirmations and header specs, plus the two row Stop
  confirmation/cancel/later-tab cases per engine, passed all 280 cases in `2a5b7a15-9b65-44ce-a9f6-0038ff64bf9b`: 140
  each on Chromium and WebKit. The original scope-cleanup failure did not recur. Both-engine validated reports are
  complete, with no failures, skips, interruptions, unstarted cases or global errors. This run precedes the final
  rebase: the new old-terminal warnings are inactive for its default new-terminal fixtures, and the Stop controls,
  callbacks, menu placement, consent and tested CSS are unchanged.
- The two menu cases passed on Chromium and WebKit, four cases total, in `fd6f4a49-ce8c-402e-8209-284b021455d9`. The
  prior Chromium run `a51d9866-16ec-4489-903c-72e0ac6d726c` failed both fixture pointer premises. Measured menu bottoms
  were 465.09375 pixels against a 460-pixel aim before, and 579.09375 against a 610-pixel aim afterwards. The refusal
  still exceeded its own scrollport on both engines. Its original assertions all passed.
- The exact skipped-confirmations case passed in three fresh both-engine attempts:
  `448436d2-e24f-4069-8bfb-9a39daec628c`, `15317a8a-6e8a-4337-b681-a77d1e60929c`, and
  `837a7da4-bb67-4aa2-bb45-c175e87306c9`, retained in batch `ddff328e-4743-42ed-b8aa-cdd15180df47`. Each passed one case
  per engine. A preceding single-engine generic run `0aa3fb44-4c97-4391-a46d-035a46bffdd4` also passed.
- `cargo clippy -j4 -p farhelm-ui --all-targets -- -D warnings` passed in `4ec7b584-ebad-457d-98f1-0f01a25337de`. The
  preceding attempt `173bac89-ee85-49c6-a5bb-40a74141b796` failed only the new callback's type-complexity lint; naming
  its identical tuple payload corrected that failure without a lint exemption or behavior change. Formatting,
  whitespace, fragment format and the source sleep checker passed; the latter inspected 420 delays with zero missing
  rationales. After rebase, formatting, whitespace and fragment format passed again; the source sleep checker inspected
  421 delays with zero missing rationales.

All completed runs above retain complete output with stream EOF, no omitted bytes, and no forced cleanup or cleanup
limitation. Validated browser runs used three workers, zero retries and three owned stacks; their complete reports show
no skips, interrupted or unstarted cases, or global errors. All owned stacks finished cleanup. Rust execution used
pinned nextest 0.9.148 with four slots and zero retries; the UI-only selections did not need tmux.

Original Stop acceptance coverage is reused per command where this round leaves the tested path unchanged:

- `b814c010-b24b-4670-9c41-8b6a9fba9656` passed ten behavioral cases plus fixture setup at clean head `18f698d46b24`,
  fingerprint `e25a73f4b331c08b3316bc392c0c96cc53216c2a88cc53f5c64eeaa078cc55d3`. The focused
  `cargo nextest run -p farhelm -p farhelm-helm --lib --bin farhelm --test e2e` selection covers tab processes,
  unreadable-tab partial cleanup, Restart preserving tabs, both consent guards, HTTP forwarding, credential injection
  across launches, and real Podman tabs. Its output has no runtime SKIPPED marker. This round changes no backend
  behavior; new upstream warning/lifecycle intersections receive the integration checks above.
- Four native-feature UI cases passed in `b154b933-ce94-4ec3-a4ae-3168859e37b6`, including mounted Stop/Delete/Replace
  callbacks retaining rendered confirmation tokens. Its fingerprint is
  `f3c33921ee23934df2de2102500d27f06d77da34c252c0c5c416341a7ef40ed9`. The optional row payload and stable list handles
  preserve that separate token contract; this round's browser row cases exercise their actual dispatch. This is Linux
  native-feature coverage, not a macOS GUI claim.
- `31383bc4-062e-4d74-8ee0-0f02f505deaf` passed all 48 selected browser cases, 24 per engine: the 23 new Stop cases plus
  narrow native-tunnel header geometry. Its fingerprint is `f08de9a76f89ce6a`; this round's wider required specs cover
  the changed row/menu behavior. `a1680181-af39-4fb3-a0a5-1fdd09e71b37` passed 12 credential-integration cases at
  fingerprint `e25a73f4b331c08b`, six per engine. Those unchanged guard/API/bulk/launch paths remain covered.
- The original 97-case Rust selection `caadbd16-5ebe-4654-be06-79d2f7665855` remains a failed aggregate: 96 passed, and
  one nominal live-agent fixture incorrectly defaulted to Exited. That premise was corrected, and the exact test passed
  in `a15d4112-1bcc-4624-8b20-47046230a28a`. Their fingerprints are `e85c6eb4b70c87e4` and `f3c33921ee23934d`. These
  passes cover unchanged Stop backend/protocol/consent behavior.
- Original targeted all-target desktop-feature Clippy `1ee9fe0c-07bb-4e3e-8ce1-b225fb39f574` and shipped-bin Clippy
  `f734d510-552f-43b6-b9e1-ff6be2676c2f` passed. Their fingerprints are `87e719572775aca4` and `d870f927978c380a`. New
  callback code has the current all-target UI lint above. Native compilation `a0c5b071-6752-421d-b6af-d943c11ecb58`
  passed at the original integrated source.
- `bun run build` passed in `8ac815a8-0c42-40de-9d88-6598f6548a7e`, with all internal links valid, at head
  `18f698d46b24` and fingerprint `e25a73f4b331c08b`. The Stop docs unit is byte-unchanged after rebase. Main's new
  warning paragraph on the session-list page and its independent new-terminals guide do not conflict with the Stop text,
  links or anchors, so another website build was not needed for this unchanged unit.

Earlier private development failures remain retained: two failed compile attempts, one invocation of a different tool
named dx, browser fixture readiness errors, and narrow-header overflow were corrected before original delivery. One
WebKit header case timed out in cleanup after its assertions passed; its cause was unproven. Its unchanged-budget exact
retry and the final 48-case run passed. No failed aggregate is represented as successful, and same-session failures were
not recorded as latent flakes. This round separately preserves the rendering reproduction, menu fixture failures and
initial lint failure above.

The full workspace/browser batteries, installer/provisioning, release pipeline and live installation checks were
skipped: targeted lifecycle, consent, rendering and launch evidence covers this change, and those operations did not
change. No CI run was dispatched; CI is on demand and its browser job is disabled. No instruction, heading or path move
requires the agent-instructions checker. The documentation-shot specs are deliberately outside this correction.

The portable `summarize-test-runs.py --scan-output` inventory covers this round's 16 command records and three
explicitly reused original records: 16 successful child commands and three failed child commands. Discovery completed
with no invalid inputs or missing reports. One console-stream gap belongs to the pre-fix rendering reproduction: 264,622
bytes were not forwarded, but its complete 4,292,752-byte raw output and failing JUnit report remain retained. No raw
output was omitted. Generic single-engine attempts supply command outcomes without validated per-engine counts. This is
a selected evidence inventory, not a count of distinct tests or all historical attempts.

## Review gate

The required single fresh-context gpt-6.1-sol reviewer at high effort re-reviewed #3132 and its final fixture/alias
addenda. It found no actionable issue in the correction. #3133's original independent gate remains applicable because
its own patch is unchanged. The original behavior gate's two consent findings were both fixed before first delivery; the
documentation gate's grammar finding was corrected then too. No unresolved review finding remains.

Fresh same-model scope and resume checks passed. The correction uses existing optional callbacks and stable handles,
without new state or cleanup machinery. Existing commit/PR messages and their successful cold reads were preserved. A
separate documentation pass covered every corrected file and its callback/test contracts.
