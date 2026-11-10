# Nonblocking session actions

## Since the last review

Both existing PRs changed. The session-action PR now settles the acted-on session before any returning dialog waits,
guards asynchronous focus, preserves independent YOLO questions, and protects menus and newer marking interactions. The
launch PR keeps successful host wake and checkout preparation hidden, ties every result to its submitted launcher, and
opens a finished New or Clone only while the user is still awaiting it. Neither PR has landed in this round. No PR was
added or dropped. The second PR also records the full run's two unrelated test-timing failures for follow-up.

## What this was about

Stopping, restarting, replacing, deleting or launching a session used to lock the whole page. The plan frees navigation
and other sessions while retaining a lock on the session being acted on. The previous landings found that finished work
still behaved as though it owned the page: it moved focus, closed newer surfaces, or kept a source session locked while
its launcher waited behind another dialog. The revised plan required closing that completion class across all these
actions, including their indirect effects.

## Things you should know

Pending launches hide their launcher and release the page while keeping their draft. Failures, unknown outcomes and
questions return that same launcher once another dialog has closed. The acted-on session settles immediately; only the
dialog waits. A New or Clone that finishes after another key or pointer press leaves selection, focus and panels alone,
skips quick-launch naming, and briefly reports **Started**, with the new session's name. A successful Replace follows
its source only while that source is still selected; Restart never changes selection.

A launch still excludes host mutations and a second launch. Another session's ordinary actions remain available.
Duplicate submission gestures, key repeat and a modifier pressed alone do not count as moving on. Waking a host and
automatic selection after Delete do not reset the user's last-answer baseline.

Two conservative choices preserve newer intent. Starting a real create from an initially empty pane retires its old
startup choice; a refused or unattended result does not later select a row created elsewhere. Manual selection remains
available, and deleting an actually selected session still chooses its successor. An older bulk batch leaves marks alone
after a newer marking gesture or marked-actions menu/question opening, even if the newer surface was cancelled; the user
can clear those marks explicitly.

### What remains available during a pending launch

Every entry below was traced through its handler, rather than inferred from button styling. A submitted launch retains
its captured destination independently of navigation and filtering.

| Place                                                              | Behavior and effect on the pending launch                                                                                                                                                  |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Sidebar session opening                                            | Available, including the source of a pending action. Switching does not cancel the request, edit its draft or release its source claim.                                                    |
| Automatic session selection                                        | Remains available under its ordinary rules; admitted Deletes delay their successor choice, while Stop and launches do not. An explicit create retires the empty pane's old startup choice. |
| Host/status filters and sorting                                    | Available. They change the displayed list, not the submitted destination or consent.                                                                                                       |
| Mark, range mark and Clear                                         | Available. They affect only marking; an older completion cannot erase a newer interaction.                                                                                                 |
| Row Stop, Delete and plain Replace                                 | Available on another unclaimed session. The same source refuses competing work. Each result remains attached to its own session.                                                           |
| Row Clone and Replace with                                         | Refused while the launch/host token is held, before any launcher state changes.                                                                                                            |
| Row Rename                                                         | Available on another unclaimed session. Its editor does not retire or replace the submitted launcher.                                                                                      |
| Row pin, move and read/unread controls                             | Available under their ordinary eligibility. They do not alter the launch or its source claim.                                                                                              |
| Top-bar and interrupted-session lifecycle actions                  | Available on another unclaimed session. Their requests outlive the selected view.                                                                                                          |
| New terminal and tab actions                                       | Available when that session's own lifecycle allows them. They do not consult an unrelated session's request.                                                                               |
| Top-bar Clone, Replace with and Restart with                       | Refused while a launch is held, including a stale queued opening. They cannot replace its draft.                                                                                           |
| Quick switcher opening, cancellation and existing-session pick     | Available while the submitted launcher is hidden. Ordinary selection and opener-focus return remain usable.                                                                                |
| Quick switcher New, quick launch and template pick                 | Refused while the existing draft is owed. Admission is rechecked after switcher removal, and refusal returns focus.                                                                        |
| New button and ordinary New shortcut                               | Refused while the submitted launcher remains busy. A brief Started report holds no such token.                                                                                             |
| Bolt, number shortcuts, quick-launch shortcut and launch sequences | Refused while a launch is held; a refused entry is not replayed when the old launch ends.                                                                                                  |
| Templates and its editor                                           | A competing opening is refused. An already-owned editor remains a separate modal surface.                                                                                                  |
| Host rail and Hosts toggle                                         | Available for navigation and inspection. They do not retarget the captured launch.                                                                                                         |
| Add, Remove, Adopt, Suspend, Resume, setup and host settings       | Retain their existing launch-versus-host exclusion. Refusal does not consume the submitted draft.                                                                                          |
| Host Update planning and its result dialog                         | Keep their existing independent admission. A result can delay an owed launcher Return or naming dialog; it cannot retire the launch or hold its source session locked.                     |
| Bulk Stop and Delete                                               | Available for other eligible marked sessions. Busy targets report not started; queued work retains its own claims.                                                                         |
| Bulk metadata                                                      | Available under each row's eligibility. It reuses the ordinary writes and leaves the launcher alone.                                                                                       |
| Global Settings                                                    | Available. Its modal can delay a genuinely owed launcher Return, but not source settlement or a successful wake-to-create continuation.                                                    |
| Trash                                                              | Retains its launch/host-token refusal and cannot consume the pending launch.                                                                                                               |
| Session-facing sandbox Suspend, Resume and Stop-and-suspend        | Retain the requested launch/host exclusion. Ordinary session actions on other sources remain independent.                                                                                  |

