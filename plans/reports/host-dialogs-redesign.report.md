## What this was about

The host setup question opened without a heading, wrapped plan steps looked like separate steps, and its three answers
had different sizes and baselines. The permanent-answer hint pushed Cancel out of line, and automatic Cancel focus could
open a tooltip over another answer. The maintainer approved one design covering both add-host steps, setup reopened from
a host row, removal, and uninstall.

All four dialogs now reuse the host settings header and pinned footer pattern. Their headings identify the operation and
destination or host, their answers name the action, and permanent answers keep the approved outlined tier beside the
filled one-off action. At desktop width the safe answer comes first in a right-aligned row; at phone width the action is
on top and every answer spans the footer. Cancel remains initially focused with its visible ring, without opening help
merely from that automatic focus. Later hover and keyboard navigation still show help.

## Things you should know

Setup and uninstall retain the helm's exact plan text, including its final newline when copied. Wrapped step
continuations are indented; headings and the uninstall plan's kept-data paragraph stay flush. Native selection needed a
one-pixel empty end marker to preserve the final newline in Chromium; the marker contributes no copied text. The UI does
not reconstruct the helm's actions or intercept copying.

Add and uninstall are 560px wide; removal remains 480px. The approved removal hint needed 10px desktop text to fit on
one line with Chromium's font fallback at that width. Its phone text remains 11px and can wrap. Titles wrap instead of
using the drawing's ellipsis. Existing host icons and the UI's name for this machine replace the mockup's placeholder
marks.

No answer's operation or permanent preference changed. Header × follows that dialog's Cancel behavior, including
retiring a setup offer and returning focus to the host row. Existing busy-operation rules for Cancel and Escape remain
intact. Quiet focus falls back to ordinary safe focus if the tooltip asset has not loaded, and the hook survives asset
reinstallation.

A pointer resting where Add was can still hover Set up when the plan arrives. This change addresses automatic focus;
deliberate pointer help remains available. Website wording and screenshot-capture selectors were updated, but published
screenshots were not regenerated and the website was not deployed. The covered TODO entry is removed in the final PR.

## Open questions and possible follow-ups

No decision is outstanding. Refreshing the published documentation screenshots remains separate work, as the plan
required.

## The PRs

