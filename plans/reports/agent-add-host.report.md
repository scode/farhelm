## What this was about

Agents could list hosts but could not ask Farhelm to add, edit or remove one. This plan adds those three commands. Every
request needs an explicit answer in the GUI, even from a host allowed to run other Farhelm commands without asking. The
add request uses the ordinary add-host dialog, prefilled with the requested destination and alias; edit and remove use
Allow/Deny cards.

## Things you should know

An accepted add is not an installed host. The user approved the destination then in the form, and the answering window
continues the ordinary discovery and setup flow. It may still fail, need setup, or be cancelled at the setup question.
The command's output, help, agent instructions and docs tell the agent to check `farhelm agent hosts` and the host's
state before relying on it. The output names the destination the user approved, including edits.

Only the focused, visible window opens a newly observed add request automatically. Other windows keep a card that opens
the dialog explicitly. Existing drafts stay intact, several arrivals wait their turn, and only the window whose answer
was accepted starts discovery. Cancelling the pending dialog denies the request. An expired request or an answer lost to
another window starts no discovery.

The user's setting to skip setup questions applies to additions the user starts. An agent's accepted add still asks
before installing Farhelm, including if a definite setup refusal reopens the edited form. The spec states this as the
meaning of the preference: it saves confirmation clicks in the user's own UI actions; it grants no permission to agents.

Edit changes only alias, icon and color, through the same host-writing code as Settings. It cannot change permissions or
a destination. A no-op edit is refused without a card. Remove repeats the GUI's explanation: Farhelm forgets the host,
while its supervisor and sessions keep running. Removing this machine or the asking session's own host is refused
without a card. Edit and remove recheck under the GUI writers' locks after approval; a remove can still proceed after a
rename, but a changed destination is refused.

The approval exchange gained a narrow add-only acceptance acknowledgement. It carries the approved destination and lets
the browser start discovery only after the waiting command has accepted that answer. It does not track installation
results, persist unfinished additions, or replay them. If the browser closes or loses the answer response after the
command accepts it, discovery may never start; the agent must still check the host list.

The docs changes are on **Approve what agents do** and **Manage hosts**. The feature fragment is updated and the source
TODO entry removed. The three PRs remain drafts, ready for the landing monitor; this executor has not merged them.

## Open questions and possible follow-ups

No maintainer decision is needed to land this plan. Returning the eventual installation result, recovering a lost
browser continuation, an extra agent-host opt-in, uninstall and setup commands remain outside the agreed scope.

Browser tests control the negative document-focus input because Playwright treats every automated page as focused. They
exercise the real admission and dialog code on both engines, retain native positive focus and visibility, and use two
real pages for the winning-answer case. They do not establish operating-system window activation or the hidden-tab
branch. Server arbitration and acceptance are tested separately in the helm; the browser race deliberately controls
server replies. The original edit-lock tests verify locks held at comparison and both writes, but do not independently
detect every possible release-and-reacquire refactor between those writes.

## The PRs

