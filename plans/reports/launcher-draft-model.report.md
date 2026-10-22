## What this was about

The session launcher kept related choices in separate reactive values, with each input handler responsible for its own
lock check, dependent-field resets and retry-key handling. One custom-flags handler already missed the live lock check:
an input callback delivered after Launch could change the form's permission words and discard its retry identity while
the captured first request continued unchanged. After an unclear reply, pressing Launch again could then send different
words under a new identity. The regression exercises that callback beyond the disabled field; it does not demonstrate a
normal browser input sequence causing duplicate sessions. Repeated fixes in the same area also showed why keeping those
rules in prose and individual handlers was fragile.

The plan moves the accepted launch choices and their raw editor ownership into one plain Rust draft, gives submission
one request builder, and puts edits behind concrete operations that select their lock and retry rules internally. Picker
visibility, errors, asynchronous observations, confirmation and dispatched attempts remain separate.

The starting inventory classified 94 local reactive values: 32 semantic draft fields and 62 presentation, lifecycle or
observation values. The parent-owned host choice is additional and remains shared with the list. The draft now owns both
retained launch kinds, raw editor ownership, one-time inputs, destination selection and checkout-name policy; 27 field
projections keep existing subscribers narrow. The audit also identified seven request-related stages and 14 effects,
including conditional reads reached through synchronous callbacks. The final conversion retains those projections and
existing effect branches; both independent reviews checked the actual callers and value consumers.

## Things you should know

Four isolated fixes are already in the stack. Custom permission flags now honor the live pending-request lock. Reset
discards custom flags owned by the previous agent, so selecting another agent afterward cannot recover those old words.
Reapplying unchanged choices or text preserves an unresolved retry rather than giving an accepted-but-unseen create a
new identity. A template that changes the proposed launch dismisses its old YOLO confirmation; an unchanged template
preserves that confirmation and retry. The confirmation fix also preserves an eligible editor's focus when that question
disappears from a quick-launch opening.

Raw text remains distinct from displayed or parsed text. An unfinished quote stays editable, an untouched Clone keeps
the source bytes, and an equal edit can still claim explicit ownership without changing the submitted launch. Both agent
and command tab drafts remain retained. Per-field reactive projections preserve the launcher's existing effect
dependencies; consolidation does not mean every effect subscribes to every field.

Restoring a Clone's source and resolving its host remain separate phases, so a late host reply cannot overwrite edits.
Checkout numbering continues to respect the accepted preview and a retained uncertain attempt. Explicit host and
repository choices still retire their old authority, while ordinary equal folder choices preserve retry identity.
Templates retain the whole retry binding until submission compares the completed request. Permission-warning Undo
restores the chosen fields and raw ownership, invalidates browse/preview work and ends ordinary retry authority; a
retained managed-checkout request remains available for reconciliation. These distinct rules are named operations, not
caller-selected flags that could bypass the shared editing boundary.

This plan leaves Restart with's draft and request path separate. The launcher still uses the same shared host choice,
request admission, asynchronous key minting and retained managed-checkout attempt.

The dialog-shell plan landed while the last checks were running. The entire stack was carefully rebased onto main
`13075557dae95f3e205913b018c6553411968843`, including its new launcher isolation, Tab boundary and release handoffs. The
overlapping focus helper needed one resolution: retain this plan's guard that keeps a surviving enabled editor focused
when a template dismisses a YOLO question, while using the shared dialog's search/loading-Cancel target. The other ten
published PRs kept identical patches; the last ownership change kept identical product code. Both sides of independent
TODO and review-queue bookkeeping were preserved. The combined code received focused validation below.

## Open questions and possible follow-ups

Of the four review findings singled out by this plan, Reset and stale template questions are fixed; two remain queued. A
template sequence starts with the sequence shortcut, queues templates with number keys, then uses Enter to apply or
launch them. If a direct launch becomes busy before its disabled state reaches the page, a sequence started in that
interval may be refused while leaving its saved template names queued. After closing that opening, later sequences could
consume an earlier sequence's names despite showing newer ones, potentially using the wrong template's host, folder or
command. The missing pairing is supported by source inspection; the required native input/render ordering and a harmful
launch remain unverified. This happens before the launcher accepts a draft edit, so consolidating draft ownership does
not settle it.

Saving a number shortcut can also overwrite an agent's intervening assignment to another shortcut: the GUI reads and
writes the shared binding map as a whole. The interleaving is supported by source inspection; a later harmful launch is
unverified. The implementation specification accepts last-write-wins, while a broader preservation promise leaves the
contract question open. A later review also records that an unsaved failed-write overlay can overwrite an assignment
completed before the next read. This plan leaves that queued decision intact rather than introducing atomic preference
writes.