### What each completion does

“Return” means a dialog the user is still owed. It waits for another dialog, then uses normal modal opening focus.
Outside that Return, a completion leaves independent focus, panels, menus and dialogs alone. Real row removal or actual
anchor movement can still retire the surface that depended on it.

| Completion                                                           | Settles immediately                                                                                    | Local launcher settlement / Return                                                                                                    | Selection and other surfaces                                                                                                                                                                                             |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Stop, including failed-tab notice and Stop-and-suspend               | Clear Stopping, publish the result, release the source claim and request a fenced refresh.             | No launcher Return.                                                                                                                   | No navigation or independent focus move. A later status read does not keep the finished source locked.                                                                                                                   |
| Plain Restart                                                        | Publish outcome, release progress/claim, refresh and reattach the source terminal after every reply.   | No launcher Return.                                                                                                                   | Never navigates. Terminal reveal still yields to another keyboard owner.                                                                                                                                                 |
| Plain Replace success                                                | Clear progress/retry state, publish outcome, release source and refresh.                               | No launcher Return.                                                                                                                   | Opens the replacement only while its source remains selected.                                                                                                                                                            |
| Plain Replace refusal, unknown outcome or failed permanent-YOLO save | Publish the source result and release the request claim.                                               | A required inline question retains its own source lane.                                                                               | Its focus and scroll yield to another terminal/control; another source's question cannot overwrite it.                                                                                                                   |
| Delete success, including sandbox Delete                             | Publish notice/archive changes, remove actual sources, fence reads, clear Deleting and release claims. | No launcher Return.                                                                                                                   | Only a removed selected source clears. Automatic successor selection waits for all admitted Deletes.                                                                                                                     |
| Delete refusal                                                       | Publish the row error, clear pending/claim and refresh.                                                | No launcher Return.                                                                                                                   | The source and unrelated selection remain.                                                                                                                                                                               |
| Bulk batch completion                                                | Each row settles through its ordinary action path.                                                     | No launcher Return.                                                                                                                   | Remove only its own targets' marks when no newer marking interaction exists; otherwise leave all current marks and surfaces alone. Guarded focus return occurs only if its actual cleanup removes the effective toolbar. |
| New, Clone or quick-create success, including recipe launch          | Record accepted session and request listing/host refresh before any naming wait.                       | Mirror successful choices, clear only the submitted opening's seeds/pill/token and unmount; optional naming only while still awaited. | Matching launcher and unchanged known press count permit ordinary selection. Otherwise keep selection/preferences/panels/focus and show the list-owned brief Started report.                                             |
| New or Clone refusal / unknown / ambiguous recovery                  | Store the retained draft's outcome and preserve its retry identity.                                    | Retain draft/pill/token until the same launcher can Return.                                                                           | No independent selection or focus move while another modal owns input.                                                                                                                                                   |
| Replace with success                                                 | Clear source progress/error, release its claim and refresh in the reply's turn.                        | Clear only the submitted launcher's seeds/pill/token and unmount.                                                                     | Open replacement only if source remains selected, without an unrelated counter await.                                                                                                                                    |
| Replace with failure / unknown                                       | Publish source result and release its claim before Return waits.                                       | Keep local draft/pill/token until that same launcher returns.                                                                         | Other sessions, current focus and open surfaces remain usable.                                                                                                                                                           |
| Restart with success                                                 | Publish durable source outcome, release claim/progress, reattach and refresh.                          | Clear only its submitted launcher and seeds.                                                                                          | Never navigates or focuses New.                                                                                                                                                                                          |
| Restart with failure / unknown                                       | Publish source error/outcome, reattach and release claim immediately.                                  | Only the returning launcher waits behind another dialog.                                                                              | Current selection and independent keyboard owner remain unchanged.                                                                                                                                                       |
| Launcher YOLO or recovery question                                   | Finish per-request progress and required Restart reattachment; a question is not a failure.            | Return the same modal question, then release its operation token so its answer can submit.                                            | The user's answer supplies the next input baseline; no success report or premature draft retirement.                                                                                                                     |
| Successful host wake before create or checkout preview               | Validate fresh installation/connection and continue the accepted submission.                           | Stay hidden and released, retaining continuous draft/token/source ownership through preparation.                                      | Another dialog cannot delay the create merely by owning foreground input.                                                                                                                                                |
| Wake or checkout-preparation refusal                                 | Publish a real source preparation failure and release its claim before Return.                         | Preserve local draft/pill/token until its launcher returns.                                                                           | A local check that starts no new work preserves the source's earlier outcome.                                                                                                                                            |
| Reply for a retired launcher opening                                 | Settle its source and request regardless of modal admission.                                           | Retire only that exact mounted opening; a newer opening keeps its seeds.                                                              | Never reveal or focus an obsolete launcher.                                                                                                                                                                              |
| Sandbox session action reply                                         | Publish row outcome, release source/host exclusions and refresh.                                       | No completion dialog.                                                                                                                 | Stop-and-suspend's asynchronous question uses guarded focus; unrelated plain Suspend behavior remains outside this revision.                                                                                             |