- [#1865 — shared host dialog header](https://github.com/scode/farhelm/pull/1865/changes).
- [#1866 — add-host and setup layout](https://github.com/scode/farhelm/pull/1866/changes).
- [#1867 — removal and uninstall layout](https://github.com/scode/farhelm/pull/1867/changes).
- [#1868 — quiet initial Cancel focus with later tooltip help](https://github.com/scode/farhelm/pull/1868/changes).

These form one linear stack and remain drafts. The executor has not marked them ready or merged them.

## Checks run, reused and skipped

Passed the required targeted checks for each code unit: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo check -p farhelm-ui --features desktop`; release web UI builds;
the isolated source delay checker (285 delays, none missing a rationale); changed Markdown formatting; and changelog
format lint. The website's frozen install and 32-page build passed after its wording changes. The UI JavaScript harness
passed 230 cases in recorder run `67d63879-2c78-4c99-aa0f-960fbaef00ce`.

Recorded Chromium and WebKit selections used one worker, zero retries, and pinned tmux 3.7c. Successful evidence covers:

- Existing host settings lifecycle and pinned controls: six cases, run `fd98b51b-e8b4-4feb-bbcb-f8a467dc56be`.
- Desktop/phone add and setup geometry, tiers, native copy and hanging indentation: four cases, run
  `33c4e3c4-5020-40b6-9d7c-289cdd96be43`.
- Setup cancellation bursts, permanent-preference reversal, doubled submit, pending cancellation, and local/row setup:
  16 cases, run `e7af3de5-885c-4bfd-83e6-0fe1a5868733`.
- Removal and real uninstall lifecycle, geometry and copy: four cases, run `a763fd50-b391-4a7b-a468-e59119ba2a3e`; the
  corrected one-line removal hint: two cases, run `3134a7a2-aaa3-482c-a20c-fbb959ebefd1`; permanent-removal reversal:
  two cases, run `3bdcc408-fd5c-4d28-8e01-e0bb0bb54e0b`.
- Final quiet-focus proof: four add/setup cases passed in `466bc536-07d0-4816-be39-9ed134424b82`; two row-setup cases
  passed in `437d24c1-da95-40f1-8f6c-b5662da39145`; four removal/uninstall cases passed in
  `f37c6405-6f86-4840-b9e6-a963b2e40c65`. The first run also had six fixture timeouts because its keyboard positive
  dismissed a menu before focusing its action. Those failures remain retained; corrected tests establish help on the
  persistent menu toggle and open it by keyboard before the confirmation.
- Ordinary tooltip focus, press/Escape lifetime, immediate pointer transition, touch refusal and actual asset
  reinstallation with a new focus event: 12 cases, run `baead2e9-0cd6-4eed-89eb-bd1a72c1a0dd`.

All successful selections had zero skipped cases. Desktop/phone captures of every redesigned dialog were individually
compared with the approved mockup in both engines.

Reused the debug helm and supervisor build made during the first header-extraction unit: its relevant binary sources
match the final first-PR source at `d6ca5b8e300eba52a64403c979c5c30244bfa2f0` and remain unchanged through final head
`3b75281e2c9ddda4b36252b0aed29d4410241445`. Browser tests use the separately rebuilt web UI, so these debug binaries do
not supply the changed dialog assets. Reused the website build from the third unit: its website source matches
`3c136aaab2542f6d4dba1fc891b69a0faa19887b` and is unchanged in the final head. Reused the fourth unit's successful
Clippy, desktop compile and Rust formatting checks after the asset-reinstallation and test-premise corrections: the Rust
source they checked matches final head `3b75281e2c9ddda4b36252b0aed29d4410241445`; subsequent edits affected JavaScript
listener state and browser tests only.

The pure-JS run `67d63879-2c78-4c99-aa0f-960fbaef00ce` recorded HEAD `3c136aaab2542f6d4dba1fc891b69a0faa19887b` with the
initial fourth-unit changes uncommitted. Its source fingerprint is
`9190ae15616f36f5dc6a25a89b2ae31781e54dc13efb909fc512236e8219e6b2`. The later asset-reinstallation correction changed
installed listener state, while the pure functions and harness that this run exercises remain unchanged in final head
`3b75281e2c9ddda4b36252b0aed29d4410241445`. The final browser state selection covers that correction. Earlier copy
experiments and same-session fixture failures remain private evidence, not latent-flake entries.

Skipped the full Rust and browser batteries: the targeted selections cover the changed dialog layout, copying,
submission, cancellation, preferences and tooltip lifecycle, and the stack changes no helm or supervisor behavior.
Installer, provisioning-system, desktop runtime and release gates were not relevant to the affected presentation paths.
No hosted CI was requested.

## The review gate's outcome

Each PR passed the required single fresh-context gpt-6.1-sol review at high effort, with source and actual captures
checked against the approved mockup. The removal review found a wrapping desktop hint; its font size was corrected and
verified with painted-line and overflow assertions. The final review found that the asset-reinstallation test refocused
an already focused control; it now establishes a different focused control before the hook and passed in both engines.
No review finding remains open. Every commit title and PR body also passed the standing fresh-context wording review.
Implementation stayed local under no-workhorse mode; only the prescribed reviews were delegated.

Fresh fetches before the final PR confirmed main still at `fc6bf5951d230d855173f0b98b6a81286b7bacac`, the stack's
reviewed base. No intervening upstream diff needed rebasing or additional runtime checks. The four draft heads are
`d6ca5b8e300eba52a64403c979c5c30244bfa2f0`, `16d890320804dee8d3594f31853b231fef5f5bbb`,
`3c136aaab2542f6d4dba1fc891b69a0faa19887b`, and `3b75281e2c9ddda4b36252b0aed29d4410241445`, in order.

### Landing

Landed on 2026-10-11 (UTC) as #1865, #1866, #1867 and #1868. The rebase met no conflicts; only plan bookkeeping had
landed on main since the plan ran.

#### Fixes made while landing

- In #1867, three multi-host browser tests confirmed a host removal by clicking the dialog's first danger button. This
  plan put **remove, don't ask again** first, so one of them saved "don't ask again" on the helm all browser tests
  share. A later test's removal then skipped its dialog and removed the remote host outright, which failed four tests
  and stopped 56 more from running (run `ccb06a1c`). The plan's own checks ran selected cases only, never these tests in
  sequence. They now click **remove** by name. The product behaviour was correct.
- The changelog fragments credited the wrong PRs with the renames: **remove** and **uninstall** were already the labels.
  #1867's fragment now names only the removal answer it renamed, and the shared setup fragment names the setup answers
  and describes the quieter confirmation in user terms.
- The shared header's doc comment (#1865) said only the add flow supplies a summary; setup from a host row does too, and
  a summary replaces the status line either way.

#### Review before merging

No blockers. #1865 is a true refactor. Focus, Escape and focus return hold for every setup, removal and uninstall path,
and the add dialog cannot submit on Enter. The tooltip suppression lasts only for the focus made when a confirmation
opens and leaks no state to other dialogs. Notes left as they are:

- The docs pages' alt text describes the new buttons, but the published screenshots still show the old layout until the
  docs screenshots are next refreshed; the setup-plan callouts were placed for the old layout and need checking then.
- The removal dialog's "don't ask again" hint is 10px on desktop, the smallest text in the app, to keep it on one line
  in the 480px card; widening removal to 560px like add and uninstall would allow 11px.
- The add dialog is a `form` with `role="dialog"`, which ARIA does not strictly allow on a form; browsers expose it as a
  dialog anyway.
- Some `rsx!` blocks in the add and setup dialogs are indented inconsistently; formatting does not check them.

#### Checks

- Run now, on the stack after the fixes: `cargo fmt --all -- --check`, `dprint check`, the changelog lint,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop`; the UI unit tests through the recorder (run `e42c2a99`, 446 of 446);
  the UI JavaScript tests (230 of 230); the docs website build; and on Chromium and WebKit with one worker and no
  retries the provisioning, settings, terminal-multihost, tooltip and modal-layout specs (run `f53d6ca9`, 264 passed).
- The earlier run of the same selection before the test fix (run `ccb06a1c`) failed as described above and is not
  counted as a pass.
- Not run: the Rust tests outside the UI crate, since only UI code, styles, tests and documentation changed.

Nothing in the report above was made untrue by the landing.