A separate trailing review reached main during delivery and added further launcher follow-ups. Its strongest finding
concerns Replace with, which creates a new session and removes its source. When a host asks about a YOLO launch, the
question can offer to start it Guarded instead. If the source is idle at that affirmative click but starts running in
another client while a checkout preview waits, the later submission can broaden deletion consent to stopping and
removing that now-running source. The queued record establishes the source mechanism, not a reproduced loss of work. A
second record questions the same alternative after an unclear earlier launch result: Guarded changes the request and can
create a second session while an accepted-but-unseen first session keeps running. Whether "instead" promises one session
remains unresolved. A possible stale answer after a superseding source opening also remains conditional on an
unestablished normal input route. These concern confirmation/attempt lifecycle and source consent, which this draft
refactor preserves; it does not claim to fix or reproduce them. They are preserved for later triage, with no decision
requested by this delivery.

That review also leaves queued the ordinary Guarded answer's focus handoff, retained submit callbacks during repeated
renders, premature "no repositories" feedback while discovery waits, and a missing browser-effect witness in the
edited-form preview-cancellation regression. This plan's positive Guarded and cancellation browser cases passed, but
that does not settle the missing witness or those separate focus, memory and consent scenarios. Their records remain for
triage; no new policy was inferred from the review's conditional findings.

The browser traces also showed that the existing deferred Custom-editor focus handoff can overtake input aimed at
another field. The affected regression fixtures now wait for that handoff before measuring completed-choice behavior;
the product focus policy itself was unchanged. A separate usability investigation could decide whether that policy
should yield to a later deliberate field interaction.

## The PRs

