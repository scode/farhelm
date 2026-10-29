# Steel sentence case

## What this was about

The final Steel redesign plan replaces authored lowercase interface labels, notices, placeholders and tooltip leads with
sentence case. The ten drafts cover the launcher and templates, hosts and Settings, approvals, Help and feedback,
session controls, checkout trash, device sign-in, terminal hints and the remaining notices. The specs and 22 guides
quote the completed labels. The final code draft removes the Steel TODO entry and completes its existing changelog
fragment.

Names, paths, commands, agent output, peer diagnostics and machine-facing values keep their original bytes. Grammatical
continuations remain lowercase: a native file-opener reason follows “not opened:”, and notification age “now” is a value
rather than a badge. New and Resolved are labels. The collapsed approval queue uses the existing fixed-phrase
presentation helper; it does not transform host names.

## Things you should know

The stack is complete for the recorded scope and remains ten open drafts. No PR was marked ready or merged by the
executor. The landing monitor owns that step. The current base is main at `dd56cc23`; its intervening changes were TODO
bookkeeping, two future plans and trailing-review records. There was no executable, spec, guide or styling interaction.
The rebase preserved the reviewed patches and upstream TODO edits.

The two recorded capture decisions expressly accept less direct layout evidence for named states. The exact Opus
5.5/high reviewer inspected their new wording, markup, value boundaries and wrapping rules from source; these states
were **not photographed**:

- Desktop App settings and update notices; Terminal settings for native file types; loaded SSH connection help;
  host-removal confirmation.
- Host identity adoption and the inline Update button; queued approvals, expired-answer notices and approval link
  errors.
- The notification list, New/Resolved labels and relative age; whole-list loading, empty, failure and partial-count
  notices.
- Interrupted-session cards; shell-tab close confirmations, tab limits and tab errors; session-action, lease and
  hidden-text copy warnings.
- Native authentication startup failures; the Terminal tunnel setting and global tunnel errors.
- File-transfer and native-open results, attachment notices, reconnect and build-version warnings, and remaining save or
  transport errors.

The source review found layouts that wrap, break long values or deliberately ellipsize metadata without changing their
styling. It cannot prove native font rendering, actual line breaks or pixel fit in these absent states. Earlier
photographed controls retain their original image provenance; a shared tooltip or notice primitive is not a photograph
of every possible value. No extra response substitutions, native modes, capture machinery, SSH changes or
live-installation actions were added.

All changed guide prose is also **unphotographed**, as the latest decision requires: no website pixels were rendered.
The affected pages are the agent wrappers, Claude, Codex, Cursor, custom commands, Grok, agent overview, OMP and Pi
guides; Add a remote host, Your first session and Uninstall; Agent hook injection; Approve agent requests, Launch
templates, Manage hosts, Send feedback, Session list, Start a session, Stop/restart/resume, Update/uninstall and Work in
a session. Opus checked quotations, ordinary Markdown/MDX flow, fonts and wrapping against existing app-image
provenance. The site build checks content and links, not pixel layout. Screenshots, published release material, the docs
Overview page and its intro drawings were left alone.

The Settings browser helper now selects the sidebar footer's Settings button. Capitalizing that button made its old
global text lookup collide with a separate trust notice's Settings button. Its existing initial-pane assertions remain
intact; the repair is folded into the owning Settings draft.

## Open questions and possible follow-ups

No decision blocks this delivery. The following inherited observations were deliberately retained rather than expanding
a capitalization sweep:

- The host-settings Save button is hard to see: effectively invisible in the inspected light view and very bare in dark.
  Welcome still describes Help as being at the top of the sidebar; it is at the rail's foot. The short rail has little
  scroll affordance.
- Start a session still quotes “Browse folders on”, although the button now says “Browse”; the editorial vocabulary
  repeats the obsolete example. A targeted wording follow-up should cover both. Some guide action names omit the
  existing ellipsis. These mismatches predate this sweep.
- Earlier image reviews recorded existing launcher/refusal placement, dialog scrolling, long approval segments,
  field/select and placeholder typography, permission outlines, tooltip placement and focus/layout nits. Their
  underlying layout and behavior were unchanged. Fixing them would require work beyond case changes.
- Session reviews retained a row-rename error presentation, persistent Copied timing, a stale connection strip,
  selected-rail contrast and bounded metadata ellipses at a 650px main pane. Trash retained a double gap in a
  host-qualified Delete all label. Its pinned-header/no-footer layout still follows today's spec; the separate queued
  Trash-footer plan changes that later.