- [#2674 — host edit/remove commands and approval enforcement](https://github.com/scode/farhelm/pull/2674/changes).
- [#2680 — edit/remove approval cards and host-settings help](https://github.com/scode/farhelm/pull/2680/changes).
- [#2681 — add command, ordinary dialog integration, instructions, docs and TODO removal](https://github.com/scode/farhelm/pull/2681/changes).

## Checks run, reused and skipped

Runtime commands used the test recorder, pinned nextest and tmux where applicable, four Rust slots or three browser
workers, and no retries. Source capture and reports were complete for the successful runs below. The real command test
used an owned supervisor and helm; browser stacks used owned local/SSH fixtures and simulated host setup.

- `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm-ui --lib`, filtered to host-add,
  host-action rows, protocol version, shared approvals, relay bounds and add-dialog helper contracts:
  `5c81fa03-79c4-477f-8247-e167dc2bc7ee`, **57 passed**, 3,469 selected out. This checks the new acceptance boundary and
  existing approval behavior alongside the wire and UI helpers.
- `cargo nextest run -p farhelm --test e2e`, filtered to
  `agent_listing_real_stack::host_add_cli_reports_accepted_edited_destination_and_declined`:
  `02b9da04-68be-4408-89ce-d1ea1b892339`, **one runtime test passed**, 398 selected out. It runs the actual command
  through both relay hops for acceptance with an edited destination and decline. JUnit also records the fixture-build
  setup task; that is not a second runtime test.
- Selected `npx playwright test` cases in `approval-layout.spec.ts`, `provisioning.spec.ts` and `ssh-help.spec.ts`:
  `5ba6f5e4-fd44-44ce-a4ea-8cefeaa6c867`, **six passed**, and `136eb52f-44e3-48a3-b548-972d1394d35e`, **24 passed**;
  together **15 per engine**, Chromium and WebKit, zero skips or failures. They cover prefill and edited fields,
  requester escaping, inactive cards, draft preservation and focus resampling, queued expiry, Deny, exact acceptance
  before discovery, refused answers starting no discovery, the sole answering window, setup confirmation after
  refusal/reopen, and existing alias, duplicate, manual-preference and SSH-help behavior.
- The first browser selection, `a86a399c-9b8a-4afc-93ac-82cd334241b3`, had 24 passes and six failures at the invalid
  headless focus premise. An exact Chromium reproduction, `a9d2998d-d40e-4fe1-ac8e-80a16c8b6696`, reproduced it.
  Correcting only the test focus input produced the successful six-case run above; exact Chromium debugging run
  `5215ab1f-d26b-4908-80c9-bc6134af41d1` also passed. Earlier attempt `adcf6d51-96df-4986-9ba1-7bb47c55c554` selected no
  tests because its grep was anchored incorrectly. All failed-run evidence is retained privately; none counts as a
  passing command or as a latent flake.
- Reused edit/remove coverage from PR1's completed implementation: `cargo nextest run -p farhelm-helm --lib`,
  `c5ad5173-43dc-4aa1-ba9f-5e5cd33fe08a`, **1,308 passed** on the pre-rebase PR1 tree later pushed as `44e576b5`. Its
  edit/remove code is unchanged through the rebases; PR3's separate add path and shared approval contracts receive the
  new focused run above.
- Reused PR2 UI row coverage: `cargo nextest run -p farhelm-ui --lib -E 'test(approvals::tests::host_edit_rows_)'`,
  `ae5d8ae6-4707-4200-9835-a49106500e01`, **two passed**. PR2 browser cards used
  `npx playwright test 'approval-layout\.spec\.ts' -g 'a host (edit|remove) card'`,
  `a79d0c19-781d-4f9d-ad2c-38ff4c57dd98`, **six passed**, three per engine. Both ran on the PR2 tree pushed as
  `ca64deec`; the literal edit/remove presentations remain unchanged.
- `cargo fmt --all -- --check`: passed on the PR3 working tree described below; checks Rust formatting.
- `cargo clippy --all-targets -- -D warnings`: passed on that PR3 tree; checks shared native code and test targets.
- `cargo clippy -p farhelm --bins -- -D warnings`: passed on that PR3 tree; checks the shipped CLI configuration without
  test-only features.
- `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`: passed on that PR3 tree;
  checks the changed UI in its browser configuration.
- `cargo check -p farhelm-ui --features desktop`: passed on that PR3 tree; checks the separate desktop renderer.
- `cargo build`: passed on that PR3 tree and prepared the native browser-stack executable.
- `dx build --package farhelm-ui --platform web --release`, from the UI crate with Dioxus 0.7.10: passed on that PR3
  tree and prepared the actual web bundle. The initial PR2 attempt resolved an unrelated executable named `dx`; the
  correctly identified Dioxus executable then built successfully.
- `<isolated-python> -B scripts/check-test-sleeps.py`: passed after the PR3 browser correction, **314 delays, zero
  missing reasons**. This checks annotated test delays, not runtime readiness.
- `dprint check SPEC.md SPEC_impl.md TODO.md website/src/content/docs/docs/using/approve-agent-requests.md website/src/content/docs/docs/using/manage-hosts.mdx`:
  passed on the final rebased tree; checks formatting of the affected Markdown.
- `python3 releasing/check-changelog.py format`: passed on the PR3 tree; checks fragment layout.
- `python3 scripts/check-agents-md.py`: passed on the PR3 tree, **zero errors, one pre-existing unrelated citation
  warning**; checks instruction paths and heading links affected by the new docs section.
- `bun run build`, from `website/`: passed on the PR3 tree, **32 pages and all internal links valid**. No preview server
  starts in an unattended run.
- Successful PR3 runtime/build/lint results identify HEAD `ca64deec` plus the captured PR3 working diff; the focus fix
  changes browser tests only. The final careful rebase onto main `0586b379` keeps each code patch unchanged. The full
  1,296-line upstream diff since the previous fetch was read: launcher refusal and directory checks, their scoped
  styles, specs and tests, plus bookkeeping. Their API functions and spec passages are separate from host approvals and
  adding hosts. The only conflict kept both TODO deletions. These results are reused per command because no tested
  behavior changed; no runtime suite was repeated merely for the rebase.
- Skipped the remaining Rust/browser batteries, desktop runtime/smoke, installer and cross-distribution installation
  suites: the selected checks address the new approval and dialog risks; installation execution and desktop-only
  persistence are unchanged. No live installation was changed. No hosted CI run was requested.

## Review gate outcome

Each PR passed the required single fresh-context **gpt-6.1-sol, high effort** adversarial review. PR1's first review
found an approval/write race and an unapproved no-op edit; both were fixed. The second accepted the production changes
and asked for stronger lock tests; those were added and mutation-checked. PR2's review corrected a test-selector mistake
before runtime. PR3's full review found no actionable issues, and a fresh review of the focus-fixture correction also
found none. Test reviews received the full test-authoring checklist.

A separate same-model scope review kept the add acceptance acknowledgement within the existing approval exchange, rather
than adding another request owner or tracking installation outcomes. Commit and PR wording passed fresh cold reads. This
report passed the required independent cold read before delivery.