- [#2515](https://github.com/scode/farhelm/pull/2515/changes): reject custom-flags edits while a launch is pending.
- [#2521](https://github.com/scode/farhelm/pull/2521/changes): move agent choices and raw ownership into the draft.
- [#2523](https://github.com/scode/farhelm/pull/2523/changes): clear the previous agent's custom flags on Reset.
- [#2526](https://github.com/scode/farhelm/pull/2526/changes): retain command tabs and one-time inputs.
- [#2530](https://github.com/scode/farhelm/pull/2530/changes): consolidate folder and checkout-name choices.
- [#2532](https://github.com/scode/farhelm/pull/2532/changes): use one submitted-request builder.
- [#2536](https://github.com/scode/farhelm/pull/2536/changes): preserve unresolved retries for unchanged launch choices.
- [#2538](https://github.com/scode/farhelm/pull/2538/changes): dismiss stale YOLO questions without losing editor focus.
- [#2539](https://github.com/scode/farhelm/pull/2539/changes): centralize ordinary editing rules.
- [#2543](https://github.com/scode/farhelm/pull/2543/changes): centralize Clone, host and naming transitions.
- [#2546](https://github.com/scode/farhelm/pull/2546/changes): centralize destination and folder edits.
- [#2553](https://github.com/scode/farhelm/pull/2553/changes): close draft editing access; remove the TODO entry.

## Checks run, reused and skipped

Runtime checks used the recorder, with zero retries: nextest 0.9.148, four slots, for the UI library; Playwright 1.64.0,
three workers and both Chromium and WebKit for browser selections. Browser runs used the repository-built tmux 3.7c.
Native UI-library tests deliberately used `--tmux none`. These are Linux web-renderer observations; WebKit exercises the
same engine family as desktop, not the desktop application itself.

Before the modal rebase, the completed ownership change passed `cargo nextest run -p farhelm-ui --lib`: **658 passed**,
no skipped cases, run `c6eba745-8cbb-45f3-a1f3-7857df4a1b71`. It includes plain transitions and the sole request
builder, same-turn live lock and retry decisions, source/host reconciliation, naming, placement admission, immutable
permission Undo, complete per-template confirmation handling and conditional field subscriptions. The exact two
corrected fixture tests also passed separately in `ba2ae2e2-ec59-49a5-a0c5-580f255883f5`; 656 selected-out cases in that
command are not passes.

`npx playwright test tests/custom-permissions.spec.ts tests/model-defaults.spec.ts tests/composer-word-search.spec.ts tests/permission-switch.spec.ts`
passed **48 cases, 24 per engine**, run `5e3d86f6-0069-4f09-a17a-3284a76469fb`. This covers direct and searched
agent/model choices, configured defaults, malformed/raw custom words, queued edits during a pending launch, Reset,
completed permission warnings and actual Undo, including picker/search focus completion.

The selected seven-file template/search integration command passed **96 cases, 48 per engine**, run
`d9fbaea4-a44f-4094-ae25-a42125429710`: `npx playwright test` over `launcher-template-shortcuts.spec.ts`,
`templates.spec.ts`, `quick-launch.spec.ts`, `yolo-guard.spec.ts`, `launcher-enter.spec.ts`,
`destination-authority.spec.ts` and `github-checkout-composer.spec.ts`, filtered to ordered shortcut batches,
whole-sequence refusal, unchanged/changed confirmations and retries, raw template/prompt fields, template saving, choice
Enter, equal folder/browse/recents and repository search. This selection does not cover those entire files. Its recorded
source fingerprint is `8057de2d73fda41eadc6090388b6ad97d64496f065420cf73594d48072b0425d`.

After integrating the landed modal boundary, the combined tree passed the same full native UI-library command: **658
passed**, no skipped cases, run `f98746b6-45f9-4304-80f0-4c7e674657f6`. It observed rebased parent
`d714a561d8f95fe81f6e46d7de55f90de079e2bc` plus the final ownership change, fingerprint
`d6186e3046fbe88c11327e8f9780709cbf303fc36c0c5e2f943e07726c4a6782`.

The focused modal-integration browser command passed **52 cases, 26 per engine**, run
`f3525f4b-5e6c-4130-98f3-1fb49c63d1bd`, at that same fingerprint. It selected `npx playwright test` over
`launcher-template-shortcuts.spec.ts`, `yolo-guard.spec.ts`, `quick-launch.spec.ts`, `github-checkout-composer.spec.ts`,
`modal-trap.spec.ts`, `clone.spec.ts` and `launcher-enter.spec.ts`, filtered to ordered shortcut batches, the
confirmation-removal focus guard, unchanged agent/command questions, a positive Guarded answer, preview
wait/cancellation, hidden preparation and manual recovery, ordinary New isolation/Tab, Clone opening and late-catalog
focus, switcher-to-Rename handoff and choice/value/action Enter. This covers the overlap with the new dialog boundary,
not those seven entire files. Both combined runs have complete source/report/output evidence, no omissions or runtime
skip messages and normal recorder cleanup; the browser's three test stacks completed cleanup.

Fresh native UI all-target Clippy, web-library Clippy, release web and backend CLI builds also passed on that combined
tree. Format, Markdown and the source-only test-delay checks passed again. The next upstream inspection found only
another plan's delivery report and queue state, followed by a trailing-review queue update; neither introduced product,
specification or instruction changes. All twelve published draft heads and their linear base chain were verified.

Those pre-rebase native and first browser runs observed parent `49a1958fe16493a30279615e32217a56ff104285` plus the
working change, fingerprint `3fbefbd56b597af3cbb9b530107f341ae3bf63714d2452ce6c1b2437ca2ec904`. Subsequent changes were
one Rust comment and the specification's stale template-path reference. The Rust file comparison is comment-only; the
draft implementation is byte-identical. Their matching release web build and runtime evidence remain applicable across
those prose corrections and commit creation. The later modal rebase received separate matching builds and integration
checks described above. The source fingerprint does not hash ignored build artifacts; matching builds were established
separately, not inferred from that fingerprint.

Earlier command-specific evidence remains useful where later source comparison left the covered operations unchanged:

- Clone/source and checkout-numbering integration:
  `npx playwright test tests/clone.spec.ts tests/github-checkouts.spec.ts` filtered to structured/command prefills,
  mouse/keyboard opening focus, deliberate name focus, source-field reseeding, template base/fixed numbering, managed
  Clone defaults, occupied-name re-preview, lost-success identity and managed Replace-with. Run
  `3c6ea94b-b881-4863-8471-a5d872fd07b5` passed 24 cases, 12 per engine, on parent `7948549f` plus the source/naming
  change, fingerprint `d6bccc258c389e2de017d2d8ec35753bd5b24205e704158be93d9facf3b1e98d`. The final unit leaves those
  concrete operations, authority checks and caller wiring unchanged; adapted native fixture setup uses supported
  operations and is included in the final 658-test pass.
- Destination installation authority, saved-folder/browse admission, host-picker interaction and repository selection:
  four selected files `destination-authority.spec.ts`, `sidebar.spec.ts`, `host-picker.spec.ts` and
  `github-checkout-composer.spec.ts`; run `fd6549fa-ab5a-4676-bb37-6c3f2e6568c4` passed 48 cases, 24 per engine, on
  parent `982748ed` plus the placement change, fingerprint
  `eec03ed2a0cce0c3bb0b9bd280873acb09f9ddeb1d88af1d89711cf41bb935a8`. Direct placement and observation operations are
  unchanged. The moved private search routing is checked again in the final selected browser command rather than relying
  on this earlier route.
- The four-file ordinary-edit selection (`launcher-enter.spec.ts`, `custom-permissions.spec.ts`,
  `launcher-tabs.spec.ts`, `named-conversations.spec.ts`) passed 98 cases, 49 per engine, run
  `4eb2ef9b-2db4-4a79-8c5c-3a7c55d71dff`, fingerprint
  `ccb714859c3b6514b8e39cdf274e86589601216a3426ecbeff6c1c6d63d4852a`. Later units preserve those ordinary methods, tab
  retention and one-time request projection. Final selections additionally recheck changed composite callers.

Each intermediate unit was validated before the next migration. Those earlier passes are historical evidence for their
own revisions, not a claim that every old browser selection was rerun at the tip. Native unit coverage was rerun after
each relevant ownership migration. Hash changes from folding test-readiness corrections into their original PRs did not
change production bytes; comparisons after each rewrite preserved the inventory and request/effect contracts.

Failures were retained before corrections or retries. The final unit's first full native command
`07857a40-2b65-49be-86b8-61f2162a8206` failed two fixture premises (656 passed); the exact reproduction
`4c2f3bb5-009f-4db9-bd5a-6a6ed12ba9a9` failed both again. Real source seeding displays an empty peer title as `(empty)`,
whereas the prior invocation-only setup left that editor empty. The converted fixture now reaccepts the original empty
title through the real editor operation, preserving the old raw/request and stale-memo assertions. The new dependency
test now establishes the actual passive Guarded value before changing only explicit ownership, instead of assuming an
absent permission. Both corrections are test-only; the exact and full passing runs above do not erase either failed
command.

Earlier retained failures include the intended pre-fix custom-flags, Reset, duplicate-retry and stale-question proofs;
one compile-heavy command reaching its execution deadline; one empty browser selection; a stale native blank-title
expectation; and three browser focus/input prerequisites. The focus fixtures were corrected to wait for the existing
handoff and the original reviewers checked those amendments. The two 98-case commands
`9f98de7e-88e7-4646-8dc7-2cc5fa08b360` and `0dde9d97-8b05-44e7-9ca8-7d006db0a50a` each remain failures, despite 97
passing cases. Their exact corrected selections passed separately before the final 98-case command. These are
same-session development observations, not established latent flakes. No unique-test total or failure-rate claim is made
by adding executions across these overlapping commands.

The final source also passed `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`,
`cargo clippy -p farhelm-ui --features web --target wasm32-unknown-unknown --lib -- -D warnings`,
`dx build --package farhelm-ui --platform web --release`, the source-only test-delay checker (314 delays, no missing
rationales), and `dprint check SPEC_impl.md TODO.md`. The web build used Dioxus 0.7.10. Final comment/spec corrections
received another format check and both original reviewers' prose follow-up. A separate documentation pass covered every
touched source file, including changed tests and private helpers.

Skipped full-workspace, backend, desktop runtime, installer/provisioning and whole-browser batteries: the changes are
confined to launcher state, frontend refusal interpretation, tests and documentation. Targeted native/web checks cover
the changed code; browser scenarios exercise its integration with real test helms and supervisors. Earlier browser
commands reused matching backend binaries on their own base. The modal rebase also inherited backend changes, so its
browser command used a fresh `cargo build -p farhelm --bin farhelm`, alongside a fresh release web build. No live
installation was modified. No CI run was requested; this repository's CI is manual-only and its browser job is disabled.
No runtime was launched just for the final prose corrections or a new commit hash.

## Review gate outcome

Every PR passed exactly two independent fresh-context reviews on gpt-6-astra at high effort: general correctness, design
and idiomaticity, and a separate review dedicated to observable behavior. The four isolated fixes were accepted as
editing-rule drift covered by the plan's exception. Refactoring units required behavior preservation. The reviews
covered concrete callers, request and authority consumers, per-field dependencies and the pinned renderer's scheduling,
not merely method names or grep counts. Each reviewer recorded its coverage and unverified runtime premises.

The original reviewers checked test-only readiness amendments in their respective PRs. The final general review found
two stale documentation references; those were corrected and both original reviewers accepted the prose-only follow-up.
Both original reviewers for the confirmation fix and the final ownership conversion also accepted the modal rebase
follow-up. No finding from these required per-PR reviews remains open; the separate queued follow-ups above remain.
Commit and PR wording received separate blind cold reads; those are wording checks, not additional code reviewers.
Runtime results above are the executor's evidence, not tests claimed by the source reviewers.