- Device sign-in retains literal command backticks and interface-font command text in a plain error, plus a shared
  tooltip that can extend over the card edge. Terminal review retained inconsistent fonts for parallel OSC addresses. An
  empty host in the missing-file capture fixture produced a leading colon; this was not established as a production
  host-loss bug.

Peer host strings, independent metadata labels after separators and grammatical continuation words were preserved
intentionally. No unsolicited TODO entries were added for these observations. The existing website
empty-language-collection and missing-404-entry warnings remain; the final build still passed.

## The PRs

- [#2767](https://github.com/scode/farhelm/pull/2767/changes): session launcher, templates and shared setup labels.
- [#2771](https://github.com/scode/farhelm/pull/2771/changes): hosts, Settings and ordinary approval labels, including
  the Settings test-helper repair.
- [#2772](https://github.com/scode/farhelm/pull/2772/changes): Help, feedback, Welcome and related update/SSH wording.
- [#2773](https://github.com/scode/farhelm/pull/2773/changes): session controls, status labels and navigation.
- [#2774](https://github.com/scode/farhelm/pull/2774/changes): checkout-trash captions and matching consumers.
- [#2775](https://github.com/scode/farhelm/pull/2775/changes): browser device sign-in wording.
- [#2776](https://github.com/scode/farhelm/pull/2776/changes): terminal, trust and shortcut hints.
- [#2781](https://github.com/scode/farhelm/pull/2781/changes): remaining notices and errors, final changelog wording and
  Steel TODO removal.
- [#2777](https://github.com/scode/farhelm/pull/2777/changes): quoted labels and sentence-case rules in both specs.
- [#2778](https://github.com/scode/farhelm/pull/2778/changes): quoted labels in the 22 guides.

## Checks run, reused and skipped

Runtime commands used the recorder. Rust used pinned nextest with four slots and no retries; browser checks used pinned
tmux, three owned stacks, Chromium and WebKit, and no retries. Successful records cited below have complete source
identity, untruncated output through EOF and ordinary cleanup. Selected-out Rust cases are not executed coverage.

Final-round checks:

- Nine selected UI Rust cases passed in `025dac6b-3ec5-4aa0-8bb7-15af19263f87`, covering transport/auth refusals,
  notification labels, count warnings, interrupted cards, restart offers, tab-close consequences and attachment
  messages. After restoring age “now”, two focused notification cases passed in `ada7db80-565e-42c0-ab7c-62080d130fd9`;
  one overlaps the earlier selection. The other seven unaffected observations remain applicable.
- Three selected file-transfer JavaScript cases passed in `52e3e21d-971e-42d6-bd71-e56fbe9a23e5`. The subsequent
  native-clause and age corrections do not alter those JavaScript inputs.
- `cargo build -p farhelm --bins` passed in `9174a26a-2679-403f-8684-132d004b13bd`. The final web release build passed
  in `6f802798-5334-4ba0-b15b-688d7685ff40`. These are this checkout's matching artifacts, not an assumed reuse of
  another executor's binaries.
- Fourteen selected browser checks passed in `fd7f9e77-78ee-447c-a7aa-d2be0be8d891`: seven per engine, covering
  notifications, identity adoption, hostile attachment tokens, reconnect, two hidden-text clipboard warnings and updater
  refresh/reduced-motion behavior. No case skipped or failed. This matches final application source fingerprint
  `e040dc54`; later changes were prose and a behavior-preserving rebase.
- Frozen-lockfile installation passed without dependency changes in `9c86244f-6fac-4d76-b413-8605383a30ad`.
  `bun run build` passed in `b11062ae-fe86-417e-877e-aa0d871291a4`: 32 pages, search index, sitemap and all internal
  links. Both bind final guide head `97543be4` and source fingerprint `98aed5f9`.
- Cargo formatting, scoped documentation formatting, 21 changed TypeScript files' syntax checks, changelog format and
  the sleep checker passed. The sleep checker found 322 delays with zero missing reasons; subsequent literal/comment
  changes added no delay. Byte guards and independent reviews confirmed the actual two specs and 22 guides equal the
  accepted candidates.

Earlier successful evidence is reused by affected command, because the first seven completed patches retain their
reviewed contents; the final remainder does not change their interaction mechanisms. The original launcher selection
passed 400 browser cases in `d03ef380-e797-4b7a-938a-dfce51c151e4`, followed by 16 missed-consumer checks in
`cc9d8be3-e8dd-4660-9eb7-0f03cf730bfe`; the JavaScript harness passed 358 cases in
`9c189b8e-d4b6-4a40-a3cc-316c38876ea4`. Their larger counts are historical observations, not a new battery at the final
head. Later bounded checks covered Help/feedback, session summaries and menus, trash and sign-in. Trash passed four
checks in `bf32b3f4-d9c7-4b5b-bc0b-f031af0f07cd`; sign-in passed two in `e3c1dedd-7214-4828-bae1-ce60088d0517`.

The terminal selection `d3140f41-fe99-4f0b-824f-d1dcfc7de5b7` had six passes and two Settings-selector failures. After
the narrowly reviewed helper fix, those two cases passed in `4e544799-29c8-40cf-bad3-7ed1fe36f003`; the six unaffected
observations are reused separately. A session-summary expectation failed on both engines in
`02f371bf-0815-4577-b1fd-6bc4e3206dd0`; its two exact corrected cases passed in `6f78dace-8a9e-411b-91a5-6a68a27b6a58`.
Failed and interrupted commands, including earlier template/refusal consumer, compile and capture-setup attempts, remain
retained in the working log and private evidence. None is counted as a passing command or relabeled as a latent flake.

The photographed units retain their complete theme/width capture evidence and actual image-delivery audits.
Representative final capture runs include 188 launcher/template images (`5bf1cb3d-2446-4ea4-b4e8-ec0b395a3fbd`), 24 Help
images (`449ac5be-e6e6-410a-a1ea-3360bcc6f272`), 16 corrected session images (`fe840707-d6ad-4aa9-b78f-f291058a7bcd`),
eight trash images (`8eac76c0-e0f2-4dac-a950-1fcf25c30510`), eight sign-in images
(`99c55ff5-f879-4170-8f23-f454b5ae935a`) and 24 terminal images (`636d772f-d4d0-4e1e-a786-1deb4b9b030c`). The
intentionally unrelated approval case skipped in narrowed capture selections; it is not missing target coverage.
Unchanged earlier images retain their original heads rather than being presented as fresh captures.

The later instruction to run small relevant checks overrides the old proposed two-hour/82-spec sweep; it was not run at
completion. Full Rust/JS/browser batteries, desktop/installer/provisioning runtime and CI dispatch were skipped because
no uncovered behavior risk justified them. Docs-only amendments ran site/content checks, not application or
feedback-handler tests. README/demo/website screenshot refresh and publication were outside scope. No preview server ran
unattended. Full per-command historical evidence and review decisions remain in
`farhelm-plan-steel-sentence-case-log.md` beside the checkouts.

## The review gate's outcome

Each PR passed a single fresh-context gpt-6.1-sol/high code reviewer, with bounded follow-ups on the same reviewer. Each
visible unit passed the exact claude-opus-5-5/high visual gate, beyond the inherited observations explicitly declined
above. No substitute reviewer or review swarm was used. Specs have no new rendered screen; their source gate applies.
Commit/PR wording passed its required cold reads.

The final application review corrected two stale test literals and two casing-boundary mistakes: native-open
continuation clauses and notification age. Its final Sol and Opus reviews are clear. Opus received all nine reference
image payloads in both application turns. The guide review corrected three stale label occurrences and retained the
inherited Browse follow-up; all nine reference reads succeeded, and the bounded correction turn reused them without
claiming new pixels. The final two specs and all 22 guide files passed independent byte-binding reviews. Required image
reads were audited from successful delivered results, not filenames or tool-call counts.

All required gates for the recorded scope are complete. The residual source-only layout limits and inherited follow-ups
are stated above for review after landing.

### Landing

Landed with fixes at landing: you asked the monitor in conversation, for this plan only, to fix failing tests rather
than send it back, and to land it unless a blocker was serious. The last Steel plan changes Farhelm's authored lowercase
labels, notices, placeholders and tooltip leads to sentence case across the app, quotes the new labels in SPEC.md,
SPEC_impl.md and 22 website guides, and removes the Steel TODO entry, since all five Steel plans have now landed.

- #2767: session setup (launcher and templates).
- #2771: hosts, Settings and approvals.
- #2772: Help and feedback.
- #2773: session controls.
- #2774: checkout trash.
- #2775: device sign-in.
- #2776: terminal hints.
- #2781: the remaining interface notices.
- #2777: the specifications.
- #2778: the website guides.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping, TODO edits and trailing-review records landed since the plan was based, and while it
merged, two TODO entries of yours (the menu highlighting bug, #2783, and the containerized-sessions plan, #2782);
nothing interacted. A fresh-context reviewer found that no machine-read value changed: stored settings, wire values,
command output and the priority value sent with phone notifications keep their old lowercase words, and only text shown
to people changed.

#### Fixes made at landing

The whole browser suite found 47 failures in 24 tests: 23 failed on both engines and one on WebKit only. 22 of those
tests still looked for the old lowercase wording, in test files the plan did not touch and in lines it missed in files
it did, for example the "Host setup" dialog, the "Set up" and "Cancel" answers, and notices that now start with a
capital letter. The landing updated those expectations to the new wording, changing only letter case, each checked
against the product's source; one became stricter. All of these fixes are in #2781. The executor's selected test runs
had not caught any of this.

One test showed a real regression, also fixed in #2781: the hover explanation of a disabled "Restart with…" menu item is
meant to end "use Replace with…", and the code that adds the closing "…" looked for the old lowercase text, so the "…"
was silently lost after the recasing. It now matches the new text. A second fresh-context reviewer checked every fix and
searched the interface and tests for any other code that compares against the old lowercase text; it found none that
still matters.

The remaining failure, a WebKit-only focus check in the Restart with tests, passed when rerun. A focus race in WebKit is
a guess, not shown, so the rerun does not clear it; it is not recorded as a flake on one observation.

#### Things you should know

- As decided earlier for this plan, the wording of many states was reviewed from source rather than photographed: the
  desktop App settings and update notices, native file-type and tunnel settings, SSH connection help, host-removal and
  host-identity confirmations, the notification list and its states, interrupted-session cards, shell-tab warnings,
  approval queue and link notices, startup failures, and file-transfer, reconnect, build-version and save errors.
- No unit test covers the code that fixes up the Restart with explanation; only a browser test caught its break. A unit
  test, or writing "Replace with…" directly in the explanation text, would stop the next wording change from silently
  breaking it. Its neighbouring rewrite for "replace starts it over" no longer matches anything and can be deleted.
- The supervisor's refusal of a plain Restart, shown when a command launch has no resume command, still says "so replace
  starts it over" in lowercase; it was outside this plan.
- The effort level label now reads "Xhigh"; no recorded decision covers that casing.
- Agents now get a template refusal starting "Template … was not applied", while the neighbouring refusal still starts
  "template … sets a host".
- SPEC.md's lists of Goose and OMP modes still say `smart approve` in lowercase, from this plan's own spec PR.
- SPEC.md and the agent instructions quote two host settings as "… here without asking" while the interface says "…
  without asking", and the website's editorial rules use two labels as examples that no longer exist in that form. Both
  predate this plan.

#### Checks

This round landed only this plan. On the stack as delivered, up to the remaining-notices PR (the specification and guide
PRs above it change only text):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, the check that every test delay states its reason, the workspace debug build, the
  UI's release web build and `scripts/check-desktop-assets.sh`: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `f03b248b-5bf9-41a3-a9da-be65917041a8`: all 805 passed;
  its desktop-build tests, run `2b744068-29b5-4e7e-81f3-839f0262ac3c`: all 988 passed. The UI's JavaScript and
  stylesheet tests (`node --test` in `crates/farhelm-ui/js-tests`), run `d3454c3f-02e0-4cb1-8679-9182604bf41f`: 358
  passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `26c0a5a9-c6c2-49b6-8207-cf6d8ab4874b`: 2721 passed, 14 skipped by design, 47 failed
  (the failures the landing fixed, above).
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, run `b4720622-4fee-4aa5-b6b1-1a1430ccdbcb`: 4486 passed, 40 skipped by design, and 1 failed: the known terminal
  flow-control flake "shallow pause resumes without reset or replay", in code this plan does not touch.

After the landing fixes, at the top of the stack:

- `cargo fmt --all -- --check`, the web-target UI lint, `dprint check` across the repository,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the docs website build and the
  UI's release web build: all passed.
- The tests whose expectations were fixed (each had stopped at its first stale string, so their later lines had not run)
  and the Restart with cases, on Chromium and WebKit, run `cc18e348-8326-4004-b292-bce5d70174c0`: all 52 passed. As you
  asked in conversation, the whole suite was not run again for case-only test fixes.
- Skipped: the desktop smoke tests (the desktop-only changes are wording, covered by the desktop-build UI tests above)
  and the native Mac app (not runnable here).

The report's checks were the executor's selected runs, and it treats them as enough; the whole browser suite at landing
showed they were not (above). This landing ran the whole browser suite and the whole Rust workspace.