The indirect effects follow the same ownership rule. Hidden launcher retirement no longer closes menus. A count change
asks an already-open menu to compare its actual anchor, so a harmless increment preserves it and a delayed resize cannot
close a newer correctly measured opening. Error-row changes, actual reordering/removal and failed-list recovery retain
their real layout invalidation. Listing refresh and source terminal reattachment never wait for launcher Return.

## Open questions and possible follow-ups

No maintainer decision is required for this delivery. Other asynchronous UI paths remain covered by the existing
whole-UI completion TODO; this plan does not extend its rules to unrelated operations. WebKit browser coverage exercises
the desktop engine family, not execution of the native macOS application.

Two test issues remain open in the existing Deflake entries: WebKit's native tunnel-layout case can exceed its 60-second
budget, and the host reconnect case can mistake an older host-list response for its simulated reconnect. Neither was
fixed as part of this session-action plan; both are accounted for below and recorded in FLAKES.md.

## PRs

- [#3276: session actions keep the rest of the page usable](https://github.com/scode/farhelm/pull/3276/changes).
- [#3278: submitted launches release the page](https://github.com/scode/farhelm/pull/3278/changes).

## Checks run, reused and skipped

The mandatory whole browser command, **`npx playwright test` with no selectors**, ran through the recorder on both
engines, with three workers, one stack each and zero retries (run `896bd5a1-3017-4c84-b942-687f9676e056`, tested head
`05bd5c12`). **It failed: 3,258 passed, 14 skipped and two failed**, over 3,274 cases. Chromium had 1,635 passes and two
skips; WebKit had 1,623 passes, 12 skips, one assertion failure and one timeout. The 14 skips are the declared
real-agent, WebKit clipboard and single-engine visual-matrix exclusions. No case was retried, interrupted or left
unstarted, and no global error or flaky result was reported.

**Evidence limit:** the raw JSON is 34,867,020 bytes, exceeding the recorder's 16 MiB inspection cap. Its report is
therefore marked incomplete. The counts above come from an independent parse of the complete raw report, reconciled with
the console; they are not recorder-validated counts. The source capture was complete, output reached EOF without
truncation, policy reported completion, and all three stacks cleaned up. The whole failed UUID, raw report and traces
were retained before the narrow retries. The full command is not reported as passing.

Every failure is accounted for:

| Failure                                                                          | Evidence and disposition                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| WebKit: native tunnel preview keeps ports and actions inside a 650px pane        | The case exhausted its existing 60-second budget during its final geometry loop; cleanup then reported a closed request context. No geometry assertion failed, but the remaining assertions did not finish. This case's tight WebKit budget is already recorded in FLAKES.md. One exact unchanged WebKit retry passed in 32.7 seconds, run `1a116c2c-a008-4774-b590-e9d3bd5fcc6f`. The timing cause remains a hypothesis; its existing TODO/exclusion stays open.                                                                    |
| WebKit: incarnation churn keeps a followed run while a target change detaches it | The host-list response wait accepted a request that had captured the old phase before the simulated reconnect. Its trace's next response contains the intended marker and new connection incarnation. This is a fixture synchronization race, distinct from the same test's older missing-step-name failure. Three exact unchanged WebKit repetitions passed in 17.4 seconds, run `d600f92e-e638-4260-83d0-99d8e35f80e6`. The new fingerprint is recorded in FLAKES.md and the existing TODO entry; no product behavior was changed. |

Both exact retries used generic recording, the same complete source fingerprint and matching builds, zero retries, and
the suite's three-worker/independent-stack setup. Their counts come from complete console output; generic mode does not
claim controlled browser-report validation. One earlier anchored-grep attempt found no tests (run
`4423842e-09d3-4b19-b052-5d5f15ca325f`); it ran no assertions and is not counted as a pass.

The stack was rebased onto main at `813db4be`. The earlier intervening change disables the SSH agent for Tensorlake
connections; it changes no session-action, launcher or browser-test source. The final standalone first PR compiled with
`cargo check -p farhelm-ui`, and the rebased backend rebuilt with `cargo build -p farhelm`. Both passed. Folding the
reviewed corrections into the two existing PRs preserved the combined tree exactly; comparison before and after rebase
also showed unchanged UI and browser-test files. The final rebase adds only the planning system's notification
instructions; those were read and retained. Subsequent changes only append the failure records and update the existing
Deflake evidence. Those documentation changes need no further runtime run.

The latest targeted browser run used the recorder's controlled Playwright mode on Chromium and WebKit, with three
workers, one stack each and zero retries: **98 passed**, 49 per engine, with no skips, flaky results or global errors
(run `f714277e-0b70-4144-b5d6-51c17b500fb5`). It covers all new launch cases, wake-to-create, managed Replace, checkout
preview modes, newer bulk interactions and heading changes. Its source and reports were complete, with complete output
and stack cleanup. This result is reused across the history-only assembly and the UI-preserving rebase.

The UI Rust run `2930d02c-2746-4e0b-810c-982d897f025c`, using `cargo nextest run --locked -p farhelm-ui` through the
recorder with four slots and zero retries, **failed: 949 passed and three failed**. The three new menu-measurement test
fixtures tried to spawn without a component scope; they were corrected to model the actual list/row ownership. The
narrow retry of the affected menu and marking modules passed **40 tests**, with 913 selected out (run
`a054f699-2d06-4f44-82ad-c3db71ff891d`). The 949 earlier passes remain separate observations; the full command is not
reported as a pass. Both reports and source identities were complete. The narrow Rust retry covers the changed menu and
marking paths, including the corrected fixtures; the 98-case browser run also covers the subsequent ownership
corrections. Those targeted checks supplied useful evidence without repeating the full Rust command.

`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the release web build, `cargo fmt --all -- --check`, the
changed Markdown formatter, the test-delay checker (429 annotated delays, zero missing reasons), and
`python3 releasing/check-changelog.py format` (41 fragments) passed. The final web build and Clippy covered the final
Rust corrections; their evidence is reused because assembly and rebase preserved those files. The desktop-feature
compile check passed before the last shared ownership corrections; it is reused because those changes add no
desktop-only code, feature branch or renderer API, and the final shared Rust was compiled by the web/UI checks. The
unchanged press-counter JavaScript tests passed **10 tests** through the recorder (run
`4d5a6bbe-bee7-4c29-ae76-c0b8a916a4dc`); the later changes did not touch that asset or its tests.

Earlier failed attempts remain retained. Browser run `07ecf8da-0860-447d-8024-483431c198be` had 64 passes and six
failures from a duplicated session-row marker on a new question; the marker and affected queries were corrected, and the
exact six cases passed in `ade2f5e1-4e7e-4b27-aa27-4333c556aab8`. Run `56241c7d-be60-4646-b26d-134649a9ea25` failed all
eight deliberate wake-to-create reproductions before the fix. Run `2fe22e9a-58d6-4331-9e08-cb467df291a7` then had 48
passes and eight failures: six Restart cases had not actually edited their fixture draft, and two exposed menus closing
on a harmless count change. The fixture premise and actual menu behavior were fixed; the latest 98-case run covers them.
None of these failed commands is counted as passing or recorded as a latent flake.

Installer, release, provisioning and unrelated Rust suites were skipped because this plan changes UI behavior. Native
macOS execution was not run; WebKit supplies browser-engine coverage. No hosted CI was dispatched: this repository's CI
is manual, and its browser job is disabled. The instruction-file checker was not repeated because this round changes no
instruction file, heading name or file location.

## Review gate outcome

The required single fresh-context GPT-6.1 Sol reviewer at high effort cleared each PR's final source corrections. The
additional fresh Claude Opus completion review read the full combined change and its corrective deltas; no product
finding remains open. Its final test-cleanup finding was fixed and checked by the same second-PR Sol reviewer. These are
source verdicts; the checks section records runtime evidence separately.
